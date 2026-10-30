//! One open document of the editor window: its canvas (document, history, selection, view),
//! where it came from, and its autosave slot (spec document-format: Autosave and recovery).

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

use gtk::{gdk, glib};

use annota::document::archive::{self, ArchiveError};
use annota::document::autosave::{Action, AutosaveId, Schedule, Store};
use annota::document::bitmap::Bitmap;
use annota::document::Document;

use super::text::Status;
use crate::capture::texture::frame_from_texture;
use crate::ui::canvas::{Canvas, HistoryInfo};

/// Where a document came from; decides what "unsaved" means (header subtitle, tray dot).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Opened from a file: unsaved once it differs from the file.
    File,
    /// A new capture: unsaved until it is saved.
    New,
    /// Restored from an autosave: it holds changes no file has.
    Restored,
}

/// The document's status for the header subtitle and the tray.
pub fn status(origin: Origin, info: &HistoryInfo) -> Status {
    match origin {
        Origin::File if info.dirty => Status::Edited,
        Origin::File => Status::Clean,
        Origin::New if info.revision == 0 => Status::NotSaved,
        Origin::New | Origin::Restored => Status::Edited,
    }
}

pub struct Session {
    /// Identifies the session within the window (canvas stack page "doc-<uid>").
    pub uid: u64,
    pub canvas: Canvas,
    pub origin: Origin,
    /// The `.annota` file the document belongs to, if any (written into its autosave).
    pub source: Option<PathBuf>,
    /// Window title: the document title, else the file name, else "Untitled Capture".
    pub name: String,
    pub autosave: AutosaveId,
    pub schedule: Cell<Schedule>,
    /// Tray thumbnail; rendered again a moment after the document changes.
    pub thumbnail: RefCell<Option<gdk::Texture>>,
    pub thumbnail_pending: Cell<Option<glib::SourceId>>,
}

impl Session {
    pub fn status(&self) -> Status {
        status(self.origin, &self.canvas.history_info())
    }

    pub fn page_name(&self) -> String {
        page_name(self.uid)
    }

    /// One autosave tick (every 30 s): write, remove or leave the slot.
    pub fn autosave_tick(&self, store: &Store) {
        let info = self.canvas.history_info();
        let mut schedule = self.schedule.get();
        let result = self
            .canvas
            .with_document(|doc| {
                tick(
                    store,
                    &self.autosave,
                    &mut schedule,
                    info.revision,
                    info.dirty,
                    doc,
                    self.source.as_deref(),
                )
            })
            .unwrap_or(Ok(Action::Nothing));
        self.schedule.set(schedule);
        if let Err(err) = result {
            // The path is Annota's own cache directory; no capture content is logged.
            eprintln!(
                "annota: autosave to {} failed: {err}",
                store.path(&self.autosave).display()
            );
        }
    }

    /// The document is closed: its autosave goes (spec: removed when it is closed).
    pub fn close(&self, store: &Store) {
        if let Some(id) = self.thumbnail_pending.take() {
            id.remove();
        }
        if let Err(err) = store.discard(&self.autosave) {
            eprintln!("annota: cannot remove an autosave: {err}");
        }
    }
}

pub fn page_name(uid: u64) -> String {
    format!("doc-{uid}")
}

#[derive(Debug, thiserror::Error)]
pub enum TickError {
    #[error(transparent)]
    Write(#[from] ArchiveError),
    #[error(transparent)]
    Remove(#[from] std::io::Error),
}

/// What a 30 s tick does for one document, carried out: [`Schedule::tick`] decides, the
/// [`Store`] writes or removes.
pub fn tick(
    store: &Store,
    id: &AutosaveId,
    schedule: &mut Schedule,
    revision: u64,
    dirty: bool,
    doc: &Document,
    source: Option<&Path>,
) -> Result<Action, TickError> {
    let action = schedule.tick(revision, dirty);
    match action {
        Action::Write => {
            store.write(id, doc, source)?;
            schedule.written(revision);
        }
        Action::Discard => {
            store.discard(id)?;
            schedule.discarded();
        }
        Action::Nothing => {}
    }
    Ok(action)
}

/// A file opened into the editor.
pub struct Loaded {
    pub document: Document,
    /// Set for `.annota` files: the file the document belongs to.
    pub source: Option<PathBuf>,
    pub name: String,
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error(transparent)]
    Archive(#[from] ArchiveError),
    #[error("{0}")]
    Image(String),
}

pub fn is_annota(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("annota"))
}

pub fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Opens a `.annota` file or an image GdkTexture can read.
pub fn load(path: &Path) -> Result<Loaded, LoadError> {
    if is_annota(path) {
        let document = archive::load(path)?;
        let name = document
            .metadata()
            .title
            .clone()
            .unwrap_or_else(|| file_stem(path));
        return Ok(Loaded {
            document,
            source: Some(path.to_path_buf()),
            name,
        });
    }
    let bitmap = load_bitmap(path)?;
    let name = file_stem(path);
    Ok(Loaded {
        document: Document::with_title(bitmap, name.clone()),
        source: None,
        name,
    })
}

/// The pixels of an image file (a tray thumbnail dropped on the canvas, or a file opened).
pub fn load_bitmap(path: &Path) -> Result<Bitmap, LoadError> {
    let texture = gdk::Texture::from_filename(path).map_err(|e| LoadError::Image(e.to_string()))?;
    let frame =
        frame_from_texture(&texture).ok_or_else(|| LoadError::Image("empty image".into()))?;
    Bitmap::from_argb32(
        frame.width(),
        frame.height(),
        frame.stride(),
        frame.data().to_vec(),
    )
    .map_err(|e| LoadError::Image(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use annota::document::color::Rgba;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("annota-session-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn info(revision: u64, dirty: bool) -> HistoryInfo {
        HistoryInfo {
            revision,
            dirty,
            ..HistoryInfo::default()
        }
    }

    #[test]
    fn status_by_origin() {
        assert_eq!(status(Origin::File, &info(0, false)), Status::Clean);
        assert_eq!(status(Origin::File, &info(3, true)), Status::Edited);
        assert_eq!(status(Origin::New, &info(0, true)), Status::NotSaved);
        assert_eq!(status(Origin::New, &info(1, true)), Status::Edited);
        assert_eq!(status(Origin::Restored, &info(0, true)), Status::Edited);
        assert!(!Status::Clean.unsaved() && Status::NotSaved.unsaved());
    }

    #[test]
    fn ticks_write_once_and_remove_when_saved() {
        // The 30 s timer's work for one document (spec "Saved document leaves no autosave").
        let tmp = TempDir::new("tick");
        let store = Store::new(&tmp.0);
        let doc = Document::with_title(Bitmap::filled(8, 8, Rgba::WHITE).unwrap(), "Checkout flow");
        let id = AutosaveId::generate();
        let mut schedule = Schedule::new();
        let source = Path::new("/tmp/Checkout flow.annota");
        let run = |schedule: &mut Schedule, revision, dirty| {
            tick(&store, &id, schedule, revision, dirty, &doc, Some(source)).unwrap()
        };
        assert_eq!(run(&mut schedule, 1, true), Action::Write);
        let listed = store.list().entries;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name.as_deref(), Some("Checkout flow"));
        assert_eq!(listed[0].source.as_deref(), Some(source));
        // Idle: nothing is written again.
        let modified = std::fs::metadata(store.path(&id))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(run(&mut schedule, 1, true), Action::Nothing);
        assert_eq!(
            std::fs::metadata(store.path(&id))
                .unwrap()
                .modified()
                .unwrap(),
            modified
        );
        // Back at the saved state: the autosave goes.
        assert_eq!(run(&mut schedule, 0, false), Action::Discard);
        assert!(store.list().entries.is_empty());
    }

    #[test]
    fn annota_files_are_recognised() {
        assert!(is_annota(Path::new("/x/Report.ANNOTA")));
        assert!(!is_annota(Path::new("/x/Report.png")));
        assert_eq!(
            file_stem(Path::new("/x/Checkout flow.annota")),
            "Checkout flow"
        );
    }
}
