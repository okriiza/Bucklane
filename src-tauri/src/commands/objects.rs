use tauri::{AppHandle, State};
use tauri_specta::Event;

use crate::app_state::AppState;
use crate::domain::object::{
    CopyObjectInput, CreateFolderInput, CreateShareLinkInput, DeleteObjectsInput,
    DeleteObjectsResult, ExistingObjectPolicy, ListObjectsRequest, ListObjectsResponse,
    ObjectMetadata, ObjectOperationOutcome, ObjectPreview, PrefixDeletePlan, PrefixRelocateInput,
    PrefixRelocatePlan, PrefixRelocateResult, ShareLink, UpdateObjectMetadataInput,
};
use crate::errors::AppError;
use crate::events::FolderRelocateProgress;

#[tauri::command]
#[specta::specta]
pub async fn list_objects(
    state: State<'_, AppState>,
    request: ListObjectsRequest,
) -> Result<ListObjectsResponse, AppError> {
    state.objects()?.list_objects(request).await
}

#[tauri::command]
#[specta::specta]
pub async fn head_object(
    state: State<'_, AppState>,
    connection_id: String,
    bucket: String,
    key: String,
) -> Result<ObjectMetadata, AppError> {
    state
        .objects()?
        .head_object(&connection_id, &bucket, &key)
        .await
}

/// Replaces an object's headers and user metadata; returns the new metadata.
#[tauri::command]
#[specta::specta]
pub async fn update_object_metadata(
    state: State<'_, AppState>,
    input: UpdateObjectMetadataInput,
) -> Result<ObjectMetadata, AppError> {
    state.objects()?.update_metadata(input).await
}

/// Returns the key of the created folder marker.
#[tauri::command]
#[specta::specta]
pub async fn create_folder(
    state: State<'_, AppState>,
    input: CreateFolderInput,
) -> Result<String, AppError> {
    state.objects()?.create_folder(input).await
}

#[tauri::command]
#[specta::specta]
pub async fn copy_object(
    state: State<'_, AppState>,
    input: CopyObjectInput,
) -> Result<ObjectOperationOutcome, AppError> {
    state.objects()?.copy_object(input).await
}

/// Also used for rename (same folder, new name).
#[tauri::command]
#[specta::specta]
pub async fn move_object(
    state: State<'_, AppState>,
    input: CopyObjectInput,
) -> Result<ObjectOperationOutcome, AppError> {
    state.objects()?.move_object(input).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_objects(
    state: State<'_, AppState>,
    input: DeleteObjectsInput,
) -> Result<DeleteObjectsResult, AppError> {
    state.objects()?.delete_objects(input).await
}

#[tauri::command]
#[specta::specta]
pub async fn prepare_prefix_delete(
    state: State<'_, AppState>,
    connection_id: String,
    bucket: String,
    prefix: String,
) -> Result<PrefixDeletePlan, AppError> {
    state
        .objects()?
        .prepare_prefix_delete(&connection_id, &bucket, &prefix)
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn execute_prefix_delete(
    state: State<'_, AppState>,
    plan_id: String,
) -> Result<DeleteObjectsResult, AppError> {
    state.objects()?.execute_prefix_delete(&plan_id).await
}

/// Snapshots a folder copy/move for confirmation; changes nothing.
#[tauri::command]
#[specta::specta]
pub async fn prepare_prefix_relocate(
    state: State<'_, AppState>,
    input: PrefixRelocateInput,
) -> Result<PrefixRelocatePlan, AppError> {
    state.objects()?.prepare_prefix_relocate(input).await
}

/// Runs a confirmed folder copy/move. Progress arrives as
/// `folder-relocate-progress` events.
#[tauri::command]
#[specta::specta]
pub async fn execute_prefix_relocate(
    app: AppHandle,
    state: State<'_, AppState>,
    plan_id: String,
    existing: ExistingObjectPolicy,
) -> Result<PrefixRelocateResult, AppError> {
    state
        .objects()?
        .execute_prefix_relocate(&plan_id, existing, |done, total| {
            let _ = FolderRelocateProgress {
                plan_id: plan_id.clone(),
                done,
                total,
            }
            .emit(&app);
        })
        .await
}

/// Stops a running folder copy/move after the objects already in progress.
#[tauri::command]
#[specta::specta]
pub fn cancel_prefix_relocate(state: State<'_, AppState>, plan_id: String) -> Result<(), AppError> {
    state.objects()?.cancel_prefix_relocate(&plan_id);
    Ok(())
}

/// Presigned download URL; anyone with the link can download until it expires.
#[tauri::command]
#[specta::specta]
pub async fn create_share_link(
    state: State<'_, AppState>,
    input: CreateShareLinkInput,
) -> Result<ShareLink, AppError> {
    state.objects()?.create_share_link(input).await
}

/// Loads an image, PDF, or the beginning of a text file for preview.
#[tauri::command]
#[specta::specta]
pub async fn preview_object(
    state: State<'_, AppState>,
    connection_id: String,
    bucket: String,
    key: String,
) -> Result<ObjectPreview, AppError> {
    state
        .objects()?
        .preview_object(&connection_id, &bucket, &key)
        .await
}
