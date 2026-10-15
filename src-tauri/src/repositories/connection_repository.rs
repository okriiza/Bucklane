use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

use crate::domain::connection::{ProviderType, StorageConnection};
use crate::errors::AppError;

/// Column list as a literal so queries stay `&'static str` via `concat!`.
macro_rules! columns {
    () => {
        "id, name, provider, endpoint, region, access_key_id, credential_ref, \
         force_path_style, allow_insecure_http, created_at, updated_at"
    };
}

pub struct ConnectionRepository<'a> {
    pool: &'a SqlitePool,
}

impl<'a> ConnectionRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> Result<Vec<StorageConnection>, AppError> {
        let rows = sqlx::query(concat!(
            "SELECT ",
            columns!(),
            " FROM connections ORDER BY name COLLATE NOCASE, created_at"
        ))
        .fetch_all(self.pool)
        .await?;
        rows.iter().map(from_row).collect()
    }

    pub async fn get(&self, id: &str) -> Result<Option<StorageConnection>, AppError> {
        let row = sqlx::query(concat!(
            "SELECT ",
            columns!(),
            " FROM connections WHERE id = ?"
        ))
        .bind(id)
        .fetch_optional(self.pool)
        .await?;
        row.as_ref().map(from_row).transpose()
    }

    pub async fn insert(&self, c: &StorageConnection) -> Result<(), AppError> {
        sqlx::query(concat!(
            "INSERT INTO connections (",
            columns!(),
            ") VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        ))
        .bind(&c.id)
        .bind(&c.name)
        .bind(c.provider.as_db_str())
        .bind(&c.endpoint)
        .bind(&c.region)
        .bind(&c.access_key_id)
        .bind(&c.credential_ref)
        .bind(c.force_path_style)
        .bind(c.allow_insecure_http)
        .bind(&c.created_at)
        .bind(&c.updated_at)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    /// Updates every mutable column, including `credential_ref`.
    /// Returns `false` if the connection no longer exists.
    pub async fn update(&self, c: &StorageConnection) -> Result<bool, AppError> {
        let result = sqlx::query(
            "UPDATE connections SET
                name = ?, provider = ?, endpoint = ?, region = ?, access_key_id = ?,
                credential_ref = ?, force_path_style = ?, allow_insecure_http = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(&c.name)
        .bind(c.provider.as_db_str())
        .bind(&c.endpoint)
        .bind(&c.region)
        .bind(&c.access_key_id)
        .bind(&c.credential_ref)
        .bind(c.force_path_style)
        .bind(c.allow_insecure_http)
        .bind(&c.updated_at)
        .bind(&c.id)
        .execute(self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    /// Returns `false` if the connection did not exist.
    pub async fn delete(&self, id: &str) -> Result<bool, AppError> {
        let result = sqlx::query("DELETE FROM connections WHERE id = ?")
            .bind(id)
            .execute(self.pool)
            .await?;
        Ok(result.rows_affected() == 1)
    }
}

fn from_row(row: &SqliteRow) -> Result<StorageConnection, AppError> {
    let provider: String = row.try_get("provider")?;
    Ok(StorageConnection {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        provider: ProviderType::from_db_str(&provider).ok_or_else(|| {
            AppError::internal(format!("Unknown provider stored in database: {provider}"))
        })?,
        endpoint: row.try_get("endpoint")?,
        region: row.try_get("region")?,
        access_key_id: row.try_get("access_key_id")?,
        credential_ref: row.try_get("credential_ref")?,
        force_path_style: row.try_get("force_path_style")?,
        allow_insecure_http: row.try_get("allow_insecure_http")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
