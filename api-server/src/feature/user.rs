use anyhow::{Context, Result, bail};
use russh_keys::key::PublicKey;
// `PublicKeyBase64` トレイトをインポートすることで
// `public_key_base64()` メソッドが使えるようになる。
use russh_keys::PublicKeyBase64;

use crate::db::Pool;

// ---------------------------------------------------------------------------
// モデル
// ---------------------------------------------------------------------------

/// DB の `users` テーブルに対応する構造体。
///
/// # Rust メモ
/// `#[derive(sqlx::FromRow)]` で sqlx がクエリ結果を自動的にこの構造体へ
/// マッピングしてくれる。フィールド名と DB のカラム名を一致させるだけでよい。
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub created_at: String,
}

/// DB の `ssh_keys` テーブルに対応する構造体。
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SshKey {
    pub id: i64,
    pub user_id: i64,
    pub fingerprint: String,
    pub key_type: String,
    pub key_data: String,
    pub comment: String,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// ユーザー操作
// ---------------------------------------------------------------------------

/// ユーザーを新規作成して返す。
pub async fn create_user(pool: &Pool, username: &str) -> Result<User> {
    validate_username(username)?;

    // # Rust メモ: ランタイムクエリ vs コンパイル時クエリ
    // `sqlx::query_as::<_, User>(sql)` はランタイムで SQL を実行する。
    // `sqlx::query_as!(User, sql)` はコンパイル時に SQL を検証するが、
    // `DATABASE_URL` 環境変数が必要。ここではシンプルさを優先してランタイム版を使う。
    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (username) VALUES (?) RETURNING *",
    )
    .bind(username)
    .fetch_one(pool)
    .await
    .with_context(|| format!("failed to create user `{username}`"))?;

    Ok(user)
}

/// 全ユーザーの一覧を返す。
pub async fn list_users(pool: &Pool) -> Result<Vec<User>> {
    let users = sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY id")
        .fetch_all(pool)
        .await
        .context("failed to list users")?;
    Ok(users)
}

/// username でユーザーを取得する。存在しない場合は `None`。
pub async fn find_user(pool: &Pool, username: &str) -> Result<Option<User>> {
    let user =
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
            .bind(username)
            .fetch_optional(pool)
            .await
            .with_context(|| format!("failed to find user `{username}`"))?;
    Ok(user)
}

// ---------------------------------------------------------------------------
// SSH 鍵操作
// ---------------------------------------------------------------------------

/// 指定ユーザーに SSH 公開鍵を登録する。
///
/// `key_line` は authorized_keys の1行形式（`ssh-ed25519 AAAA... comment`）を想定。
pub async fn add_ssh_key(pool: &Pool, username: &str, key_line: &str) -> Result<SshKey> {
    let user = find_user(pool, username)
        .await?
        .with_context(|| format!("user `{username}` not found"))?;

    let (key_type, key_data, comment) = parse_public_key_line(key_line)?;

    // russh_keys で公開鍵をパース・検証し、フィンガープリントを取得する。
    let public_key = decode_public_key(&key_type, &key_data)
        .context("invalid SSH public key")?;
    let fingerprint = public_key.fingerprint();

    let key = sqlx::query_as::<_, SshKey>(
        "INSERT INTO ssh_keys (user_id, fingerprint, key_type, key_data, comment)
         VALUES (?, ?, ?, ?, ?)
         RETURNING *",
    )
    .bind(user.id)
    .bind(&fingerprint)
    .bind(&key_type)
    .bind(&key_data)
    .bind(&comment)
    .fetch_one(pool)
    .await
    .context("failed to add SSH key")?;

    Ok(key)
}

/// 指定ユーザーの SSH 鍵一覧を返す。
pub async fn list_ssh_keys(pool: &Pool, username: &str) -> Result<Vec<SshKey>> {
    let user = find_user(pool, username)
        .await?
        .with_context(|| format!("user `{username}` not found"))?;

    let keys = sqlx::query_as::<_, SshKey>(
        "SELECT * FROM ssh_keys WHERE user_id = ? ORDER BY id",
    )
    .bind(user.id)
    .fetch_all(pool)
    .await
    .context("failed to list SSH keys")?;

    Ok(keys)
}

/// SSH 鍵を ID で削除する。該当するレコードが存在した場合 `true` を返す。
pub async fn delete_ssh_key(pool: &Pool, username: &str, key_id: i64) -> Result<bool> {
    let user = find_user(pool, username)
        .await?
        .with_context(|| format!("user `{username}` not found"))?;

    // # Rust メモ: `query` はマッピング先の構造体が不要なとき使う。
    // `execute` は影響を受けた行数を持つ `SqliteQueryResult` を返す。
    let result = sqlx::query("DELETE FROM ssh_keys WHERE id = ? AND user_id = ?")
        .bind(key_id)
        .bind(user.id)
        .execute(pool)
        .await
        .context("failed to delete SSH key")?;

    Ok(result.rows_affected() > 0)
}

/// SSH 認証時に使用。受け取った公開鍵のフィンガープリントが DB に存在するか確認する。
///
/// # なぜフィンガープリントを使うのか？
/// - フィンガープリントは公開鍵の SHA256 ハッシュ（`SHA256:...` 形式）
/// - DB に UNIQUE インデックスがあるので O(1) で照合できる
/// - 鍵本体をデシリアライズせず済むので高速・シンプル
pub async fn is_key_authorized(pool: &Pool, public_key: &PublicKey) -> Result<bool> {
    let fingerprint = public_key.fingerprint();

    let row = sqlx::query("SELECT id FROM ssh_keys WHERE fingerprint = ? LIMIT 1")
        .bind(&fingerprint)
        .fetch_optional(pool)
        .await
        .context("failed to check SSH key authorization")?;

    Ok(row.is_some())
}

// ---------------------------------------------------------------------------
// ヘルパー
// ---------------------------------------------------------------------------

fn validate_username(username: &str) -> Result<()> {
    if username.is_empty() {
        bail!("username must not be empty");
    }
    if username.len() > 64 {
        bail!("username must be 64 characters or fewer");
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("username may only contain letters, digits, hyphens, and underscores");
    }
    Ok(())
}

/// `"ssh-ed25519 AAAA== optional comment"` 形式を (type, data_base64, comment) に分解する。
fn parse_public_key_line(line: &str) -> Result<(String, String, String)> {
    let line = line.trim();
    let mut parts = line.splitn(3, ' ');

    let key_type = parts
        .next()
        .filter(|s| !s.is_empty())
        .context("missing key type in public key line")?
        .to_string();

    let key_data = parts
        .next()
        .filter(|s| !s.is_empty())
        .context("missing key data in public key line")?
        .to_string();

    let comment = parts.next().unwrap_or("").trim().to_string();

    Ok((key_type, key_data, comment))
}

/// Base64 エンコードされた公開鍵データを russh_keys の `PublicKey` に変換する。
///
/// # SSH 公開鍵のワイヤーフォーマット
/// authorized_keys の base64 部分は SSH ワイヤーフォーマットをエンコードしたもの。
/// Ed25519 の例: `[u32: type_len]["ssh-ed25519"][u32: key_len][32 bytes: key]`
/// `russh_keys::parse_public_key_base64` はこの base64 文字列を直接受け取れる。
fn decode_public_key(_key_type: &str, key_data_b64: &str) -> Result<PublicKey> {
    russh_keys::parse_public_key_base64(key_data_b64)
        .map_err(|e| anyhow::anyhow!("failed to parse public key: {e}"))
}

// ---------------------------------------------------------------------------
// テスト
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    /// インメモリ SQLite DB を使ったテスト用プールを作成する。
    ///
    /// # Rust メモ
    /// `":memory:"` は SQLite のインメモリ DB パス。
    /// テストが終わるとデータは破棄されるので、テスト間で干渉しない。
    async fn test_pool() -> Pool {
        db::connect(":memory:").await.expect("test DB should open")
    }

    #[tokio::test]
    async fn create_and_find_user() {
        let pool = test_pool().await;
        create_user(&pool, "alice").await.unwrap();
        let user = find_user(&pool, "alice").await.unwrap();
        assert!(user.is_some());
        assert_eq!(user.unwrap().username, "alice");
    }

    #[tokio::test]
    async fn duplicate_username_fails() {
        let pool = test_pool().await;
        create_user(&pool, "bob").await.unwrap();
        assert!(create_user(&pool, "bob").await.is_err());
    }

    #[tokio::test]
    async fn invalid_username_is_rejected() {
        let pool = test_pool().await;
        assert!(create_user(&pool, "").await.is_err());
        assert!(create_user(&pool, "bad name").await.is_err());
        assert!(create_user(&pool, "bad/name").await.is_err());
    }

    #[tokio::test]
    async fn list_users_returns_all() {
        let pool = test_pool().await;
        create_user(&pool, "alice").await.unwrap();
        create_user(&pool, "bob").await.unwrap();
        let users = list_users(&pool).await.unwrap();
        assert_eq!(users.len(), 2);
    }

    #[tokio::test]
    async fn add_ssh_key_with_real_key() {
        let pool = test_pool().await;
        create_user(&pool, "carol").await.unwrap();

        // russh_keys で鍵ペアを生成して authorized_keys 形式に変換する
        let keypair = russh_keys::key::KeyPair::generate_ed25519().unwrap();
        let pubkey = keypair.clone_public_key().unwrap();
        let key_line = pubkey_to_key_line(&pubkey, "test-key");

        add_ssh_key(&pool, "carol", &key_line).await.unwrap();
        let keys = list_ssh_keys(&pool, "carol").await.unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].key_type, "ssh-ed25519");
        assert_eq!(keys[0].comment, "test-key");
    }

    #[tokio::test]
    async fn is_key_authorized_returns_true_for_registered_key() {
        let pool = test_pool().await;
        create_user(&pool, "dave").await.unwrap();

        let keypair = russh_keys::key::KeyPair::generate_ed25519().unwrap();
        let pubkey = keypair.clone_public_key().unwrap();
        let key_line = pubkey_to_key_line(&pubkey, "");

        add_ssh_key(&pool, "dave", &key_line).await.unwrap();

        assert!(is_key_authorized(&pool, &pubkey).await.unwrap());
    }

    #[tokio::test]
    async fn is_key_authorized_returns_false_for_unknown_key() {
        let pool = test_pool().await;
        let unknown_key = russh_keys::key::KeyPair::generate_ed25519()
            .unwrap()
            .clone_public_key()
            .unwrap();
        assert!(!is_key_authorized(&pool, &unknown_key).await.unwrap());
    }

    #[tokio::test]
    async fn delete_ssh_key_removes_it() {
        let pool = test_pool().await;
        create_user(&pool, "eve").await.unwrap();

        let keypair = russh_keys::key::KeyPair::generate_ed25519().unwrap();
        let pubkey = keypair.clone_public_key().unwrap();
        add_ssh_key(&pool, "eve", &pubkey_to_key_line(&pubkey, "")).await.unwrap();

        let keys = list_ssh_keys(&pool, "eve").await.unwrap();
        let removed = delete_ssh_key(&pool, "eve", keys[0].id).await.unwrap();
        assert!(removed);
        assert!(list_ssh_keys(&pool, "eve").await.unwrap().is_empty());
    }

    #[test]
    fn parse_key_line_with_comment() {
        let (t, d, c) = parse_public_key_line("ssh-ed25519 AAAA== my comment").unwrap();
        assert_eq!(t, "ssh-ed25519");
        assert_eq!(d, "AAAA==");
        assert_eq!(c, "my comment");
    }

    #[test]
    fn parse_key_line_without_comment() {
        let (t, d, c) = parse_public_key_line("ssh-rsa BBBB==").unwrap();
        assert_eq!(t, "ssh-rsa");
        assert_eq!(d, "BBBB==");
        assert_eq!(c, "");
    }

    /// テスト用: russh_keys の PublicKey を authorized_keys 形式の文字列に変換する。
    ///
    /// # 実装メモ
    /// `PublicKeyBase64` トレイトの `public_key_base64()` が SSH ワイヤーフォーマットを
    /// base64 エンコードした文字列を返してくれる。
    fn pubkey_to_key_line(pubkey: &PublicKey, comment: &str) -> String {
        let b64 = pubkey.public_key_base64();
        if comment.is_empty() {
            format!("{} {}", pubkey.name(), b64)
        } else {
            format!("{} {} {}", pubkey.name(), b64, comment)
        }
    }
}
