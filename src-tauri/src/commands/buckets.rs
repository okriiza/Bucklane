use tauri::State;

use crate::app_state::AppState;
use crate::domain::bucket::{
    AddBucketBookmarkInput, BucketBookmark, BucketInfo, UpdateBucketBookmarkInput,
};
use crate::errors::AppError;

/// Fails with `ACCESS_DENIED` when the credentials may not list buckets; the
/// UI then falls back to bookmarked buckets.
#[tauri::command]
#[specta::specta]
pub async fn list_buckets(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<Vec<BucketInfo>, AppError> {
    state.storage()?.list_buckets(&connection_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn head_bucket(
    state: State<'_, AppState>,
    connection_id: String,
    bucket: String,
) -> Result<(), AppError> {
    state.storage()?.head_bucket(&connection_id, &bucket).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_bucket_bookmarks(
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<Vec<BucketBookmark>, AppError> {
    state.storage()?.list_bookmarks(&connection_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn add_bucket_bookmark(
    state: State<'_, AppState>,
    input: AddBucketBookmarkInput,
) -> Result<BucketBookmark, AppError> {
    state.storage()?.add_bookmark(input).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_bucket_bookmark(
    state: State<'_, AppState>,
    input: UpdateBucketBookmarkInput,
) -> Result<BucketBookmark, AppError> {
    state.storage()?.update_bookmark(input).await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_bucket_bookmark(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), AppError> {
    state.storage()?.remove_bookmark(&id).await
}
