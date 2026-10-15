use sqlx::SqlitePool;
use sqlx::migrate::{MigrateError, Migrator};

/// Versioned migrations embedded from `src-tauri/migrations` at compile time.
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub async fn run(pool: &SqlitePool) -> Result<(), MigrateError> {
    MIGRATOR.run(pool).await
}
