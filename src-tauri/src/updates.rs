//! In-app updates via `tauri-plugin-updater`, driven from Rust only.
//!
//! The plugin is registered so its updater can be used here, but no updater
//! permission is granted to the frontend: the UI goes through the commands in
//! `commands::updates`. Packages are verified against the public key in
//! `tauri.conf.json` before they are installed.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;
use tauri_plugin_updater::{Update, UpdaterExt};
use tauri_specta::Event;

use crate::errors::{AppError, ErrorCode};

const PROGRESS_INTERVAL: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
}

/// Download progress of the update package.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    #[specta(type = f64)]
    pub downloaded: i64,
    #[specta(type = Option<f64>)]
    pub total: Option<i64>,
}

/// The update found by the last check, kept so installing uses exactly the
/// version the user was shown.
#[derive(Default)]
pub struct UpdateState {
    pending: Mutex<Option<Update>>,
    installing: AtomicBool,
}

impl UpdateState {
    pub async fn check(&self, app: &AppHandle) -> Result<Option<UpdateInfo>, AppError> {
        let update = app
            .updater()
            .map_err(update_error)?
            .check()
            .await
            .map_err(update_error)?;
        let info = update.as_ref().map(|u| UpdateInfo {
            version: u.version.clone(),
            current_version: u.current_version.clone(),
        });
        tracing::info!(available = ?info.as_ref().map(|i| &i.version), "update check finished");
        *self.pending.lock().expect("update lock") = update;
        Ok(info)
    }

    /// Downloads, verifies and installs the pending update. On Windows the
    /// installer takes over and the app exits; elsewhere the caller restarts.
    pub async fn install(&self, app: &AppHandle) -> Result<(), AppError> {
        let update = self
            .pending
            .lock()
            .expect("update lock")
            .clone()
            .ok_or_else(|| AppError::new(ErrorCode::InvalidInput, "Check for updates first."))?;
        if self.installing.swap(true, Ordering::SeqCst) {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                "The update is already being installed.",
            ));
        }

        tracing::info!(version = %update.version, "installing update");
        let mut downloaded = 0usize;
        let mut last_emit: Option<Instant> = None;
        let result = update
            .download_and_install(
                |chunk, total| {
                    downloaded += chunk;
                    let finished = total.is_some_and(|t| downloaded as u64 >= t);
                    if finished || last_emit.is_none_or(|t| t.elapsed() >= PROGRESS_INTERVAL) {
                        last_emit = Some(Instant::now());
                        let _ = UpdateProgress {
                            downloaded: downloaded as i64,
                            total: total.map(|t| t as i64),
                        }
                        .emit(app);
                    }
                },
                || {},
            )
            .await
            .map_err(update_error);
        self.installing.store(false, Ordering::SeqCst);
        result
    }
}

fn update_error(err: tauri_plugin_updater::Error) -> AppError {
    use tauri_plugin_updater::Error;
    let (code, message) = match &err {
        Error::Reqwest(_) => (
            ErrorCode::EndpointUnreachable,
            "Could not reach the update server. Check your internet connection and try again.",
        ),
        Error::ReleaseNotFound => (
            ErrorCode::EndpointUnreachable,
            "No published release was found to update from.",
        ),
        Error::Minisign(_) | Error::Base64(_) | Error::SignatureUtf8(_) => (
            ErrorCode::InternalError,
            "The downloaded update failed signature verification and was not installed.",
        ),
        _ => (
            ErrorCode::InternalError,
            "The update could not be completed.",
        ),
    };
    tracing::warn!(error = %err, "update failed");
    AppError::new(code, message).with_details(err.to_string())
}
