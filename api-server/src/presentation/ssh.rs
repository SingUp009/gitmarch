use std::env;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use ed25519_dalek::SigningKey;
use russh::server::{Auth, Handler, Msg, Server, Session};
use russh::{Channel, ChannelId, CryptoVec};
use russh_keys::key::{KeyPair, PublicKey};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::db::Pool;
use crate::feature::user;

pub async fn serve(base_dir: PathBuf, pool: Arc<Pool>) -> Result<()> {
    let bind_addr = env::var("SSH_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:2222".to_string());

    let key_path = env::var("SSH_HOST_KEY_PATH").unwrap_or_else(|_| "ssh_host_key".to_string());
    let host_key = load_or_create_host_key(Path::new(&key_path))
        .context("failed to initialise SSH host key")?;

    let config = Arc::new(russh::server::Config {
        keys: vec![host_key],
        ..Default::default()
    });

    let mut server = SshServer {
        base_dir: Arc::new(base_dir),
        pool,
    };

    println!("SSH listening on {bind_addr}");
    server
        .run_on_address(config, &bind_addr)
        .await
        .context("SSH server failed")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

struct SshServer {
    base_dir: Arc<PathBuf>,
    pool: Arc<Pool>,
}

impl Server for SshServer {
    type Handler = SshSession;

    fn new_client(&mut self, _peer_addr: Option<std::net::SocketAddr>) -> SshSession {
        SshSession {
            base_dir: self.base_dir.clone(),
            pool: self.pool.clone(),
            child_stdin: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Per-connection session
// ---------------------------------------------------------------------------

struct SshSession {
    base_dir: Arc<PathBuf>,
    pool: Arc<Pool>,
    /// Stdin of the spawned git process; populated in `exec_request`.
    child_stdin: Option<tokio::process::ChildStdin>,
}

#[async_trait]
impl Handler for SshSession {
    type Error = anyhow::Error;

    /// DB に登録済みの SSH 公開鍵と照合して認証する。
    ///
    /// # 認証の流れ
    /// 1. 受け取った公開鍵のフィンガープリント（SHA256ハッシュ）を計算
    /// 2. DB の `ssh_keys.fingerprint` カラムと照合
    /// 3. 一致すれば Accept、なければ Reject
    async fn auth_publickey(
        &mut self,
        _user: &str,
        public_key: &PublicKey,
    ) -> Result<Auth, Self::Error> {
        let authorized = user::is_key_authorized(&self.pool, public_key)
            .await
            .unwrap_or(false);

        if authorized {
            Ok(Auth::Accept)
        } else {
            Ok(Auth::Reject {
                proceed_with_methods: None,
            })
        }
    }

    async fn channel_open_session(
        &mut self,
        _channel: Channel<Msg>,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }

    /// Called when the client sends a command, e.g.
    /// `git-receive-pack 'myrepo.git'` or `git-upload-pack 'myrepo.git'`.
    async fn exec_request(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let command = String::from_utf8_lossy(data).into_owned();

        let (git_cmd, repo_path) = parse_git_ssh_command(&command).map_err(|e| anyhow!(e))?;

        let resolved = resolve_repo_path(&self.base_dir, &repo_path).map_err(|e| anyhow!(e))?;

        let mut child = tokio::process::Command::new(&git_cmd)
            .arg(resolved.to_str().unwrap_or_default())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to spawn git process")?;

        let stdin = child.stdin.take().expect("stdin must be piped");
        let mut stdout = child.stdout.take().expect("stdout must be piped");
        let mut stderr = child.stderr.take().expect("stderr must be piped");

        self.child_stdin = Some(stdin);

        let handle = session.handle();

        // Forward stdout and stderr to the SSH channel concurrently,
        // then send the exit status.
        tokio::spawn(async move {
            let mut out_buf = vec![0u8; 32 * 1024];
            let mut err_buf = vec![0u8; 32 * 1024];
            let mut stdout_done = false;
            let mut stderr_done = false;

            while !stdout_done || !stderr_done {
                tokio::select! {
                    result = stdout.read(&mut out_buf), if !stdout_done => {
                        match result {
                            Ok(0) | Err(_) => stdout_done = true,
                            Ok(n) => {
                                if handle
                                    .data(channel, CryptoVec::from_slice(&out_buf[..n]))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                        }
                    }
                    result = stderr.read(&mut err_buf), if !stderr_done => {
                        match result {
                            Ok(0) | Err(_) => stderr_done = true,
                            Ok(n) => {
                                if handle
                                    .extended_data(
                                        channel,
                                        1,
                                        CryptoVec::from_slice(&err_buf[..n]),
                                    )
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            let exit_code = child
                .wait()
                .await
                .map(|s| s.code().unwrap_or(1) as u32)
                .unwrap_or(1);

            let _ = handle.exit_status_request(channel, exit_code).await;
            let _ = handle.eof(channel).await;
            let _ = handle.close(channel).await;
        });

        Ok(())
    }

    /// Forward data received from the client to the git process's stdin.
    async fn data(
        &mut self,
        _channel: ChannelId,
        data: &[u8],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        if let Some(stdin) = &mut self.child_stdin {
            stdin
                .write_all(data)
                .await
                .context("failed to write to git stdin")?;
        }
        Ok(())
    }

    /// Client closed its send side; drop stdin to signal EOF to the git process.
    async fn channel_eof(
        &mut self,
        _channel: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.child_stdin = None;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// ディスクからSSHホスト鍵を読み込む。ファイルが存在しなければ新しい Ed25519 鍵を
/// 生成してファイルに保存し、次回の起動で再利用できるようにする。
///
/// # ファイル形式
/// Ed25519 秘密鍵の生バイト（32バイト）をそのままファイルに書き込む。
/// ファイルのパーミッションは Unix では 0o600 (owner-read-write only) に設定する。
///
/// # Rust メモ
/// `KeyPair::Ed25519(ed25519_dalek::SigningKey)` は pub な enum バリアントなので
/// パターンマッチでフィールドを取り出せる。
/// `signing_key.to_bytes()` は `[u8; 32]` を返し、
/// `SigningKey::from_bytes(&bytes)` で復元できる。
fn load_or_create_host_key(key_path: &Path) -> Result<KeyPair> {
    if key_path.exists() {
        let bytes = std::fs::read(key_path)
            .with_context(|| format!("failed to read SSH host key from {}", key_path.display()))?;
        let bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow!("SSH host key file has invalid length (expected 32 bytes)"))?;
        let signing_key = SigningKey::from_bytes(&bytes);
        println!("SSH host key loaded from {}", key_path.display());
        return Ok(KeyPair::Ed25519(signing_key));
    }

    // 新しいホスト鍵を生成して保存する
    let key =
        KeyPair::generate_ed25519().ok_or_else(|| anyhow!("failed to generate SSH host key"))?;

    if let KeyPair::Ed25519(ref signing_key) = key {
        let bytes = signing_key.to_bytes();

        // 親ディレクトリが存在しない場合は作成する
        if let Some(parent) = key_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).with_context(|| {
                    format!(
                        "failed to create directory for SSH host key: {}",
                        parent.display()
                    )
                })?;
            }
        }

        std::fs::write(key_path, bytes)
            .with_context(|| format!("failed to write SSH host key to {}", key_path.display()))?;

        // Unix ではファイルパーミッションを 0o600 に設定する（SSH の慣例）
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(key_path, std::fs::Permissions::from_mode(0o600))
                .with_context(|| format!("failed to set permissions on {}", key_path.display()))?;
        }

        println!("SSH host key generated and saved to {}", key_path.display());
    }

    Ok(key)
}

/// Parse `git-receive-pack 'repo.git'` / `git-upload-pack 'repo.git'` and
/// variants into `(command, repo_path)`.
fn parse_git_ssh_command(command: &str) -> std::result::Result<(String, String), String> {
    let command = command.trim();

    let (cmd, rest) = if let Some(r) = command.strip_prefix("git-receive-pack") {
        ("git-receive-pack", r)
    } else if let Some(r) = command.strip_prefix("git-upload-pack") {
        ("git-upload-pack", r)
    } else if let Some(r) = command.strip_prefix("git receive-pack") {
        ("git-receive-pack", r)
    } else if let Some(r) = command.strip_prefix("git upload-pack") {
        ("git-upload-pack", r)
    } else {
        return Err(format!("unsupported git SSH command: `{command}`"));
    };

    let repo = rest.trim().trim_matches('\'').trim_matches('"');
    if repo.is_empty() {
        return Err("missing repository path in SSH command".to_string());
    }

    Ok((cmd.to_string(), repo.to_string()))
}

/// Resolve `repo_path` (relative or absolute-looking from the client's
/// perspective) to a canonical path inside `base_dir`.
fn resolve_repo_path(base_dir: &Path, repo_path: &str) -> std::result::Result<PathBuf, String> {
    // Strip a leading '/' that git clients typically include.
    let repo_path = repo_path.trim_start_matches('/');
    let relative = Path::new(repo_path);

    if relative.is_absolute() {
        return Err("absolute repository path is not allowed".to_string());
    }

    if relative
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return Err("path traversal (`..`) is not allowed".to_string());
    }

    let candidate = base_dir.join(relative);

    if !candidate.exists() {
        return Err(format!("repository `{repo_path}` not found"));
    }

    let canonical = std::fs::canonicalize(&candidate)
        .map_err(|e| format!("failed to resolve repository path: {e}"))?;

    if !canonical.starts_with(base_dir) {
        return Err("resolved path escapes GIT_BASE_DIR".to_string());
    }

    Ok(canonical)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{load_or_create_host_key, parse_git_ssh_command, resolve_repo_path};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("ssh_test_{name}_{}", stamp))
    }

    #[test]
    fn parse_receive_pack_with_single_quotes() {
        let (cmd, repo) = parse_git_ssh_command("git-receive-pack 'myrepo.git'").unwrap();
        assert_eq!(cmd, "git-receive-pack");
        assert_eq!(repo, "myrepo.git");
    }

    #[test]
    fn parse_upload_pack_without_quotes() {
        let (cmd, repo) = parse_git_ssh_command("git-upload-pack myrepo.git").unwrap();
        assert_eq!(cmd, "git-upload-pack");
        assert_eq!(repo, "myrepo.git");
    }

    #[test]
    fn parse_git_space_variant() {
        let (cmd, _) = parse_git_ssh_command("git receive-pack 'repo.git'").unwrap();
        assert_eq!(cmd, "git-receive-pack");
    }

    #[test]
    fn parse_unsupported_command_returns_error() {
        assert!(parse_git_ssh_command("git status").is_err());
    }

    #[test]
    fn resolve_valid_path_succeeds() {
        let base = temp_dir("valid");
        let repo = base.join("myrepo.git");
        fs::create_dir_all(&repo).unwrap();

        let result = resolve_repo_path(&base, "myrepo.git");
        assert!(result.is_ok());

        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn resolve_with_leading_slash_strips_it() {
        let base = temp_dir("leading_slash");
        let repo = base.join("myrepo.git");
        fs::create_dir_all(&repo).unwrap();

        let result = resolve_repo_path(&base, "/myrepo.git");
        assert!(result.is_ok());

        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn resolve_path_traversal_is_rejected() {
        let base = temp_dir("traversal");
        fs::create_dir_all(&base).unwrap();

        let result = resolve_repo_path(&base, "../escape");
        assert!(result.is_err());

        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn resolve_missing_repo_returns_error() {
        let base = temp_dir("missing");
        fs::create_dir_all(&base).unwrap();

        let result = resolve_repo_path(&base, "nonexistent.git");
        assert!(result.is_err());

        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn host_key_is_created_when_missing() {
        let dir = temp_dir("hostkey_create");
        fs::create_dir_all(&dir).unwrap();
        let key_path = dir.join("ssh_host_key");

        assert!(!key_path.exists());
        let key = load_or_create_host_key(&key_path).unwrap();
        assert!(key_path.exists());
        assert_eq!(fs::read(&key_path).unwrap().len(), 32);
        drop(key);

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn host_key_is_reloaded_consistently() {
        let dir = temp_dir("hostkey_reload");
        fs::create_dir_all(&dir).unwrap();
        let key_path = dir.join("ssh_host_key");

        // 1回目: 生成
        let key1 = load_or_create_host_key(&key_path).unwrap();
        // 2回目: 同じファイルから読み込み
        let key2 = load_or_create_host_key(&key_path).unwrap();

        // 同じ公開鍵（＝同じ秘密鍵）であることを確認
        use russh_keys::key::KeyPair;
        let pub1 = if let KeyPair::Ed25519(ref sk) = key1 {
            sk.verifying_key().to_bytes()
        } else {
            panic!("expected Ed25519");
        };
        let pub2 = if let KeyPair::Ed25519(ref sk) = key2 {
            sk.verifying_key().to_bytes()
        } else {
            panic!("expected Ed25519");
        };
        assert_eq!(pub1, pub2);

        fs::remove_dir_all(dir).unwrap();
    }
}
