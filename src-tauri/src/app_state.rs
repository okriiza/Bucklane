use sqlx::SqlitePool;

use crate::errors::AppError;

/// Shared state managed by Tauri and injected into commands.
pub struct AppState {
    /// The database may be unavailable if startup migration failed; commands
    /// that need it surface `DATABASE_UNAVAILABLE` instead of crashing.
    db: Result<SqlitePool, AppError>,
}

impl AppState {
    pub fn new(db: Result<SqlitePool, AppError>) -> Self {
        Self { db }
    }

    pub fn db(&self) -> Result<&SqlitePool, AppError> {
        self.db.as_ref().map_err(Clone::clone)
    }
}
