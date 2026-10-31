//! Batch export (add-export-share task 2.5, mockups 11–13; spec export-share "Batch export"):
//! several documents of the tray exported with the same format, size and file name pattern.
//!
//! [`BatchForm`] holds the dialog's values without widgets and plans where each document goes
//! ([`BatchForm::plan`]); [`present`] binds it to `batch-export-dialog.ui`. The format rows are
//! the export dialog's own ([`FormatRows`], [`Encoding`]).
//!
//! **File names.** The pattern's tokens are `%name` (the document's name, as the tray and the
//! window title show it), `%n` (its number in the selection, zero-padded to the selection's
//! size: `1`–`9`, `01`–`12`) and every strftime conversion glib knows (`%Y`, `%m`, `%d`, `%H`,
//! `%M`, `%S` …); `%%` is a literal `%`. The tokens are replaced before strftime runs, so a `%`
//! in a document's name stays as it is. The date and time are the moment the dialog opened: the
//! names it shows are the names it writes. Names inside one batch never collide — a later
//! document gets " 2", " 3" … like quick save — whatever "If a file exists" says, which is only
//! about files already in the folder: Add a number, Replace, or Skip that document.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gettextrs::{gettext, ngettext};
use gtk::{gio, glib};

use annota::document::Document;
use annota::export::{self, ExportError};
use annota::render;

use super::dialogs::thumbnail_area;
use super::export_dialog::{self, output_size, Encoding, FormatRows, Kind, PRESETS};
use super::saving::{self, Problem};
use crate::preferences;
use crate::ui::canvas::Canvas;

const UI_BATCH: &str = "/io/github/trosinde/Annota/ui/batch-export-dialog.ui";
/// Redline "Batch": the pattern's default.
pub const DEFAULT_PATTERN: &str = "%name";
/// Redline "Batch": 64 × 40 thumbnails in the list of documents.
const THUMB: (i32, i32) = (64, 40);

/// "If a file exists" (redline "Batch": Add a number / Replace / Skip), in the dropdown's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IfExists {
    AddNumber,
    Replace,
    Skip,
}

impl IfExists {
    pub fn from_index(index: u32) -> Self {
        match index {
            1 => IfExists::Replace,
            2 => IfExists::Skip,
            _ => IfExists::AddNumber,
        }
    }
}

/// Where one document of the batch goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// A new file.
    Write(PathBuf),
    /// Over a file that is there already ("Replace").
    Replace(PathBuf),
    /// Not exported: the file is there already ("Skip").
    Skip(PathBuf),
}

impl Outcome {
    pub fn path(&self) -> &Path {
        match self {
            Outcome::Write(p) | Outcome::Replace(p) | Outcome::Skip(p) => p,
        }
    }

    pub fn writes(&self) -> bool {
        !matches!(self, Outcome::Skip(_))
    }
}

/// `pattern` expanded for the document `name`, number `number` of `count`, at `now`: `%name`
/// and `%n` first (so the name's own `%` signs stay literal), then strftime on the rest.
pub fn expand_pattern(
    pattern: &str,
    name: &str,
    number: usize,
    count: usize,
    now: &glib::DateTime,
) -> String {
    let width = count.max(1).to_string().len();
    let number = format!("{number:0width$}");
    let mut out = String::new();
    // Text for strftime, collected up to the next token.
    let mut chunk = String::new();
    let flush = |chunk: &mut String, out: &mut String| {
        if !chunk.is_empty() {
            out.push_str(&preferences::expand_filename_pattern(chunk, now));
            chunk.clear();
        }
    };
    let mut rest = pattern;
    while let Some(at) = rest.find('%') {
        chunk.push_str(&rest[..at]);
        let token = &rest[at..];
        if let Some(after) = token.strip_prefix("%name") {
            flush(&mut chunk, &mut out);
            out.push_str(name);
            rest = after;
        } else if let Some(after) = token.strip_prefix("%%") {
            chunk.push_str("%%");
            rest = after;
        } else if let Some(after) = token.strip_prefix("%n") {
            flush(&mut chunk, &mut out);
            out.push_str(&number);
            rest = after;
        } else {
            chunk.push('%');
            rest = &token[1..];
        }
    }
    chunk.push_str(rest);
    flush(&mut chunk, &mut out);
    out
}

/// The file name (without extension) of document `name`, number `number` of `count`: the
/// expanded pattern made a valid file name; if nothing is left of it, the document's name.
pub fn stem(
    pattern: &str,
    name: &str,
    number: usize,
    count: usize,
    now: &glib::DateTime,
) -> String {
    saving::file_stem(&expand_pattern(pattern, name, number, count, now))
        .or_else(|| saving::file_stem(name))
        .unwrap_or_else(|| "Annota".to_string())
}

/// Where each of `stems` goes in `folder` with `extension`. Names inside the batch never
/// collide (later ones get " 2", " 3" …); `if_exists` decides about files already there
/// (`exists`).
pub fn plan(
    stems: &[String],
    folder: &Path,
    extension: &str,
    if_exists: IfExists,
    exists: impl Fn(&Path) -> bool,
) -> Vec<Outcome> {
    let mut taken: HashSet<PathBuf> = HashSet::new();
    stems
        .iter()
        .map(|stem| {
            let outcome = match if_exists {
                IfExists::AddNumber => {
                    Outcome::Write(saving::unique_path(folder, stem, extension, |p| {
                        exists(p) || taken.contains(p)
                    }))
                }
                IfExists::Replace | IfExists::Skip => {
                    let path = saving::unique_path(folder, stem, extension, |p| taken.contains(p));
                    match (exists(&path), if_exists) {
                        (false, _) => Outcome::Write(path),
                        (true, IfExists::Replace) => Outcome::Replace(path),
                        (true, _) => Outcome::Skip(path),
                    }
                }
            };
            taken.insert(outcome.path().to_path_buf());
            outcome
        })
        .collect()
}

/// The batch dialog's values.
#[derive(Clone, Debug, PartialEq)]
pub struct BatchForm {
    pub encoding: Encoding,
    /// The Size row: a percentage of each document's canvas.
    pub percent: u32,
    pub pattern: String,
    pub folder: PathBuf,
    pub if_exists: IfExists,
}

impl BatchForm {
    /// The defaults: the format of the `default-export-format` preference with the export
    /// dialog's defaults, 100 %, `%name`, Add a number.
    pub fn new(kind: Kind, folder: PathBuf) -> Self {
        Self {
            encoding: Encoding::new(kind),
            percent: 100,
            pattern: DEFAULT_PATTERN.to_string(),
            folder,
            if_exists: IfExists::AddNumber,
        }
    }

    pub fn scale(&self) -> f64 {
        f64::from(self.percent) / 100.0
    }

    /// Where the documents named `names` (in selection order) go.
    pub fn plan(
        &self,
        names: &[String],
        now: &glib::DateTime,
        exists: impl Fn(&Path) -> bool,
    ) -> Vec<Outcome> {
        let stems: Vec<String> = names
            .iter()
            .enumerate()
            .map(|(k, name)| stem(&self.pattern, name, k + 1, names.len(), now))
            .collect();
        plan(
            &stems,
            &self.folder,
            self.encoding.format().extension(),
            self.if_exists,
            exists,
        )
    }

    /// What keeps the batch from being exported, checked before anything is written: a
    /// document too large for the format at this size (`canvases`), a folder that cannot be
    /// written to.
    pub fn problem(
        &self,
        canvases: &[(i32, i32)],
        writable: impl Fn(&Path) -> bool,
    ) -> Option<Problem> {
        let scale = self.scale();
        canvases
            .iter()
            .find_map(|c| self.encoding.limit_problem(output_size(*c, scale)))
            .or_else(|| (!writable(&self.folder)).then_some(Problem::NoPermission))
    }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The dialog's title (mockup 12): "Export 3 Images".
pub fn title(n: usize) -> String {
    ngettext("Export {n} Image", "Export {n} Images", count(n)).replace("{n}", &n.to_string())
}

/// The tray's suggested button in selection mode (mockup 11): "Export 3…".
pub fn export_button_label(n: usize) -> String {
    gettext("Export {n}…").replace("{n}", &n.to_string())
}

/// The tray's selection bar (mockup 11): "3 Selected".
pub fn selected_label(n: usize) -> String {
    ngettext("{n} Selected", "{n} Selected", count(n)).replace("{n}", &n.to_string())
}

/// A document's line in the dialog (mockup 12: "→ Checkout flow.jpg"); Replace and Skip say so
/// (not drawn).
pub fn target_label(outcome: &Outcome) -> String {
    let name = file_name(outcome.path());
    match outcome {
        Outcome::Write(_) => gettext("→ {file}").replace("{file}", &name),
        Outcome::Replace(_) => {
            gettext("→ {file} · replaces the existing file").replace("{file}", &name)
        }
        Outcome::Skip(_) => gettext("Skipped · “{file}” already exists").replace("{file}", &name),
    }
}

/// Mockup 13: "3 images exported to Exports" (the folder's own name).
pub fn done_toast(n: usize, folder: &Path) -> String {
    ngettext(
        "{n} image exported to {folder}",
        "{n} images exported to {folder}",
        count(n),
    )
    .replace("{n}", &n.to_string())
    .replace("{folder}", &file_name(folder))
}

/// Some documents could not be written (not drawn): "1 image could not be exported: There is
/// not enough space on the disk".
pub fn failed_toast(n: usize, problem: Problem) -> String {
    ngettext(
        "{n} image could not be exported: {reason}",
        "{n} images could not be exported: {reason}",
        count(n),
    )
    .replace("{n}", &n.to_string())
    .replace("{reason}", &problem.message())
}

/// The Size row's entries: "200 %" … "25 %" (the export dialog's presets; no pixel sizes —
/// they differ per document — and no Custom Width…).
fn size_label(percent: u32) -> String {
    gettext("{percent} %").replace("{percent}", &percent.to_string())
}

/// One document of the batch.
pub struct Item {
    pub name: String,
    pub source: Source,
}

/// Where an item's document is.
pub enum Source {
    /// Open in the editor (its redactions converted there, as an undo step, if the user chose
    /// Convert to Solid).
    Open(Canvas),
    /// A recent file of the tray, read for this batch only (its file is not changed).
    Closed(Box<Document>),
}

impl Item {
    fn with_document<T>(&self, f: impl FnOnce(&Document) -> T) -> Option<T> {
        match &self.source {
            Source::Open(canvas) => canvas.with_document(f),
            Source::Closed(doc) => Some(f(doc)),
        }
    }

    fn canvas_size(&self) -> (i32, i32) {
        self.with_document(|d| (d.canvas().rect.width, d.canvas().rect.height))
            .unwrap_or((1, 1))
    }
}

/// What a batch did.
pub struct BatchResult {
    pub exported: Vec<PathBuf>,
    pub failed: usize,
    /// Why the first failure happened.
    pub problem: Option<Problem>,
    pub folder: PathBuf,
}

/// The dialog's widgets (from `batch-export-dialog.ui`).
struct Widgets {
    dialog: adw::Dialog,
    cancel_button: gtk::Button,
    export_button: gtk::Button,
    banner: adw::Banner,
    items_list: gtk::ListBox,
    rows: FormatRows,
    size_row: adw::ComboRow,
    pattern_row: adw::EntryRow,
    folder_row: adw::ActionRow,
    folder_button: gtk::Button,
    exists_row: adw::ComboRow,
}

macro_rules! obj {
    ($builder:expr, $name:expr) => {
        match $builder.object($name) {
            Some(object) => object,
            None => {
                eprintln!(
                    "annota: batch-export-dialog.ui is missing object '{}'",
                    $name
                );
                return None;
            }
        }
    };
}

fn widgets(builder: &gtk::Builder) -> Option<Widgets> {
    Some(Widgets {
        dialog: obj!(builder, "batch_dialog"),
        cancel_button: obj!(builder, "cancel_button"),
        export_button: obj!(builder, "export_button"),
        banner: obj!(builder, "error_banner"),
        items_list: obj!(builder, "items_list"),
        rows: FormatRows::new(builder, "batch-export-dialog.ui")?,
        size_row: obj!(builder, "size_row"),
        pattern_row: obj!(builder, "pattern_row"),
        folder_row: obj!(builder, "folder_row"),
        folder_button: obj!(builder, "folder_button"),
        exists_row: obj!(builder, "exists_row"),
    })
}

/// The open dialog.
struct State {
    w: Widgets,
    items: Vec<Item>,
    /// Each item's row: its "→ <file>" line.
    item_rows: Vec<adw::ActionRow>,
    form: RefCell<BatchForm>,
    /// The time strftime conversions use: when the dialog opened.
    now: glib::DateTime,
    /// What the last export run reported; cleared by the next change.
    failure: Cell<Option<Problem>>,
    /// Set while the files are written.
    busy: Cell<bool>,
    /// Set while the widgets follow the form, so that is not an edit.
    updating: Cell<bool>,
    on_done: Box<dyn Fn(BatchResult)>,
}

impl State {
    fn names(&self) -> Vec<String> {
        self.items.iter().map(|i| i.name.clone()).collect()
    }

    fn plan(&self) -> Vec<Outcome> {
        self.form
            .borrow()
            .plan(&self.names(), &self.now, |p| p.exists())
    }

    fn problem(&self) -> Option<Problem> {
        if let Some(failure) = self.failure.get() {
            return Some(failure);
        }
        let canvases: Vec<(i32, i32)> = self.items.iter().map(Item::canvas_size).collect();
        self.form
            .borrow()
            .problem(&canvases, export_dialog::folder_writable)
    }

    fn refresh(&self) {
        let form = self.form.borrow().clone();
        let w = &self.w;
        self.updating.set(true);
        w.rows.show(&form.encoding);
        w.folder_row
            .set_subtitle(&preferences::display_path(&form.folder));
        self.updating.set(false);

        let plan = self.plan();
        for (row, outcome) in self.item_rows.iter().zip(&plan) {
            row.set_subtitle(&target_label(outcome));
        }
        let problem = self.problem();
        match problem {
            Some(problem) => {
                w.banner.set_title(&problem.message());
                let button = (problem == Problem::NoPermission).then(|| gettext("Choose Folder…"));
                w.banner.set_button_label(button.as_deref());
                w.banner.set_revealed(true);
            }
            None => w.banner.set_revealed(false),
        }
        let blocked = problem.is_some_and(Problem::blocks_export);
        let writes = plan.iter().any(Outcome::writes);
        w.export_button
            .set_sensitive(!self.busy.get() && !blocked && writes);
    }

    fn edit(&self, change: impl FnOnce(&mut BatchForm)) {
        if self.updating.get() || self.busy.get() {
            return;
        }
        change(&mut self.form.borrow_mut());
        self.failure.set(None);
        self.refresh();
    }

    /// Export: every document that is not skipped, one after the other (the window redraws
    /// in between), then the dialog closes and `on_done` reports.
    fn export(self: &Rc<Self>) {
        self.edit(|f| f.pattern = self.w.pattern_row.text().to_string());
        if self.busy.get() || self.problem().is_some_and(Problem::blocks_export) {
            return;
        }
        let plan = self.plan();
        if !plan.iter().any(Outcome::writes) {
            return;
        }
        self.busy.set(true);
        self.w.export_button.set_sensitive(false);
        self.w.cancel_button.set_sensitive(false);
        self.w.dialog.set_can_close(false);
        let form = self.form.borrow().clone();
        let state = self.clone();
        glib::spawn_future_local(async move {
            let (format, scale) = (form.encoding.format(), form.scale());
            let mut result = BatchResult {
                exported: Vec::new(),
                failed: 0,
                problem: None,
                folder: form.folder.clone(),
            };
            // The folder is created if missing; if it cannot be, nothing can be written.
            let folder = std::fs::create_dir_all(&form.folder).map_err(|err| {
                eprintln!(
                    "annota: cannot create the export folder {}: {err}",
                    form.folder.display()
                );
                saving::problem(&ExportError::Io(err))
            });
            for (item, outcome) in state.items.iter().zip(&plan) {
                if !outcome.writes() {
                    continue;
                }
                let path = outcome.path();
                let written = folder.and_then(|()| {
                    item.with_document(|doc| export::export(doc, path, &format, scale))
                        .unwrap_or(Ok(()))
                        .map_err(|err| {
                            // The user's chosen location; no capture content is logged.
                            eprintln!("annota: batch export to {} failed: {err}", path.display());
                            saving::problem(&err)
                        })
                });
                match written {
                    Ok(()) => result.exported.push(path.to_path_buf()),
                    Err(problem) => {
                        result.failed += 1;
                        result.problem.get_or_insert(problem);
                    }
                }
                glib::timeout_future(Duration::ZERO).await;
            }
            state.busy.set(false);
            state.w.dialog.set_can_close(true);
            state.w.dialog.close();
            (state.on_done)(result);
        });
    }

    /// The folder button and the banner's Choose Folder…: GTK's folder chooser.
    fn choose_folder(self: &Rc<Self>) {
        let initial = self.form.borrow().folder.clone();
        let mut builder = gtk::FileDialog::builder()
            .title(gettext("Choose Folder"))
            .modal(true);
        if initial.is_dir() {
            builder = builder.initial_folder(&gio::File::for_path(&initial));
        }
        let chooser = builder.build();
        let window = self.w.dialog.root().and_downcast::<gtk::Window>();
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let Ok(folder) = chooser.select_folder_future(window.as_ref()).await else {
                return;
            };
            if let (Some(state), Some(path)) = (weak.upgrade(), folder.path()) {
                state.edit(|f| f.folder = path);
            }
        });
    }
}

/// A document's row: 64 × 40 thumbnail, name, "→ <file>" (mockup 12).
fn item_row(item: &Item) -> adw::ActionRow {
    let thumbnail = item
        .with_document(|doc| {
            let c = doc.canvas().rect;
            let scale = (f64::from(THUMB.0 * 2) / f64::from(c.width.max(1)))
                .max(f64::from(THUMB.1 * 2) / f64::from(c.height.max(1)))
                .min(1.0);
            render::flatten(doc, scale).ok()
        })
        .flatten();
    let area = thumbnail_area(thumbnail, THUMB, 4.0);
    area.add_css_class("annota-export-thumb");
    let row = adw::ActionRow::builder()
        .title(&item.name)
        .use_markup(false)
        .title_lines(1)
        .subtitle_lines(1)
        .activatable(false)
        .build();
    row.add_prefix(&area);
    row
}

/// Shows "Export <n> Images" (mockup 12) for `items` over `parent`, starting from the
/// `default-export-format` and `export-folder` preferences. `on_done` gets what was written
/// once the dialog has closed.
pub fn present(
    parent: &impl IsA<gtk::Widget>,
    items: Vec<Item>,
    settings: &gio::Settings,
    on_done: impl Fn(BatchResult) + 'static,
) {
    if items.is_empty() {
        return;
    }
    let builder = gtk::Builder::from_resource(UI_BATCH);
    let Some(w) = widgets(&builder) else {
        return;
    };
    let Ok(now) = glib::DateTime::now_local() else {
        return;
    };
    let kind = Kind::from_setting(settings.enum_("default-export-format"));
    let folder = preferences::resolved_export_folder(&settings.string("export-folder"));
    let form = BatchForm::new(kind, folder);

    w.dialog.set_title(&title(items.len()));
    let item_rows: Vec<adw::ActionRow> = items.iter().map(item_row).collect();
    for row in &item_rows {
        w.items_list.append(row);
    }
    let sizes = gtk::StringList::new(&[]);
    for percent in PRESETS {
        sizes.append(&size_label(percent));
    }
    w.size_row.set_model(Some(&sizes));
    w.size_row.set_selected(
        PRESETS
            .iter()
            .position(|p| *p == form.percent)
            .and_then(|i| u32::try_from(i).ok())
            .unwrap_or(1),
    );
    w.pattern_row.set_text(&form.pattern);

    let state = Rc::new(State {
        w,
        items,
        item_rows,
        form: RefCell::new(form),
        now,
        failure: Cell::new(None),
        busy: Cell::new(false),
        updating: Cell::new(false),
        on_done: Box::new(on_done),
    });
    connect(&state);
    state.refresh();
    let w = &state.w;
    w.dialog.set_default_widget(Some(&w.export_button));
    w.dialog.present(Some(parent));
}

fn connect(state: &Rc<State>) {
    let w = &state.w;
    let weak = || Rc::downgrade(state);

    let dialog = w.dialog.downgrade();
    w.cancel_button.connect_clicked(move |_| {
        if let Some(dialog) = dialog.upgrade() {
            dialog.close();
        }
    });
    let s = weak();
    w.export_button.connect_clicked(move |_| {
        if let Some(state) = s.upgrade() {
            state.export();
        }
    });
    let s = weak();
    w.banner.connect_button_clicked(move |_| {
        if let Some(state) = s.upgrade() {
            if state.problem() == Some(Problem::NoPermission) {
                state.choose_folder();
            }
        }
    });
    let s = weak();
    w.rows
        .connect(Rc::new(move |change: &dyn Fn(&mut Encoding)| {
            if let Some(state) = s.upgrade() {
                state.edit(|f| change(&mut f.encoding));
            }
        }));
    let s = weak();
    w.size_row.connect_selected_notify(move |row| {
        let percent = usize::try_from(row.selected())
            .ok()
            .and_then(|i| PRESETS.get(i).copied());
        if let (Some(state), Some(percent)) = (s.upgrade(), percent) {
            state.edit(|f| f.percent = percent);
        }
    });
    let s = weak();
    w.pattern_row.connect_changed(move |row| {
        if let Some(state) = s.upgrade() {
            state.edit(|f| f.pattern = row.text().to_string());
        }
    });
    let s = weak();
    w.pattern_row.connect_entry_activated(move |_| {
        if let Some(state) = s.upgrade() {
            if state.w.export_button.is_sensitive() {
                state.export();
            }
        }
    });
    let s = weak();
    w.folder_button.connect_clicked(move |_| {
        if let Some(state) = s.upgrade() {
            state.choose_folder();
        }
    });
    let s = weak();
    w.exists_row.connect_selected_notify(move |row| {
        if let Some(state) = s.upgrade() {
            state.edit(|f| f.if_exists = IfExists::from_index(row.selected()));
        }
    });
    // The dialog owns its state until it closes (the handlers above only hold it weakly).
    let owned = RefCell::new(Some(state.clone()));
    w.dialog.connect_closed(move |_| {
        owned.take();
    });
}

/// The dialog's state, for the live test: its form and each document's line.
#[cfg(test)]
pub fn shown_targets(dialog: &adw::Dialog) -> Vec<String> {
    fn walk(widget: &gtk::Widget, out: &mut Vec<String>) {
        if let Some(row) = widget.downcast_ref::<adw::ActionRow>() {
            let subtitle = row.subtitle().map(|s| s.to_string()).unwrap_or_default();
            if subtitle.starts_with('→') || subtitle.starts_with("Skipped") {
                out.push(subtitle);
            }
        }
        let mut child = widget.first_child();
        while let Some(c) = child {
            walk(&c, out);
            child = c.next_sibling();
        }
    }
    let mut out = Vec::new();
    walk(dialog.upcast_ref(), &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn time() -> glib::DateTime {
        glib::DateTime::from_local(2026, 10, 31, 9, 5, 7.0).unwrap()
    }

    #[test]
    fn the_pattern_knows_name_number_and_strftime() {
        let now = time();
        // Redline default: the document's name.
        assert_eq!(
            expand_pattern("%name", "Checkout flow", 1, 3, &now),
            "Checkout flow"
        );
        assert_eq!(
            expand_pattern("%n %name", "Checkout flow", 2, 3, &now),
            "2 Checkout flow"
        );
        // %n is padded to the selection's size.
        assert_eq!(expand_pattern("Shot %n", "x", 3, 12, &now), "Shot 03");
        assert_eq!(expand_pattern("Shot %n", "x", 12, 12, &now), "Shot 12");
        assert_eq!(
            expand_pattern("%name %Y-%m-%d %H-%M-%S", "Chat", 1, 1, &now),
            "Chat 2026-10-31 09-05-07"
        );
        // A name's own % signs are not strftime conversions; %% is a literal %.
        assert_eq!(
            expand_pattern("%name", "50% %d off", 1, 1, &now),
            "50% %d off"
        );
        assert_eq!(expand_pattern("100%% %name", "x", 1, 1, &now), "100% x");
        assert_eq!(expand_pattern("%%name", "x", 1, 1, &now), "%name");
        // glib's strftime drops a lone trailing %; the file name is what is left.
        assert_eq!(stem("Shot %", "x", 1, 1, &now), "Shot");
    }

    #[test]
    fn stems_are_file_names() {
        let now = time();
        assert_eq!(stem("%name", "Checkout/flow", 1, 1, &now), "Checkout-flow");
        // Nothing left of the pattern: the document's name.
        assert_eq!(stem("  ", "Chat with Jonas", 1, 1, &now), "Chat with Jonas");
        assert_eq!(stem("", "", 1, 1, &now), "Annota");
    }

    fn names(plan: &[Outcome]) -> Vec<(String, &'static str)> {
        plan.iter()
            .map(|o| {
                let kind = match o {
                    Outcome::Write(_) => "write",
                    Outcome::Replace(_) => "replace",
                    Outcome::Skip(_) => "skip",
                };
                (file_name(o.path()), kind)
            })
            .collect()
    }

    #[test]
    fn if_a_file_exists_adds_a_number_replaces_or_skips() {
        let folder = Path::new("/home/mara/Pictures/Annota/Exports");
        let stems: Vec<String> = ["Checkout flow", "September dashboard", "Chat with Jonas"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let there = [
            folder.join("Checkout flow.jpg"),
            folder.join("Checkout flow 2.jpg"),
        ];
        let exists = |p: &Path| there.iter().any(|t| t == p);
        // Mockup 12: nothing there yet.
        assert_eq!(
            names(&plan(&stems, folder, "jpg", IfExists::AddNumber, |_| false)),
            [
                ("Checkout flow.jpg".into(), "write"),
                ("September dashboard.jpg".into(), "write"),
                ("Chat with Jonas.jpg".into(), "write")
            ]
        );
        assert_eq!(
            names(&plan(&stems, folder, "jpg", IfExists::AddNumber, exists))[0],
            ("Checkout flow 3.jpg".into(), "write")
        );
        assert_eq!(
            names(&plan(&stems, folder, "jpg", IfExists::Replace, exists))[0],
            ("Checkout flow.jpg".into(), "replace")
        );
        assert_eq!(
            names(&plan(&stems, folder, "jpg", IfExists::Skip, exists))[..2],
            [
                ("Checkout flow.jpg".into(), "skip"),
                ("September dashboard.jpg".into(), "write")
            ]
        );
    }

    #[test]
    fn names_inside_a_batch_never_collide() {
        // A pattern without %name or %n, or two documents of the same name.
        let folder = Path::new("/tmp/out");
        let stems = vec!["Shot".to_string(), "Shot".to_string(), "Shot".to_string()];
        for policy in [IfExists::AddNumber, IfExists::Replace, IfExists::Skip] {
            assert_eq!(
                names(&plan(&stems, folder, "png", policy, |_| false)),
                [
                    ("Shot.png".into(), "write"),
                    ("Shot 2.png".into(), "write"),
                    ("Shot 3.png".into(), "write")
                ],
                "{policy:?}"
            );
        }
        // Replace replaces what is in the folder, never what the batch itself just wrote.
        let there = folder.join("Shot.png");
        assert_eq!(
            names(&plan(&stems, folder, "png", IfExists::Replace, |p| p == there)),
            [
                ("Shot.png".into(), "replace"),
                ("Shot 2.png".into(), "write"),
                ("Shot 3.png".into(), "write")
            ]
        );
        assert_eq!(IfExists::from_index(0), IfExists::AddNumber);
        assert_eq!(IfExists::from_index(1), IfExists::Replace);
        assert_eq!(IfExists::from_index(2), IfExists::Skip);
    }

    #[test]
    fn the_form_plans_with_its_format_and_folder() {
        // Scenario "Batch": three documents as JPG.
        let mut form = BatchForm::new(Kind::Png, PathBuf::from("/x/Exports"));
        assert_eq!(form.pattern, "%name");
        assert_eq!(form.scale(), 1.0);
        form.encoding.kind = Kind::Jpeg;
        let names = ["Checkout flow", "September dashboard", "Chat with Jonas"]
            .map(String::from)
            .to_vec();
        let plan = form.plan(&names, &time(), |_| false);
        assert_eq!(
            plan.iter()
                .map(|o| o.path().to_path_buf())
                .collect::<Vec<_>>(),
            [
                PathBuf::from("/x/Exports/Checkout flow.jpg"),
                PathBuf::from("/x/Exports/September dashboard.jpg"),
                PathBuf::from("/x/Exports/Chat with Jonas.jpg")
            ]
        );
        form.pattern = "%n-%name".into();
        form.encoding.kind = Kind::Pdf;
        assert_eq!(
            file_name(form.plan(&names, &time(), |_| false)[2].path()),
            "3-Chat with Jonas.pdf"
        );
    }

    #[test]
    fn problems_before_writing() {
        let mut form = BatchForm::new(Kind::WebP, PathBuf::from("/x"));
        let yes = |_: &Path| true;
        assert_eq!(form.problem(&[(1600, 1000)], yes), None);
        assert_eq!(
            form.problem(&[(1600, 1000)], |_| false),
            Some(Problem::NoPermission)
        );
        // One document too large for WebP at 200 % is enough to stop the batch.
        form.percent = 200;
        assert_eq!(
            form.problem(&[(800, 600), (9000, 1000)], yes),
            Some(Problem::TooLarge {
                format: "WebP",
                max: 16383
            })
        );
        form.encoding.kind = Kind::Png;
        assert_eq!(form.problem(&[(800, 600), (9000, 1000)], yes), None);
    }

    #[test]
    fn texts_follow_mockups_11_to_13() {
        assert_eq!(title(3), "Export 3 Images");
        assert_eq!(title(1), "Export 1 Image");
        assert_eq!(export_button_label(3), "Export 3…");
        assert_eq!(selected_label(3), "3 Selected");
        assert_eq!(
            target_label(&Outcome::Write(PathBuf::from("/x/Checkout flow.jpg"))),
            "→ Checkout flow.jpg"
        );
        assert_eq!(
            target_label(&Outcome::Replace(PathBuf::from("/x/a.jpg"))),
            "→ a.jpg · replaces the existing file"
        );
        assert_eq!(
            target_label(&Outcome::Skip(PathBuf::from("/x/a.jpg"))),
            "Skipped · “a.jpg” already exists"
        );
        assert_eq!(
            done_toast(3, Path::new("/home/mara/Pictures/Annota/Exports")),
            "3 images exported to Exports"
        );
        assert_eq!(
            done_toast(1, Path::new("/x/Out")),
            "1 image exported to Out"
        );
        assert_eq!(
            failed_toast(2, Problem::NoSpace),
            "2 images could not be exported: There is not enough space on the disk"
        );
        assert_eq!(size_label(50), "50 %");
    }
}
