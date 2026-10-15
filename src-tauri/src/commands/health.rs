use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, State};

use crate::app_state::AppState;
use crate::errors::AppError;

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HealthStatus {
    pub app_version: String,
    pub database_ok: bool,
    pub database_error: Option<AppError>,
}

/// Reports whether the backend and its local database are usable.
#[tauri::command]
#[specta::specta]
pub async fn health(app: AppHandle, state: State<'_, AppState>) -> Result<HealthStatus, AppError> {
    let database_error = match state.db() {
        Ok(pool) => sqlx::query("SELECT 1")
            .execute(pool)
            .await
            .err()
            .map(AppError::from),
        Err(err) => Some(err),
    };

    Ok(HealthStatus {
        app_version: app.package_info().version.to_string(),
        database_ok: database_error.is_none(),
        database_error,
    })
}
