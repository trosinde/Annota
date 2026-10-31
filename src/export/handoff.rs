//! The flattened image handed to another application as a file (add-export-share task 2.3 and
//! 2.4; spec export-share "Drag and drop", "Share"): dragging the window title out, Share →
//! Email… and Share → Open With… all need a real PNG file named after the document, which the
//! other application reads after Annota handed it over.
//!
//! Each hand-off gets a folder of its own below `root` (the bin passes
//! `$XDG_RUNTIME_DIR/annota/handoff`: private to the user, in memory, emptied at logout), so the
//! file can be called exactly "<document title>.png" — as the drop creates it in Files (mockup
//! 09) and as the mail's attachment is named (mockup 14) — without two hand-offs of the same
//! document colliding. The other application may read the file long after the drop or the
//! portal call returned (a mail client attaches at send time), so a hand-off is not removed
//! right away: every new hand-off removes the ones older than [`MAX_AGE`].

use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use crate::document::archive;
use crate::document::bitmap::Bitmap;

use super::{encode_png, ExportError};

/// How long a handed-over file is kept (until the next hand-off after that).
pub const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// The name used when the document's title gives no usable file name.
const FALLBACK_STEM: &str = "Annota";

/// A flattened image written for another application.
#[derive(Debug)]
pub struct Handoff {
    /// The PNG file, `<root>/<unique folder>/<stem>.png`.
    pub path: PathBuf,
    /// The file's content, for the drag's `image/png` (encoded once for both).
    pub png: Vec<u8>,
}

/// `<stem>.png`, with path separators and NUL replaced and surrounding blanks trimmed; the
/// fallback name if nothing usable is left. (The bin passes a cleaned name already; this keeps
/// the library safe on its own.)
pub fn file_name(stem: &str) -> String {
    let cleaned: String = stem
        .chars()
        .map(|c| if c == '/' || c == '\0' { '-' } else { c })
        .collect();
    let cleaned = cleaned.trim();
    let stem = if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        FALLBACK_STEM
    } else {
        cleaned
    };
    format!("{stem}.png")
}

/// Writes `bitmap` (the flattened document, `render::flatten` at 100 %) as `<stem>.png` into a
/// new folder below `root`, after removing hand-offs older than [`MAX_AGE`]. `root` and the
/// new folder are private to the user (mode 0700).
pub fn write(bitmap: &Bitmap, root: &Path, stem: &str) -> Result<Handoff, ExportError> {
    prune(root, SystemTime::now(), MAX_AGE);
    let png = encode_png(bitmap)?;
    let folder = new_folder(root)?;
    let path = folder.join(file_name(stem));
    archive::write_atomically(&path, &png)?;
    Ok(Handoff { path, png })
}

/// Removes the hand-off folders below `root` last modified before `now - max_age`. Anything
/// that cannot be read or removed is left alone (a later hand-off tries again).
pub fn prune(root: &Path, now: SystemTime, max_age: Duration) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let stale = meta
            .modified()
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age > max_age);
        if meta.is_dir() && stale {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// A new, empty folder below `root` (created with `root` if needed, both mode 0700).
fn new_folder(root: &Path) -> Result<PathBuf, ExportError> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(root)?;
    // An existing root keeps its mode from `create`; make sure it is private.
    std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))?;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    loop {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let folder = root.join(format!("{nanos:x}-{}-{n}", std::process::id()));
        match std::fs::DirBuilder::new().mode(0o700).create(&folder) {
            Ok(()) => return Ok(folder),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err.into()),
        }
    }
}
