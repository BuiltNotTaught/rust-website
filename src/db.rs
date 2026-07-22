use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;

/// Opens (creating if needed) the SQLite database and ensures the schema exists.
pub async fn init_db(path: &str) -> Result<sqlx::SqlitePool> {
    let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path))?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(20)
        .connect_with(options)
        .await?;

    // Blog posts.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS posts (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          title TEXT NOT NULL,
          slug TEXT NOT NULL UNIQUE,
          content TEXT NOT NULL,
          created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .execute(&pool)
    .await?;

    // Contact-form submissions.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS notes (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          body TEXT NOT NULL,
          created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .execute(&pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_posts_created_at ON posts(created_at DESC)")
        .execute(&pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_posts_slug ON posts(slug)")
        .execute(&pool)
        .await?;

    Ok(pool)
}
