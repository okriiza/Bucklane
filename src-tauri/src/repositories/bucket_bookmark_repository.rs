use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

use crate::domain::bucket::BucketBookmark;
use crate::errors::AppError;

pub struct BucketBookmarkRepository<'a> {
    pool: &'a SqlitePool,
}

impl<'a> BucketBookmarkRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self, connection_id: &str) -> Result<Vec<BucketBookmark>, AppError> {
        let rows = sqlx::query(
            "SELECT id, connection_id, bucket_name, display_name, created_at
             FROM connection_buckets WHERE connection_id = ?
             ORDER BY bucket_name COLLATE NOCASE",
        )
        .bind(connection_id)
        .fetch_all(self.pool)
        .await?;
        rows.iter().map(from_row).collect()
    }

    pub async fn get(&self, id: &str) -> Result<Option<BucketBookmark>, AppError> {
        let row = sqlx::query(
            "SELECT id, connection_id, bucket_name, display_name, created_at
             FROM connection_buckets WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(self.pool)
        .await?;
        row.as_ref().map(from_row).transpose()
    }

    /// Returns `false` if the bucket is already bookmarked for this connection.
    pub async fn insert(&self, bookmark: &BucketBookmark) -> Result<bool, AppError> {
        let result = sqlx::query(
            "INSERT INTO connection_buckets (id, connection_id, bucket_name, display_name, created_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (connection_id, bucket_name) DO NOTHING",
        )
        .bind(&bookmark.id)
        .bind(&bookmark.connection_id)
        .bind(&bookmark.bucket_name)
        .bind(&bookmark.display_name)
        .bind(&bookmark.created_at)
        .execute(self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn update_display_name(
        &self,
        id: &str,
        display_name: Option<&str>,
    ) -> Result<bool, AppError> {
        let result = sqlx::query("UPDATE connection_buckets SET display_name = ? WHERE id = ?")
            .bind(display_name)
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn delete(&self, id: &str) -> Result<bool, AppError> {
        let result = sqlx::query("DELETE FROM connection_buckets WHERE id = ?")
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(result.rows_affected() == 1)
    }
}

fn from_row(row: &SqliteRow) -> Result<BucketBookmark, AppError> {
    Ok(BucketBookmark {
        id: row.try_get("id")?,
        connection_id: row.try_get("connection_id")?,
        bucket_name: row.try_get("bucket_name")?,
        display_name: row.try_get("display_name")?,
        created_at: row.try_get("created_at")?,
    })
}
