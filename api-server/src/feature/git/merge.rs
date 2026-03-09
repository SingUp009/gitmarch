use std::path::Path;

pub async fn execute(cwd: &Path, args: &[String]) -> Result<std::process::Output, std::io::Error> {
    tokio::process::Command::new("git")
        .arg("merge")
        .args(args)
        .current_dir(cwd)
        .output()
        .await
}
