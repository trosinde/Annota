//! Autosave and crash recovery (spec document-format: Autosave and recovery).
//!
//! Every open document with unsaved changes is written every [`INTERVAL`] to
//! `~/.cache/annota/autosave/<id>.annota` — a regular `.annota` file that additionally
//! remembers the path the document belongs to. Saving the document (or closing it without
//! saving, or undoing back to the saved state) removes its autosave. Whatever is left in the
//! directory when Annota starts was not closed properly and is offered for recovery
//! ("Restore Unsaved Captures?", mockup 14).
//!
//! This module is UI-independent: [`Schedule`] decides what a 30 s tick should do, [`Store`]
//! writes, lists, restores and discards. The timer and the dialog belong to the editor UI.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::archive::{self, ArchiveError, AutosaveJson};
use super::Document;

/// Autosave period (redline: "Autosave every 30 s").
pub const INTERVAL: Duration = Duration::from_secs(30);

/// `<user cache dir>/annota/autosave` — pass `glib::user_cache_dir()`.
pub fn default_dir(cache_dir: &Path) -> PathBuf {
    cache_dir.join("annota").join("autosave")
}

/// Names one document's autosave slot for the lifetime of its editor tab.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AutosaveId(String);

impl AutosaveId {
    /// A new id, unique across runs: time, process id and a counter.
    pub fn generate() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        Self(format!("{millis:x}-{:x}-{n:x}", std::process::id()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn from_file_name(name: &str) -> Option<Self> {
        let stem = name.strip_suffix(".annota")?;
        let valid = !stem.is_empty()
            && stem
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase() || b == b'-');
        valid.then(|| Self(stem.to_string()))
    }
}

/// What a 30 s tick should do for one document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Nothing,
    /// Write the current state (it has changes that are neither saved nor autosaved).
    Write,
    /// Remove the autosave (the document is back at its saved state).
    Discard,
}

/// Per-document autosave bookkeeping. Keeps idle documents idle: nothing is written while the
/// document does not change (CLAUDE.md §7: idle CPU 0 %).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Schedule {
    /// History revision of the autosave on disk.
    on_disk: Option<u64>,
}

impl Schedule {
    pub fn new() -> Self {
        Self::default()
    }

    /// Decides for the document state `revision` (from `History::revision`) and `dirty`
    /// (`History::is_dirty`).
    pub fn tick(&self, revision: u64, dirty: bool) -> Action {
        match (dirty, self.on_disk) {
            (true, Some(r)) if r == revision => Action::Nothing,
            (true, _) => Action::Write,
            (false, Some(_)) => Action::Discard,
            (false, None) => Action::Nothing,
        }
    }

    pub fn written(&mut self, revision: u64) {
        self.on_disk = Some(revision);
    }

    /// After the autosave was removed (document saved, closed or back at its saved state).
    pub fn discarded(&mut self) {
        self.on_disk = None;
    }
}

/// One recoverable document (a row of mockup 14).
#[derive(Clone, Debug, PartialEq)]
pub struct RecoveryEntry {
    pub id: AutosaveId,
    pub path: PathBuf,
    /// Document title, else the name of the file it belongs to; `None` for a new capture.
    pub name: Option<String>,
    /// When it was autosaved ("Autosaved today at 10:42").
    pub saved_at: SystemTime,
    pub object_count: usize,
    /// `thumbnail.png` of the autosave, for the 64 × 40 row thumbnail.
    pub thumbnail_png: Option<Vec<u8>>,
    /// The file the document belongs to, if it had been saved or opened from one.
    pub source: Option<PathBuf>,
}

/// Result of [`Store::list`].
#[derive(Debug, Default)]
pub struct Listing {
    /// Newest first.
    pub entries: Vec<RecoveryEntry>,
    /// Autosave files that could not be read; left in place, not offered.
    pub unreadable: Vec<PathBuf>,
}

/// A restored document and the file it belongs to.
#[derive(Debug)]
pub struct Restored {
    pub document: Document,
    pub source: Option<PathBuf>,
}

/// The autosave directory.
#[derive(Clone, Debug)]
pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn path(&self, id: &AutosaveId) -> PathBuf {
        self.dir.join(format!("{}.annota", id.0))
    }

    /// Writes `doc` into its slot, atomically. `source` is the file it belongs to.
    pub fn write(
        &self,
        id: &AutosaveId,
        doc: &Document,
        source: Option<&Path>,
    ) -> Result<PathBuf, ArchiveError> {
        let bytes = archive::encode_with(
            doc,
            Some(AutosaveJson {
                source: source.and_then(Path::to_str).map(str::to_string),
            }),
        )?;
        create_private_dir(&self.dir)?;
        let path = self.path(id);
        archive::write_atomically(&path, &bytes)?;
        Ok(path)
    }

    /// Removes a slot; a slot that does not exist is not an error.
    pub fn discard(&self, id: &AutosaveId) -> std::io::Result<()> {
        match std::fs::remove_file(self.path(id)) {
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(err),
            _ => Ok(()),
        }
    }

    /// Everything that can be recovered, newest first.
    pub fn list(&self) -> Listing {
        let mut listing = Listing::default();
        let Ok(dir) = std::fs::read_dir(&self.dir) else {
            return listing;
        };
        for entry in dir.flatten() {
            let path = entry.path();
            let Some(id) = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(AutosaveId::from_file_name)
            else {
                continue; // temporary files and anything else
            };
            let saved_at = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(UNIX_EPOCH);
            match archive::read_summary(&path) {
                Ok(summary) => {
                    let name = summary.title.clone().or_else(|| {
                        summary
                            .source
                            .as_deref()
                            .and_then(Path::file_stem)
                            .map(|s| s.to_string_lossy().into_owned())
                    });
                    listing.entries.push(RecoveryEntry {
                        id,
                        path,
                        name,
                        saved_at,
                        object_count: summary.object_count,
                        thumbnail_png: summary.thumbnail_png,
                        source: summary.source,
                    });
                }
                Err(_) => listing.unreadable.push(path),
            }
        }
        listing.entries.sort_by(|a, b| {
            b.saved_at
                .cmp(&a.saved_at)
                .then_with(|| a.id.0.cmp(&b.id.0))
        });
        listing.unreadable.sort();
        listing
    }

    /// Reads an autosaved document back.
    pub fn restore(&self, entry: &RecoveryEntry) -> Result<Restored, ArchiveError> {
        let file = std::fs::File::open(&entry.path)?;
        let opened = archive::decode_from(std::io::BufReader::new(file))?;
        Ok(Restored {
            document: opened.document,
            source: opened.autosave.and_then(|a| a.source).map(PathBuf::from),
        })
    }
}

/// Autosaves hold capture contents: keep the directory private to the user.
fn create_private_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::archive::{document_json, tests::sample};
    use crate::document::command::{Command, History};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("annota-autosave-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn default_dir_is_in_the_cache() {
        assert_eq!(
            default_dir(Path::new("/home/u/.cache")),
            PathBuf::from("/home/u/.cache/annota/autosave")
        );
    }

    #[test]
    fn schedule_writes_only_changes() {
        let mut s = Schedule::new();
        // Unsaved new capture: written once, then idle until it changes.
        assert_eq!(s.tick(0, true), Action::Write);
        s.written(0);
        assert_eq!(s.tick(0, true), Action::Nothing);
        assert_eq!(s.tick(1, true), Action::Write);
        s.written(1);
        // Saved (or undone back to the saved state): the autosave goes away.
        assert_eq!(s.tick(1, false), Action::Discard);
        s.discarded();
        assert_eq!(s.tick(1, false), Action::Nothing);
    }

    #[test]
    fn crash_leaves_a_recoverable_entry() {
        // Spec "Crash recovery", data layer: an autosaved, never-discarded document is listed
        // with name, time, object count and thumbnail, and restores to the same document.
        let tmp = TempDir::new("crash");
        let store = Store::new(&tmp.0);
        assert!(
            store.list().entries.is_empty(),
            "missing directory = nothing to recover"
        );

        let doc = sample();
        let id = AutosaveId::generate();
        let source = Path::new("/home/u/Pictures/Annota/Checkout flow.annota");
        let path = store.write(&id, &doc, Some(source)).unwrap();
        assert!(path.starts_with(&tmp.0));
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&tmp.0).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);

        // "Restart": a new store on the same directory.
        let listing = Store::new(&tmp.0).list();
        assert!(listing.unreadable.is_empty());
        assert_eq!(listing.entries.len(), 1);
        let entry = &listing.entries[0];
        assert_eq!(entry.id, id);
        assert_eq!(entry.name.as_deref(), Some("Checkout flow"));
        assert_eq!(entry.object_count, 2);
        assert!(entry.thumbnail_png.is_some());
        assert_eq!(entry.source.as_deref(), Some(source));
        let age = SystemTime::now().duration_since(entry.saved_at).unwrap();
        assert!(age < Duration::from_secs(60));

        let restored = store.restore(entry).unwrap();
        assert_eq!(restored.source.as_deref(), Some(source));
        assert_eq!(
            document_json(&restored.document).unwrap(),
            document_json(&doc).unwrap(),
            "the autosave key never leaks into the restored document"
        );

        store.discard(&id).unwrap();
        store.discard(&id).unwrap();
        assert!(store.list().entries.is_empty());
    }

    #[test]
    fn autosave_follows_edits() {
        let tmp = TempDir::new("edits");
        let store = Store::new(&tmp.0);
        let mut doc = sample();
        let mut history = History::new();
        let mut schedule = Schedule::new();
        let id = AutosaveId::generate();

        let tick = |doc: &Document, history: &History, schedule: &mut Schedule| match schedule
            .tick(history.revision(), history.is_dirty())
        {
            Action::Write => {
                store.write(&id, doc, None).unwrap();
                schedule.written(history.revision());
            }
            Action::Discard => {
                store.discard(&id).unwrap();
                schedule.discarded();
            }
            Action::Nothing => {}
        };
        tick(&doc, &history, &mut schedule);
        assert_eq!(store.list().entries[0].object_count, 2);
        let ids = doc.scene().ids();
        history
            .execute(&mut doc, "Delete", Command::Remove { ids: vec![ids[0]] })
            .unwrap();
        tick(&doc, &history, &mut schedule);
        let listing = store.list();
        assert_eq!(listing.entries.len(), 1, "one slot per document");
        assert_eq!(listing.entries[0].object_count, 1);
        assert_eq!(listing.entries[0].name.as_deref(), Some("Checkout flow"));
        // Saved: the autosave disappears on the next tick.
        history.mark_saved();
        tick(&doc, &history, &mut schedule);
        assert!(store.list().entries.is_empty());
    }

    #[test]
    fn listing_skips_foreign_and_reports_damaged_files() {
        let tmp = TempDir::new("list");
        let store = Store::new(&tmp.0);
        let first = AutosaveId::generate();
        let second = AutosaveId::generate();
        assert_ne!(first, second);
        store.write(&first, &sample(), None).unwrap();
        store.write(&second, &sample(), None).unwrap();
        std::fs::write(tmp.0.join("abc.annota"), b"garbage").unwrap();
        std::fs::write(tmp.0.join(".x.annota.12.tmp"), b"partial").unwrap();
        std::fs::write(tmp.0.join("notes.txt"), b"hello").unwrap();
        std::fs::write(tmp.0.join("Mixed-Case.annota"), b"hello").unwrap();
        let listing = store.list();
        assert_eq!(listing.entries.len(), 2);
        assert_eq!(listing.unreadable, vec![tmp.0.join("abc.annota")]);
        // A new capture without title or file has no name; the UI shows its own fallback.
        let mut untitled = sample();
        let mut h = History::new();
        h.execute(
            &mut untitled,
            "Rename",
            Command::SetMetadata(crate::document::Metadata::default()),
        )
        .unwrap();
        let third = AutosaveId::generate();
        store.write(&third, &untitled, None).unwrap();
        let entry = store
            .list()
            .entries
            .into_iter()
            .find(|e| e.id == third)
            .unwrap();
        assert_eq!(entry.name, None);
    }

    #[test]
    fn ids_are_file_name_safe() {
        let id = AutosaveId::generate();
        let name = format!("{}.annota", id.as_str());
        assert_eq!(AutosaveId::from_file_name(&name), Some(id));
        assert_eq!(AutosaveId::from_file_name("../x.annota"), None);
        assert_eq!(AutosaveId::from_file_name(".annota"), None);
        assert_eq!(AutosaveId::from_file_name("abc.png"), None);
    }
}
