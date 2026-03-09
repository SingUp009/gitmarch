mod db;
mod feature;
mod presentation;

use std::io::{Error as IoError, ErrorKind};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let base_dir = resolve_base_dir()?;

    let db_path = std::env::var("DB_PATH").unwrap_or_else(|_| "gitmarch.db".to_string());
    let pool = Arc::new(db::connect(&db_path).await?);

    println!("Database opened at {db_path}");

    tokio::try_join!(
        presentation::http::serve(base_dir.clone(), pool.clone()),
        presentation::ssh::serve(base_dir, pool),
    )?;

    Ok(())
}

fn resolve_base_dir() -> Result<PathBuf> {
    let raw = std::env::var("GIT_BASE_DIR")
        .map_err(|_| IoError::new(ErrorKind::InvalidInput, "GIT_BASE_DIR is required"))?;

    let path = std::fs::canonicalize(&raw).with_context(|| {
        format!("failed to canonicalize GIT_BASE_DIR (`{raw}`)")
    })?;

    if !path.is_dir() {
        return Err(IoError::new(
            ErrorKind::InvalidInput,
            "GIT_BASE_DIR must point to a directory",
        )
        .into());
    }

    Ok(path)
}
