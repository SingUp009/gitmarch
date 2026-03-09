use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateRepoError {
    InvalidName(String),
    AlreadyExists(String),
    ExecutionFailed(String),
}

impl std::fmt::Display for CreateRepoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidName(msg) => write!(f, "invalid repository name: {msg}"),
            Self::AlreadyExists(name) => write!(f, "repository `{name}` already exists"),
            Self::ExecutionFailed(msg) => write!(f, "failed to create repository: {msg}"),
        }
    }
}

/// Create a new bare git repository at `base_dir/<name>`.
/// `name` must end with `.git` and must not contain path separators or `..`.
pub async fn create_bare_repo(
    base_dir: &Path,
    name: &str,
) -> Result<PathBuf, CreateRepoError> {
    validate_repo_name(name)?;

    let repo_path = base_dir.join(name);

    if repo_path.exists() {
        return Err(CreateRepoError::AlreadyExists(name.to_string()));
    }

    let output = tokio::process::Command::new("git")
        .arg("init")
        .arg("--bare")
        .arg(&repo_path)
        .output()
        .await
        .map_err(|e| CreateRepoError::ExecutionFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(CreateRepoError::ExecutionFailed(stderr));
    }

    Ok(repo_path)
}

fn validate_repo_name(name: &str) -> Result<(), CreateRepoError> {
    if name.is_empty() {
        return Err(CreateRepoError::InvalidName(
            "name must not be empty".to_string(),
        ));
    }

    if !name.ends_with(".git") {
        return Err(CreateRepoError::InvalidName(
            "name must end with `.git`".to_string(),
        ));
    }

    let path = Path::new(name);

    if path.is_absolute() {
        return Err(CreateRepoError::InvalidName(
            "name must not be an absolute path".to_string(),
        ));
    }

    let has_invalid_component = path.components().any(|c| {
        matches!(
            c,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    });
    if has_invalid_component {
        return Err(CreateRepoError::InvalidName(
            "name must not contain path separators or `..`".to_string(),
        ));
    }

    // Disallow slashes (single component only).
    if path.components().count() != 1 {
        return Err(CreateRepoError::InvalidName(
            "name must be a single path component (no slashes)".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CreateRepoError, create_bare_repo, validate_repo_name};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("repo_test_{label}_{stamp}"))
    }

    // --- validate_repo_name ---

    #[test]
    fn valid_name_is_accepted() {
        assert!(validate_repo_name("myrepo.git").is_ok());
    }

    #[test]
    fn name_without_git_suffix_is_rejected() {
        assert!(matches!(
            validate_repo_name("myrepo"),
            Err(CreateRepoError::InvalidName(_))
        ));
    }

    #[test]
    fn empty_name_is_rejected() {
        assert!(matches!(
            validate_repo_name(""),
            Err(CreateRepoError::InvalidName(_))
        ));
    }

    #[test]
    fn name_with_slash_is_rejected() {
        assert!(matches!(
            validate_repo_name("org/repo.git"),
            Err(CreateRepoError::InvalidName(_))
        ));
    }

    #[test]
    fn name_with_parent_dir_is_rejected() {
        assert!(matches!(
            validate_repo_name("../escape.git"),
            Err(CreateRepoError::InvalidName(_))
        ));
    }

    // --- create_bare_repo ---

    #[tokio::test]
    async fn creates_bare_repo_successfully() {
        let base = temp_dir("create_ok");
        fs::create_dir_all(&base).unwrap();

        let repo_path = create_bare_repo(&base, "myrepo.git").await.unwrap();
        assert!(repo_path.join("HEAD").exists());

        fs::remove_dir_all(base).unwrap();
    }

    #[tokio::test]
    async fn returns_error_when_repo_already_exists() {
        let base = temp_dir("already_exists");
        fs::create_dir_all(base.join("myrepo.git")).unwrap();

        let err = create_bare_repo(&base, "myrepo.git").await.unwrap_err();
        assert!(matches!(err, CreateRepoError::AlreadyExists(_)));

        fs::remove_dir_all(base).unwrap();
    }
}
