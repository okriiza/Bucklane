use tauri::State;

use crate::app_state::AppState;
use crate::domain::transfer::{
    EnqueueDownloadInput, EnqueueItemResult, EnqueueUploadInput, TransferDto,
};
use crate::errors::AppError;

/// Queues uploads of selected files. Items whose destination exists are
/// returned as `conflict` (policy `ASK`) and are not queued.
#[tauri::command]
#[specta::specta]
pub async fn enqueue_upload(
    state: State<'_, AppState>,
    input: EnqueueUploadInput,
) -> Result<Vec<EnqueueItemResult>, AppError> {
    state
        .transfers()?
        .enqueue_upload(input, &state.selections)
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn enqueue_download(
    state: State<'_, AppState>,
    input: EnqueueDownloadInput,
) -> Result<Vec<EnqueueItemResult>, AppError> {
    state
        .transfers()?
        .enqueue_download(input, &state.selections)
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn list_transfers(state: State<'_, AppState>) -> Result<Vec<TransferDto>, AppError> {
    state.transfers()?.list().await
}

#[tauri::command]
#[specta::specta]
pub async fn get_transfer(state: State<'_, AppState>, id: String) -> Result<TransferDto, AppError> {
    state.transfers()?.get(&id).await
}

#[tauri::command]
#[specta::specta]
pub fn cancel_transfer(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    state.transfers()?.cancel(&id)
}

#[tauri::command]
#[specta::specta]
pub async fn retry_transfer(
    state: State<'_, AppState>,
    id: String,
) -> Result<TransferDto, AppError> {
    state.transfers()?.retry(&id).await
}

/// Removes finished transfers from history; returns how many were removed.
#[tauri::command]
#[specta::specta]
pub async fn clear_transfer_history(state: State<'_, AppState>) -> Result<u32, AppError> {
    let removed = state.transfers()?.clear_history().await?;
    Ok(u32::try_from(removed).unwrap_or(u32::MAX))
}

/// Reveals the transfer's local file in the system file manager.
#[tauri::command]
#[specta::specta]
pub async fn reveal_transfer(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    state.transfers()?.reveal(&id).await
}
