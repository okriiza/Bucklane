use std::path::Path;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_CONNECTIONS: u32 = 4;

/// Options applied to every pooled connection, so foreign keys and the busy
/// timeout hold regardless of which connection serves a query (DB design §1.2).
fn base_options(options: SqliteConnectOptions) -> SqliteConnectOptions {
    options.foreign_keys(true).busy_timeout(BUSY_TIMEOUT)
}

pub async fn open(path: &Path) -> Result<SqlitePool, sqlx::Error> {
    let options = base_options(SqliteConnectOptions::new())
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal);

    SqlitePoolOptions::new()
        .max_connections(MAX_CONNECTIONS)
        .connect_with(options)
        .await
}

/// Single-connection in-memory database; each SQLite in-memory connection is
/// its own database, so the pool must not open more than one.
#[cfg(test)]
pub async fn open_in_memory() -> Result<SqlitePool, sqlx::Error> {
    use std::str::FromStr;

    let options = base_options(SqliteConnectOptions::from_str("sqlite::memory:")?);

    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
}
