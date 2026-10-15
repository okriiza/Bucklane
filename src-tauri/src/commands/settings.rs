use tauri::State;

use crate::app_state::AppState;
use crate::domain::settings::AppSettings;
use crate::errors::AppError;

#[tauri::command]
#[specta::specta]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, AppError> {
    state.settings()?.get().await
}

/// Validates, saves, and applies the settings immediately.
#[tauri::command]
#[specta::specta]
pub async fn update_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<AppSettings, AppError> {
    let saved = state.settings()?.update(settings).await?;
    if let Ok(transfers) = state.transfers() {
        transfers.apply_settings(&saved);
    }
    Ok(saved)
}

#[tauri::command]
#[specta::specta]
pub async fn reset_settings(state: State<'_, AppState>) -> Result<AppSettings, AppError> {
    let defaults = state.settings()?.reset().await?;
    if let Ok(transfers) = state.transfers() {
        transfers.apply_settings(&defaults);
    }
    Ok(defaults)
}
