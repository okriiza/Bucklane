use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

use crate::domain::transfer::{StoredTransfer, TransferDirection, TransferStatus};
use crate::errors::AppError;

macro_rules! columns {
    () => {
        "id, connection_id, connection_name_snapshot, direction, bucket, object_key, local_path, \
         bytes_total, bytes_transferred, status, retry_count, error_code, error_message, \
         multipart_upload_id, overwrite, created_at, started_at, completed_at"
    };
}

/// How many transfers the panel shows; older history stays in the database.
const LIST_LIMIT: i64 = 500;

#[derive(Clone)]
pub struct TransferRepository {
    pool: SqlitePool,
}

impl TransferRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, t: &StoredTransfer) -> Result<(), AppError> {
        sqlx::query(concat!(
            "INSERT INTO transfers (",
            columns!(),
            ") VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        ))
        .bind(&t.id)
        .bind(&t.connection_id)
        .bind(&t.connection_name_snapshot)
        .bind(t.direction.as_db_str())
        .bind(&t.bucket)
        .bind(&t.object_key)
        .bind(&t.local_path)
        .bind(t.bytes_total)
        .bind(t.bytes_transferred)
        .bind(t.status.as_db_str())
        .bind(t.retry_count)
        .bind(&t.error_code)
        .bind(&t.error_message)
        .bind(&t.multipart_upload_id)
        .bind(t.overwrite)
        .bind(&t.created_at)
        .bind(&t.started_at)
        .bind(&t.completed_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get(&self, id: &str) -> Result<Option<StoredTransfer>, AppError> {
        let row = sqlx::query(concat!(
            "SELECT ",
            columns!(),
            " FROM transfers WHERE id = ?"
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(from_row).transpose()
    }

    pub async fn list_recent(&self) -> Result<Vec<StoredTransfer>, AppError> {
        let rows = sqlx::query(concat!(
            "SELECT ",
            columns!(),
            " FROM transfers ORDER BY created_at DESC LIMIT ?"
        ))
        .bind(LIST_LIMIT)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(from_row).collect()
    }

    /// Writes every mutable column. The transfer task is the only writer of a
    /// row while it runs, so whole-row updates cannot clobber each other.
    pub async fn update(&self, t: &StoredTransfer) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE transfers SET
                bytes_total = ?, bytes_transferred = ?, status = ?, retry_count = ?,
                error_code = ?, error_message = ?, multipart_upload_id = ?, overwrite = ?,
                started_at = ?, completed_at = ?, object_key = ?, local_path = ?
             WHERE id = ?",
        )
        .bind(t.bytes_total)
        .bind(t.bytes_transferred)
        .bind(t.status.as_db_str())
        .bind(t.retry_count)
        .bind(&t.error_code)
        .bind(&t.error_message)
        .bind(&t.multipart_upload_id)
        .bind(t.overwrite)
        .bind(&t.started_at)
        .bind(&t.completed_at)
        .bind(&t.object_key)
        .bind(&t.local_path)
        .bind(&t.id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_progress(&self, id: &str, bytes: i64) -> Result<(), AppError> {
        sqlx::query("UPDATE transfers SET bytes_transferred = ? WHERE id = ?")
            .bind(bytes)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn add_part(
        &self,
        transfer_id: &str,
        part_number: i32,
        etag: &str,
        size: i64,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO transfer_parts (id, transfer_id, part_number, etag, size_bytes)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (transfer_id, part_number) DO UPDATE SET etag = excluded.etag, size_bytes = excluded.size_bytes",
        )
        .bind(uuid::Uuid::now_v7().to_string())
        .bind(transfer_id)
        .bind(part_number)
        .bind(etag)
        .bind(size)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_parts(&self, transfer_id: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM transfer_parts WHERE transfer_id = ?")
            .bind(transfer_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Transfers left QUEUED or RUNNING by a previous session (TSD §9.2).
    pub async fn unfinished(&self) -> Result<Vec<StoredTransfer>, AppError> {
        let rows = sqlx::query(concat!(
            "SELECT ",
            columns!(),
            " FROM transfers WHERE status IN ('QUEUED', 'RUNNING')"
        ))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(from_row).collect()
    }

    /// Ended transfers whose multipart upload may still exist remotely.
    pub async fn with_orphaned_uploads(&self) -> Result<Vec<StoredTransfer>, AppError> {
        let rows = sqlx::query(concat!(
            "SELECT ",
            columns!(),
            " FROM transfers WHERE multipart_upload_id IS NOT NULL
               AND status IN ('FAILED', 'CANCELED')"
        ))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(from_row).collect()
    }

    /// Removes finished history, keeping rows that still track a multipart
    /// upload awaiting cleanup. Returns the number of rows removed.
    pub async fn clear_history(&self) -> Result<u64, AppError> {
        let result = sqlx::query(
            "DELETE FROM transfers
             WHERE status IN ('COMPLETED', 'FAILED', 'CANCELED') AND multipart_upload_id IS NULL",
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}

fn from_row(row: &SqliteRow) -> Result<StoredTransfer, AppError> {
    let direction: String = row.try_get("direction")?;
    let status: String = row.try_get("status")?;
    Ok(StoredTransfer {
        id: row.try_get("id")?,
        connection_id: row.try_get("connection_id")?,
        connection_name_snapshot: row.try_get("connection_name_snapshot")?,
        direction: TransferDirection::from_db_str(&direction)
            .ok_or_else(|| AppError::internal("Unknown transfer direction in database."))?,
        bucket: row.try_get("bucket")?,
        object_key: row.try_get("object_key")?,
        local_path: row.try_get("local_path")?,
        bytes_total: row.try_get("bytes_total")?,
        bytes_transferred: row.try_get("bytes_transferred")?,
        status: TransferStatus::from_db_str(&status)
            .ok_or_else(|| AppError::internal("Unknown transfer status in database."))?,
        retry_count: row.try_get("retry_count")?,
        error_code: row.try_get("error_code")?,
        error_message: row.try_get("error_message")?,
        multipart_upload_id: row.try_get("multipart_upload_id")?,
        overwrite: row.try_get("overwrite")?,
        created_at: row.try_get("created_at")?,
        started_at: row.try_get("started_at")?,
        completed_at: row.try_get("completed_at")?,
    })
}
