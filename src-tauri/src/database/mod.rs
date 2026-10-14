mod migrations;
mod sqlite;

use std::path::Path;

use sqlx::SqlitePool;

use crate::errors::{AppError, ErrorCode};

const DATABASE_FILE: &str = "bucklane.db";

/// Opens the application database and applies pending migrations.
///
/// On failure the caller must keep the app running without database-backed
/// features rather than continuing with an unknown schema (DB design §4.1).
pub async fn init(data_dir: &Path) -> Result<SqlitePool, AppError> {
    create_private_dir(data_dir).map_err(|err| {
        tracing::error!(error = %err, "failed to create application data directory");
        unavailable()
    })?;

    let pool = sqlite::open(&data_dir.join(DATABASE_FILE))
        .await
        .map_err(|err| {
            tracing::error!(error = %err, "failed to open database");
            unavailable()
        })?;

    migrations::run(&pool).await.map_err(|err| {
        tracing::error!(error = %err, "database migration failed");
        unavailable()
    })?;

    Ok(pool)
}

fn unavailable() -> AppError {
    AppError::new(
        ErrorCode::DatabaseUnavailable,
        "Bucklane could not open its local database.",
    )
}

/// Creates the data directory readable only by the current user where the
/// platform supports it (DB design §9).
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }

    Ok(())
}

/// Migrated in-memory database for service and repository tests.
#[cfg(test)]
pub async fn test_pool() -> SqlitePool {
    let pool = sqlite::open_in_memory().await.unwrap();
    migrations::run(&pool).await.unwrap();
    pool
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn migrated_pool() -> SqlitePool {
        test_pool().await
    }

    async fn insert_connection(
        pool: &SqlitePool,
        id: &str,
        provider: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO connections
                (id, name, provider, region, access_key_id, credential_ref, created_at, updated_at)
             VALUES (?, 'Test', ?, 'us-east-1', 'AKIA', ?, '2026-10-14T00:00:00Z', '2026-10-14T00:00:00Z')",
        )
        .bind(id)
        .bind(provider)
        .bind(format!("ref-{id}"))
        .execute(pool)
        .await
        .map(|_| ())
    }

    async fn insert_transfer(pool: &SqlitePool, id: &str, connection_id: &str) {
        sqlx::query(
            "INSERT INTO transfers
                (id, connection_id, connection_name_snapshot, direction, bucket, object_key, local_path, created_at)
             VALUES (?, ?, 'Test', 'UPLOAD', 'b', 'k', '/tmp/f', '2026-10-14T00:00:00Z')",
        )
        .bind(id)
        .bind(connection_id)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn count(pool: &SqlitePool, sql: &'static str) -> i64 {
        sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
    }

    // DB-001
    #[tokio::test]
    async fn fresh_database_has_all_tables() {
        let pool = migrated_pool().await;
        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '_sqlx%'
             ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(
            tables,
            [
                "connection_buckets",
                "connections",
                "settings",
                "transfer_parts",
                "transfers"
            ]
        );
    }

    // DB-002
    #[tokio::test]
    async fn rerunning_migrations_is_a_no_op() {
        let pool = migrated_pool().await;
        migrations::run(&pool).await.unwrap();
    }

    // DB-003
    #[tokio::test]
    async fn rejects_unknown_provider() {
        let pool = migrated_pool().await;
        assert!(insert_connection(&pool, "c1", "dropbox").await.is_err());
        assert!(
            insert_connection(&pool, "c2", "digitalocean_spaces")
                .await
                .is_ok()
        );
    }

    // DB-004
    #[tokio::test]
    async fn rejects_duplicate_bucket_bookmark() {
        let pool = migrated_pool().await;
        insert_connection(&pool, "c1", "aws").await.unwrap();
        let insert = "INSERT INTO connection_buckets (id, connection_id, bucket_name, created_at)
                      VALUES (?, 'c1', 'photos', '2026-10-14T00:00:00Z')";

        sqlx::query(insert).bind("b1").execute(&pool).await.unwrap();
        assert!(sqlx::query(insert).bind("b2").execute(&pool).await.is_err());
    }

    // DB-005, DB-006
    #[tokio::test]
    async fn deleting_connection_cascades_bookmarks_and_keeps_history() {
        let pool = migrated_pool().await;
        insert_connection(&pool, "c1", "minio").await.unwrap();
        sqlx::query(
            "INSERT INTO connection_buckets (id, connection_id, bucket_name, created_at)
             VALUES ('b1', 'c1', 'photos', '2026-10-14T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        insert_transfer(&pool, "t1", "c1").await;

        sqlx::query("DELETE FROM connections WHERE id = 'c1'")
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(
            count(&pool, "SELECT COUNT(*) FROM connection_buckets").await,
            0
        );
        assert_eq!(
            count(
                &pool,
                "SELECT COUNT(*) FROM transfers WHERE id = 't1' AND connection_id IS NULL"
            )
            .await,
            1
        );
    }

    // DB-007
    #[tokio::test]
    async fn deleting_transfer_cascades_parts() {
        let pool = migrated_pool().await;
        insert_connection(&pool, "c1", "aws").await.unwrap();
        insert_transfer(&pool, "t1", "c1").await;
        sqlx::query(
            "INSERT INTO transfer_parts (id, transfer_id, part_number, etag, size_bytes)
             VALUES ('p1', 't1', 1, 'etag', 100)",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("DELETE FROM transfers WHERE id = 't1'")
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(count(&pool, "SELECT COUNT(*) FROM transfer_parts").await, 0);
    }

    // DB-011
    #[tokio::test]
    async fn rejects_negative_byte_counters() {
        let pool = migrated_pool().await;
        insert_connection(&pool, "c1", "aws").await.unwrap();
        insert_transfer(&pool, "t1", "c1").await;

        let result = sqlx::query("UPDATE transfers SET bytes_transferred = -1 WHERE id = 't1'")
            .execute(&pool)
            .await;
        assert!(result.is_err());
    }

    // DB-013
    #[tokio::test]
    async fn enforces_foreign_keys() {
        let pool = migrated_pool().await;
        let result = sqlx::query(
            "INSERT INTO transfer_parts (id, transfer_id, part_number, etag, size_bytes)
             VALUES ('p1', 'missing', 1, 'etag', 100)",
        )
        .execute(&pool)
        .await;
        assert!(result.is_err());
    }
}
