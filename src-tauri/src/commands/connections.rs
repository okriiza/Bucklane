use tauri::State;

use crate::app_state::AppState;
use crate::domain::connection::{
    ConnectionConfigInput, ConnectionDto, TestConnectionInput, TestConnectionResult,
    UpdateConnectionInput,
};
use crate::errors::AppError;
use crate::providers::provider_profiles::{ProviderProfile, all_profiles};
use crate::services::connection_service::DeleteConnectionResult;

/// Provider defaults and form hints, so the frontend has no hardcoded copy.
#[tauri::command]
#[specta::specta]
pub fn list_provider_profiles() -> Vec<ProviderProfile> {
    all_profiles()
}

#[tauri::command]
#[specta::specta]
pub async fn list_connections(state: State<'_, AppState>) -> Result<Vec<ConnectionDto>, AppError> {
    state.connections()?.list().await
}

#[tauri::command]
#[specta::specta]
pub async fn get_connection(
    state: State<'_, AppState>,
    id: String,
) -> Result<ConnectionDto, AppError> {
    state.connections()?.get(&id).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_connection(
    state: State<'_, AppState>,
    input: ConnectionConfigInput,
) -> Result<ConnectionDto, AppError> {
    state.connections()?.create(input).await
}

/// Leaving `secretAccessKey` blank keeps the stored secret.
#[tauri::command]
#[specta::specta]
pub async fn update_connection(
    state: State<'_, AppState>,
    input: UpdateConnectionInput,
) -> Result<ConnectionDto, AppError> {
    state.connections()?.update(input).await
}

/// Removes the connection and its secret. Remote buckets are never touched.
#[tauri::command]
#[specta::specta]
pub async fn delete_connection(
    state: State<'_, AppState>,
    id: String,
) -> Result<DeleteConnectionResult, AppError> {
    // Active transfers must finish or be canceled first (TSD §7.2).
    if state.transfers().is_ok_and(|t| t.has_active(&id)) {
        return Err(AppError::new(
            crate::errors::ErrorCode::InvalidInput,
            "This connection has active transfers. Cancel them or wait for them to finish before deleting it.",
        ));
    }
    state.connections()?.delete(&id).await
}

#[tauri::command]
#[specta::specta]
pub async fn test_connection(
    state: State<'_, AppState>,
    input: TestConnectionInput,
) -> Result<TestConnectionResult, AppError> {
    state.connections()?.test(input).await
}
