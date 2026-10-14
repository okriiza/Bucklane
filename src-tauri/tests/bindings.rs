//! Regenerates `src/services/tauri/bindings.ts`. CI runs this and fails if the
//! committed file differs, keeping the IPC contract in sync (TSD §6.0).

#[test]
fn export_typescript_bindings() {
    bucklane_lib::export_bindings(&bucklane_lib::specta_builder());
}
