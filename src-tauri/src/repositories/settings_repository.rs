use std::collections::HashMap;

use chrono::{SecondsFormat, Utc};
use sqlx::SqlitePool;

use crate::errors::AppError;

pub struct SettingsRepository<'a> {
    pool: &'a SqlitePool,
}

impl<'a> SettingsRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn all(&self) -> Result<HashMap<String, String>, AppError> {
        let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM settings")
            .fetch_all(self.pool)
            .await?;
        Ok(rows.into_iter().collect())
    }

    /// Writes all values atomically.
    pub async fn put_all(&self, values: &[(&str, String)]) -> Result<(), AppError> {
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let mut tx = self.pool.begin().await?;
        for (key, value) in values {
            sqlx::query(
                "INSERT INTO settings (key, value, updated_at) VALUES (?, ?, ?)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            )
            .bind(key)
            .bind(value)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn delete_all(&self) -> Result<(), AppError> {
        sqlx::query("DELETE FROM settings")
            .execute(self.pool)
            .await?;
        Ok(())
    }
}
