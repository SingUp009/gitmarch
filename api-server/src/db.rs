use std::path::Path;

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};

pub type Pool = SqlitePool;

/// データベースファイルを開き（なければ作成し）、マイグレーションを適用して
/// 接続プールを返す。
///
/// # Rust メモ
/// `impl AsRef<Path>` は「`Path` として参照できるものなら何でも受け取る」という
/// トレイト境界。`&str` や `PathBuf` をそのまま渡せるので呼び出しが楽になる。
pub async fn connect(db_path: impl AsRef<Path>) -> Result<Pool> {
    // # Rust メモ: SQLite インメモリ DB の注意点
    // `:memory:` を使うと接続ごとに別々のDBが作成される。
    // プールが複数接続を持つと「マイグレーション済みの接続」と
    // 「空のDB接続」が混在してしまう。
    // テスト等でインメモリDBを使う場合は接続数を1に制限することで回避する。
    let is_memory = db_path.as_ref() == std::path::Path::new(":memory:");
    let max_connections = if is_memory { 1 } else { 5 };

    let options = SqliteConnectOptions::new()
        .filename(db_path)
        // ファイルが存在しない場合は自動で作成する
        .create_if_missing(true)
        // 外部キー制約を有効にする（SQLite はデフォルトで無効）
        .pragma("foreign_keys", "ON");

    let pool = SqlitePoolOptions::new()
        .max_connections(max_connections)
        .connect_with(options)
        .await
        .context("failed to open database")?;

    // migrations/ フォルダの SQL を順番に実行する。
    // 既に適用済みのものはスキップされる（_sqlx_migrations テーブルで管理）。
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("failed to run database migrations")?;

    Ok(pool)
}
