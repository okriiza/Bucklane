use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::errors::{AppError, ErrorCode};

const SELECTION_TTL: Duration = Duration::from_secs(30 * 60);
/// Upper bound for one folder upload or download, like folder deletes.
pub const MAX_FOLDER_FILES: usize = 10_000;
/// OS metadata files that are never worth uploading.
const JUNK_FILES: [&str; 3] = [".DS_Store", "Thumbs.db", "desktop.ini"];

/// A local file or folder the user picked or dropped; sent to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LocalFileSelection {
    pub selection_id: String,
    pub file_name: String,
    #[specta(type = f64)]
    pub size_bytes: i64,
    /// For display only; never accepted back as input.
    pub display_path: String,
    /// A folder to upload recursively; `size_bytes` is 0 for folders.
    pub is_directory: bool,
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
    /// A download destination; the only kind that may be written to.
    Directory,
    /// A folder picked or dropped for upload; read-only by intent.
    UploadDirectory,
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
    /// Registers regular files and folders for upload; unreadable paths and
    /// other entries (sockets, devices) are skipped.
    pub fn register_upload_items(
        &self,
        paths: impl IntoIterator<Item = PathBuf>,
    ) -> Vec<LocalFileSelection> {
        paths
            .into_iter()
            .filter_map(|path| {
                let meta = std::fs::metadata(&path).ok()?;
                let (kind, size) = if meta.is_file() {
                    (SelectionKind::File, meta.len())
                } else if meta.is_dir() {
                    (SelectionKind::UploadDirectory, 0)
                } else {
                    return None;
                };
                let file_name = path.file_name()?.to_string_lossy().into_owned();
                let selection_id = self.insert(path.clone(), kind);
                Some(LocalFileSelection {
                    selection_id,
                    file_name,
                    size_bytes: i64::try_from(size).unwrap_or(i64::MAX),
                    display_path: path.display().to_string(),
                    is_directory: kind == SelectionKind::UploadDirectory,
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
        self.get_any(id, &[kind])
    }

    /// Like `get`, accepting any of `kinds`.
    pub fn get_any(&self, id: &str, kinds: &[SelectionKind]) -> Result<Selection, AppError> {
        let selections = self.selections.lock().expect("selection registry poisoned");
        selections
            .get(id)
            .filter(|s| kinds.contains(&s.kind) && s.expires_at > Instant::now())
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

/// Every regular file under `dir`, with its path relative to `dir` using `/`
/// separators. Symlinks are skipped (no loops, no escaping the folder) and so
/// are OS metadata files. Fails rather than truncating past `limit` files.
pub fn collect_files(dir: &Path, limit: usize) -> Result<Vec<(PathBuf, String)>, AppError> {
    let mut files = Vec::new();
    let mut pending = vec![(dir.to_path_buf(), String::new())];
    while let Some((current, relative)) = pending.pop() {
        let entries = std::fs::read_dir(&current).map_err(|err| {
            AppError::new(ErrorCode::LocalFileError, "A folder could not be read.")
                .with_details(err.to_string())
        })?;
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            let child = format!("{relative}{name}");
            if file_type.is_dir() {
                pending.push((entry.path(), format!("{child}/")));
            } else if file_type.is_file() && !JUNK_FILES.contains(&name.as_str()) {
                files.push((entry.path(), child));
                if files.len() > limit {
                    return Err(AppError::new(
                        ErrorCode::OperationUnsupported,
                        format!(
                            "This folder contains more than {limit} files, which is more than Bucklane can transfer at once."
                        ),
                    ));
                }
            }
        }
    }
    files.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(files)
}

/// Joins a `/`-separated relative key path under `dir`, sanitizing every
/// segment. Returns `None` if any segment is unusable, so an object key can
/// never place a file outside `dir`.
pub fn join_relative(dir: &Path, relative: &str) -> Option<PathBuf> {
    let mut path = dir.to_path_buf();
    let mut pushed = false;
    for segment in relative.split('/').filter(|s| !s.is_empty()) {
        path.push(safe_local_file_name(segment)?);
        pushed = true;
    }
    (pushed && path.starts_with(dir)).then_some(path)
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
    Some(avoid_reserved_name(cleaned))
}

/// Windows treats `CON`, `NUL`, `COM1`… as devices whatever the extension,
/// so `con.txt` becomes `con_.txt`. Applied everywhere for consistent names.
fn avoid_reserved_name(name: String) -> String {
    let stem_len = name.find('.').unwrap_or(name.len());
    let stem = name[..stem_len].trim_end().to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0');
    if reserved {
        format!("{}_{}", &name[..stem_len], &name[stem_len..])
    } else {
        name
    }
}

/// Whether `path` would land inside `dir` once its existing parent folders
/// are resolved. Catches a symlink already present in the destination that
/// would redirect a folder download elsewhere.
pub fn stays_within(dir: &Path, path: &Path) -> bool {
    let Ok(root) = dir.canonicalize() else {
        return false;
    };
    let mut ancestor = path.parent();
    while let Some(candidate) = ancestor {
        if let Ok(resolved) = candidate.canonicalize() {
            return resolved.starts_with(&root);
        }
        ancestor = candidate.parent();
    }
    false
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
    fn upload_folders_cannot_be_used_as_download_destinations() {
        let registry = SelectionRegistry::default();
        let items = registry.register_upload_items([std::env::temp_dir()]);
        assert_eq!(items.len(), 1);
        assert!(items[0].is_directory);
        let id = &items[0].selection_id;
        assert!(registry.get(id, SelectionKind::UploadDirectory).is_ok());
        assert!(registry.get(id, SelectionKind::Directory).is_err());
    }

    #[test]
    fn collects_files_recursively_and_skips_junk() {
        let dir = std::env::temp_dir().join(format!("bucklane-walk-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(dir.join("a/b")).unwrap();
        std::fs::write(dir.join("top.txt"), b"1").unwrap();
        std::fs::write(dir.join("a/b/deep.txt"), b"2").unwrap();
        std::fs::write(dir.join(".DS_Store"), b"x").unwrap();
        std::fs::create_dir_all(dir.join("empty")).unwrap();

        let files = collect_files(&dir, 10).unwrap();
        let relative: Vec<_> = files.iter().map(|(_, r)| r.as_str()).collect();
        assert_eq!(relative, ["a/b/deep.txt", "top.txt"]);
        assert!(collect_files(&dir, 1).is_err(), "limit is enforced");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn join_relative_stays_inside_and_sanitizes() {
        let dir = Path::new("/tmp/dl");
        assert_eq!(
            join_relative(dir, "reports/2026/q3.pdf"),
            Some(PathBuf::from("/tmp/dl/reports/2026/q3.pdf"))
        );
        assert_eq!(
            join_relative(dir, "a//b"),
            Some(PathBuf::from("/tmp/dl/a/b"))
        );
        assert_eq!(join_relative(dir, "../escape.txt"), None);
        assert_eq!(join_relative(dir, "x/../../y"), None);
        assert_eq!(join_relative(dir, ""), None);
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
    fn renames_windows_device_names() {
        assert_eq!(safe_local_file_name("CON").as_deref(), Some("CON_"));
        assert_eq!(safe_local_file_name("con.txt").as_deref(), Some("con_.txt"));
        assert_eq!(
            safe_local_file_name("Lpt9.tar.gz").as_deref(),
            Some("Lpt9_.tar.gz")
        );
        assert_eq!(
            safe_local_file_name("COM0.log").as_deref(),
            Some("COM0.log")
        );
        assert_eq!(
            safe_local_file_name("console.log").as_deref(),
            Some("console.log")
        );
    }

    #[cfg(unix)]
    #[test]
    fn detects_symlinks_leading_out_of_the_destination() {
        let base = std::env::temp_dir().join(format!("bucklane-sl-{}", uuid::Uuid::now_v7()));
        let dest = base.join("dest");
        let outside = base.join("outside");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, dest.join("photos")).unwrap();

        assert!(stays_within(&dest, &dest.join("new/deep/a.txt")));
        assert!(stays_within(&dest, &dest.join("a.txt")));
        assert!(!stays_within(&dest, &dest.join("photos/a.txt")));
        assert!(!stays_within(&dest, &dest.join("photos/sub/a.txt")));
        std::fs::remove_dir_all(&base).unwrap();
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
