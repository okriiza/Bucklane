use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::errors::{AppError, ErrorCode};

const SELECTION_TTL: Duration = Duration::from_secs(30 * 60);

/// A local file the user picked or dropped; sent to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LocalFileSelection {
    pub selection_id: String,
    pub file_name: String,
    #[specta(type = f64)]
    pub size_bytes: i64,
    /// For display only; never accepted back as input.
    pub display_path: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LocalDirectorySelection {
    pub selection_id: String,
    pub display_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionKind {
    File,
    Directory,
}

#[derive(Debug, Clone)]
pub struct Selection {
    pub path: PathBuf,
    pub kind: SelectionKind,
    expires_at: Instant,
}

/// Paths the user chose through a native dialog or drag-and-drop (TSD §6.4).
///
/// The frontend only ever holds opaque IDs, so IPC cannot be used to read or
/// write arbitrary local paths. Entries expire and are never persisted.
#[derive(Default)]
pub struct SelectionRegistry {
    selections: Mutex<HashMap<String, Selection>>,
}

impl SelectionRegistry {
    /// Registers regular files; directories and unreadable paths are skipped
    /// (folder upload is post-MVP).
    pub fn register_files(
        &self,
        paths: impl IntoIterator<Item = PathBuf>,
    ) -> Vec<LocalFileSelection> {
        paths
            .into_iter()
            .filter_map(|path| {
                let meta = std::fs::metadata(&path).ok().filter(|m| m.is_file())?;
                let file_name = path.file_name()?.to_string_lossy().into_owned();
                let selection_id = self.insert(path.clone(), SelectionKind::File);
                Some(LocalFileSelection {
                    selection_id,
                    file_name,
                    size_bytes: i64::try_from(meta.len()).unwrap_or(i64::MAX),
                    display_path: path.display().to_string(),
                })
            })
            .collect()
    }

    pub fn register_directory(&self, path: PathBuf) -> Option<LocalDirectorySelection> {
        if !path.is_dir() {
            return None;
        }
        let display_path = path.display().to_string();
        Some(LocalDirectorySelection {
            selection_id: self.insert(path, SelectionKind::Directory),
            display_path,
        })
    }

    pub fn get(&self, id: &str, kind: SelectionKind) -> Result<Selection, AppError> {
        let selections = self.selections.lock().expect("selection registry poisoned");
        selections
            .get(id)
            .filter(|s| s.kind == kind && s.expires_at > Instant::now())
            .cloned()
            .ok_or_else(|| {
                AppError::new(
                    ErrorCode::LocalFileError,
                    "The selected file is no longer available. Select it again.",
                )
            })
    }

    fn insert(&self, path: PathBuf, kind: SelectionKind) -> String {
        let id = uuid::Uuid::now_v7().to_string();
        let mut selections = self.selections.lock().expect("selection registry poisoned");
        let now = Instant::now();
        selections.retain(|_, s| s.expires_at > now);
        selections.insert(
            id.clone(),
            Selection {
                path,
                kind,
                expires_at: now + SELECTION_TTL,
            },
        );
        id
    }
}

/// Makes an S3 key segment safe to use as a local file name. Separators and
/// characters invalid on common filesystems are replaced; `.`/`..` and empty
/// names are rejected so a download can never escape its directory.
pub fn safe_local_file_name(name: &str) -> Option<String> {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let cleaned = cleaned.trim_end_matches(['.', ' ']).to_owned();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        return None;
    }
    Some(cleaned)
}

/// `report.pdf` → `report (1).pdf`, `report (2).pdf`, …
pub fn numbered_name(name: &str, n: u32) -> String {
    match name.rfind('.') {
        Some(dot) if dot > 0 => format!("{} ({n}){}", &name[..dot], &name[dot..]),
        _ => format!("{name} ({n})"),
    }
}

/// Joins `dir` and a single file name, guaranteeing the result stays in `dir`.
pub fn join_in_dir(dir: &Path, file_name: &str) -> Option<PathBuf> {
    let path = dir.join(file_name);
    (path.parent() == Some(dir)).then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_wrong_kind_and_unknown_ids() {
        let registry = SelectionRegistry::default();
        let dir = std::env::temp_dir();
        let selection = registry.register_directory(dir).unwrap();

        assert!(
            registry
                .get(&selection.selection_id, SelectionKind::Directory)
                .is_ok()
        );
        assert!(
            registry
                .get(&selection.selection_id, SelectionKind::File)
                .is_err()
        );
        assert!(registry.get("unknown", SelectionKind::Directory).is_err());
    }

    #[test]
    fn skips_directories_when_registering_files() {
        let registry = SelectionRegistry::default();
        assert!(registry.register_files([std::env::temp_dir()]).is_empty());
    }

    #[test]
    fn sanitizes_local_file_names() {
        assert_eq!(
            safe_local_file_name("report.pdf").as_deref(),
            Some("report.pdf")
        );
        assert_eq!(
            safe_local_file_name("a:b?.txt").as_deref(),
            Some("a_b_.txt")
        );
        assert_eq!(safe_local_file_name("..").as_deref(), None);
        assert_eq!(safe_local_file_name("").as_deref(), None);
        assert_eq!(safe_local_file_name("name. ").as_deref(), Some("name"));
    }

    #[test]
    fn numbers_names_before_the_extension() {
        assert_eq!(numbered_name("report.pdf", 1), "report (1).pdf");
        assert_eq!(numbered_name("Makefile", 2), "Makefile (2)");
        assert_eq!(numbered_name(".env", 1), ".env (1)");
    }

    #[test]
    fn join_stays_inside_directory() {
        let dir = Path::new("/tmp/downloads");
        assert!(join_in_dir(dir, "a.txt").is_some());
        assert!(join_in_dir(dir, "../a.txt").is_none());
    }
}
