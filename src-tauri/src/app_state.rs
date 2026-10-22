use std::sync::Arc;

use sqlx::SqlitePool;
use tauri::AppHandle;

use crate::errors::{AppError, ErrorCode};
use crate::providers::s3_client_factory::ClientCache;
use crate::repositories::bucket_bookmark_repository::BucketBookmarkRepository;
use crate::repositories::settings_repository::SettingsRepository;
use crate::security::credential_store::CredentialStore;
use crate::security::local_selection::SelectionRegistry;
use crate::services::connection_service::ConnectionService;
use crate::services::delete_plans::DeletePlanRegistry;
use crate::services::object_service::ObjectService;
use crate::services::relocate_plans::RelocatePlanRegistry;
use crate::services::settings_service::SettingsService;
use crate::services::storage_service::StorageService;
use crate::transfers::TransferManager;

/// Shared state managed by Tauri and injected into commands.
pub struct AppState {
    /// The database may be unavailable if startup migration failed; commands
    /// that need it surface `DATABASE_UNAVAILABLE` instead of crashing.
    db: Result<SqlitePool, AppError>,
    credentials: Arc<dyn CredentialStore>,
    clients: Arc<ClientCache>,
    delete_plans: DeletePlanRegistry,
    relocate_plans: RelocatePlanRegistry,
    pub selections: SelectionRegistry,
    transfers: Option<TransferManager>,
}

impl AppState {
    pub fn new(
        db: Result<SqlitePool, AppError>,
        credentials: Arc<dyn CredentialStore>,
        app: AppHandle,
    ) -> Self {
        let clients = Arc::new(ClientCache::default());
        let transfers = db.as_ref().ok().map(|pool| {
            TransferManager::new(app, pool.clone(), credentials.clone(), clients.clone())
        });
        Self {
            db,
            credentials,
            clients,
            delete_plans: DeletePlanRegistry::default(),
            relocate_plans: RelocatePlanRegistry::default(),
            selections: SelectionRegistry::default(),
            transfers,
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
        Ok(ObjectService::new(
            self.connections()?,
            &self.delete_plans,
            &self.relocate_plans,
        ))
    }

    pub fn settings(&self) -> Result<SettingsService<'_>, AppError> {
        Ok(SettingsService::new(SettingsRepository::new(self.db()?)))
    }

    pub fn transfers(&self) -> Result<&TransferManager, AppError> {
        self.transfers.as_ref().ok_or_else(|| {
            AppError::new(
                ErrorCode::DatabaseUnavailable,
                "Transfers are unavailable because the local database could not be opened.",
            )
        })
    }
}
