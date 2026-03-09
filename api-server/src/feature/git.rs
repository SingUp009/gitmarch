use std::path::{Component, Path, PathBuf};

mod branch;
mod checkout;
mod merge;
mod pull;
mod push;
mod switch;

const ALLOWED_COMMANDS: [&str; 6] = ["branch", "checkout", "merge", "pull", "push", "switch"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunGitOutput {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub cwd: String,
    pub command: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunGitError {
    InvalidInput(String),
    ExecutionFailed(String),
}

pub async fn run_git_command(
    base_dir: &Path,
    relative_path: &str,
    cmd: &str,
    args: &[String],
) -> Result<RunGitOutput, RunGitError> {
    let cmd = cmd.trim();
    if cmd.is_empty() {
        return Err(RunGitError::InvalidInput(
            "query parameter `cmd` must not be empty".to_string(),
        ));
    }

    let base_dir = std::fs::canonicalize(base_dir).map_err(|error| {
        RunGitError::ExecutionFailed(format!("failed to access base directory: {error}"))
    })?;

    let resolved_path = resolve_target_dir(&base_dir, relative_path)?;

    let mut command_args = Vec::with_capacity(1 + args.len());
    command_args.push(cmd.to_string());
    command_args.extend(args.iter().cloned());

    let output = execute_operation(cmd, &resolved_path, args).await?;
    let exit_code = output.status.code();

    Ok(RunGitOutput {
        success: matches!(exit_code, Some(0)),
        exit_code,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        cwd: resolved_path.display().to_string(),
        command: command_args,
    })
}

async fn execute_operation(
    cmd: &str,
    cwd: &Path,
    args: &[String],
) -> Result<std::process::Output, RunGitError> {
    let output = match cmd {
        "branch" => branch::execute(cwd, args).await,
        "checkout" => checkout::execute(cwd, args).await,
        "merge" => merge::execute(cwd, args).await,
        "pull" => pull::execute(cwd, args).await,
        "push" => push::execute(cwd, args).await,
        "switch" => switch::execute(cwd, args).await,
        _ => {
            return Err(RunGitError::InvalidInput(format!(
                "unsupported `cmd`: {cmd}. allowed commands: {}",
                ALLOWED_COMMANDS.join(", ")
            )));
        }
    };

    output.map_err(|error| {
        RunGitError::ExecutionFailed(format!("failed to execute git command: {error}"))
    })
}

fn resolve_target_dir(base_dir: &Path, relative_path: &str) -> Result<PathBuf, RunGitError> {
    if relative_path.trim().is_empty() {
        return Err(RunGitError::InvalidInput(
            "query parameter `path` must not be empty".to_string(),
        ));
    }

    let relative = Path::new(relative_path);
    if relative.is_absolute() {
        return Err(RunGitError::InvalidInput(
            "`path` must be relative to GIT_BASE_DIR".to_string(),
        ));
    }

    if relative.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(RunGitError::InvalidInput(
            "`path` must not contain `..` or absolute components".to_string(),
        ));
    }

    let candidate = base_dir.join(relative);
    let canonical = std::fs::canonicalize(&candidate)
        .map_err(|error| RunGitError::InvalidInput(format!("failed to resolve `path`: {error}")))?;

    if !canonical.starts_with(base_dir) {
        return Err(RunGitError::InvalidInput(
            "resolved path is outside GIT_BASE_DIR".to_string(),
        ));
    }

    if !canonical.is_dir() {
        return Err(RunGitError::InvalidInput(
            "resolved `path` is not a directory".to_string(),
        ));
    }

    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::{RunGitError, run_git_command};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();

        path.push(format!(
            "api_server_{name}_{}_{}",
            std::process::id(),
            stamp
        ));
        path
    }

    fn init_repo(path: &Path) {
        fs::create_dir_all(path).expect("failed to create repository directory");
        let output = Command::new("git")
            .arg("init")
            .current_dir(path)
            .output()
            .expect("failed to run git init");

        assert!(
            output.status.success(),
            "git init failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[tokio::test]
    async fn run_git_branch_in_repo_returns_success() {
        let base = unique_temp_path("base_ok");
        let repo = base.join("repo");
        init_repo(&repo);

        let args: Vec<String> = vec![];
        let output = run_git_command(&base, "repo", "branch", &args)
            .await
            .expect("git branch should run");

        assert_eq!(output.exit_code, Some(0));
        assert!(output.success);
        assert_eq!(output.command, vec!["branch"]);

        fs::remove_dir_all(base).expect("cleanup should succeed");
    }

    #[tokio::test]
    async fn unsupported_command_is_rejected() {
        let base = unique_temp_path("base_unsupported");
        let repo = base.join("repo");
        init_repo(&repo);

        let args: Vec<String> = vec![];
        let error = run_git_command(&base, "repo", "status", &args)
            .await
            .expect_err("unsupported command should be rejected");

        match error {
            RunGitError::InvalidInput(message) => {
                assert!(message.contains("unsupported `cmd`"));
                assert!(message.contains("branch"));
            }
            _ => panic!("expected invalid input for unsupported command"),
        }

        fs::remove_dir_all(base).expect("cleanup should succeed");
    }

    #[tokio::test]
    async fn absolute_path_is_rejected() {
        let base = unique_temp_path("base_absolute");
        fs::create_dir_all(&base).expect("failed to create base dir");

        let args: Vec<String> = vec![];
        let error = run_git_command(&base, "/tmp", "branch", &args)
            .await
            .expect_err("absolute path should be rejected");

        assert!(matches!(error, RunGitError::InvalidInput(_)));

        fs::remove_dir_all(base).expect("cleanup should succeed");
    }

    #[tokio::test]
    async fn parent_dir_in_path_is_rejected() {
        let base = unique_temp_path("base_parent");
        fs::create_dir_all(&base).expect("failed to create base dir");

        let args: Vec<String> = vec![];
        let error = run_git_command(&base, "../repo", "branch", &args)
            .await
            .expect_err("parent dir should be rejected");

        assert!(matches!(error, RunGitError::InvalidInput(_)));

        fs::remove_dir_all(base).expect("cleanup should succeed");
    }

    #[tokio::test]
    async fn non_repo_directory_returns_success_false_with_200_semantics() {
        let base = unique_temp_path("base_non_repo");
        let target = base.join("not_a_repo");
        fs::create_dir_all(&target).expect("failed to create target dir");

        let args: Vec<String> = vec![];
        let output = run_git_command(&base, "not_a_repo", "branch", &args)
            .await
            .expect("command should execute even for non-repo dir");

        assert!(!output.success);
        assert_ne!(output.exit_code, Some(0));

        fs::remove_dir_all(base).expect("cleanup should succeed");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn symlink_escape_is_rejected() {
        use std::os::unix::fs::symlink;

        let base = unique_temp_path("base_symlink");
        let outside = unique_temp_path("outside_symlink");

        fs::create_dir_all(&base).expect("failed to create base dir");
        fs::create_dir_all(&outside).expect("failed to create outside dir");

        symlink(&outside, base.join("linked"))
            .expect("failed to create symlink from base to outside dir");

        let args: Vec<String> = vec![];
        let error = run_git_command(&base, "linked", "branch", &args)
            .await
            .expect_err("symlink escape should be rejected");

        assert!(matches!(error, RunGitError::InvalidInput(_)));

        fs::remove_dir_all(base).expect("cleanup base should succeed");
        fs::remove_dir_all(outside).expect("cleanup outside should succeed");
    }
}
