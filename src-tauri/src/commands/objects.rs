use tauri::State;

use crate::app_state::AppState;
use crate::domain::object::{ListObjectsRequest, ListObjectsResponse, ObjectMetadata};
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
