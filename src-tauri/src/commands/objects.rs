use tauri::State;

use crate::app_state::AppState;
use crate::domain::object::{
    CopyObjectInput, CreateFolderInput, CreateShareLinkInput, DeleteObjectsInput,
    DeleteObjectsResult, ListObjectsRequest, ListObjectsResponse, ObjectMetadata,
    ObjectOperationOutcome, ObjectPreview, PrefixDeletePlan, ShareLink,
};
use crate::errors::AppError;

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
