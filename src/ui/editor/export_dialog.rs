//! "Export Image" (add-export-share mockups 02–05; redline "Export dialog"; spec export-share
//! "Export formats"): the document flattened to PNG, JPG, WebP or PDF through
//! `annota::export`, with the format's own settings, a size factor, file name and folder.
//!
//! [`Form`] holds the dialog's values without widgets (and is what the tests check);
//! [`present`] binds it to `export-dialog.ui`. The error banner (mockup 04) shows what
//! [`Form::problem`] finds before anything is written — no permission for the folder, a file
//! that already exists, an image too large for the format — and what an export that failed
//! reports (`saving::problem`). The estimated size is the real encoder's output for a sample
//! of the image ([`estimate`]), worked out a moment after the settings last changed.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gettextrs::{gettext, pgettext};
use gtk::{gio, glib};

use annota::document::bitmap::MAX_SIDE;
use annota::document::Document;
use annota::export::raster::WEBP_MAX_SIDE;
use annota::export::{
    self, ExportError, Format, PageSize, PdfOptions, WebPCompression, DEFAULT_JPEG_QUALITY,
    DEFAULT_WEBP_QUALITY,
};
use annota::render;

use super::dialogs::thumbnail_area;
use super::saving::{self, Problem};
use crate::preferences;
use crate::ui::canvas::Canvas;

const UI_EXPORT: &str = "/io/github/trosinde/Annota/ui/export-dialog.ui";
/// Redline: 160 × 100 preview thumbnail, radius 6.
const THUMB: (i32, i32) = (160, 100);
/// The size menu's presets (redline "Size": 200 %, 100 %, 50 %, 25 %).
pub const PRESETS: [u32; 4] = [200, 100, 50, 25];
/// The estimate encodes at most this many pixels and scales the result up.
const ESTIMATE_PIXELS: f64 = 500_000.0;
/// The estimate is worked out this long after the settings last changed.
const ESTIMATE_DELAY: Duration = Duration::from_millis(250);

/// The Format toggle (PNG | JPG | WebP | PDF), in the order of the `default-export-format`
/// preference's enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Png,
    Jpeg,
    WebP,
    Pdf,
}

impl Kind {
    /// The `default-export-format` preference (png = 0, jpg, webp, pdf).
    pub fn from_setting(value: i32) -> Kind {
        match value {
            1 => Kind::Jpeg,
            2 => Kind::WebP,
            3 => Kind::Pdf,
            _ => Kind::Png,
        }
    }
}

/// The "Size" row: a preset percentage or a width typed in "Custom Width…".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    Percent(u32),
    Width(i32),
}

/// The dialog's values.
#[derive(Clone, Debug, PartialEq)]
pub struct Form {
    /// The document's canvas in pixels.
    pub canvas: (i32, i32),
    pub kind: Kind,
    pub jpeg_quality: u8,
    pub webp_lossy: bool,
    pub webp_quality: u8,
    pub pdf: PdfOptions,
    pub size: Size,
    /// The file name as typed, without the extension (shown dim and not editable).
    pub name: String,
    pub folder: PathBuf,
}

impl Form {
    /// The defaults of the redline: 100 %, JPG 85, WebP lossless (lossy at 80), PDF with
    /// vectors on a page the size of the image.
    pub fn new(canvas: (i32, i32), kind: Kind, name: &str, folder: PathBuf) -> Self {
        Self {
            canvas: (canvas.0.max(1), canvas.1.max(1)),
            kind,
            jpeg_quality: DEFAULT_JPEG_QUALITY,
            webp_lossy: false,
            webp_quality: DEFAULT_WEBP_QUALITY,
            pdf: PdfOptions::default(),
            size: Size::Percent(100),
            name: saving::file_stem(name).unwrap_or_default(),
            folder,
        }
    }

    pub fn format(&self) -> Format {
        match self.kind {
            Kind::Png => Format::Png,
            Kind::Jpeg => Format::Jpeg {
                quality: self.jpeg_quality,
            },
            Kind::WebP => Format::WebP {
                compression: if self.webp_lossy {
                    WebPCompression::Lossy {
                        quality: self.webp_quality,
                    }
                } else {
                    WebPCompression::Lossless
                },
            },
            Kind::Pdf => Format::Pdf(self.pdf),
        }
    }

    /// Output pixels per document pixel.
    pub fn scale(&self) -> f64 {
        match self.size {
            Size::Percent(p) => f64::from(p) / 100.0,
            Size::Width(w) => f64::from(w.max(1)) / f64::from(self.canvas.0),
        }
    }

    /// The exported image's size in pixels, rounded the way the renderer rounds it.
    pub fn output_size(&self) -> (i32, i32) {
        output_size(self.canvas, self.scale())
    }

    /// The file name without the extension: what was typed, made a valid file name, without
    /// the format's own extension if it was typed too ("Shot.png" → "Shot"). `None` while the
    /// name is empty.
    pub fn stem(&self) -> Option<String> {
        let typed = self.name.trim();
        let ext = format!(".{}", self.format().extension());
        let bare = match typed.len().checked_sub(ext.len()) {
            Some(cut)
                if cut > 0
                    && typed.is_char_boundary(cut)
                    && typed[cut..].eq_ignore_ascii_case(&ext) =>
            {
                &typed[..cut]
            }
            _ => typed,
        };
        saving::file_stem(bare)
    }

    /// Where the file goes.
    pub fn path(&self) -> Option<PathBuf> {
        let stem = self.stem()?;
        Some(
            self.folder
                .join(format!("{stem}.{}", self.format().extension())),
        )
    }

    /// An image larger than the format allows (WebP 16383 px per side, the renderer 32767).
    pub fn limit_problem(&self) -> Option<Problem> {
        let (w, h) = self.output_size();
        let (format, max) = match self.kind {
            Kind::WebP => ("WebP", WEBP_MAX_SIDE),
            _ => (self.format().name(), MAX_SIDE),
        };
        (w > max || h > max).then_some(Problem::TooLarge { format, max })
    }

    /// What keeps the export from working, checked before anything is written: the size, the
    /// folder (`writable`), an existing file (unless `replace` is that file).
    pub fn problem(
        &self,
        writable: impl Fn(&Path) -> bool,
        exists: impl Fn(&Path) -> bool,
        replace: Option<&Path>,
    ) -> Option<Problem> {
        if let Some(problem) = self.limit_problem() {
            return Some(problem);
        }
        let path = self.path()?;
        if !writable(&self.folder) {
            return Some(Problem::NoPermission);
        }
        (exists(&path) && replace != Some(path.as_path())).then_some(Problem::Exists)
    }
}

/// `canvas` × `scale`, as `render::flatten` rounds it.
pub fn output_size(canvas: (i32, i32), scale: f64) -> (i32, i32) {
    let side = |v: i32| (f64::from(v) * scale).round().max(1.0) as i32;
    (side(canvas.0), side(canvas.1))
}

/// The size button: "100 % · 1600 × 1000 px" (mockups 02–05).
pub fn size_label(form: &Form) -> String {
    let (w, h) = form.output_size();
    let percent = (form.scale() * 100.0).round() as i64;
    gettext("{percent} % · {width} × {height} px")
        .replace("{percent}", &percent.to_string())
        .replace("{width}", &w.to_string())
        .replace("{height}", &h.to_string())
}

/// A size menu entry: "200 % · 3200 × 2000" (mockup 03).
pub fn preset_label(canvas: (i32, i32), percent: u32) -> String {
    let (w, h) = output_size(canvas, f64::from(percent) / 100.0);
    gettext("{percent} % · {width} × {height}")
        .replace("{percent}", &percent.to_string())
        .replace("{width}", &w.to_string())
        .replace("{height}", &h.to_string())
}

/// "≈ 412 KB", "≈ 1.4 MB" (decimal units, like GNOME Files).
pub fn approx_size(bytes: u64) -> String {
    let kb = (bytes as f64 / 1000.0).ceil().max(1.0);
    if kb < 1000.0 {
        return gettext("≈ {size} KB").replace("{size}", &(kb as u64).to_string());
    }
    let mb = bytes as f64 / 1_000_000.0;
    let text = format!("{mb:.1}").replace('.', &pgettext("decimal separator", "."));
    gettext("≈ {size} MB").replace("{size}", &text)
}

/// The preview's second line: "1600 × 1000 px · PNG · ≈ 412 KB" (without the size while it is
/// being worked out).
pub fn summary(form: &Form, estimate: Option<u64>) -> String {
    let (w, h) = form.output_size();
    let line = gettext("{width} × {height} px · {format}")
        .replace("{width}", &w.to_string())
        .replace("{height}", &h.to_string())
        .replace("{format}", form.format().name());
    match estimate {
        Some(bytes) => format!("{line} · {}", approx_size(bytes)),
        None => line,
    }
}

/// The fraction of the output's sides the estimate encodes: all of it up to
/// [`ESTIMATE_PIXELS`], else a smaller copy with about that many pixels.
pub fn sample_factor(size: (i32, i32)) -> f64 {
    let pixels = f64::from(size.0) * f64::from(size.1);
    if pixels <= ESTIMATE_PIXELS {
        1.0
    } else {
        (ESTIMATE_PIXELS / pixels).sqrt()
    }
}

/// About how large the file of `doc` in `form`'s settings gets: the encoder's output for a
/// sample of at most [`ESTIMATE_PIXELS`] (the whole image if it is smaller — then exact),
/// scaled by the pixel count. `None` if it cannot be encoded.
pub fn estimate(doc: &Document, form: &Form) -> Option<u64> {
    let full = form.output_size();
    let factor = sample_factor(full);
    let scale = form.scale() * factor;
    let bytes = export::encode(doc, &form.format(), scale).ok()?.len() as f64;
    let sample = output_size(form.canvas, scale);
    let ratio =
        (f64::from(full.0) * f64::from(full.1)) / (f64::from(sample.0) * f64::from(sample.1));
    Some((bytes * ratio).round() as u64)
}

/// Whether files can be created in `folder` — or, if it does not exist yet, in the nearest
/// folder above it that does (the export creates the rest). Unknown counts as writable; the
/// export itself reports what really happens.
fn folder_writable(folder: &Path) -> bool {
    let Some(existing) = folder.ancestors().find(|dir| dir.exists()) else {
        return true;
    };
    gio::File::for_path(existing)
        .query_info(
            gio::FILE_ATTRIBUTE_ACCESS_CAN_WRITE,
            gio::FileQueryInfoFlags::NONE,
            gio::Cancellable::NONE,
        )
        .map(|info| info.boolean(gio::FILE_ATTRIBUTE_ACCESS_CAN_WRITE))
        .unwrap_or(true)
}

/// The dialog's widgets (from `export-dialog.ui`).
struct Widgets {
    dialog: adw::Dialog,
    cancel_button: gtk::Button,
    export_button: gtk::Button,
    banner: adw::Banner,
    summary_label: gtk::Label,
    name_row: adw::EntryRow,
    extension_label: gtk::Label,
    folder_row: adw::ActionRow,
    folder_button: gtk::Button,
    kinds: [(Kind, gtk::ToggleButton); 4],
    compression_row: adw::ActionRow,
    lossy_button: gtk::ToggleButton,
    quality_row: adw::ActionRow,
    quality_scale: gtk::Scale,
    size_button: gtk::MenuButton,
    flatten_row: adw::ActionRow,
    vectors_row: adw::SwitchRow,
    page_row: adw::ComboRow,
    custom_width: adw::AlertDialog,
    width_spin: gtk::SpinButton,
}

/// A size estimate: the format and output size it was worked out for, and the bytes.
type Estimate = (Format, (i32, i32), u64);

/// The open dialog.
struct State {
    w: Widgets,
    canvas: Canvas,
    form: RefCell<Form>,
    /// The existing file Replace was chosen for.
    replace: RefCell<Option<PathBuf>>,
    /// What the last export reported; cleared by the next change.
    failure: Cell<Option<Problem>>,
    /// The size estimate of the settings it was worked out for.
    estimate: RefCell<Option<Estimate>>,
    estimate_pending: Cell<Option<glib::SourceId>>,
    /// Set while the widgets follow the form, so that is not an edit.
    updating: Cell<bool>,
    on_exported: Box<dyn Fn(PathBuf)>,
}

impl State {
    fn problem(&self) -> Option<Problem> {
        if let Some(failure) = self.failure.get() {
            return Some(failure);
        }
        let replace = self.replace.borrow().clone();
        self.form
            .borrow()
            .problem(folder_writable, |p| p.exists(), replace.as_deref())
    }

    /// Shows the form: which rows apply to the format, the labels, the banner, whether Export
    /// can be pressed, and the estimate.
    fn refresh(self: &Rc<Self>) {
        let form = self.form.borrow().clone();
        let w = &self.w;
        self.updating.set(true);
        let raster = form.kind != Kind::Pdf;
        w.compression_row.set_visible(form.kind == Kind::WebP);
        w.quality_row
            .set_visible(form.kind == Kind::Jpeg || (form.kind == Kind::WebP && form.webp_lossy));
        w.flatten_row.set_visible(raster);
        w.vectors_row.set_visible(!raster);
        w.page_row.set_visible(!raster);
        let quality = match form.kind {
            Kind::WebP => form.webp_quality,
            _ => form.jpeg_quality,
        };
        w.quality_scale.set_value(f64::from(quality));
        w.extension_label
            .set_text(&format!(".{}", form.format().extension()));
        w.size_button.set_label(&size_label(&form));
        let folder = preferences::display_path(&form.folder);
        w.folder_row.set_subtitle(&folder);
        self.updating.set(false);

        let problem = self.problem();
        match problem {
            Some(problem) => {
                w.banner.set_title(&problem.message());
                w.banner.set_button_label(problem.button().as_deref());
                if problem == Problem::Exists {
                    w.banner.add_css_class("annota-banner-destructive");
                } else {
                    w.banner.remove_css_class("annota-banner-destructive");
                }
                w.banner.set_revealed(true);
            }
            None => w.banner.set_revealed(false),
        }
        let blocked = problem.is_some_and(Problem::blocks_export);
        w.export_button
            .set_sensitive(form.path().is_some() && !blocked);

        let key = (form.format(), form.output_size());
        let known = self
            .estimate
            .borrow()
            .as_ref()
            .filter(|(format, size, _)| (*format, *size) == key)
            .map(|(_, _, bytes)| *bytes);
        w.summary_label.set_text(&summary(&form, known));
        if known.is_none() {
            self.schedule_estimate();
        }
    }

    fn schedule_estimate(self: &Rc<Self>) {
        if let Some(pending) = self.estimate_pending.take() {
            pending.remove();
        }
        let weak = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(ESTIMATE_DELAY, move || {
            let Some(state) = weak.upgrade() else {
                return;
            };
            state.estimate_pending.set(None);
            let form = state.form.borrow().clone();
            if let Some(bytes) = state
                .canvas
                .with_document(|doc| estimate(doc, &form))
                .flatten()
            {
                state
                    .estimate
                    .replace(Some((form.format(), form.output_size(), bytes)));
            }
            state
                .w
                .summary_label
                .set_text(&summary(&form, state.estimate_for(&form)));
        });
        self.estimate_pending.set(Some(id));
    }

    fn estimate_for(&self, form: &Form) -> Option<u64> {
        let key = (form.format(), form.output_size());
        self.estimate
            .borrow()
            .as_ref()
            .filter(|(format, size, _)| (*format, *size) == key)
            .map(|(_, _, bytes)| *bytes)
    }

    /// Changes the form (a widget was edited) and shows the result.
    fn edit(self: &Rc<Self>, change: impl FnOnce(&mut Form)) {
        if self.updating.get() {
            return;
        }
        change(&mut self.form.borrow_mut());
        self.failure.set(None);
        self.refresh();
    }

    /// Export (header button, Enter) or Replace in the banner.
    fn export(self: &Rc<Self>) {
        // A name typed but not confirmed yet counts.
        self.edit(|f| f.name = self.w.name_row.text().to_string());
        let form = self.form.borrow().clone();
        let Some(path) = form.path() else {
            return;
        };
        if self.problem().is_some_and(Problem::blocks_export) {
            return;
        }
        let written = std::fs::create_dir_all(&form.folder)
            .map_err(ExportError::Io)
            .and_then(|()| {
                self.canvas
                    .with_document(|doc| export::export(doc, &path, &form.format(), form.scale()))
                    .unwrap_or(Ok(()))
            });
        match written {
            Ok(()) => {
                self.w.dialog.close();
                (self.on_exported)(path);
            }
            Err(err) => {
                // The path is the user's chosen location; no capture content is logged.
                eprintln!("annota: export to {} failed: {err}", path.display());
                self.failure.set(Some(saving::problem(&err)));
                self.refresh();
            }
        }
    }

    /// The banner's button: Choose Folder… or Replace.
    fn banner_clicked(self: &Rc<Self>) {
        match self.problem() {
            Some(Problem::Exists) => {
                let path = self.form.borrow().path();
                self.replace.replace(path);
                self.export();
            }
            Some(Problem::NoPermission) => self.choose_folder(),
            _ => {}
        }
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

    /// "Custom Width…": the width in pixels, the height follows.
    fn custom_width(self: &Rc<Self>) {
        let w = &self.w;
        w.width_spin
            .set_value(f64::from(self.form.borrow().output_size().0));
        w.custom_width.present(Some(&w.dialog));
        w.width_spin.grab_focus();
    }
}

macro_rules! obj {
    ($builder:expr, $name:expr) => {
        match $builder.object($name) {
            Some(object) => object,
            None => {
                eprintln!("annota: export-dialog.ui is missing object '{}'", $name);
                return None;
            }
        }
    };
}

fn widgets(builder: &gtk::Builder) -> Option<Widgets> {
    Some(Widgets {
        dialog: obj!(builder, "export_dialog"),
        cancel_button: obj!(builder, "cancel_button"),
        export_button: obj!(builder, "export_button"),
        banner: obj!(builder, "error_banner"),
        summary_label: obj!(builder, "summary_label"),
        name_row: obj!(builder, "name_row"),
        extension_label: obj!(builder, "extension_label"),
        folder_row: obj!(builder, "folder_row"),
        folder_button: obj!(builder, "folder_button"),
        kinds: [
            (Kind::Png, obj!(builder, "png_button")),
            (Kind::Jpeg, obj!(builder, "jpg_button")),
            (Kind::WebP, obj!(builder, "webp_button")),
            (Kind::Pdf, obj!(builder, "pdf_button")),
        ],
        compression_row: obj!(builder, "compression_row"),
        lossy_button: obj!(builder, "lossy_button"),
        quality_row: obj!(builder, "quality_row"),
        quality_scale: obj!(builder, "quality_scale"),
        size_button: obj!(builder, "size_button"),
        flatten_row: obj!(builder, "flatten_row"),
        vectors_row: obj!(builder, "vectors_row"),
        page_row: obj!(builder, "page_row"),
        custom_width: obj!(builder, "custom_width_dialog"),
        width_spin: obj!(builder, "width_spin"),
    })
}

/// The size menu: the presets with their pixel sizes, then Custom Width… (mockup 03).
fn size_menu(canvas: (i32, i32)) -> gio::Menu {
    let menu = gio::Menu::new();
    let presets = gio::Menu::new();
    for percent in PRESETS {
        let item = gio::MenuItem::new(Some(&preset_label(canvas, percent)), None);
        item.set_action_and_target_value(
            Some("export.size"),
            Some(&percent.to_string().to_variant()),
        );
        presets.append_item(&item);
    }
    menu.append_section(None, &presets);
    let custom = gio::Menu::new();
    custom.append(Some(&gettext("Custom Width…")), Some("export.custom-width"));
    menu.append_section(None, &custom);
    menu
}

/// Shows "Export Image" for the document of `canvas` (named `name`) over `parent`, with the
/// `default-export-format` and `export-folder` preferences. `on_exported` gets the written
/// file once the dialog has closed.
pub fn present(
    parent: &impl IsA<gtk::Widget>,
    canvas: Canvas,
    name: &str,
    settings: &gio::Settings,
    on_exported: impl Fn(PathBuf) + 'static,
) {
    let builder = gtk::Builder::from_resource(UI_EXPORT);
    let Some(w) = widgets(&builder) else {
        return;
    };
    let Some(size) =
        canvas.with_document(|doc| (doc.canvas().rect.width, doc.canvas().rect.height))
    else {
        return;
    };
    let kind = Kind::from_setting(settings.enum_("default-export-format"));
    let folder = preferences::resolved_export_folder(&settings.string("export-folder"));
    let form = Form::new(size, kind, name, folder);

    // Preview row: the thumbnail (drawn once) and the document's name.
    let thumbnail = canvas
        .with_document(|doc| {
            let scale = (f64::from(THUMB.0 * 2) / f64::from(size.0.max(1)))
                .max(f64::from(THUMB.1 * 2) / f64::from(size.1.max(1)))
                .min(1.0);
            render::flatten(doc, scale).ok()
        })
        .flatten();
    let area = thumbnail_area(thumbnail, THUMB, 6.0);
    area.add_css_class("annota-export-thumb");
    if let Some(name_label) = builder.object::<gtk::Label>("name_label") {
        name_label.set_text(name);
    }
    if let Some(thumbnail_box) = builder.object::<gtk::Box>("thumbnail_box") {
        thumbnail_box.append(&area);
    }
    w.name_row.set_text(&form.name);
    for (k, button) in &w.kinds {
        button.set_active(*k == form.kind);
    }
    w.size_button.set_menu_model(Some(&size_menu(size)));

    let state = Rc::new(State {
        w,
        canvas,
        form: RefCell::new(form),
        replace: RefCell::new(None),
        failure: Cell::new(None),
        estimate: RefCell::new(None),
        estimate_pending: Cell::new(None),
        updating: Cell::new(false),
        on_exported: Box::new(on_exported),
    });
    connect(&state);
    state.refresh();
    let w = &state.w;
    w.dialog.set_default_widget(Some(&w.export_button));
    w.dialog.present(Some(parent));
    w.name_row.grab_focus();
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
            state.banner_clicked();
        }
    });
    let s = weak();
    w.name_row.connect_changed(move |row| {
        if let Some(state) = s.upgrade() {
            state.edit(|f| f.name = row.text().to_string());
        }
    });
    let s = weak();
    w.name_row.connect_entry_activated(move |_| {
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
    for (kind, button) in &w.kinds {
        let (kind, s) = (*kind, weak());
        button.connect_toggled(move |button| {
            if !button.is_active() {
                return;
            }
            if let Some(state) = s.upgrade() {
                state.edit(|f| f.kind = kind);
            }
        });
    }
    let s = weak();
    w.lossy_button.connect_toggled(move |button| {
        if let Some(state) = s.upgrade() {
            state.edit(|f| f.webp_lossy = button.is_active());
        }
    });
    let s = weak();
    w.quality_scale.connect_value_changed(move |scale| {
        if let Some(state) = s.upgrade() {
            let quality = scale.value().round().clamp(1.0, 100.0) as u8;
            state.edit(|f| match f.kind {
                Kind::WebP => f.webp_quality = quality,
                _ => f.jpeg_quality = quality,
            });
        }
    });
    let s = weak();
    w.vectors_row.connect_active_notify(move |row| {
        if let Some(state) = s.upgrade() {
            state.edit(|f| f.pdf.vectors = row.is_active());
        }
    });
    let s = weak();
    w.page_row.connect_selected_notify(move |row| {
        if let Some(state) = s.upgrade() {
            let page = match row.selected() {
                1 => PageSize::A4,
                2 => PageSize::Letter,
                _ => PageSize::FitToImage,
            };
            state.edit(|f| f.pdf.page = page);
        }
    });

    // The size menu's actions.
    let group = gio::SimpleActionGroup::new();
    let size =
        gio::SimpleAction::new_stateful("size", Some(glib::VariantTy::STRING), &"100".to_variant());
    let s = weak();
    size.connect_activate(move |action, target| {
        let percent = target
            .and_then(|t| t.get::<String>())
            .and_then(|t| t.parse::<u32>().ok());
        if let (Some(state), Some(percent)) = (s.upgrade(), percent) {
            action.set_state(&percent.to_string().to_variant());
            state.edit(|f| f.size = Size::Percent(percent));
        }
    });
    group.add_action(&size);
    let custom = gio::SimpleAction::new("custom-width", None);
    let s = weak();
    custom.connect_activate(move |_, _| {
        if let Some(state) = s.upgrade() {
            state.custom_width();
        }
    });
    group.add_action(&custom);
    w.dialog.insert_action_group("export", Some(&group));
    let s = weak();
    let size_action = size.downgrade();
    w.custom_width.connect_response(None, move |_, response| {
        let Some(state) = s.upgrade() else {
            return;
        };
        if response != "set" {
            return;
        }
        state.w.width_spin.update();
        let width = (state.w.width_spin.value().round() as i32).clamp(1, MAX_SIDE);
        if let Some(action) = size_action.upgrade() {
            action.set_state(&"custom".to_variant());
        }
        state.edit(|f| f.size = Size::Width(width));
    });

    // The dialog owns its state until it closes (the handlers above only hold it weakly);
    // closing also stops a pending estimate.
    let owned = RefCell::new(Some(state.clone()));
    w.dialog.connect_closed(move |_| {
        if let Some(state) = owned.take() {
            if let Some(pending) = state.estimate_pending.take() {
                pending.remove();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use annota::document::bitmap::Bitmap;
    use annota::document::color::palette;

    fn form(kind: Kind) -> Form {
        Form::new(
            (1600, 1000),
            kind,
            "Checkout flow",
            PathBuf::from("/home/mara/Pictures/Annota/Exports"),
        )
    }

    #[test]
    fn defaults_follow_the_redline() {
        let f = form(Kind::Png);
        assert_eq!(f.format(), Format::Png);
        assert_eq!(f.scale(), 1.0);
        assert_eq!(f.output_size(), (1600, 1000));
        assert_eq!(
            f.path(),
            Some(PathBuf::from(
                "/home/mara/Pictures/Annota/Exports/Checkout flow.png"
            ))
        );
        assert_eq!(f.jpeg_quality, 85);
        assert_eq!(f.webp_quality, 80);
        assert_eq!(f.pdf, PdfOptions::default());
        // The `default-export-format` preference picks the toggle.
        assert_eq!(Kind::from_setting(0), Kind::Png);
        assert_eq!(Kind::from_setting(1), Kind::Jpeg);
        assert_eq!(Kind::from_setting(2), Kind::WebP);
        assert_eq!(Kind::from_setting(3), Kind::Pdf);
        assert_eq!(Kind::from_setting(-1), Kind::Png);
    }

    #[test]
    fn the_controls_become_export_settings() {
        // Mockup 03: JPG, quality 85, 50 %.
        let mut f = form(Kind::Jpeg);
        f.size = Size::Percent(50);
        assert_eq!(f.format(), Format::Jpeg { quality: 85 });
        assert_eq!(f.output_size(), (800, 500));
        assert_eq!(size_label(&f), "50 % · 800 × 500 px");
        assert_eq!(f.path().unwrap().file_name().unwrap(), "Checkout flow.jpg");
        // Mockup 04: WebP lossy 80.
        let mut f = form(Kind::WebP);
        assert_eq!(
            f.format(),
            Format::WebP {
                compression: WebPCompression::Lossless
            }
        );
        f.webp_lossy = true;
        assert_eq!(
            f.format(),
            Format::WebP {
                compression: WebPCompression::Lossy { quality: 80 }
            }
        );
        // Mockup 05: PDF, vectors, A4.
        let mut f = form(Kind::Pdf);
        f.pdf.page = PageSize::A4;
        assert_eq!(
            f.format(),
            Format::Pdf(PdfOptions {
                vectors: true,
                page: PageSize::A4
            })
        );
        // Custom width: the height follows.
        let mut f = form(Kind::Png);
        f.size = Size::Width(1200);
        assert_eq!(f.output_size(), (1200, 750));
        assert_eq!(size_label(&f), "75 % · 1200 × 750 px");
    }

    #[test]
    fn size_menu_entries_show_pixels() {
        // Mockup 03's popover.
        let labels: Vec<String> = PRESETS
            .iter()
            .map(|p| preset_label((1600, 1000), *p))
            .collect();
        assert_eq!(
            labels,
            [
                "200 % · 3200 × 2000",
                "100 % · 1600 × 1000",
                "50 % · 800 × 500",
                "25 % · 400 × 250"
            ]
        );
    }

    #[test]
    fn the_extension_is_not_part_of_the_name() {
        let mut f = form(Kind::Png);
        f.name = "Shot.PNG".into();
        assert_eq!(f.stem().as_deref(), Some("Shot"));
        f.name = "Shot.jpg".into();
        assert_eq!(f.stem().as_deref(), Some("Shot.jpg"));
        f.name = ".png".into();
        assert_eq!(f.stem().as_deref(), Some(".png"));
        f.name = "  ".into();
        assert_eq!(f.path(), None);
        f.name = "a/b".into();
        assert_eq!(f.stem().as_deref(), Some("a-b"));
    }

    #[test]
    fn problems_are_found_before_writing() {
        // Mockup 04: the folder is not writable; then a file of that name exists.
        let f = form(Kind::WebP);
        let yes = |_: &Path| true;
        let no = |_: &Path| false;
        assert_eq!(f.problem(no, no, None), Some(Problem::NoPermission));
        assert_eq!(f.problem(yes, yes, None), Some(Problem::Exists));
        assert_eq!(f.problem(yes, no, None), None);
        // Replace accepts that file (and only that one).
        let path = f.path().unwrap();
        assert_eq!(f.problem(yes, yes, Some(&path)), None);
        assert_eq!(
            f.problem(yes, yes, Some(Path::new("/elsewhere.webp"))),
            Some(Problem::Exists)
        );
        // WebP has a size limit (16383 px), the other formats the renderer's.
        let mut big = form(Kind::WebP);
        big.size = Size::Percent(1100);
        assert_eq!(
            big.problem(yes, no, None),
            Some(Problem::TooLarge {
                format: "WebP",
                max: 16383
            })
        );
        big.kind = Kind::Png;
        assert_eq!(big.problem(yes, no, None), None);
        big.size = Size::Percent(2100);
        assert_eq!(
            big.limit_problem(),
            Some(Problem::TooLarge {
                format: "PNG",
                max: 32767
            })
        );
    }

    #[test]
    fn folder_check_looks_at_the_nearest_existing_folder() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("annota-export-dialog-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(folder_writable(&dir));
        assert!(folder_writable(&dir.join("Exports/2026")));
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();
        let read_only = folder_writable(&dir.join("Exports"));
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        // Root may write anyway; only a normal user sees the folder as read-only.
        let root = std::env::var("USER").is_ok_and(|u| u == "root");
        assert!(root || !read_only);
    }

    #[test]
    fn summary_line_and_sizes() {
        let f = form(Kind::Png);
        assert_eq!(summary(&f, None), "1600 × 1000 px · PNG");
        assert_eq!(
            summary(&f, Some(412_000)),
            "1600 × 1000 px · PNG · ≈ 412 KB"
        );
        assert_eq!(approx_size(1), "≈ 1 KB");
        assert_eq!(approx_size(96_001), "≈ 97 KB");
        assert_eq!(approx_size(1_400_000), "≈ 1.4 MB");
    }

    #[test]
    fn estimates_use_the_real_encoder() {
        let doc = Document::new(Bitmap::filled(400, 250, palette::PAPER).unwrap());
        // Small enough to be encoded whole: exact.
        let f = Form::new((400, 250), Kind::Png, "x", PathBuf::from("/tmp"));
        assert_eq!(sample_factor(f.output_size()), 1.0);
        assert_eq!(
            estimate(&doc, &f),
            Some(export::encode(&doc, &Format::Png, 1.0).unwrap().len() as u64)
        );
        // Larger outputs are sampled, then scaled by the pixel count.
        let mut big = f.clone();
        big.size = Size::Percent(400);
        assert!(sample_factor(big.output_size()) < 1.0);
        let sampled = estimate(&doc, &big).unwrap();
        assert!(sampled > 0);
        // Every format can be estimated.
        for kind in [Kind::Jpeg, Kind::WebP, Kind::Pdf] {
            let mut g = f.clone();
            g.kind = kind;
            assert!(estimate(&doc, &g).is_some_and(|b| b > 0), "{kind:?}");
        }
    }
}
