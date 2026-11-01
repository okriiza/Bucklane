use sqlx::SqlitePool;

use crate::errors::AppError;

/// Per-bucket public base URLs (custom domain or CDN).
pub struct BucketPublicUrlRepository<'a> {
    pool: &'a SqlitePool,
}

impl<'a> BucketPublicUrlRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get(&self, connection_id: &str, bucket: &str) -> Result<Option<String>, AppError> {
        Ok(sqlx::query_scalar(
            "SELECT base_url FROM bucket_public_urls WHERE connection_id = ? AND bucket_name = ?",
        )
        .bind(connection_id)
        .bind(bucket)
        .fetch_optional(self.pool)
        .await?)
    }

    pub async fn set(
        &self,
        connection_id: &str,
        bucket: &str,
        base_url: &str,
        updated_at: &str,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO bucket_public_urls (connection_id, bucket_name, base_url, updated_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT (connection_id, bucket_name)
             DO UPDATE SET base_url = excluded.base_url, updated_at = excluded.updated_at",
        )
        .bind(connection_id)
        .bind(bucket)
        .bind(base_url)
        .bind(updated_at)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, connection_id: &str, bucket: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM bucket_public_urls WHERE connection_id = ? AND bucket_name = ?")
            .bind(connection_id)
            .bind(bucket)
            .execute(self.pool)
            .await?;
        Ok(())
    }
}
