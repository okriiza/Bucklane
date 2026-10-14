use std::sync::Arc;

use sqlx::SqlitePool;

use crate::errors::AppError;
use crate::providers::s3_client_factory::ClientCache;
use crate::repositories::bucket_bookmark_repository::BucketBookmarkRepository;
use crate::security::credential_store::CredentialStore;
use crate::services::connection_service::ConnectionService;
use crate::services::object_service::ObjectService;
use crate::services::storage_service::StorageService;

/// Shared state managed by Tauri and injected into commands.
pub struct AppState {
    /// The database may be unavailable if startup migration failed; commands
    /// that need it surface `DATABASE_UNAVAILABLE` instead of crashing.
    db: Result<SqlitePool, AppError>,
    credentials: Arc<dyn CredentialStore>,
    clients: ClientCache,
}

impl AppState {
    pub fn new(db: Result<SqlitePool, AppError>, credentials: Arc<dyn CredentialStore>) -> Self {
        Self {
            db,
            credentials,
            clients: ClientCache::default(),
        }
    }

    pub fn db(&self) -> Result<&SqlitePool, AppError> {
        self.db.as_ref().map_err(Clone::clone)
    }

    pub fn connections(&self) -> Result<ConnectionService<'_>, AppError> {
        Ok(ConnectionService::new(
            self.db()?,
            self.credentials.as_ref(),
            &self.clients,
        ))
    }

    pub fn storage(&self) -> Result<StorageService<'_>, AppError> {
        Ok(StorageService::new(
            self.connections()?,
            BucketBookmarkRepository::new(self.db()?),
        ))
    }

    pub fn objects(&self) -> Result<ObjectService<'_>, AppError> {
        Ok(ObjectService::new(self.connections()?))
    }
}
