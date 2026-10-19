use std::sync::atomic::{AtomicU32, Ordering};

use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager, State};

use crate::app_state::AppState;
use crate::errors::{AppError, ErrorCode};

/// Longest frontend error message kept in the log.
const MAX_FRONTEND_ERROR_LEN: usize = 2_000;
/// Frontend errors logged per run, so a looping error cannot fill the disk.
const MAX_FRONTEND_ERRORS: u32 = 200;
static FRONTEND_ERRORS: AtomicU32 = AtomicU32::new(0);

/// Environment details for bug reports. Contains nothing about connections,
/// buckets, files, or credentials.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsInfo {
    pub app_version: String,
    pub tauri_version: String,
    pub webview_version: Option<String>,
    pub os: String,
    pub arch: String,
    pub database_ok: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn diagnostics(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DiagnosticsInfo, AppError> {
    let database_ok = match state.db() {
        Ok(pool) => sqlx::query("SELECT 1").execute(pool).await.is_ok(),
        Err(_) => false,
    };
    Ok(DiagnosticsInfo {
        app_version: app.package_info().version.to_string(),
        tauri_version: tauri::VERSION.to_owned(),
        webview_version: tauri::webview_version().ok(),
        os: std::env::consts::OS.to_owned(),
        arch: std::env::consts::ARCH.to_owned(),
        database_ok,
    })
}

/// Opens the folder holding Bucklane's log files in the system file manager.
#[tauri::command]
#[specta::specta]
pub fn open_logs_folder(app: AppHandle) -> Result<(), AppError> {
    let dir = app.path().app_log_dir().map_err(|err| {
        AppError::new(
            ErrorCode::LocalFileError,
            "The log folder could not be found.",
        )
        .with_details(err.to_string())
    })?;
    let _ = std::fs::create_dir_all(&dir);
    tauri_plugin_opener::open_path(&dir, None::<&str>).map_err(|err| {
        AppError::new(
            ErrorCode::LocalFileError,
            "The log folder could not be opened.",
        )
        .with_details(err.to_string())
    })
}

/// Records an unexpected frontend error in the log file so it can be
/// attached to a bug report. URLs and signature parameters are redacted.
#[tauri::command]
#[specta::specta]
pub fn log_frontend_error(message: String) {
    let count = FRONTEND_ERRORS.fetch_add(1, Ordering::Relaxed);
    if count < MAX_FRONTEND_ERRORS {
        tracing::error!(target: "frontend", message = %sanitize(&message), "unexpected UI error");
    } else if count == MAX_FRONTEND_ERRORS {
        tracing::warn!(target: "frontend", "too many UI errors; no longer logging them");
    }
}

/// Words that may carry a URL or a SigV4 signature.
fn is_sensitive(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    lower.contains("://")
        || lower.contains("x-amz-")
        || lower.contains("signature=")
        || lower.contains("credential=")
}

fn sanitize(message: &str) -> String {
    let redacted: Vec<&str> = message
        .split_whitespace()
        .map(|word| {
            if is_sensitive(word) {
                "<redacted>"
            } else {
                word
            }
        })
        .collect();
    let joined = redacted.join(" ");
    match joined.char_indices().nth(MAX_FRONTEND_ERROR_LEN) {
        Some((cut, _)) => format!("{}…", &joined[..cut]),
        None => joined,
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize;

    #[test]
    fn redacts_urls_and_truncates() {
        assert_eq!(
            sanitize("failed https://b.s3.amazonaws.com/k?X-Amz-Signature=abc now"),
            "failed <redacted> now"
        );
        assert_eq!(
            sanitize("bad ?X-Amz-Credential=AKIA%2F x-amz-signature=ff end"),
            "bad <redacted> <redacted> end"
        );
        let long = "x".repeat(5_000);
        assert_eq!(sanitize(&long).chars().count(), 2_001);
    }
}
