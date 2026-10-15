use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

const LOG_FILE_PREFIX: &str = "bucklane";
const MAX_LOG_FILES: usize = 7;
const DEFAULT_LEVEL: &str = "info";

/// Keeps the non-blocking file writer alive; dropping it flushes and stops
/// file logging, so it is stored in Tauri state for the app lifetime.
pub struct LogGuard(#[allow(dead_code)] Option<WorkerGuard>);

/// Initializes structured logging with daily rotation (TSD §12).
///
/// Falls back to stderr-only logging if the log directory is unusable; a
/// logging failure must not prevent the app from starting.
pub fn init(log_dir: &Path) -> LogGuard {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LEVEL));
    let stderr_layer = fmt::layer().with_writer(std::io::stderr);

    // The appender prunes old files on startup and errors if the dir is missing.
    let _ = std::fs::create_dir_all(log_dir);
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(LOG_FILE_PREFIX)
        .filename_suffix("log")
        .max_log_files(MAX_LOG_FILES)
        .build(log_dir);

    match appender {
        Ok(appender) => {
            let (writer, guard) = tracing_appender::non_blocking(appender);
            tracing_subscriber::registry()
                .with(filter)
                .with(stderr_layer)
                .with(fmt::layer().with_ansi(false).with_writer(writer))
                .init();
            LogGuard(Some(guard))
        }
        Err(err) => {
            tracing_subscriber::registry()
                .with(filter)
                .with(stderr_layer)
                .init();
            tracing::warn!(error = %err, "file logging unavailable; logging to stderr only");
            LogGuard(None)
        }
    }
}
