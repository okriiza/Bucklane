mod app_state;
mod commands;
mod database;
mod domain;
mod errors;
mod logging;
mod providers;
mod repositories;
mod security;
mod services;

use std::sync::Arc;

use tauri::Manager;

use crate::app_state::AppState;
use crate::security::credential_store::KeyringCredentialStore;

/// Generated TypeScript bindings consumed by the frontend (ADR-011).
pub const BINDINGS_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../src/services/tauri/bindings.ts"
);

/// Registers every IPC command; the frontend can only call what is listed here.
pub fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        commands::health::health,
        commands::connections::list_provider_profiles,
        commands::connections::list_connections,
        commands::connections::get_connection,
        commands::connections::create_connection,
        commands::connections::update_connection,
        commands::connections::delete_connection,
        commands::connections::test_connection,
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
        .invoke_handler(builder.invoke_handler())
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
            app.manage(AppState::new(db, Arc::new(KeyringCredentialStore)));

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Bucklane");
}
