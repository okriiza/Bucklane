use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::oneshot;

use crate::app_state::AppState;
use crate::errors::AppError;
use crate::security::local_selection::{LocalDirectorySelection, LocalFileSelection};

/// Opens the native file picker from Rust and returns opaque selections; the
/// frontend never sees a path it could send back (TSD §6.4, ADR-012).
#[tauri::command]
#[specta::specta]
pub async fn pick_upload_files(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<LocalFileSelection>, AppError> {
    let (tx, rx) = oneshot::channel();
    app.dialog().file().pick_files(move |files| {
        let _ = tx.send(files);
    });
    let paths = rx
        .await
        .map_err(|_| AppError::internal("The file dialog closed unexpectedly."))?
        .unwrap_or_default()
        .into_iter()
        .filter_map(|p| p.into_path().ok());
    Ok(state.selections.register_upload_items(paths))
}

/// Picks a folder to upload recursively. Returns `None` when canceled.
#[tauri::command]
#[specta::specta]
pub async fn pick_upload_folder(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<LocalFileSelection>, AppError> {
    let (tx, rx) = oneshot::channel();
    app.dialog().file().pick_folder(move |folder| {
        let _ = tx.send(folder);
    });
    let path = rx
        .await
        .map_err(|_| AppError::internal("The folder dialog closed unexpectedly."))?
        .and_then(|p| p.into_path().ok());
    Ok(path.and_then(|p| state.selections.register_upload_items([p]).pop()))
}

/// Returns `None` when the user cancels the dialog.
#[tauri::command]
#[specta::specta]
pub async fn pick_download_directory(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<LocalDirectorySelection>, AppError> {
    let (tx, rx) = oneshot::channel();
    app.dialog().file().pick_folder(move |folder| {
        let _ = tx.send(folder);
    });
    let path = rx
        .await
        .map_err(|_| AppError::internal("The folder dialog closed unexpectedly."))?
        .and_then(|p| p.into_path().ok());
    Ok(path.and_then(|p| state.selections.register_directory(p)))
}
