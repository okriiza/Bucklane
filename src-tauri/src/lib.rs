mod app_state;
mod commands;
mod database;
mod domain;
mod errors;
mod events;
mod logging;
mod providers;
mod repositories;
mod security;
mod services;
mod transfers;

use std::sync::Arc;

use tauri::{DragDropEvent, Manager, WindowEvent};
use tauri_specta::Event;

use crate::app_state::AppState;
use crate::security::credential_store::KeyringCredentialStore;

/// Generated TypeScript bindings consumed by the frontend (ADR-011).
pub const BINDINGS_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../src/services/tauri/bindings.ts"
);

/// Registers every IPC command; the frontend can only call what is listed here.
pub fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .events(tauri_specta::collect_events![
            events::TransferUpdate,
            events::FilesDropped,
            events::FileDragHover,
        ])
        .commands(tauri_specta::collect_commands![
            commands::health::health,
            commands::connections::list_provider_profiles,
            commands::connections::list_connections,
            commands::connections::get_connection,
            commands::connections::create_connection,
            commands::connections::update_connection,
            commands::connections::delete_connection,
            commands::connections::test_connection,
            commands::buckets::list_buckets,
            commands::buckets::head_bucket,
            commands::buckets::list_bucket_bookmarks,
            commands::buckets::add_bucket_bookmark,
            commands::buckets::update_bucket_bookmark,
            commands::buckets::remove_bucket_bookmark,
            commands::objects::list_objects,
            commands::objects::head_object,
            commands::objects::create_folder,
            commands::objects::copy_object,
            commands::objects::move_object,
            commands::objects::delete_objects,
            commands::objects::prepare_prefix_delete,
            commands::objects::execute_prefix_delete,
            commands::objects::create_share_link,
            commands::objects::preview_object,
            commands::files::pick_upload_files,
            commands::files::pick_upload_folder,
            commands::files::pick_download_directory,
            commands::transfers::enqueue_upload,
            commands::transfers::enqueue_download,
            commands::transfers::list_transfers,
            commands::transfers::get_transfer,
            commands::transfers::cancel_transfer,
            commands::transfers::retry_transfer,
            commands::transfers::clear_transfer_history,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::reset_settings,
        ])
}

pub fn export_bindings(builder: &tauri_specta::Builder<tauri::Wry>) {
    builder
        .export(specta_typescript::Typescript::default(), BINDINGS_PATH)
        .expect("failed to export TypeScript bindings");
}

pub fn run() {
    let builder = specta_builder();

    #[cfg(debug_assertions)]
    export_bindings(&builder);

    tauri::Builder::default()
        // Used from Rust only; no dialog permission is granted to the frontend.
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(builder.invoke_handler())
        .on_window_event(handle_file_drop)
        .setup(move |app| {
            builder.mount_events(app);

            let log_guard = logging::init(&app.path().app_log_dir()?);
            app.manage(log_guard);
            tracing::info!(version = %app.package_info().version, "starting Bucklane");

            let data_dir = app.path().app_data_dir()?;
            let db = tauri::async_runtime::block_on(database::init(&data_dir));
            if db.is_err() {
                tracing::error!("database unavailable; database-backed features are disabled");
            }
            let state = AppState::new(db, Arc::new(KeyringCredentialStore), app.handle().clone());
            if let Ok(transfers) = state.transfers() {
                if let Ok(settings) = state.settings() {
                    match tauri::async_runtime::block_on(settings.get()) {
                        Ok(saved) => transfers.apply_settings(&saved),
                        Err(err) => tracing::warn!(error = %err, "using default settings"),
                    }
                }
                tauri::async_runtime::block_on(transfers.reconcile_on_startup());
            }
            app.manage(state);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Bucklane");
}

/// OS files dropped on the window are registered as selections in Rust and
/// only their opaque IDs reach the frontend (TSD §6.4).
fn handle_file_drop(window: &tauri::Window, event: &WindowEvent) {
    let WindowEvent::DragDrop(drag) = event else {
        return;
    };
    match drag {
        DragDropEvent::Enter { .. } => {
            let _ = events::FileDragHover(true).emit(window);
        }
        DragDropEvent::Leave => {
            let _ = events::FileDragHover(false).emit(window);
        }
        DragDropEvent::Drop { paths, .. } => {
            let _ = events::FileDragHover(false).emit(window);
            let state = window.state::<AppState>();
            let files = state
                .selections
                .register_upload_items(paths.iter().cloned());
            let skipped = u32::try_from(paths.len().saturating_sub(files.len())).unwrap_or(0);
            let _ = events::FilesDropped { files, skipped }.emit(window);
        }
        _ => {}
    }
}
