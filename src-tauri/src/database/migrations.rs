use sqlx::SqlitePool;
use sqlx::migrate::{MigrateError, Migrator};

/// Versioned migrations embedded from `src-tauri/migrations` at compile time.
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub async fn run(pool: &SqlitePool) -> Result<(), MigrateError> {
    MIGRATOR.run(pool).await
}

/// Applies only the migrations older than `version`, to test a migration
/// against data written by the previous schema.
#[cfg(test)]
pub async fn run_before(pool: &SqlitePool, version: i64) -> Result<(), MigrateError> {
    let older = MIGRATOR
        .migrations
        .iter()
        .filter(|m| m.version < version)
        .cloned()
        .collect();
    let migrator = Migrator {
        migrations: std::borrow::Cow::Owned(older),
        ..Migrator::DEFAULT
    };
    migrator.run(pool).await
}
