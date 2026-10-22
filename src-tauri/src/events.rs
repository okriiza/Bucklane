use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;

use crate::domain::transfer::TransferDto;
use crate::security::local_selection::LocalFileSelection;

/// Full transfer state, emitted on every status change and periodically
/// while running. Informational only: SQLite and the transfer manager remain
/// authoritative, and the frontend reloads via `list_transfers` on startup.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct TransferUpdate(pub TransferDto);

/// Files dropped onto the window from the OS, already registered as
/// selections (TSD §6.4).
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct FilesDropped {
    pub files: Vec<LocalFileSelection>,
    /// Number of dropped items that were skipped (folders, unreadable).
    pub skipped: u32,
}

/// Progress of a running folder copy/move.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct FolderRelocateProgress {
    pub plan_id: String,
    pub done: u32,
    pub total: u32,
}

/// Whether OS files are currently being dragged over the window.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct FileDragHover(pub bool);
