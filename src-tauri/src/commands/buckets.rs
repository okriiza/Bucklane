use tauri::State;

use crate::app_state::AppState;
use crate::domain::bucket::{
    AddBucketBookmarkInput, BucketBookmark, BucketInfo, PublicUrl, SetBucketPublicUrlInput,
    UpdateBucketBookmarkInput,
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

/// Unsigned URL of an object, built from the bucket's base URL if one is set.
#[tauri::command]
#[specta::specta]
pub async fn get_public_url(
    state: State<'_, AppState>,
    connection_id: String,
    bucket: String,
    key: String,
) -> Result<PublicUrl, AppError> {
    state
        .storage()?
        .public_url(&connection_id, &bucket, &key)
        .await
}

/// Sets or clears the base URL (custom domain or CDN) of a bucket's public
/// objects. Returns the normalized value.
#[tauri::command]
#[specta::specta]
pub async fn set_bucket_public_url(
    state: State<'_, AppState>,
    input: SetBucketPublicUrlInput,
) -> Result<Option<String>, AppError> {
    state.storage()?.set_public_base_url(input).await
}
