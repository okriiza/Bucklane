use crate::errors::{AppError, ErrorCode};

/// Where people can support Bucklane's development.
pub const SUPPORT_URL: &str = "https://trakteer.id/okriiza";

/// Opens the support page in the default browser. The URL is fixed here, so
/// the frontend cannot use this to open arbitrary addresses.
#[tauri::command]
#[specta::specta]
pub fn open_support_page() -> Result<(), AppError> {
    tauri_plugin_opener::open_url(SUPPORT_URL, None::<&str>).map_err(|err| {
        AppError::new(ErrorCode::InternalError, "Could not open the browser.")
            .with_details(err.to_string())
    })
}
