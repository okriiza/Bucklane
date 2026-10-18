use tauri::{AppHandle, State};

use crate::errors::{AppError, ErrorCode};
use crate::updates::{UpdateInfo, UpdateState};

/// Asks the update server for a newer release; `None` when up to date.
#[tauri::command]
#[specta::specta]
pub async fn check_for_update(
    app: AppHandle,
    updates: State<'_, UpdateState>,
) -> Result<Option<UpdateInfo>, AppError> {
    updates.check(&app).await
}

/// Installs the update found by the last check. Progress arrives as
/// `update-progress` events. On Windows the app exits to run the installer.
#[tauri::command]
#[specta::specta]
pub async fn install_update(
    app: AppHandle,
    updates: State<'_, UpdateState>,
) -> Result<(), AppError> {
    updates.install(&app).await
}

/// Opens the GitHub release page for `version` in the default browser. The
/// URL is built here, so the frontend cannot open arbitrary addresses.
#[tauri::command]
#[specta::specta]
pub fn open_release_page(version: String) -> Result<(), AppError> {
    let url = release_page_url(&version)
        .ok_or_else(|| AppError::new(ErrorCode::InvalidInput, "Invalid version."))?;
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|err| {
        AppError::new(ErrorCode::InternalError, "Could not open the browser.")
            .with_details(err.to_string())
    })
}

/// Relaunches the app to finish an installed update.
#[tauri::command]
#[specta::specta]
pub fn restart_app(app: AppHandle) {
    tracing::info!("restarting to apply update");
    app.restart();
}

fn release_page_url(version: &str) -> Option<String> {
    let valid = !version.is_empty()
        && version.len() <= 32
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'));
    valid.then(|| format!("https://github.com/okriiza/Bucklane/releases/tag/v{version}"))
}

#[cfg(test)]
mod tests {
    use super::release_page_url;

    #[test]
    fn release_page_only_accepts_version_strings() {
        assert_eq!(
            release_page_url("0.4.0").as_deref(),
            Some("https://github.com/okriiza/Bucklane/releases/tag/v0.4.0")
        );
        assert!(release_page_url("1.0.0-beta.1").is_some());
        assert!(release_page_url("").is_none());
        assert!(release_page_url("0.4.0/../../evil").is_none());
        assert!(release_page_url("0.4.0?x=1").is_none());
    }
}
