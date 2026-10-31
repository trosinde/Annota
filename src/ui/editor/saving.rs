//! Quick save, Save As and the export dialog's file handling (add-export-share task 2.2 and
//! the export dialog, mockups 02–06), kept free of widgets: where `Ctrl+S` writes a new
//! document (spec export-share "Quick save": the filename pattern in the default folder), the
//! texts of the toasts, and how an [`ExportError`] is told to the user (mockup 04's banner;
//! the same words in quick save's toast).

use std::path::{Path, PathBuf};

use gettextrs::gettext;
use gtk::glib;

use annota::export::ExportError;

/// The extension of Annota's editable documents.
pub const ANNOTA_EXTENSION: &str = "annota";

/// Why a file could not be written, as the user is told (mockup 04 and its redline: "No
/// permission to write to this folder" with Choose Folder…, "A file with this name already
/// exists" with Replace; other failures in the same banner).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    NoPermission,
    Exists,
    /// The output is larger than the format (or Annota's renderer) allows.
    TooLarge {
        format: &'static str,
        max: i32,
    },
    /// The renderer cannot draw an image this large (32767 px per side, or out of memory).
    TooLargeToDraw,
    NoSpace,
    Failed,
}

impl Problem {
    /// The banner's (or toast's) text.
    pub fn message(self) -> String {
        match self {
            Problem::NoPermission => gettext("No permission to write to this folder"),
            Problem::Exists => gettext("A file with this name already exists"),
            Problem::TooLarge { format, max } => {
                gettext("{format} images can be at most {max} pixels wide and high")
                    .replace("{format}", format)
                    .replace("{max}", &max.to_string())
            }
            Problem::TooLargeToDraw => gettext("The image is too large to export at this size"),
            Problem::NoSpace => gettext("There is not enough space on the disk"),
            Problem::Failed => gettext("The file could not be written"),
        }
    }

    /// The export dialog's banner button: Choose Folder… (no permission) or Replace (exists).
    pub fn button(self) -> Option<String> {
        match self {
            Problem::NoPermission => Some(gettext("Choose Folder…")),
            Problem::Exists => Some(gettext("Replace")),
            _ => None,
        }
    }

    /// Whether Export stays disabled while the banner shows (redline: "Export disabled until
    /// fixed"). Replace is the fix for an existing file, a different folder or name for the
    /// rest; a failure without a known cause may be retried.
    pub fn blocks_export(self) -> bool {
        !matches!(self, Problem::Failed | Problem::NoSpace)
    }
}

/// How an export or save error is told (`ExportError::io_kind` for file system errors).
pub fn problem(err: &ExportError) -> Problem {
    match err {
        ExportError::TooLarge { format, max, .. } => Problem::TooLarge { format, max: *max },
        ExportError::Render(_) => Problem::TooLargeToDraw,
        _ => match err.io_kind() {
            Some(std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::ReadOnlyFilesystem) => {
                Problem::NoPermission
            }
            Some(std::io::ErrorKind::AlreadyExists) => Problem::Exists,
            Some(std::io::ErrorKind::StorageFull | std::io::ErrorKind::QuotaExceeded) => {
                Problem::NoSpace
            }
            _ => Problem::Failed,
        },
    }
}

/// A name usable as a file name: no path separators or NUL, no surrounding blanks; `None` if
/// nothing is left (or only `.`/`..`).
pub fn file_stem(name: &str) -> Option<String> {
    let cleaned: String = name
        .chars()
        .map(|c| if c == '/' || c == '\0' { '-' } else { c })
        .collect();
    let cleaned = cleaned.trim();
    (!cleaned.is_empty() && cleaned != "." && cleaned != "..").then(|| cleaned.to_string())
}

/// The quick-save name of a new document: the `filename-pattern` preference (strftime-style,
/// default `Annota %Y-%m-%d %H-%M-%S`) expanded for `now`, made a valid file name.
pub fn pattern_stem(pattern: &str, now: &glib::DateTime) -> String {
    let expanded = crate::preferences::expand_filename_pattern(pattern, now);
    file_stem(&expanded).unwrap_or_else(|| "Annota".to_string())
}

/// `folder/stem.extension`, or — if that exists — `stem 2.extension`, `stem 3.extension` …
/// (two quick saves in the same second never overwrite each other).
pub fn unique_path(
    folder: &Path,
    stem: &str,
    extension: &str,
    exists: impl Fn(&Path) -> bool,
) -> PathBuf {
    let first = folder.join(format!("{stem}.{extension}"));
    if !exists(&first) {
        return first;
    }
    (2u32..)
        .map(|n| folder.join(format!("{stem} {n}.{extension}")))
        .find(|path| !exists(path))
        .unwrap_or(first)
}

/// Where `Ctrl+S` writes (spec "Quick save", mockup 15 "Save = exactly what Ctrl+S does"): the
/// document's own `.annota` file if it has one, else a new file named after the pattern in
/// the save folder — no dialog either way.
pub fn quick_save_path(
    source: Option<&Path>,
    folder: &Path,
    stem: &str,
    exists: impl Fn(&Path) -> bool,
) -> PathBuf {
    match source {
        Some(file) => file.to_path_buf(),
        None => unique_path(folder, stem, ANNOTA_EXTENSION, exists),
    }
}

/// A path chosen in Save As, with `.annota` appended unless it has it already.
pub fn with_annota_extension(path: &Path) -> PathBuf {
    let has = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(ANNOTA_EXTENSION));
    if has {
        path.to_path_buf()
    } else {
        let mut name = path.as_os_str().to_owned();
        name.push(".");
        name.push(ANNOTA_EXTENSION);
        PathBuf::from(name)
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Mockup 06: “Saved as “Annota 2026-10-30 14-22-08.annota””.
pub fn saved_toast(path: &Path) -> String {
    gettext("Saved as “{name}”").replace("{name}", &file_name(path))
}

/// After the export dialog wrote a file (not drawn; worded like mockup 06's save toast).
pub fn exported_toast(path: &Path) -> String {
    gettext("Exported as “{name}”").replace("{name}", &file_name(path))
}

/// The header subtitle right after a save (mockup 06): "~/Pictures/Annota · Saved".
pub fn saved_subtitle(folder: &str) -> String {
    gettext("{folder} · Saved").replace("{folder}", folder)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn io(kind: std::io::ErrorKind) -> ExportError {
        ExportError::Io(std::io::Error::from(kind))
    }

    #[test]
    fn errors_become_the_banners_of_mockup_04() {
        use std::io::ErrorKind;
        assert_eq!(
            problem(&io(ErrorKind::PermissionDenied)),
            Problem::NoPermission
        );
        assert_eq!(
            problem(&io(ErrorKind::ReadOnlyFilesystem)),
            Problem::NoPermission
        );
        // A `.annota` save reports through the archive error, the same way.
        let archive = ExportError::Archive(annota::document::archive::ArchiveError::Io(
            std::io::Error::from(ErrorKind::PermissionDenied),
        ));
        assert_eq!(problem(&archive), Problem::NoPermission);
        assert_eq!(problem(&io(ErrorKind::AlreadyExists)), Problem::Exists);
        assert_eq!(problem(&io(ErrorKind::StorageFull)), Problem::NoSpace);
        assert_eq!(problem(&io(ErrorKind::Other)), Problem::Failed);
        assert_eq!(
            problem(&ExportError::TooLarge {
                format: "WebP",
                max: 16383,
                width: 20000,
                height: 100
            }),
            Problem::TooLarge {
                format: "WebP",
                max: 16383
            }
        );
        assert_eq!(problem(&ExportError::Quality(0)), Problem::Failed);
        // The texts and buttons of the redline.
        assert_eq!(
            Problem::NoPermission.message(),
            "No permission to write to this folder"
        );
        assert_eq!(
            Problem::NoPermission.button().as_deref(),
            Some("Choose Folder…")
        );
        assert_eq!(
            Problem::Exists.message(),
            "A file with this name already exists"
        );
        assert_eq!(Problem::Exists.button().as_deref(), Some("Replace"));
        assert_eq!(Problem::Failed.button(), None);
        assert!(Problem::NoPermission.blocks_export() && Problem::Exists.blocks_export());
        assert!(!Problem::Failed.blocks_export());
        assert_eq!(
            Problem::TooLarge {
                format: "WebP",
                max: 16383
            }
            .message(),
            "WebP images can be at most 16383 pixels wide and high"
        );
    }

    #[test]
    fn quick_save_uses_the_pattern_in_the_save_folder() {
        // Scenario "Pattern": Ctrl+S on a new capture, default pattern.
        let time = glib::DateTime::from_local(2026, 10, 30, 14, 22, 8.0).unwrap();
        let stem = pattern_stem("Annota %Y-%m-%d %H-%M-%S", &time);
        assert_eq!(stem, "Annota 2026-10-30 14-22-08");
        let folder = Path::new("/home/mara/Pictures/Annota");
        let nothing = |_: &Path| false;
        assert_eq!(
            quick_save_path(None, folder, &stem, nothing),
            folder.join("Annota 2026-10-30 14-22-08.annota")
        );
        // A document that has a file is saved over it.
        let own = Path::new("/srv/docs/Checkout flow.annota");
        assert_eq!(quick_save_path(Some(own), folder, &stem, nothing), own);
        // Never over another file: a number is added.
        let taken = [
            folder.join("Annota 2026-10-30 14-22-08.annota"),
            folder.join("Annota 2026-10-30 14-22-08 2.annota"),
        ];
        assert_eq!(
            quick_save_path(None, folder, &stem, |p| taken.iter().any(|t| t == p)),
            folder.join("Annota 2026-10-30 14-22-08 3.annota")
        );
    }

    #[test]
    fn names_are_valid_file_names() {
        // A pattern with `/` must not create folders.
        let time = glib::DateTime::from_local(2026, 10, 30, 14, 22, 8.0).unwrap();
        assert_eq!(pattern_stem("Shot %Y/%m", &time), "Shot 2026-10");
        assert_eq!(pattern_stem("   ", &time), "Annota");
        assert_eq!(file_stem(" Checkout/flow "), Some("Checkout-flow".into()));
        assert_eq!(file_stem(".."), None);
        assert_eq!(file_stem(""), None);
    }

    #[test]
    fn save_as_adds_the_extension() {
        assert_eq!(
            with_annota_extension(Path::new("/tmp/Report")),
            Path::new("/tmp/Report.annota")
        );
        assert_eq!(
            with_annota_extension(Path::new("/tmp/Report.ANNOTA")),
            Path::new("/tmp/Report.ANNOTA")
        );
        assert_eq!(
            with_annota_extension(Path::new("/tmp/Report.v2")),
            Path::new("/tmp/Report.v2.annota")
        );
    }

    #[test]
    fn toasts_follow_mockup_06() {
        assert_eq!(
            saved_toast(Path::new("/p/Annota 2026-10-30 14-22-08.annota")),
            "Saved as “Annota 2026-10-30 14-22-08.annota”"
        );
        assert_eq!(
            exported_toast(Path::new("/p/Checkout flow.png")),
            "Exported as “Checkout flow.png”"
        );
        assert_eq!(
            saved_subtitle("~/Pictures/Annota"),
            "~/Pictures/Annota · Saved"
        );
    }
}
