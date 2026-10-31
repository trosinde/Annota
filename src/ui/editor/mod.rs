//! The editor window (add-annotation-editor tasks 4.1–4.5; spec editor-canvas: Editor layout,
//! View control, Image operations, Canvas extension, Flatten, Context menu, Recent captures
//! tray; spec document-format: Format versions, Autosave and recovery). `--open`, captures,
//! pasted and opened images arrive here through [`open_file`] and [`open_capture`].
//!
//! **Architecture.** One editor window holds any number of open documents; the tray's "Open"
//! group switches between them (add-screen-capture's mockups 12/13 open each capture as a new
//! "tab" there). Each document is a [`session::Session`]: its own [`Canvas`] (document,
//! history, selection, view — phases 1 and 2a), its origin and its autosave slot. The canvases
//! sit in a `GtkStack`; the header bar, zoom OSD, sidebar and tray always show the current
//! one, refreshed through `Canvas::connect_changed` / `connect_view_changed`. The window
//! changes documents only through the canvas, which executes `Command`s on the history.
//!
//! **Keys.** A capture-phase key controller hands key presses to the current canvas (Select
//! tool, nudge, undo …) unless a text field or a dialog has the focus, or Space/Enter would
//! activate the focused button. Window actions with accelerators cover the header bar, zoom
//! OSD and menus; keys that text fields also use (Ctrl+C, Ctrl+Z) are deliberately not
//! application accelerators, because those run before the focused widget.
//!
//! **Export and save** (add-export-share). `Ctrl+E` opens "Export Image" ([`export_dialog`]),
//! `Ctrl+S` saves the document's `.annota` file — a new document into the save folder, named
//! after the filename pattern, without a dialog — and `Ctrl+Shift+S` asks where (GTK's file
//! chooser). A saved document's autosave is removed. Before any of them, a document with blur
//! or pixelate redactions gets "Make Redactions Solid?" ([`Editor::guard_redactions`], the
//! one check every way out of Annota goes through; [`redaction_warning`]).
//!
//! **Placeholders.** Copy and Share (Email…, Open With…, Copy to Folder…) belong to later
//! phases of add-export-share, Open Library to add-capture-library: Copy shows a "… arrives in
//! milestone 0.1" toast; the Share entries (D1) are present but insensitive.
//!
//! **Closing keeps autosaves.** Until "Save changes before closing?" exists (add-export-share
//! task 2.6), closing the window — or quitting — writes a last autosave of every document with
//! unsaved changes and keeps it, so nothing is lost; "Restore Unsaved Captures?" offers them at
//! the next start, and only its Discard (or saving the document) removes them. See design.md
//! ("Decided during apply — phase 2b-ii").

mod colour;
mod dialogs;
mod export_dialog;
mod glyphs;
mod panel;
pub(crate) mod properties;
mod quick_styles;
mod redaction_warning;
mod resize;
mod saving;
mod session;
mod sidebar;
mod styles;
mod text;
mod theme_editor;
mod tray;

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::time::Duration;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, gio, glib};

use annota::document::annotation::PropertyValue;
use annota::document::archive::ArchiveError;
use annota::document::autosave::{self, AutosaveId, RecoveryEntry, Schedule, Store};
use annota::document::bitmap::Bitmap;
use annota::document::color::Rgba;
use annota::document::geometry::Mirror;
use annota::document::{Background, Document};
use annota::editing::canvas_edit;
use annota::editing::image_ops::{self, Turn};
use annota::editing::ops::{self, Stacking, Subject};
use annota::editing::redactions;
use annota::editing::selection::Selection;
use annota::editing::tool::{Tool, Toolbox};
use annota::export::{self, ExportError};
use annota::render::{self, view};
use annota::styles::store::StylePaths;
use annota::styles::theme::Theme;
use annota::styles::{generate, value as style_value};
use annota::tools::group::Group;

use self::panel::PanelEvent;
use self::quick_styles::QuickEvent;
use self::redaction_warning::{Purpose, Response};
use self::session::{LoadError, Loaded, Origin, Session};
use self::sidebar::{CropInfo, Event, Sidebar};
use self::styles::Styles;
use self::tray::{OpenTile, Tray, TrayKey};
use crate::cli::CaptureMode;
use crate::config;
use crate::ui::canvas::{Canvas, Done, Notice};
use crate::ui::labels;

const UI_EDITOR: &str = "/io/github/trosinde/Annota/ui/editor-window.ui";

/// The Blueprint id of a drawing tool's button.
fn tool_button_id(tool: Tool) -> String {
    let name = match tool {
        Tool::Select => "select",
        Tool::Arrow => "arrow",
        Tool::Line => "line",
        Tool::Pen => "pen",
        Tool::Shape => "shape",
        Tool::Text => "text",
        Tool::Callout => "callout",
        Tool::Step => "step",
        Tool::Highlighter => "highlighter",
        Tool::Redact => "redact",
    };
    format!("{name}_tool_button")
}

/// The tool strip's icon of `tool` (kit icons, `data/icons/scalable/actions/`).
pub fn tool_icon(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => "annota-select-symbolic",
        Tool::Arrow => "annota-arrow-symbolic",
        Tool::Line => "annota-line-symbolic",
        Tool::Pen => "annota-pen-symbolic",
        Tool::Shape => "annota-shape-symbolic",
        Tool::Text => "annota-text-symbolic",
        Tool::Callout => "annota-callout-symbolic",
        Tool::Step => "annota-step-symbolic",
        Tool::Highlighter => "annota-highlighter-symbolic",
        Tool::Redact => "annota-redact-symbolic",
    }
}
/// Redline: below 900 px the sidebar becomes a bottom sheet and the tray and Copy go.
const NARROW: &str = "max-width: 899px";
/// After a change, the document's tray thumbnail is rendered again this much later.
const THUMBNAIL_DELAY: Duration = Duration::from_millis(400);

thread_local! {
    /// The application's editor window. Owned here (the widgets only hold it weakly) from
    /// creation until the window closes.
    static EDITOR: RefCell<Option<Rc<Inner>>> = const { RefCell::new(None) };
    /// Whether this process has offered the autosaves of a previous run already: once, at the
    /// primary instance's start or — when Annota was started by a capture hotkey
    /// (`annota --capture`) or the editor opens before any start-up offer — the first time the
    /// editor window opens.
    static RECOVERY_OFFERED: Cell<bool> = const { Cell::new(false) };
}

/// The autosaves of a previous run to offer now, the first time this process asks; `None`
/// afterwards and when there are none. Listed before this process adds documents of its own.
fn recovery_entries(store: &Store) -> Option<Vec<RecoveryEntry>> {
    if RECOVERY_OFFERED.with(|offered| offered.replace(true)) {
        return None;
    }
    let listing = store.list();
    for path in &listing.unreadable {
        eprintln!("annota: damaged autosave left in place: {}", path.display());
    }
    (!listing.entries.is_empty()).then_some(listing.entries)
}

/// The editor window, created on first use and presented, with the autosaves of a previous
/// run if they are still to be offered (the caller shows them once its own document is open,
/// so the dialog sits on top of it).
fn present_editor(app: &adw::Application) -> Option<(Editor, Option<Vec<RecoveryEntry>>)> {
    let editor = Editor::get_or_create(app)?;
    let pending = recovery_entries(&editor.inner.store);
    editor.inner.w.window.present();
    Some((editor, pending))
}

/// Opens `path` (a `.annota` file or an image) in the editor window and presents it
/// (`annota --open FILE`, `Ctrl+O`, a file dropped on Annota, a recent capture). A file that is
/// open already becomes the current document. With `announce`, an image gets the "New capture
/// from file" toast with Undo (add-screen-capture mockup 13). Returns whether the editor could
/// be shown.
pub fn open_file(app: &adw::Application, path: &Path, announce: bool) -> bool {
    let Some((editor, pending)) = present_editor(app) else {
        return false;
    };
    editor.open_file_announced(path, announce);
    if let Some(entries) = pending {
        editor.offer_recovery(entries);
    }
    true
}

/// What the toast of a new capture offers (add-screen-capture mockups 12, 13).
pub enum Offer {
    /// "Copy": put the capture on the clipboard (it went to the editor only).
    Copy(gdk::Texture),
    /// "Undo": close the new document again (pasted or opened images).
    Undo,
    Nothing,
}

/// Opens a new capture — a region, window or screen capture, or a pasted image — as a new
/// document ("Not saved") and announces it with `title` and `offer`. Returns `false` if the
/// editor window could not be built (the caller then keeps the capture another way).
///
/// When this is the first time the process opens the editor — Annota started by a capture
/// hotkey (`annota --capture`), which does not go through the start-up offer — "Restore Unsaved
/// Captures?" is shown over the new capture, so autosaves never pile up unseen.
pub fn open_capture(
    app: &adw::Application,
    bitmap: Bitmap,
    name: &str,
    title: &str,
    offer: Offer,
) -> bool {
    let Some((editor, pending)) = present_editor(app) else {
        return false;
    };
    let loaded = Loaded {
        document: Document::with_title(bitmap, name),
        source: None,
        path: None,
        name: name.to_string(),
    };
    let uid = editor.add_session(loaded, Origin::New, AutosaveId::generate());
    editor.announce(uid, title, offer);
    if let Some(entries) = pending {
        editor.offer_recovery(entries);
    }
    true
}

/// If a previous run left autosaves (and this process has not offered them yet), presents the
/// editor window with "Restore Unsaved Captures?" (mockup 14) and returns `true`. The primary
/// instance's start; the capture and open paths offer them too if the editor opens first
/// ([`open_capture`], [`open_file`]).
pub fn offer_recovery(app: &adw::Application) -> bool {
    let Some(entries) = recovery_entries(&default_store()) else {
        return false;
    };
    let Some(editor) = Editor::get_or_create(app) else {
        return false;
    };
    editor.inner.w.window.present();
    editor.offer_recovery(entries);
    true
}

/// Annota quits: every open document's unsaved changes are autosaved and kept (see the module
/// documentation), exactly as when the window is closed.
pub fn shut_down() {
    if let Some(inner) = EDITOR.with(|e| e.borrow().clone()) {
        Editor { inner }.close_all();
    }
}

fn default_store() -> Store {
    Store::new(autosave::default_dir(&glib::user_cache_dir()))
}

/// `~/.local/share/annota/styles` and `/usr/share/annota/themes` (add-style-system).
fn default_style_paths() -> StylePaths {
    StylePaths::new(&glib::user_data_dir())
}

/// The header bar, layout and OSD widgets of `editor-window.ui`.
struct Widgets {
    window: adw::ApplicationWindow,
    title: adw::WindowTitle,
    undo_button: gtk::Button,
    redo_button: gtk::Button,
    copy_button: gtk::Button,
    main_menu_button: gtk::MenuButton,
    split_view: adw::OverlaySplitView,
    sidebar_scroller: gtk::ScrolledWindow,
    sheet_revealer: gtk::Revealer,
    sheet_scroller: gtk::ScrolledWindow,
    toast_overlay: adw::ToastOverlay,
    canvas_area: gtk::Overlay,
    canvas_stack: gtk::Stack,
    zoom_osd: gtk::Box,
    zoom_level_label: gtk::Label,
    show_tray_button: gtk::Button,
    drop_ring: gtk::Box,
    drop_chip: gtk::Label,
    pick_chip: gtk::Label,
    select_tool_button: gtk::ToggleButton,
    crop_tool_button: gtk::ToggleButton,
    /// The drawing tools' buttons (add-core-annotation-tools), in strip order.
    tool_buttons: Vec<(Tool, gtk::ToggleButton)>,
    canvas_empty: gtk::DrawingArea,
    main_menu: gio::MenuModel,
    context_menu: gtk::PopoverMenu,
}

impl Widgets {
    fn new(builder: &gtk::Builder) -> Option<Self> {
        let get = |name: &str| -> Option<glib::Object> {
            let object = builder.object(name);
            if object.is_none() {
                eprintln!("annota: editor-window.ui is missing object '{name}'");
            }
            object
        };
        macro_rules! obj {
            ($name:expr) => {
                get($name)?.downcast().ok()?
            };
        }
        Some(Self {
            window: obj!("editor_window"),
            title: obj!("window_title"),
            undo_button: obj!("undo_button"),
            redo_button: obj!("redo_button"),
            copy_button: obj!("copy_button"),
            main_menu_button: obj!("main_menu_button"),
            split_view: obj!("split_view"),
            sidebar_scroller: obj!("sidebar_scroller"),
            sheet_revealer: obj!("sheet_revealer"),
            sheet_scroller: obj!("sheet_scroller"),
            toast_overlay: obj!("toast_overlay"),
            canvas_area: obj!("canvas_area"),
            canvas_stack: obj!("canvas_stack"),
            zoom_osd: obj!("zoom_osd"),
            zoom_level_label: obj!("zoom_level_label"),
            show_tray_button: obj!("show_tray_button"),
            drop_ring: obj!("drop_ring"),
            drop_chip: obj!("drop_chip"),
            pick_chip: obj!("pick_chip"),
            select_tool_button: obj!("select_tool_button"),
            crop_tool_button: obj!("crop_tool_button"),
            tool_buttons: {
                let mut buttons = Vec::new();
                for tool in Tool::ALL.into_iter().filter(|t| *t != Tool::Select) {
                    let button: gtk::ToggleButton = obj!(&tool_button_id(tool));
                    buttons.push((tool, button));
                }
                buttons
            },
            canvas_empty: obj!("canvas_empty"),
            main_menu: obj!("main_menu"),
            context_menu: {
                let model: gio::MenuModel = obj!("context_menu");
                gtk::PopoverMenu::builder()
                    .menu_model(&model)
                    .has_arrow(false)
                    .halign(gtk::Align::Start)
                    .build()
            },
        })
    }
}

/// Window actions whose state follows the current document.
struct Actions {
    undo: gio::SimpleAction,
    redo: gio::SimpleAction,
    /// Need an open document.
    document: Vec<gio::SimpleAction>,
    /// Image operations: need a document, no crop in progress and no objects of a newer Annota.
    image: Vec<gio::SimpleAction>,
    /// Need a selection: Duplicate, Delete, the stacking order.
    selected: Vec<gio::SimpleAction>,
    group: gio::SimpleAction,
    ungroup: gio::SimpleAction,
    /// Like `selected`, and like the image operations no objects of a newer Annota.
    flatten_selection: gio::SimpleAction,
    flatten_all: gio::SimpleAction,
    crop_apply: gio::SimpleAction,
    crop_cancel: gio::SimpleAction,
    zoom_preset: gio::SimpleAction,
}

struct Inner {
    w: Widgets,
    sidebar: Rc<Sidebar>,
    tray: Tray,
    actions: Actions,
    sessions: RefCell<Vec<Rc<Session>>>,
    current: Cell<Option<u64>>,
    next_uid: Cell<u64>,
    store: Store,
    settings: gio::Settings,
    narrow: Cell<bool>,
    /// Set while the tool buttons follow the canvas, so that is not a click.
    syncing_tool: Cell<bool>,
    /// The tool of the tool strip; every document draws with it.
    tool: Cell<Tool>,
    /// The tools' defaults, shared by the window's documents.
    toolbox: Rc<RefCell<Toolbox>>,
    /// The screen eyedropper (xdg-desktop-portal `PickColor`; a stand-in in tests).
    screen_picker: RefCell<ScreenPicker>,
    /// The property whose colour the screen eyedropper is picking.
    picking: Cell<Option<&'static str>>,
    /// Quick styles, favourites and themes (add-style-system).
    styles: Rc<Styles>,
    /// The theme the tool defaults and the palette swatches were last set from.
    applied_theme: RefCell<Option<Theme>>,
}

/// Picks a colour anywhere on the screen.
type ScreenPicker = Rc<
    dyn Fn() -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Rgba, crate::capture::portal::CaptureError>>>,
    >,
>;

/// The real eyedropper: the Screenshot portal's `PickColor` (`capture::portal::pick_colour`,
/// the thin boundary that is not exercised against a live portal in development).
fn portal_picker() -> ScreenPicker {
    Rc::new(|| {
        Box::pin(async {
            crate::capture::portal::pick_colour()
                .await
                .map(|(r, g, b)| colour::from_portal(r, g, b))
        })
    })
}

/// The editor window; cheap to clone.
#[derive(Clone)]
pub struct Editor {
    inner: Rc<Inner>,
}

impl Editor {
    /// The application's editor window, created on first use.
    fn get_or_create(app: &adw::Application) -> Option<Self> {
        if let Some(inner) = EDITOR.with(|e| e.borrow().clone()) {
            return Some(Self { inner });
        }
        let editor = Self::build(app, default_store(), default_style_paths())?;
        EDITOR.with(|e| e.replace(Some(editor.inner.clone())));
        Some(editor)
    }

    fn build(app: &adw::Application, store: Store, style_paths: StylePaths) -> Option<Self> {
        let builder = gtk::Builder::from_resource(UI_EDITOR);
        let w = Widgets::new(&builder)?;
        let styles = Styles::load(style_paths);
        let sidebar = Sidebar::new(&builder, styles.clone())?;
        let tray = Tray::new(&builder)?;
        w.window.set_application(Some(app));
        let settings = gio::Settings::new(config::APP_ID);
        let actions = install_actions(&w.window, &settings);
        set_accels(app);
        let inner = Rc::new(Inner {
            w,
            sidebar,
            tray,
            actions,
            sessions: RefCell::new(Vec::new()),
            current: Cell::new(None),
            next_uid: Cell::new(1),
            store,
            settings,
            narrow: Cell::new(false),
            syncing_tool: Cell::new(false),
            tool: Cell::new(Tool::Select),
            toolbox: Rc::new(RefCell::new(Toolbox::default())),
            screen_picker: RefCell::new(portal_picker()),
            picking: Cell::new(None),
            styles,
            applied_theme: RefCell::new(None),
        });
        let editor = Self { inner };
        editor.setup();
        Some(editor)
    }

    fn downgrade(&self) -> WeakEditor {
        WeakEditor(Rc::downgrade(&self.inner))
    }

    fn setup(&self) {
        let w = &self.inner.w;
        let settings = &self.inner.settings;
        // Window size and state, kept across runs (keys exist since bootstrap-app-shell).
        settings
            .bind("window-width", &w.window, "default-width")
            .build();
        settings
            .bind("window-height", &w.window, "default-height")
            .build();
        settings
            .bind("window-maximized", &w.window, "maximized")
            .build();
        self.inner.sidebar.properties_box.unparent_from_any_parent();
        w.sidebar_scroller
            .set_child(Some(&self.inner.sidebar.properties_box));
        settings
            .bind("snap-to-guides", &self.inner.sidebar.snap_row, "active")
            .build();
        w.canvas_empty.set_draw_func(|_, cr, _, _| {
            crate::ui::canvas::background().set_source(cr);
            let _ = cr.paint();
        });
        let empty = w.canvas_empty.downgrade();
        adw::StyleManager::default().connect_dark_notify(move |_| {
            if let Some(empty) = empty.upgrade() {
                empty.queue_draw();
            }
        });

        w.context_menu.set_parent(&w.canvas_area);
        self.setup_actions();
        self.setup_tools();
        self.setup_layout();
        self.setup_keys();
        self.setup_tooltips();
        self.setup_drop();
        self.setup_tray();
        self.setup_sidebar();
        self.setup_autosave();

        let weak = self.downgrade();
        w.window.connect_close_request(move |_| {
            if let Some(editor) = weak.upgrade() {
                // Keeps every unsaved document's autosave (module documentation).
                editor.close_all();
                editor.inner.w.context_menu.unparent();
                // Released when this handler returns; the window is destroyed after it.
                EDITOR.with(|e| {
                    let mut owned = e.borrow_mut();
                    if owned
                        .as_ref()
                        .is_some_and(|inner| Rc::ptr_eq(inner, &editor.inner))
                    {
                        *owned = None;
                    }
                });
            }
            glib::Propagation::Proceed
        });
        self.refresh();
    }

    // ---- documents ----

    fn sessions(&self) -> Vec<Rc<Session>> {
        self.inner.sessions.borrow().clone()
    }

    fn session(&self, uid: u64) -> Option<Rc<Session>> {
        self.inner
            .sessions
            .borrow()
            .iter()
            .find(|s| s.uid == uid)
            .cloned()
    }

    fn current(&self) -> Option<Rc<Session>> {
        self.session(self.inner.current.get()?)
    }

    fn current_canvas(&self) -> Option<Canvas> {
        self.current().map(|s| s.canvas.clone())
    }

    /// Opens a file as a new document (or switches to it if it is open already); a file that
    /// needs a newer Annota gets the alert of mockup 15, other failures a toast.
    fn open_file(&self, path: &Path) {
        self.open_file_announced(path, false);
    }

    fn open_file_announced(&self, path: &Path, announce: bool) {
        let canonical = session::canonical(path);
        if let Some(open) = self
            .sessions()
            .into_iter()
            .find(|s| s.path.borrow().as_deref() == Some(canonical.as_path()))
        {
            self.switch_to(open.uid);
            return;
        }
        match session::load(path) {
            Ok(loaded) => {
                // An image is a new capture (it is not an Annota document until saved); an
                // `.annota` file is its own saved state.
                let origin = if loaded.source.is_some() {
                    Origin::File
                } else {
                    Origin::Image
                };
                let uid = self.add_session(loaded, origin, AutosaveId::generate());
                if announce && origin == Origin::Image {
                    let title = crate::capture::delivery::title(
                        crate::capture::delivery::Source::File,
                        0,
                        0,
                    );
                    self.announce(uid, &title, Offer::Undo);
                }
            }
            Err(LoadError::Archive(ArchiveError::NewerFormat { found, requires })) => {
                dialogs::present_newer_version(
                    &self.inner.w.window,
                    path,
                    found,
                    requires.as_deref(),
                );
            }
            Err(err) => {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.toast(
                    &gettext("Could not open “{name}”: {error}")
                        .replace("{name}", &name)
                        .replace("{error}", &err.to_string()),
                );
            }
        }
    }

    /// The toast of a new document (add-screen-capture mockups 12, 13).
    fn announce(&self, uid: u64, title: &str, offer: Offer) {
        let toast = adw::Toast::builder().title(title).build();
        toast.set_use_markup(false);
        match offer {
            Offer::Copy(texture) => {
                toast.set_button_label(Some(&gettext("Copy")));
                let window = self.inner.w.window.downgrade();
                toast.connect_button_clicked(move |_| {
                    let app = window
                        .upgrade()
                        .and_then(|w| w.application())
                        .and_then(|a| a.downcast::<adw::Application>().ok());
                    if let Some(app) = app {
                        crate::capture::delivery::copy_to_clipboard(&app, &texture);
                    }
                });
            }
            Offer::Undo => {
                toast.set_button_label(Some(&gettext("Undo")));
                let weak = self.downgrade();
                toast.connect_button_clicked(move |_| {
                    if let Some(editor) = weak.upgrade() {
                        editor.close_session(uid, true);
                    }
                });
            }
            Offer::Nothing => {}
        }
        self.inner.w.toast_overlay.add_toast(toast);
    }

    /// Closes one document. `discard`: it is dropped on purpose (the Undo of a new capture),
    /// so its autosave goes too; otherwise it is kept like when the window closes.
    fn close_session(&self, uid: u64, discard: bool) {
        let Some(session) = self.session(uid) else {
            return;
        };
        if discard {
            session.discard(&self.inner.store);
        } else {
            session.close(&self.inner.store);
        }
        self.inner.sessions.borrow_mut().retain(|s| s.uid != uid);
        self.inner.w.canvas_stack.remove(session.canvas.widget());
        if self.inner.current.get() == Some(uid) {
            let next = self.sessions().last().map(|s| s.uid);
            self.inner.current.set(None);
            match next {
                Some(next) => self.switch_to(next),
                None => {
                    self.inner.w.canvas_stack.set_visible_child_name("empty");
                    self.refresh();
                }
            }
        } else {
            self.refresh();
        }
        self.refresh_recent_tray();
    }

    fn add_session(&self, loaded: Loaded, origin: Origin, autosave: AutosaveId) -> u64 {
        let uid = self.inner.next_uid.get();
        self.inner.next_uid.set(uid + 1);
        let canvas = Canvas::new(loaded.document);
        canvas.set_snapping(self.inner.settings.boolean("snap-to-guides"));
        canvas.set_toolbox(self.inner.toolbox.clone());
        let weak = self.downgrade();
        canvas.connect_notice(move |notice| {
            if let Some(editor) = weak.upgrade() {
                editor.notice(notice);
            }
        });
        // A file's own state is saved; an opened image's pixels are on disk too, so it is
        // only autosaved once it changes (session::Origin).
        if matches!(origin, Origin::File | Origin::Image) {
            canvas.mark_saved();
        }
        let weak = self.downgrade();
        canvas.connect_context_menu(move |x, y| {
            if let Some(editor) = weak.upgrade() {
                editor.show_context_menu(x, y);
            }
        });
        let weak = self.downgrade();
        canvas.connect_changed(move || {
            if let Some(editor) = weak.upgrade() {
                editor.document_changed(uid);
            }
        });
        // The view changes during the canvas's size allocation (Fit follows the window); the
        // labels are updated right after it, not inside it.
        let weak = self.downgrade();
        let pending = Rc::new(Cell::new(false));
        canvas.connect_view_changed(move |_, _| {
            if pending.replace(true) {
                return;
            }
            let weak = WeakEditor(weak.0.clone());
            let pending = pending.clone();
            glib::idle_add_local_once(move || {
                pending.set(false);
                if let Some(editor) = weak.upgrade() {
                    if editor.inner.current.get() == Some(uid) {
                        editor.refresh_header();
                    }
                }
            });
        });
        let name = Some(loaded.name)
            .filter(|n| !n.is_empty())
            .unwrap_or_else(text::untitled);
        let session = Rc::new(Session {
            uid,
            canvas,
            origin: Cell::new(origin),
            source: RefCell::new(loaded.source),
            path: RefCell::new(loaded.path),
            name: RefCell::new(name),
            saved: Cell::new(false),
            autosave,
            schedule: Cell::new(Schedule::new()),
            thumbnail: RefCell::new(None),
            thumbnail_pending: Cell::new(None),
        });
        self.inner
            .w
            .canvas_stack
            .add_named(session.canvas.widget(), Some(&session.page_name()));
        self.inner.sessions.borrow_mut().push(session.clone());
        self.render_thumbnail(&session);
        self.switch_to(uid);
        // Exclude the file this session was opened from (if any) from Recent immediately,
        // rather than waiting for the window to next be activated — otherwise reopening it
        // from its own Recent tile is still offered until then.
        self.refresh_recent_tray();
        uid
    }

    fn switch_to(&self, uid: u64) {
        let Some(session) = self.session(uid) else {
            return;
        };
        if let Some(previous) = self.current().filter(|p| p.uid != uid) {
            previous.canvas.finish_text();
            previous.canvas.end_crop();
        }
        session.canvas.set_tool(self.inner.tool.get());
        self.inner.current.set(Some(uid));
        self.inner
            .w
            .canvas_stack
            .set_visible_child_name(&session.page_name());
        session.canvas.grab_focus();
        self.refresh();
    }

    /// The window closes (or Annota quits): every document with unsaved changes is autosaved
    /// one last time and its autosave kept for the next start (spec document-format: Autosave
    /// and recovery — until "Save changes before closing?", add-export-share task 2.6).
    fn close_all(&self) {
        // Text being typed belongs to the document: it is kept.
        for session in self.inner.sessions.borrow().iter() {
            session.canvas.finish_text();
        }
        let sessions = self.inner.sessions.take();
        for session in &sessions {
            session.close(&self.inner.store);
        }
        self.inner.current.set(None);
    }

    fn document_changed(&self, uid: u64) {
        if self.inner.current.get() == Some(uid) {
            self.sync_tool();
            self.refresh_header();
            self.refresh_sidebar();
            // Ctrl+; on the canvas: the sidebar switch and the setting follow.
            if let Some(canvas) = self.current_canvas() {
                let on = canvas.snapping();
                if self.inner.settings.boolean("snap-to-guides") != on {
                    let _ = self.inner.settings.set_boolean("snap-to-guides", on);
                }
            }
        }
        self.schedule_thumbnail(uid);
        self.refresh_open_tray();
    }

    // ---- refresh ----

    fn refresh(&self) {
        self.sync_tool();
        self.refresh_header();
        self.refresh_sidebar();
        self.refresh_open_tray();
        self.apply_layout();
    }

    fn refresh_header(&self) {
        let w = &self.inner.w;
        let a = &self.inner.actions;
        let current = self.current();
        let has_doc = current.is_some();
        for action in &a.document {
            action.set_enabled(has_doc);
        }
        w.zoom_osd.set_visible(has_doc);
        w.select_tool_button.set_sensitive(has_doc);
        for (_, button) in &w.tool_buttons {
            button.set_sensitive(has_doc);
        }
        self.refresh_action_states();
        let Some(session) = current else {
            w.title.set_title(&gettext("Annota"));
            w.title.set_subtitle("");
            w.window.set_title(Some(&gettext("Annota")));
            a.undo.set_enabled(false);
            a.redo.set_enabled(false);
            return;
        };
        let canvas = &session.canvas;
        let info = canvas.history_info();
        a.undo.set_enabled(info.undo_label.is_some());
        a.redo.set_enabled(info.redo_label.is_some());
        let size = canvas
            .with_document(|d| (d.canvas().rect.width, d.canvas().rect.height))
            .unwrap_or_default();
        let (zoom, fitted) = (canvas.zoom(), canvas.fitted());
        let name = session.name();
        w.title.set_title(&name);
        w.window.set_title(Some(&name));
        let crop = canvas.crop_frame().map(|f| (f.width, f.height));
        // Mockup 17: the eyedropper's instruction; mockup 07: "1600 × 1000 · Drawing" while a
        // polygon is drawn.
        let subtitle = if self.inner.picking.get().is_some() {
            gettext("Pick a colour anywhere on the screen")
        } else if canvas.drawing_polygon() {
            text::drawing_subtitle(size)
        } else if let Some(folder) = session.saved_folder().filter(|_| fitted && crop.is_none()) {
            // Mockup 06: right after a save, where it went.
            saving::saved_subtitle(&crate::preferences::display_path(&folder))
        } else {
            text::subtitle(
                size,
                session::status(session.origin.get(), &info),
                (!fitted).then_some(zoom),
                crop,
            )
        };
        w.title.set_subtitle(&subtitle);
        w.zoom_level_label.set_text(&text::zoom_label(zoom, fitted));
        let preset = view::preset(zoom)
            .filter(|_| !fitted)
            .map(|p| view::percent(p).to_string())
            .unwrap_or_default();
        a.zoom_preset.set_state(&preset.to_variant());
    }

    /// Enables the actions that depend on the document, the selection and crop mode.
    fn refresh_action_states(&self) {
        let a = &self.inner.actions;
        struct Facts {
            cropping: bool,
            can_crop: bool,
            selected: usize,
            group_selected: bool,
            flattenable: bool,
            /// Objects of a newer Annota: no crop, rotate, flip, resize image or flatten.
            newer_objects: bool,
        }
        let facts = self.current_canvas().and_then(|canvas| {
            let selection = canvas.selection();
            let cropping = canvas.cropping();
            let can_crop = canvas.can_apply_crop();
            canvas.with_document(|doc| Facts {
                cropping,
                can_crop,
                selected: selection.len(),
                group_selected: selection.iter().any(|id| {
                    doc.scene()
                        .get(*id)
                        .is_some_and(|n| n.object.type_name() == Group::TYPE)
                }),
                flattenable: !image_ops::flattenable(doc).is_empty(),
                newer_objects: image_ops::has_unknown_objects(doc),
            })
        });
        let editing = facts.as_ref().is_some_and(|f| !f.cropping);
        let newer_objects = facts.as_ref().is_some_and(|f| f.newer_objects);
        let image_ops = editing && !newer_objects;
        for action in &a.image {
            action.set_enabled(image_ops);
        }
        a.flatten_selection
            .set_enabled(image_ops && facts.as_ref().is_some_and(|f| f.selected > 0));
        // Crop is a tool button rather than an action; it says why it is unavailable.
        let crop = &self.inner.w.crop_tool_button;
        crop.set_sensitive(facts.is_some() && !newer_objects);
        let crop_tooltip = gettext("Crop (C)");
        crop.set_tooltip_text(Some(&if newer_objects {
            text::newer_objects_tooltip(Some(&crop_tooltip))
        } else {
            crop_tooltip
        }));
        for action in &a.selected {
            action.set_enabled(editing && facts.as_ref().is_some_and(|f| f.selected > 0));
        }
        a.group
            .set_enabled(editing && facts.as_ref().is_some_and(|f| f.selected > 1));
        a.ungroup
            .set_enabled(editing && facts.as_ref().is_some_and(|f| f.group_selected));
        a.flatten_all
            .set_enabled(image_ops && facts.as_ref().is_some_and(|f| f.flattenable));
        a.crop_cancel
            .set_enabled(facts.as_ref().is_some_and(|f| f.cropping));
        a.crop_apply
            .set_enabled(facts.as_ref().is_some_and(|f| f.cropping && f.can_crop));
    }

    fn refresh_sidebar(&self) {
        let state = match self.current() {
            Some(session) => match session.canvas.crop_frame() {
                Some(frame) => sidebar::State::Crop(CropInfo {
                    frame,
                    aspect: session.canvas.crop_aspect().unwrap_or_default(),
                }),
                None => {
                    let selection = session.canvas.selection();
                    let state = session
                        .canvas
                        .with_document(|doc| sidebar::state(doc, &selection))
                        .unwrap_or(sidebar::State::Empty);
                    // The type properties (add-core-annotation-tools): of the selection, or —
                    // nothing selected — of the text being typed, the polygon being drawn or
                    // the drawing tool's defaults.
                    match (state, session.canvas.panel_info()) {
                        (sidebar::State::Selection(mut info), panel) => {
                            info.panel = panel;
                            sidebar::State::Selection(info)
                        }
                        (sidebar::State::Document(_), Some(panel)) => sidebar::State::Tool(panel),
                        (state, _) => state,
                    }
                }
            },
            None => sidebar::State::Empty,
        };
        self.inner.sidebar.show(&state);
    }

    fn refresh_open_tray(&self) {
        let current = self.inner.current.get();
        let tiles: Vec<OpenTile> = self
            .sessions()
            .iter()
            .map(|s| OpenTile {
                uid: s.uid,
                name: s.name(),
                thumbnail: s.thumbnail.borrow().clone(),
                current: Some(s.uid) == current,
                unsaved: s.status().unsaved(),
            })
            .collect();
        self.inner.tray.set_open(&tiles);
    }

    fn refresh_recent_tray(&self) {
        let dir =
            crate::preferences::resolved_save_folder(&self.inner.settings.string("save-folder"));
        // Every open file, images included (bug fix: only `.annota` files used to count).
        let open: Vec<PathBuf> = self
            .sessions()
            .iter()
            .filter_map(|s| {
                s.path
                    .borrow()
                    .clone()
                    .or_else(|| s.source.borrow().as_deref().map(session::canonical))
            })
            .collect();
        self.inner.tray.refresh_recent(&dir, &open);
    }

    fn schedule_thumbnail(&self, uid: u64) {
        let Some(session) = self.session(uid) else {
            return;
        };
        if let Some(pending) = session.thumbnail_pending.take() {
            pending.remove();
        }
        let weak = self.downgrade();
        let id = glib::timeout_add_local_once(THUMBNAIL_DELAY, move || {
            let Some(editor) = weak.upgrade() else {
                return;
            };
            if let Some(session) = editor.session(uid) {
                session.thumbnail_pending.set(None);
                editor.render_thumbnail(&session);
                editor.refresh_open_tray();
            }
        });
        session.thumbnail_pending.set(Some(id));
    }

    fn render_thumbnail(&self, session: &Session) {
        let (w, h) = (tray::THUMB_WIDTH * 2, tray::THUMB_HEIGHT * 2);
        let texture = session
            .canvas
            .with_document(|doc| {
                let c = doc.canvas().rect;
                let scale = (f64::from(w) / f64::from(c.width.max(1)))
                    .max(f64::from(h) / f64::from(c.height.max(1)))
                    .min(1.0);
                render::flatten(doc, scale).ok()
            })
            .flatten()
            .and_then(|bitmap| tray::bitmap_texture(&bitmap));
        session.thumbnail.replace(texture);
    }

    // ---- layout ----

    fn setup_layout(&self) {
        let w = &self.inner.w;
        let narrow_menu = narrow_menu(&w.main_menu);
        match adw::BreakpointCondition::parse(NARROW) {
            Ok(condition) => {
                let breakpoint = adw::Breakpoint::new(condition);
                breakpoint.add_setter(&w.copy_button, "visible", Some(&false.to_value()));
                breakpoint.add_setter(
                    &self.inner.tray.revealer,
                    "visible",
                    Some(&false.to_value()),
                );
                breakpoint.add_setter(
                    &w.main_menu_button,
                    "menu-model",
                    Some(&narrow_menu.to_value()),
                );
                let weak = self.downgrade();
                breakpoint.connect_apply(move |_| {
                    if let Some(editor) = weak.upgrade() {
                        editor.set_narrow(true);
                    }
                });
                let weak = self.downgrade();
                breakpoint.connect_unapply(move |_| {
                    if let Some(editor) = weak.upgrade() {
                        editor.set_narrow(false);
                    }
                });
                w.window.add_breakpoint(breakpoint);
            }
            Err(err) => eprintln!("annota: editor breakpoint: {err}"),
        }
        for key in ["show-properties", "show-tray"] {
            let weak = self.downgrade();
            self.inner.settings.connect_changed(Some(key), move |_, _| {
                if let Some(editor) = weak.upgrade() {
                    editor.apply_layout();
                }
            });
        }
    }

    /// Below 900 px the properties move into the bottom sheet (mockup 13), and back.
    fn set_narrow(&self, narrow: bool) {
        if self.inner.narrow.replace(narrow) == narrow {
            return;
        }
        let w = &self.inner.w;
        let properties = &self.inner.sidebar.properties_box;
        properties.unparent_from_any_parent();
        if narrow {
            w.sidebar_scroller.set_child(None::<&gtk::Widget>);
            w.sheet_scroller.set_child(Some(properties));
        } else {
            w.sheet_scroller.set_child(None::<&gtk::Widget>);
            w.sidebar_scroller.set_child(Some(properties));
        }
        self.apply_layout();
    }

    fn apply_layout(&self) {
        let w = &self.inner.w;
        let show = self.inner.settings.boolean("show-properties");
        let narrow = self.inner.narrow.get();
        let content = !self.inner.sidebar.is_empty();
        w.split_view.set_show_sidebar(show && !narrow);
        w.sheet_revealer.set_reveal_child(show && narrow && content);
        let tray = self.inner.settings.boolean("show-tray");
        self.inner.tray.revealer.set_reveal_child(tray);
        w.show_tray_button.set_visible(!tray && !narrow);
    }

    // ---- actions ----

    fn setup_actions(&self) {
        let window = &self.inner.w.window;
        let entry = |name: &str, f: fn(&Editor)| {
            let weak = self.downgrade();
            let action = window
                .lookup_action(name)
                .and_then(|a| a.downcast::<gio::SimpleAction>().ok())
                .unwrap_or_else(|| {
                    let action = gio::SimpleAction::new(name, None);
                    window.add_action(&action);
                    action
                });
            action.connect_activate(move |_, _| {
                if let Some(editor) = weak.upgrade() {
                    f(&editor);
                }
            });
        };
        entry("undo", |e| {
            if let Some(c) = e.current_canvas() {
                c.undo();
            }
        });
        entry("redo", |e| {
            if let Some(c) = e.current_canvas() {
                c.redo();
            }
        });
        entry("zoom-fit", |e| {
            if let Some(c) = e.current_canvas() {
                c.zoom_fit();
            }
        });
        entry("zoom-fit-width", |e| {
            if let Some(c) = e.current_canvas() {
                c.zoom_fit_width();
            }
        });
        entry("zoom-in", |e| {
            if let Some(c) = e.current_canvas() {
                c.zoom_in();
            }
        });
        entry("zoom-out", |e| {
            if let Some(c) = e.current_canvas() {
                c.zoom_out();
            }
        });
        entry("ungroup", |e| {
            if let Some(c) = e.current_canvas() {
                c.ungroup();
            }
        });
        entry("new-capture", |e| {
            // The default mode of the capture launcher (add-screen-capture); where the capture
            // goes is still the `after-capture` preference (tasks 4.5).
            let mode = CaptureMode::from_index(e.inner.settings.enum_("capture-mode"));
            if let Some(app) = e.inner.w.window.application() {
                app.activate_action("capture", Some(&mode.as_str().to_variant()));
            }
        });
        // Task 4.3: image operations (main menu, sidebar; mockups 09, 17).
        entry("rotate-left", |e| {
            e.run(|doc, _| image_ops::rotate(doc, Turn::Left));
        });
        entry("rotate-right", |e| {
            e.run(|doc, _| image_ops::rotate(doc, Turn::Right));
        });
        entry("flip-horizontal", |e| {
            e.run(|doc, _| image_ops::flip(doc, Mirror::Horizontal));
        });
        entry("flip-vertical", |e| {
            e.run(|doc, _| image_ops::flip(doc, Mirror::Vertical));
        });
        entry("resize-image", |e| e.present_resize());
        entry("flatten-all", |e| {
            if let Some(done) = e.run(|doc, _| image_ops::flatten_all(doc)) {
                e.toast_flattened(&done);
            }
        });
        entry("flatten-selection", |e| {
            if let Some(done) = e.run(image_ops::flatten) {
                e.toast_flattened(&done);
            }
        });
        entry("crop-apply", |e| {
            if let Some(c) = e.current_canvas() {
                c.apply_crop();
            }
        });
        entry("crop-cancel", |e| {
            if let Some(c) = e.current_canvas() {
                c.end_crop();
            }
        });
        // The context menu (mockup 05); the keys themselves are the canvas's.
        entry("duplicate", |e| {
            e.run(ops::duplicate);
        });
        entry("delete", |e| {
            e.run(ops::delete);
        });
        entry("group", |e| {
            e.run(ops::group);
        });
        for (name, how) in [
            ("bring-to-front", Stacking::ToFront),
            ("bring-forward", Stacking::Forward),
            ("send-backward", Stacking::Backward),
            ("send-to-back", Stacking::ToBack),
        ] {
            let weak = self.downgrade();
            if let Some(action) = window
                .lookup_action(name)
                .and_then(|a| a.downcast::<gio::SimpleAction>().ok())
            {
                action.connect_activate(move |_, _| {
                    if let Some(editor) = weak.upgrade() {
                        editor.run(|doc, sel| ops::restack(doc, sel, how));
                    }
                });
            }
        }
        // Placeholder until add-export-share task 2.3.
        entry("copy", |e| {
            e.toast(&gettext("Copying the image arrives in milestone 0.1"))
        });
        // add-export-share: the export dialog (mockups 02–05), quick save and Save As (task
        // 2.2, mockup 06), each behind the redaction warning (task 2.7, mockup 10).
        entry("export", |e| e.export());
        entry("save", |e| e.quick_save());
        entry("save-as", |e| e.save_as());

        let weak = self.downgrade();
        let zoom_preset = &self.inner.actions.zoom_preset;
        zoom_preset.connect_activate(move |_, target| {
            let percent = target
                .and_then(|t| t.get::<String>())
                .and_then(|t| t.parse::<f64>().ok());
            if let (Some(editor), Some(percent)) = (weak.upgrade(), percent) {
                if let Some(c) = editor.current_canvas() {
                    c.zoom_to(percent / 100.0, None);
                }
            }
        });
        let weak = self.downgrade();
        let select_type = gio::SimpleAction::new("select-type", Some(glib::VariantTy::STRING));
        select_type.connect_activate(move |_, target| {
            let type_name = target.and_then(|t| t.get::<String>());
            if let (Some(editor), Some(type_name)) = (weak.upgrade(), type_name) {
                if let Some(c) = editor.current_canvas() {
                    c.select_type(&type_name);
                }
            }
        });
        window.add_action(&select_type);
    }

    /// Runs an operation on the current document as one undo step.
    fn run(&self, op: impl FnOnce(&Document, &Selection) -> Option<ops::Edit>) -> Option<Done> {
        self.current_canvas()?.run(op)
    }

    /// "7 objects flattened into the image" with Undo (mockup 18; no confirmation, D3).
    fn toast_flattened(&self, done: &Done) {
        let n = match done.subject {
            Subject::Many(n) => n,
            Subject::One(_) => 1,
        };
        let toast = adw::Toast::builder()
            .title(labels::flattened(n))
            .button_label(gettext("Undo"))
            .build();
        toast.set_use_markup(false);
        if let Some(canvas) = self.current_canvas() {
            let done = *done;
            toast.connect_button_clicked(move |_| canvas.undo_if_last(&done));
        }
        self.inner.w.toast_overlay.add_toast(toast);
    }

    /// "Resize Image" (mockup 09).
    fn present_resize(&self) {
        let Some(canvas) = self.current_canvas() else {
            return;
        };
        let Some((w, h)) = canvas.with_document(|d| {
            let r = image_ops::image_rect(d);
            (r.width, r.height)
        }) else {
            return;
        };
        resize::present(&self.inner.w.window, w, h, move |w, h, scale| {
            canvas.run(|doc, _| image_ops::resize(doc, w, h, scale));
        });
    }

    /// The context menu at (x, y) of the current canvas's widget (mockup 05: at the pointer,
    /// no arrow).
    fn show_context_menu(&self, x: f64, y: f64) {
        let w = &self.inner.w;
        let Some(canvas) = self.current_canvas() else {
            return;
        };
        let at = canvas
            .widget()
            .compute_point(
                &w.canvas_area,
                &gtk::graphene::Point::new(x as f32, y as f32),
            )
            .map_or((x, y), |p| (f64::from(p.x()), f64::from(p.y())));
        self.refresh_action_states();
        w.context_menu
            .set_pointing_to(Some(&gdk::Rectangle::new(at.0 as i32, at.1 as i32, 1, 1)));
        w.context_menu.popup();
    }

    /// The tool strip: Select (V), the drawing tools (A, L, P, U, T, K, S, H, B) and Crop (C,
    /// mockup 08).
    fn setup_tools(&self) {
        let w = &self.inner.w;
        let all = std::iter::once((Tool::Select, w.select_tool_button.clone()))
            .chain(w.tool_buttons.iter().cloned());
        for (tool, button) in all {
            let weak = self.downgrade();
            button.connect_toggled(move |button| {
                if let Some(editor) = weak.upgrade() {
                    if button.is_active() && !editor.inner.syncing_tool.get() {
                        editor.select_tool(tool);
                    }
                }
            });
            // Mockup 01: "Arrow  A" — the name and the key as a keycap.
            if tool != Tool::Select {
                button.connect_query_tooltip(move |_, _, _, _, tooltip| {
                    let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    content.append(&gtk::Label::new(Some(&labels::tool_name(tool))));
                    content.append(&gtk::ShortcutLabel::new(&tool.shortcut().to_string()));
                    tooltip.set_custom(Some(&content));
                    true
                });
            }
        }
        let weak = self.downgrade();
        w.crop_tool_button.connect_toggled(move |button| {
            let Some(editor) = weak.upgrade() else {
                return;
            };
            if editor.inner.syncing_tool.get() {
                return;
            }
            if let Some(canvas) = editor.current_canvas() {
                if button.is_active() {
                    canvas.start_crop();
                } else {
                    canvas.end_crop();
                }
            }
        });
    }

    /// The tool of the strip, for every document (a button, a key, Esc on the canvas).
    fn select_tool(&self, tool: Tool) {
        self.inner.tool.set(tool);
        if let Some(canvas) = self.current_canvas() {
            canvas.set_tool(tool);
        }
        self.sync_tool();
    }

    /// The tool buttons follow the canvas (crop mode ends with Crop, Cancel, Esc, undo …; Esc
    /// goes back to Select after drawing).
    fn sync_tool(&self) {
        let w = &self.inner.w;
        let canvas = self.current_canvas();
        let cropping = canvas.as_ref().is_some_and(|c| c.cropping());
        if let Some(canvas) = &canvas {
            self.inner.tool.set(canvas.tool());
        }
        let tool = self.inner.tool.get();
        let button = if cropping {
            &w.crop_tool_button
        } else if tool == Tool::Select {
            &w.select_tool_button
        } else {
            match w.tool_buttons.iter().find(|(t, _)| *t == tool) {
                Some((_, button)) => button,
                None => &w.select_tool_button,
            }
        };
        if !button.is_active() {
            self.inner.syncing_tool.set(true);
            button.set_active(true);
            self.inner.syncing_tool.set(false);
        }
    }

    /// Announces what the canvas reports (mockup 13's toast with Undo).
    fn notice(&self, notice: Notice) {
        match notice {
            Notice::StepsRenumbered {
                deleted,
                renumbered,
                done,
            } => {
                let toast =
                    adw::Toast::new(&labels::steps_renumbered(deleted.as_deref(), renumbered));
                toast.set_use_markup(false);
                toast.set_button_label(Some(&gettext("Undo")));
                let weak = self.downgrade();
                toast.connect_button_clicked(move |_| {
                    if let Some(canvas) = weak.upgrade().and_then(|e| e.current_canvas()) {
                        canvas.undo_if_last(&done);
                    }
                });
                self.inner.w.toast_overlay.add_toast(toast);
            }
        }
    }

    fn toast(&self, text: &str) {
        let toast = adw::Toast::new(text);
        toast.set_use_markup(false);
        self.inner.w.toast_overlay.add_toast(toast);
    }

    // ---- export and save (add-export-share) ----

    /// The check every way out of Annota goes through first (add-export-share task 2.7, spec
    /// annotation-tools "Redaction export warning", decision D7): if the current document has
    /// blur or pixelate redactions — at any depth, groups included — "Make Redactions Solid?"
    /// (mockup 10) asks first. Cancel stops; the middle button goes on as it is; Convert to
    /// Solid makes every one of them solid as one undo step, then goes on. Without such
    /// redactions `proceed` runs at once.
    ///
    /// Used by Export (`Ctrl+E`), quick save (`Ctrl+S`) and Save As (`Ctrl+Shift+S`) so far;
    /// Copy, drag-out, Share and batch export (tasks 2.3–2.5) are to call it too.
    fn guard_redactions(&self, purpose: Purpose, proceed: impl FnOnce(&Editor) + 'static) {
        let Some(canvas) = self.current_canvas() else {
            return;
        };
        let found = canvas
            .with_document(redactions::reversible)
            .unwrap_or_default();
        if found.is_empty() {
            proceed(self);
            return;
        }
        let weak = self.downgrade();
        redaction_warning::present(&self.inner.w.window, found, purpose, move |response| {
            let Some(editor) = weak.upgrade() else {
                return;
            };
            match response {
                Response::Cancel => {}
                Response::Anyway => proceed(&editor),
                Response::Convert => {
                    canvas.run(|doc, _| redactions::make_solid(doc));
                    proceed(&editor);
                }
            }
        });
    }

    /// The current document, unless a dialog is open over the window: the window's
    /// accelerators still work then, and Export, Save and Save As must not start a second
    /// round behind the dialog (a second export dialog, a save during the redaction warning).
    fn current_without_dialog(&self) -> Option<Rc<Session>> {
        if self.inner.w.window.visible_dialog().is_some() {
            return None;
        }
        self.current()
    }

    /// `Ctrl+E`, the header's Export: the redaction warning, then "Export Image".
    fn export(&self) {
        let Some(session) = self.current_without_dialog() else {
            return;
        };
        // Text being typed belongs to the document: it is exported too.
        session.canvas.finish_text();
        let uid = session.uid;
        self.guard_redactions(Purpose::Export, move |editor| editor.present_export(uid));
    }

    fn present_export(&self, uid: u64) {
        let Some(session) = self.session(uid) else {
            return;
        };
        let weak = self.downgrade();
        export_dialog::present(
            &self.inner.w.window,
            session.canvas.clone(),
            &session.name(),
            &self.inner.settings,
            move |path| {
                if let Some(editor) = weak.upgrade() {
                    editor.toast_with_files(&saving::exported_toast(&path), path);
                }
            },
        );
    }

    /// `Ctrl+S` (spec export-share "Quick save", mockup 06): the redaction warning, then the
    /// document's own `.annota` file — or, for a document without one, a new file named after
    /// the `filename-pattern` preference in the save folder. No dialog either way.
    fn quick_save(&self) {
        let Some(session) = self.current_without_dialog() else {
            return;
        };
        session.canvas.finish_text();
        let uid = session.uid;
        self.guard_redactions(Purpose::Save, move |editor| editor.quick_save_now(uid));
    }

    fn quick_save_now(&self, uid: u64) {
        let Some(session) = self.session(uid) else {
            return;
        };
        let source = session.source.borrow().clone();
        let target = match source {
            Some(file) => file,
            None => {
                let settings = &self.inner.settings;
                let folder =
                    crate::preferences::resolved_save_folder(&settings.string("save-folder"));
                if let Err(err) = std::fs::create_dir_all(&folder) {
                    self.save_failed(uid, &folder, &ExportError::Io(err));
                    return;
                }
                let pattern = settings.string("filename-pattern");
                let stem = glib::DateTime::now_local()
                    .map(|now| saving::pattern_stem(&pattern, &now))
                    .unwrap_or_else(|_| "Annota".to_string());
                saving::quick_save_path(None, &folder, &stem, Path::exists)
            }
        };
        self.save_to(uid, &target);
    }

    /// `Ctrl+Shift+S`: the redaction warning, then GTK's file chooser for the `.annota` file.
    fn save_as(&self) {
        let Some(session) = self.current_without_dialog() else {
            return;
        };
        session.canvas.finish_text();
        let uid = session.uid;
        self.guard_redactions(Purpose::Save, move |editor| editor.choose_save_path(uid));
    }

    fn choose_save_path(&self, uid: u64) {
        let Some(session) = self.session(uid) else {
            return;
        };
        let folder = session
            .source
            .borrow()
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| {
                crate::preferences::resolved_save_folder(&self.inner.settings.string("save-folder"))
            });
        let stem = saving::file_stem(&session.name()).unwrap_or_else(text::untitled);
        let filter = gtk::FileFilter::new();
        filter.set_name(Some(&gettext("Annota Documents")));
        filter.add_suffix(saving::ANNOTA_EXTENSION);
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let mut builder = gtk::FileDialog::builder()
            .title(gettext("Save As"))
            .modal(true)
            .filters(&filters)
            .default_filter(&filter)
            .initial_name(format!("{stem}.{}", saving::ANNOTA_EXTENSION));
        if folder.is_dir() {
            builder = builder.initial_folder(&gio::File::for_path(&folder));
        }
        let chooser = builder.build();
        let window = self.inner.w.window.clone();
        let weak = self.downgrade();
        glib::spawn_future_local(async move {
            let Ok(file) = chooser.save_future(Some(&window)).await else {
                return;
            };
            if let (Some(editor), Some(path)) = (weak.upgrade(), file.path()) {
                editor.save_to(uid, &saving::with_annota_extension(&path));
            }
        });
    }

    /// Writes document `uid` to the `.annota` file `path`. On success the document belongs to
    /// that file and is named after it, its history is marked saved, its autosave goes, and
    /// the toast of mockup 06 says where it went; on failure the document stays as it was and
    /// a toast says why. Returns whether it was saved.
    fn save_to(&self, uid: u64, path: &Path) -> bool {
        let Some(session) = self.session(uid) else {
            return false;
        };
        let written = session
            .canvas
            .with_document(|doc| export::save_annota_named(doc, path));
        match written {
            Some(Ok(())) => {
                session.saved_as(path);
                session.forget_autosave(&self.inner.store);
                session.canvas.mark_saved();
                self.refresh();
                self.refresh_recent_tray();
                self.toast_with_files(&saving::saved_toast(path), path.to_path_buf());
                true
            }
            Some(Err(err)) => {
                self.save_failed(uid, path, &err);
                false
            }
            None => false,
        }
    }

    /// A save failed: the same words as the export dialog's banner (mockup 04), in a toast
    /// whose Save As… lets the user pick another place. The document stays unsaved.
    fn save_failed(&self, uid: u64, path: &Path, err: &ExportError) {
        // The path is the user's chosen location; no capture content is logged.
        eprintln!("annota: saving {} failed: {err}", path.display());
        let toast = adw::Toast::builder()
            .title(saving::problem(err).message())
            .button_label(gettext("Save As…"))
            .priority(adw::ToastPriority::High)
            .build();
        toast.set_use_markup(false);
        let weak = self.downgrade();
        toast.connect_button_clicked(move |_| {
            if let Some(editor) = weak.upgrade() {
                editor.choose_save_path(uid);
            }
        });
        self.inner.w.toast_overlay.add_toast(toast);
    }

    /// A toast about a written file with "Show in Files" (mockup 06).
    fn toast_with_files(&self, title: &str, path: PathBuf) {
        let toast = adw::Toast::builder()
            .title(title)
            .button_label(gettext("Show in Files"))
            .build();
        toast.set_use_markup(false);
        let window = self.inner.w.window.downgrade();
        toast.connect_button_clicked(move |_| {
            let file = gio::File::for_path(&path);
            let window = window.upgrade();
            gtk::FileLauncher::new(Some(&file)).open_containing_folder(
                window.as_ref(),
                gio::Cancellable::NONE,
                |result| {
                    if let Err(err) = result {
                        eprintln!("annota: cannot show the file: {err}");
                    }
                },
            );
        });
        self.inner.w.toast_overlay.add_toast(toast);
    }

    // ---- keys ----

    fn setup_keys(&self) {
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = self.downgrade();
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(editor) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if editor.key_pressed(key, modifiers) {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        let weak = self.downgrade();
        keys.connect_key_released(move |_, key, _, _| {
            if let Some(canvas) = weak.upgrade().and_then(|e| e.current_canvas()) {
                canvas.key_released(key);
            }
        });
        self.inner.w.window.add_controller(keys);
    }

    /// Whether a key press goes to the canvas: not while a text field or a dialog has the
    /// focus, and not Space/Enter for a focused button (keyboard activation).
    fn routes_to_canvas(&self, key: gdk::Key) -> bool {
        let Some(focus) = gtk::prelude::GtkWindowExt::focus(&self.inner.w.window) else {
            return true;
        };
        if focus.ancestor(adw::Dialog::static_type()).is_some()
            || focus.is::<gtk::Editable>()
            || focus.ancestor(gtk::Editable::static_type()).is_some()
        {
            return false;
        }
        let on_canvas = focus.is_ancestor(&self.inner.w.canvas_stack);
        let activates = matches!(
            key,
            gdk::Key::space | gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter
        );
        // A focused slider of the properties (opacity, shadow …) moves with the arrow keys
        // instead of nudging the selection (keyboard path, CLAUDE.md §7).
        let on_slider =
            focus.is::<gtk::Range>() || focus.ancestor(gtk::Range::static_type()).is_some();
        if on_slider && is_navigation_key(key) {
            return false;
        }
        // A quick style grid moves, applies, renames, reorders and deletes its tiles with
        // these keys (add-style-system redline "Keyboard"), not the selection.
        let on_style_grid = focus
            .ancestor(gtk::FlowBox::static_type())
            .is_some_and(|grid| grid.has_css_class("annota-style-grid"));
        if on_style_grid && is_style_grid_key(key) {
            return false;
        }
        on_canvas || !activates
    }

    fn key_pressed(&self, key: gdk::Key, modifiers: gdk::ModifierType) -> bool {
        if !self.routes_to_canvas(key) {
            return false;
        }
        let Some(canvas) = self.current_canvas() else {
            return false;
        };
        if is_image_op_key(key, modifiers)
            && !canvas.cropping()
            && canvas
                .with_document(image_ops::has_unknown_objects)
                .unwrap_or(false)
        {
            // Their accelerators are disabled, so the keys arrive here: say why nothing happens.
            self.toast(&text::newer_objects_toast());
            return true;
        }
        if canvas.key_pressed(key, modifiers) {
            return true;
        }
        // Typing in place: letters are text, not tool keys (they go on to the input method).
        if canvas.editing_text() {
            return false;
        }
        let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
        let plain = !modifiers.intersects(
            gdk::ModifierType::CONTROL_MASK
                | gdk::ModifierType::ALT_MASK
                | gdk::ModifierType::SUPER_MASK,
        );
        match key.to_lower() {
            // Copy image (Ctrl+C) — not an application accelerator, text fields need it.
            gdk::Key::c if ctrl => {
                let _ = WidgetExt::activate_action(&self.inner.w.window, "win.copy", None);
                true
            }
            // Crop (C) is a mode of the canvas.
            gdk::Key::c if plain => {
                self.inner.w.crop_tool_button.set_active(true);
                true
            }
            // The tools' single keys (spec "Tool switching"): V, A, L, P, U, T, K, S, H, B.
            k if plain => match k.to_unicode().and_then(Tool::from_shortcut) {
                Some(tool) => {
                    self.select_tool(tool);
                    true
                }
                None => false,
            },
            _ => false,
        }
    }

    // ---- tooltips ----

    /// "Undo Move Group  Ctrl Z" (mockup 06): the step's name and the keys as keycaps.
    fn setup_tooltips(&self) {
        let w = &self.inner.w;
        for (button, redo) in [(&w.undo_button, false), (&w.redo_button, true)] {
            let weak = self.downgrade();
            button.connect_query_tooltip(move |_, _, _, _, tooltip| {
                let Some(editor) = weak.upgrade() else {
                    return false;
                };
                let info = editor
                    .current_canvas()
                    .map(|c| c.history_info())
                    .unwrap_or_default();
                let (label, accel) = if redo {
                    (
                        text::history_tooltip(true, info.redo_label.as_deref()),
                        "<Control><Shift>z",
                    )
                } else {
                    (
                        text::history_tooltip(false, info.undo_label.as_deref()),
                        "<Control>z",
                    )
                };
                let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                content.append(&gtk::Label::new(Some(&label)));
                content.append(&gtk::ShortcutLabel::new(accel));
                tooltip.set_custom(Some(&content));
                true
            });
        }
    }

    // ---- sidebar ----

    fn setup_sidebar(&self) {
        let weak = self.downgrade();
        self.inner.sidebar.connect_event(move |event| {
            if let Some(editor) = weak.upgrade() {
                editor.sidebar_event(event);
            }
        });
        let weak = self.downgrade();
        self.inner.sidebar.panel.connect_event(move |event| {
            if let Some(editor) = weak.upgrade() {
                editor.panel_event(event);
            }
        });
        let weak = self.downgrade();
        self.inner.sidebar.quick.connect_event(move |event| {
            if let Some(editor) = weak.upgrade() {
                editor.quick_event(event);
            }
        });
        let weak = self.downgrade();
        self.inner.styles.connect_changed(move || {
            if let Some(editor) = weak.upgrade() {
                editor.styles_changed();
            }
        });
        // The current theme's defaults and palette from the start.
        self.apply_theme();
        let weak = self.downgrade();
        self.inner
            .settings
            .connect_changed(Some("snap-to-guides"), move |settings, key| {
                let on = settings.boolean(key);
                if let Some(editor) = weak.upgrade() {
                    for session in editor.sessions() {
                        session.canvas.set_snapping(on);
                    }
                }
            });
    }

    fn sidebar_event(&self, event: Event) {
        let Some(canvas) = self.current_canvas() else {
            return;
        };
        match event {
            Event::Arrange(arrange) => {
                if !canvas.set_arrange(arrange) {
                    // Refused or unchanged: show the real values again.
                    self.refresh_sidebar();
                }
            }
            Event::CornerRadius(radius) => {
                if canvas
                    .run(|doc, _| image_ops::set_corner_radius(doc, radius))
                    .is_none()
                {
                    self.refresh_sidebar();
                }
            }
            Event::Background(background) => {
                canvas.run(|doc, _| image_ops::set_background(doc, background));
            }
            Event::CustomBackground => self.choose_background(),
            Event::PickBackground => {
                let weak = self.downgrade();
                canvas.pick_colour(move |colour| {
                    let (Some(editor), Some(color)) = (weak.upgrade(), colour) else {
                        return;
                    };
                    if let Some(canvas) = editor.current_canvas() {
                        canvas.run(|doc, _| {
                            image_ops::set_background(doc, Background::Color { color })
                        });
                    }
                });
            }
            Event::CropFrame(frame) => canvas.set_crop_frame(frame),
            Event::CropAspect(aspect) => canvas.set_crop_aspect(aspect),
            // The keyboard path of canvas extension (mockup 10's edge drag): one undo step.
            Event::CanvasExtensions(extensions) => {
                if canvas
                    .run(|doc, _| {
                        image_ops::set_canvas_rect(
                            doc,
                            canvas_edit::with_extensions(doc, extensions),
                        )
                    })
                    .is_none()
                {
                    // Limited to what it was (into the image, too large): show the real values.
                    self.refresh_sidebar();
                }
            }
        }
    }

    /// The type properties (task 3.1): one undo step per committed change, previews while a
    /// slider, the dial or the colour picker is dragged.
    fn panel_event(&self, event: PanelEvent) {
        let Some(canvas) = self.current_canvas() else {
            return;
        };
        match event {
            PanelEvent::Property(key, value) => {
                if !canvas.set_property(key, value) {
                    // Refused or unchanged: show the real values again.
                    self.refresh_sidebar();
                }
            }
            PanelEvent::Preview(key, value) => canvas.preview_property(key, value),
            PanelEvent::EndPreview => canvas.end_preview(),
            PanelEvent::PickScreen(key) => self.pick_screen_colour(key),
            PanelEvent::RestartSequence => {
                canvas.restart_sequence();
                self.refresh_sidebar();
            }
            PanelEvent::NextStep(n) => canvas.set_next_step(Some(n)),
        }
    }

    // ---- quick styles and themes (add-style-system) ----

    /// What the quick styles ask for.
    fn quick_event(&self, event: QuickEvent) {
        match event {
            QuickEvent::Apply(values) => {
                if let Some(canvas) = self.current_canvas() {
                    if !canvas.apply_style(&values) {
                        self.refresh_sidebar();
                    }
                }
            }
            QuickEvent::Favourite(tool, values) => self.apply_favourite(tool, &values),
            QuickEvent::UseTheme(id) => self.inner.styles.set_theme(&id),
            QuickEvent::ManageThemes => self.present_themes(),
            QuickEvent::Toast(title, undo) => {
                let toast = adw::Toast::new(&title);
                toast.set_use_markup(false);
                // Its Undo is for what was just done: shown at once, not queued behind an
                // earlier toast.
                toast.set_priority(adw::ToastPriority::High);
                if let Some(undo) = undo {
                    toast.set_button_label(Some(&gettext("Undo")));
                    toast.connect_button_clicked(move |_| undo());
                }
                self.inner.w.toast_overlay.add_toast(toast);
            }
        }
    }

    /// A favourite (scenario "Use favourite", mockups 04 → 05): its tool becomes active with
    /// that style. A selection of that tool's objects gets the style (one undo step); any other
    /// selection is let go, so the tool's defaults show and take the style.
    fn apply_favourite(&self, tool: Tool, values: &[annota::document::annotation::Property]) {
        let Some(canvas) = self.current_canvas() else {
            return;
        };
        let selection = canvas.selection();
        let own = canvas.with_document(|doc| {
            !selection.is_empty()
                && selection.iter().all(|id| {
                    doc.scene()
                        .get(*id)
                        .is_some_and(|n| Some(n.object.type_name()) == tool.object_type())
                })
        });
        if own != Some(true) {
            canvas.deselect();
        }
        self.select_tool(tool);
        canvas.apply_style(values);
        self.refresh_sidebar();
    }

    /// The styles or themes changed: a different (or edited) current theme sets the tools'
    /// defaults to its first styles and the palette swatches to its colours.
    fn styles_changed(&self) {
        self.apply_theme();
        self.refresh_sidebar();
    }

    fn apply_theme(&self) {
        let theme = self.inner.styles.current_theme();
        if self.inner.applied_theme.borrow().as_ref() == Some(&theme) {
            return;
        }
        {
            let mut toolbox = self.inner.toolbox.borrow_mut();
            for tool in style_value::TOOLS {
                for p in generate::style(&theme, tool, 0) {
                    let _ = toolbox.set_property(tool, p.key, &p.value);
                }
            }
        }
        self.inner.sidebar.panel.set_palette(&theme.colours);
        self.inner.applied_theme.replace(Some(theme));
    }

    /// The theme editor (mockup 06).
    fn present_themes(&self) {
        theme_editor::present(&self.inner.w.window, self.inner.styles.clone());
    }

    /// The eyedropper (redline "Eyedropper", mockup 17): xdg-desktop-portal picks a colour
    /// anywhere on the screen; meanwhile the eyedropper swatch is accent-filled, the subtitle
    /// says what to do and the canvas chip stays. The colour goes to `key` of whatever the
    /// properties edit when it arrives (selection, text being typed or tool defaults).
    fn pick_screen_colour(&self, key: &'static str) {
        if self.inner.picking.get().is_some() {
            return;
        }
        let Some(canvas) = self.current_canvas() else {
            return;
        };
        self.inner.picking.set(Some(key));
        self.inner.sidebar.panel.set_picking(Some(key));
        self.inner.w.pick_chip.set_visible(true);
        self.refresh_header();
        let pick = (self.inner.screen_picker.borrow())();
        let weak = self.downgrade();
        glib::spawn_future_local(async move {
            let result = pick.await;
            if let Some(editor) = weak.upgrade() {
                editor.screen_colour_picked(&canvas, key, result);
            }
        });
    }

    fn screen_colour_picked(
        &self,
        canvas: &Canvas,
        key: &'static str,
        result: Result<Rgba, crate::capture::portal::CaptureError>,
    ) {
        use crate::capture::portal::CaptureError;
        self.inner.picking.set(None);
        self.inner.sidebar.panel.set_picking(None);
        self.inner.w.pick_chip.set_visible(false);
        match result {
            Ok(colour) => {
                self.inner.sidebar.panel.remember(colour);
                canvas.set_property(key, PropertyValue::Color(colour));
            }
            Err(CaptureError::Cancelled) => {}
            Err(CaptureError::PortalUnavailable) => self.toast(&gettext(
                "Picking a colour from the screen needs xdg-desktop-portal",
            )),
            Err(CaptureError::PermissionDenied) => {
                self.toast(&gettext("Annota may not read colours from the screen"))
            }
            Err(err) => {
                eprintln!("annota: eyedropper: {err}");
                self.toast(&gettext("The colour could not be picked"));
            }
        }
        self.refresh_header();
        self.refresh_sidebar();
    }

    /// Replaces the screen eyedropper (tests: no portal on the developer's desktop).
    #[cfg(test)]
    fn set_screen_picker(&self, picker: ScreenPicker) {
        self.inner.screen_picker.replace(picker);
    }

    /// The Custom swatch: GTK's colour chooser (portal-free, part of GTK).
    fn choose_background(&self) {
        let Some(canvas) = self.current_canvas() else {
            return;
        };
        let initial = canvas.with_document(|d| match d.canvas().background {
            Background::Color { color } => {
                let (r, g, b, a) = color.to_f64();
                gdk::RGBA::new(r as f32, g as f32, b as f32, a as f32)
            }
            _ => gdk::RGBA::WHITE,
        });
        let dialog = gtk::ColorDialog::builder()
            .title(gettext("Canvas Colour"))
            .with_alpha(false)
            .modal(true)
            .build();
        let window = self.inner.w.window.clone();
        glib::spawn_future_local(async move {
            let Ok(rgba) = dialog
                .choose_rgba_future(Some(&window), initial.as_ref())
                .await
            else {
                return;
            };
            let channel = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            let color = annota::document::color::Rgba::opaque(
                channel(rgba.red()),
                channel(rgba.green()),
                channel(rgba.blue()),
            );
            canvas.run(|doc, _| image_ops::set_background(doc, Background::Color { color }));
        });
    }

    // ---- tray ----

    fn setup_tray(&self) {
        let weak = self.downgrade();
        self.inner.tray.connect_activate(move |key| {
            let Some(editor) = weak.upgrade() else {
                return;
            };
            match key {
                TrayKey::Open(uid) => editor.switch_to(uid),
                TrayKey::Recent(path) => editor.open_file(&path),
            }
        });
        // The save folder changes while Annota runs: look again whenever the window is
        // activated and when the tray is shown.
        let weak = self.downgrade();
        self.inner.w.window.connect_is_active_notify(move |window| {
            if window.is_active() {
                if let Some(editor) = weak.upgrade() {
                    editor.refresh_recent_tray();
                }
            }
        });
        self.refresh_recent_tray();
    }

    /// Dropping a tray thumbnail on the canvas inserts its image (mockup 11, spec "Picture in
    /// picture").
    fn setup_drop(&self) {
        let area = &self.inner.w.canvas_area;
        let target = gtk::DropTarget::new(glib::Type::STRING, gdk::DragAction::COPY);
        let weak = self.downgrade();
        target.connect_accept(move |_, drop| {
            let ours = drop.formats().contains_type(glib::Type::STRING);
            ours && weak.upgrade().and_then(|e| e.current()).is_some()
        });
        let weak = self.downgrade();
        target.connect_enter(move |_, x, y| {
            if let Some(editor) = weak.upgrade() {
                editor.show_drop_feedback(Some((x, y)));
            }
            gdk::DragAction::COPY
        });
        let weak = self.downgrade();
        target.connect_motion(move |_, x, y| {
            if let Some(editor) = weak.upgrade() {
                editor.show_drop_feedback(Some((x, y)));
            }
            gdk::DragAction::COPY
        });
        let weak = self.downgrade();
        target.connect_leave(move |_| {
            if let Some(editor) = weak.upgrade() {
                editor.show_drop_feedback(None);
            }
        });
        let weak = self.downgrade();
        target.connect_drop(move |_, value, x, y| {
            let Some(editor) = weak.upgrade() else {
                return false;
            };
            editor.show_drop_feedback(None);
            let key = value
                .get::<String>()
                .ok()
                .and_then(|text| TrayKey::from_payload(&text));
            match key {
                Some(key) => editor.insert_from_tray(&key, x, y),
                None => false,
            }
        });
        area.add_controller(target);
    }

    fn show_drop_feedback(&self, at: Option<(f64, f64)>) {
        let w = &self.inner.w;
        w.drop_ring.set_visible(at.is_some());
        w.drop_chip.set_visible(at.is_some());
        if let Some((x, y)) = at {
            // Mockup 11: below the left end of the drag image, which is centred on the pointer.
            let (width, height) = (
                f64::from(w.canvas_area.width()),
                f64::from(w.canvas_area.height()),
            );
            let (_, natural) = w.drop_chip.preferred_size();
            let (chip_w, chip_h) = (
                f64::from(natural.width() - w.drop_chip.margin_start()),
                f64::from(natural.height() - w.drop_chip.margin_top()),
            );
            let (half_w, half_h) = tray::drag_half_size();
            let x = (x - half_w).clamp(0.0, (width - chip_w).max(0.0));
            let y = (y + half_h + 6.0).clamp(0.0, (height - chip_h).max(0.0));
            w.drop_chip.set_margin_start(x.round() as i32);
            w.drop_chip.set_margin_top(y.round() as i32);
        }
    }

    fn insert_from_tray(&self, key: &TrayKey, x: f64, y: f64) -> bool {
        let Some(canvas) = self.current_canvas() else {
            return false;
        };
        let bitmap = match key {
            TrayKey::Open(uid) => self
                .session(*uid)
                .and_then(|s| s.canvas.with_document(|d| render::flatten(d, 1.0).ok()))
                .flatten(),
            TrayKey::Recent(path) => load_flattened(path),
        };
        let Some(bitmap) = bitmap else {
            self.toast(&gettext("Could not insert the image"));
            return false;
        };
        let Some(centre) = canvas.document_point(x, y) else {
            return false;
        };
        let Some(revision) = canvas.insert_image(bitmap, centre) else {
            return false;
        };
        // Redline toast "Image inserted" with Undo.
        let toast = adw::Toast::builder()
            .title(gettext("Image inserted"))
            .button_label(gettext("Undo"))
            .build();
        let weak = canvas.clone();
        toast.connect_button_clicked(move |_| {
            // Only if nothing happened since.
            if weak.history_info().revision == revision {
                weak.undo();
            }
        });
        self.inner.w.toast_overlay.add_toast(toast);
        true
    }

    // ---- autosave and recovery ----

    /// The 30 s autosave timer (spec: Autosave and recovery). It stops with the window.
    fn setup_autosave(&self) {
        let weak = self.downgrade();
        let seconds = u32::try_from(autosave::INTERVAL.as_secs()).unwrap_or(30);
        glib::timeout_add_seconds_local(seconds, move || {
            let Some(editor) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            editor.autosave_now();
            glib::ControlFlow::Continue
        });
    }

    fn autosave_now(&self) {
        for session in self.sessions() {
            session.autosave_tick(&self.inner.store);
        }
    }

    fn offer_recovery(&self, entries: Vec<RecoveryEntry>) {
        let restore = self.downgrade();
        let discard = self.downgrade();
        dialogs::present_restore(
            &self.inner.w.window,
            entries,
            move |picked| {
                if let Some(editor) = restore.upgrade() {
                    editor.restore(picked);
                }
            },
            move |picked| {
                if let Some(editor) = discard.upgrade() {
                    for entry in picked {
                        if let Err(err) = editor.inner.store.discard(&entry.id) {
                            eprintln!("annota: cannot remove an autosave: {err}");
                        }
                    }
                }
            },
        );
    }

    /// Opens the picked autosaves. Each keeps its slot, so it stays recoverable until it is
    /// saved or closed.
    fn restore(&self, picked: Vec<RecoveryEntry>) {
        for entry in picked {
            match self.inner.store.restore(&entry) {
                Ok(restored) => {
                    let name = restored
                        .document
                        .metadata()
                        .title
                        .clone()
                        .or_else(|| restored.source.as_deref().map(session::file_stem))
                        .unwrap_or_else(text::untitled);
                    self.add_session(
                        Loaded {
                            document: restored.document,
                            path: restored.source.as_deref().map(session::canonical),
                            source: restored.source,
                            name,
                        },
                        Origin::Restored,
                        entry.id.clone(),
                    );
                }
                Err(err) => self.toast(
                    &gettext("Could not restore “{name}”: {error}")
                        .replace("{name}", &dialogs::entry_name(&entry))
                        .replace("{error}", &err.to_string()),
                ),
            }
        }
    }
}

struct WeakEditor(Weak<Inner>);

impl WeakEditor {
    fn upgrade(&self) -> Option<Editor> {
        self.0.upgrade().map(|inner| Editor { inner })
    }
}

/// Removes a widget from whatever holds it (the sidebar or the bottom sheet).
trait Unparent {
    fn unparent_from_any_parent(&self);
}

impl Unparent for gtk::Box {
    fn unparent_from_any_parent(&self) {
        let Some(parent) = self.parent() else {
            return;
        };
        // A scrolled window wraps a non-scrollable child in a viewport.
        if let Some(viewport) = parent.downcast_ref::<gtk::Viewport>() {
            viewport.set_child(None::<&gtk::Widget>);
        } else if let Some(scroller) = parent.downcast_ref::<gtk::ScrolledWindow>() {
            scroller.set_child(None::<&gtk::Widget>);
        }
    }
}

/// The main menu below 900 px: Copy moves in (redline "Window and shell").
fn narrow_menu(main: &gio::MenuModel) -> gio::Menu {
    let menu = gio::Menu::new();
    let copy = gio::Menu::new();
    copy.append(Some(&gettext("Copy to Clipboard")), Some("win.copy"));
    menu.append_section(None, &copy);
    for i in 0..main.n_items() {
        if let Some(section) = main.item_link(i, gio::MENU_LINK_SECTION) {
            menu.append_section(None, &section);
        }
    }
    menu
}

/// A recent file's flattened image (a tray thumbnail dropped on the canvas).
fn load_flattened(path: &Path) -> Option<annota::document::bitmap::Bitmap> {
    if session::is_annota(path) {
        let doc: Document = annota::document::archive::load(path).ok()?;
        render::flatten(&doc, 1.0).ok()
    } else {
        session::load_bitmap(path).ok()
    }
}

/// Creates the window actions. Share (add-export-share task 2.4) exists but stays disabled; the others
/// get their handlers in [`Editor::setup_actions`] and their state in
/// [`Editor::refresh_action_states`].
fn install_actions(window: &adw::ApplicationWindow, settings: &gio::Settings) -> Actions {
    let simple = |name: &str| {
        let action = gio::SimpleAction::new(name, None);
        window.add_action(&action);
        action
    };
    // Decision D1's Share menu (add-export-share).
    for name in ["share-email", "share-open-with", "share-copy-to-folder"] {
        simple(name).set_enabled(false);
    }
    window.add_action(&settings.create_action("show-properties"));
    window.add_action(&settings.create_action("show-tray"));
    let zoom_preset = gio::SimpleAction::new_stateful(
        "zoom-preset",
        Some(glib::VariantTy::STRING),
        &String::new().to_variant(),
    );
    window.add_action(&zoom_preset);
    // Only useful with a document open.
    let document = [
        "copy",
        "export",
        "save",
        "save-as",
        "zoom-fit",
        "zoom-fit-width",
        "zoom-in",
        "zoom-out",
    ]
    .into_iter()
    .map(simple)
    .collect();
    let image = [
        "resize-image",
        "rotate-left",
        "rotate-right",
        "flip-horizontal",
        "flip-vertical",
    ]
    .into_iter()
    .map(simple)
    .collect();
    let selected = [
        "duplicate",
        "delete",
        "bring-to-front",
        "bring-forward",
        "send-backward",
        "send-to-back",
    ]
    .into_iter()
    .map(simple)
    .collect();
    Actions {
        undo: simple("undo"),
        redo: simple("redo"),
        document,
        image,
        selected,
        group: simple("group"),
        ungroup: simple("ungroup"),
        flatten_selection: simple("flatten-selection"),
        flatten_all: simple("flatten-all"),
        crop_apply: simple("crop-apply"),
        crop_cancel: simple("crop-cancel"),
        zoom_preset,
    }
}

/// The keys of the image operations that a document with objects of a newer Annota cannot have:
/// Rotate (Ctrl+R, Ctrl+Shift+R), Flatten (Ctrl+T, Ctrl+Shift+T) and the Crop tool (C).
/// Keys a focused slider uses.
fn is_navigation_key(key: gdk::Key) -> bool {
    matches!(
        key,
        gdk::Key::Left
            | gdk::Key::Right
            | gdk::Key::Up
            | gdk::Key::Down
            | gdk::Key::KP_Left
            | gdk::Key::KP_Right
            | gdk::Key::KP_Up
            | gdk::Key::KP_Down
            | gdk::Key::Home
            | gdk::Key::End
            | gdk::Key::Page_Up
            | gdk::Key::Page_Down
    )
}

/// Keys a focused quick style grid uses itself.
fn is_style_grid_key(key: gdk::Key) -> bool {
    is_navigation_key(key)
        || matches!(
            key,
            gdk::Key::Delete
                | gdk::Key::KP_Delete
                | gdk::Key::BackSpace
                | gdk::Key::F2
                | gdk::Key::F10
                | gdk::Key::Menu
                | gdk::Key::Return
                | gdk::Key::KP_Enter
                | gdk::Key::ISO_Enter
                | gdk::Key::space
        )
}

fn is_image_op_key(key: gdk::Key, modifiers: gdk::ModifierType) -> bool {
    let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
    let other = modifiers.intersects(gdk::ModifierType::ALT_MASK | gdk::ModifierType::SUPER_MASK);
    if other {
        return false;
    }
    match key.to_lower() {
        gdk::Key::r | gdk::Key::t => ctrl,
        gdk::Key::c => !ctrl && !modifiers.contains(gdk::ModifierType::SHIFT_MASK),
        _ => false,
    }
}

/// Accelerators of the editor's actions, one each: menus show an action's accelerator only
/// if it has exactly one (GTK 4.22). The keypad and `Ctrl+=` variants of the zoom keys are
/// handled by the canvas. Keys text fields use too (Ctrl+C, Ctrl+Z, Ctrl+A, arrows, Delete)
/// are handled by the window's key controller instead (see the module docs).
fn set_accels(app: &adw::Application) {
    for (action, accel) in [
        ("win.show-properties", "F9"),
        ("win.export", "<Control>e"),
        ("win.save", "<Control>s"),
        ("win.save-as", "<Control><Shift>s"),
        ("win.zoom-fit", "<Control>0"),
        ("win.zoom-preset('100')", "<Control>1"),
        ("win.zoom-in", "<Control>plus"),
        ("win.zoom-out", "<Control>minus"),
        ("win.rotate-right", "<Control>r"),
        ("win.rotate-left", "<Control><Shift>r"),
        ("win.flatten-all", "<Control><Shift>t"),
        // Decision D3: Ctrl+T flattens the selection in the editor.
        ("win.flatten-selection", "<Control>t"),
    ] {
        app.set_accels_for_action(action, &[accel]);
    }
}

/// Opens a real window, so opt-in; run on its own (see `crate::ui::test_support`):
/// `GDK_BACKEND=x11 xvfb-run -a dbus-run-session cargo test --bin annota -- --ignored --exact
/// ui::editor::tests::editor_window_on_a_live_display` (needs `meson compile -C _build` for the
/// resources, or `ANNOTA_GRESOURCE` pointing at them).
#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::ui::test_support::{init, pump};
    use annota::document::bitmap::Bitmap;
    use annota::document::color::palette;
    use annota::document::command::History;
    use annota::document::geometry::Point;
    use annota::tools::image::Image;

    struct TempDir(PathBuf);

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Resources, a private GSettings schema and backend, and a registered application.
    fn environment(tmp: &Path) -> adw::Application {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let schemas = tmp.join("schemas");
        std::fs::create_dir_all(&schemas).unwrap();
        std::fs::copy(
            root.join("data/io.github.trosinde.Annota.gschema.xml"),
            schemas.join("io.github.trosinde.Annota.gschema.xml"),
        )
        .unwrap();
        let status = std::process::Command::new("glib-compile-schemas")
            .arg(&schemas)
            .status()
            .unwrap();
        assert!(status.success());
        // Edition 2021: setting the environment is safe here, before GTK starts threads.
        std::env::set_var("GSETTINGS_SCHEMA_DIR", &schemas);
        std::env::set_var("GSETTINGS_BACKEND", "memory");
        let resources = std::env::var_os("ANNOTA_GRESOURCE")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("_build/data/resources.gresource"));
        gio::resources_register(&gio::Resource::load(&resources).unwrap());
        init();
        let app = adw::Application::builder()
            .application_id("io.github.trosinde.Annota.EditorTest")
            .flags(gio::ApplicationFlags::NON_UNIQUE)
            .resource_base_path("/io/github/trosinde/Annota")
            .build();
        app.register(gio::Cancellable::NONE).unwrap();
        app
    }

    /// Quick styles and themes in the test's directory, no system themes.
    fn style_paths(tmp: &Path) -> StylePaths {
        StylePaths {
            data: tmp.join("styles"),
            system_themes: tmp.join("system-themes"),
        }
    }

    fn sample(path: &Path) {
        let mut doc = Document::with_title(
            Bitmap::filled(1600, 1000, palette::PAPER).unwrap(),
            "Checkout flow",
        );
        let mut history = History::new();
        for centre in [Point::new(300.0, 300.0), Point::new(1200.0, 700.0)] {
            let pip = Bitmap::filled(440, 66, palette::CORAL).unwrap();
            let cmd = Image::insert_command(&doc, pip, centre);
            history.execute(&mut doc, "Insert", cmd).unwrap();
        }
        annota::document::archive::save(&doc, path).unwrap();
    }

    /// (label, accelerator) of every item of a menu button's popover.
    fn menu_accels(button: &gtk::MenuButton) -> Vec<(String, Option<String>)> {
        fn walk(widget: &gtk::Widget, out: &mut Vec<(String, Option<String>)>) {
            if widget.type_().name() == "GtkModelButton" {
                let text: Option<String> = widget.property("text");
                out.push((text.unwrap_or_default(), widget.property("accel")));
            }
            let mut child = widget.first_child();
            while let Some(c) = child {
                walk(&c, out);
                child = c.next_sibling();
            }
        }
        let mut out = Vec::new();
        if let Some(popover) = button.popover() {
            walk(popover.upcast_ref(), &mut out);
        }
        out
    }

    fn properties_in_sheet(editor: &Editor) -> bool {
        editor
            .inner
            .sidebar
            .properties_box
            .is_ancestor(&editor.inner.w.sheet_scroller)
    }

    #[test]
    fn previous_autosaves_are_offered_once_per_process() {
        // S2: whichever path opens the editor first (start-up, a capture, --open) offers the
        // autosaves; later ones do not offer them again. (Thread-local, so per test thread.)
        let tmp = TempDir(std::env::temp_dir().join(format!(
            "annota-recovery-once-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        )));
        let store = Store::new(tmp.0.join("autosave"));
        let doc = Document::new(Bitmap::filled(20, 10, palette::LIME).unwrap());
        store.write(&AutosaveId::generate(), &doc, None).unwrap();
        let entries = recovery_entries(&store).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(recovery_entries(&store).is_none(), "offered once");
    }

    #[test]
    fn nothing_to_offer_still_counts_as_offered() {
        // An empty start-up check means later autosaves are this process's own: never offered
        // back to it.
        let tmp = TempDir(std::env::temp_dir().join(format!(
            "annota-recovery-empty-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        )));
        let store = Store::new(tmp.0.join("autosave"));
        assert!(recovery_entries(&store).is_none());
        let doc = Document::new(Bitmap::filled(20, 10, palette::LIME).unwrap());
        store.write(&AutosaveId::generate(), &doc, None).unwrap();
        assert!(recovery_entries(&store).is_none());
    }

    /// S2: Annota started by a capture hotkey (`annota --capture`, which skips the start-up
    /// offer) offers the autosaves of a previous run when the capture opens the editor —
    /// through the real `open_capture` → `get_or_create` path, with the cache in a temporary
    /// directory. Run on its own like the tests below.
    #[test]
    #[ignore = "opens a window; run under xvfb-run with GDK_BACKEND=x11"]
    fn first_capture_offers_recovery_on_a_live_display() {
        let tmp = TempDir(
            std::env::temp_dir().join(format!("annota-capture-recovery-{}", std::process::id())),
        );
        let _ = std::fs::remove_dir_all(&tmp.0);
        // Before GLib reads it: the default autosave store lives in the temporary directory.
        std::env::set_var("XDG_CACHE_HOME", tmp.0.join("cache"));
        // Quick styles too (add-style-system): never the real ones.
        std::env::set_var("XDG_DATA_HOME", tmp.0.join("data"));
        let app = environment(&tmp.0);
        assert!(default_style_paths().data.starts_with(&tmp.0));
        assert!(
            default_store().dir().starts_with(&tmp.0),
            "never the real cache"
        );
        // A previous run left one autosave.
        let left = Document::with_title(
            Bitmap::filled(300, 200, palette::AZURE).unwrap(),
            "Left behind",
        );
        default_store()
            .write(&AutosaveId::generate(), &left, None)
            .unwrap();
        assert!(EDITOR.with(|e| e.borrow().is_none()), "no editor yet");

        // The first capture opens the editor: the capture is there, the dialog on top of it.
        assert!(open_capture(
            &app,
            Bitmap::filled(400, 300, palette::LIME).unwrap(),
            "Capture 2026-10-31 09-00-00",
            "Region captured · 400 × 300",
            Offer::Nothing,
        ));
        pump(300);
        let editor = Editor {
            inner: EDITOR.with(|e| e.borrow().clone()).unwrap(),
        };
        let w = &editor.inner.w;
        assert_eq!(editor.sessions().len(), 1);
        assert_eq!(w.title.title(), "Capture 2026-10-31 09-00-00");
        let dialog = w
            .window
            .visible_dialog()
            .expect("Restore Unsaved Captures?");
        assert!(
            shows_label(dialog.upcast_ref(), "Left behind"),
            "lists the autosave"
        );
        // Only the previous run's autosave is listed, not the new capture.
        let rows = {
            fn count(widget: &gtk::Widget) -> usize {
                let own = usize::from(widget.is::<adw::ActionRow>());
                let mut n = own;
                let mut child = widget.first_child();
                while let Some(c) = child {
                    n += count(&c);
                    child = c.next_sibling();
                }
                n
            }
            count(dialog.upcast_ref())
        };
        assert_eq!(rows, 1);
        dialog.close();
        pump(300);
        assert!(w.window.visible_dialog().is_none());

        // Once per process: the next capture, a later start-up check and --open do not ask again.
        editor.autosave_now();
        assert!(open_capture(
            &app,
            Bitmap::filled(200, 100, palette::CORAL).unwrap(),
            "Capture 2026-10-31 09-05-00",
            "Region captured · 200 × 100",
            Offer::Nothing,
        ));
        pump(200);
        assert!(w.window.visible_dialog().is_none(), "offered once");
        assert!(!offer_recovery(&app));
        assert_eq!(editor.sessions().len(), 2);
        // Closing the dialog kept the autosave for the next start.
        assert!(default_store()
            .list()
            .entries
            .iter()
            .any(|e| e.name.as_deref() == Some("Left behind")));
        w.window.destroy();
        pump(50);
    }

    /// Whether a label with exactly `text` is shown somewhere under `root` (a toast's title).
    fn shows_label(root: &gtk::Widget, text: &str) -> bool {
        if root
            .downcast_ref::<gtk::Label>()
            .is_some_and(|l| l.label() == text && l.is_mapped())
        {
            return true;
        }
        let mut child = root.first_child();
        while let Some(c) = child {
            if shows_label(&c, text) {
                return true;
            }
            child = c.next_sibling();
        }
        false
    }

    /// Clicks the shown button labelled `text` under `root` (a toast's Undo); whether there was
    /// one.
    fn click_button(root: &gtk::Widget, text: &str) -> bool {
        if let Some(button) = root.downcast_ref::<gtk::Button>() {
            if button.label().as_deref() == Some(text) && button.is_mapped() {
                button.emit_clicked();
                return true;
            }
        }
        let mut child = root.first_child();
        while let Some(c) = child {
            if click_button(&c, text) {
                return true;
            }
            child = c.next_sibling();
        }
        false
    }

    /// N1: a document with an object of a type from a newer Annota disables crop, rotate, flip,
    /// resize image and flatten (menu, sidebar, keys), says why, and keeps the rest editable.
    /// Run on its own like the test below.
    #[test]
    #[ignore = "opens a window; run under xvfb-run with GDK_BACKEND=x11"]
    fn newer_objects_block_image_operations_on_a_live_display() {
        let tmp =
            TempDir(std::env::temp_dir().join(format!("annota-newer-test-{}", std::process::id())));
        let _ = std::fs::remove_dir_all(&tmp.0);
        std::fs::create_dir_all(tmp.0.join("captures")).unwrap();
        let app = environment(&tmp.0);
        let editor = Editor::build(
            &app,
            Store::new(tmp.0.join("autosave")),
            style_paths(&tmp.0),
        )
        .unwrap();
        EDITOR.with(|e| e.replace(Some(editor.inner.clone())));
        let w = &editor.inner.w;
        w.window.set_default_size(1280, 800);
        w.window.present();
        pump(200);

        // The sample plus an object of a type this version does not know.
        let plain = tmp.0.join("captures/Checkout flow.annota");
        sample(&plain);
        let newer = tmp.0.join("captures/From a newer Annota.annota");
        let mut doc = annota::document::archive::load(&plain).unwrap();
        let fields = serde_json::json!({"x": 10, "y": 10, "width": 50, "height": 20});
        let serde_json::Value::Object(fields) = fields else {
            panic!("object expected");
        };
        let id = doc.scene().fresh_ids(1)[0];
        let node = annota::document::scene::Node::new(
            id,
            Box::new(annota::document::registry::UnknownObject::new(
                "hologram", fields,
            )),
        );
        let at = doc.scene().len();
        History::new()
            .execute(
                &mut doc,
                "Read",
                annota::document::command::Command::Insert {
                    nodes: vec![(at, node)],
                    resources: Vec::new(),
                },
            )
            .unwrap();
        annota::document::archive::save(&doc, &newer).unwrap();

        editor.open_file(&newer);
        pump(300);
        let a = &editor.inner.actions;
        let canvas = editor.current_canvas().unwrap();
        let unchanged = || canvas.history_info().undo_label.is_none();
        assert!(a.image.iter().all(|a| !a.is_enabled()), "image operations");
        assert!(!a.flatten_all.is_enabled());
        assert!(!w.crop_tool_button.is_sensitive());
        let crop_tip = w.crop_tool_button.tooltip_text().unwrap_or_default();
        assert!(
            crop_tip.starts_with("Crop (C) · Needs a newer Annota"),
            "{crop_tip}"
        );
        let sidebar = &editor.inner.sidebar;
        for (button, _) in &sidebar.image_op_buttons {
            assert!(!button.is_sensitive());
            let tip = button.tooltip_text().unwrap_or_default();
            assert!(tip.contains("Needs a newer Annota"), "{tip}");
        }
        // The menu actions and their keys do nothing; the keys explain why in a toast.
        let _ = WidgetExt::activate_action(&w.window, "win.rotate-right", None);
        let _ = WidgetExt::activate_action(&w.window, "win.flatten-all", None);
        pump(50);
        assert!(unchanged());
        let toast = text::newer_objects_toast();
        assert!(!shows_label(w.toast_overlay.upcast_ref(), &toast));
        assert!(editor.key_pressed(gdk::Key::r, gdk::ModifierType::CONTROL_MASK));
        pump(300);
        assert!(shows_label(w.toast_overlay.upcast_ref(), &toast), "toast");
        assert!(editor.key_pressed(gdk::Key::c, gdk::ModifierType::empty()));
        assert!(editor.key_pressed(
            gdk::Key::T,
            gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK
        ));
        pump(50);
        assert!(!canvas.cropping());
        canvas.start_crop();
        pump(50);
        assert!(!canvas.cropping(), "no crop mode, however it is asked for");
        assert!(unchanged());

        // The known objects stay editable: select all, nudge, undo.
        assert!(canvas.key_pressed(gdk::Key::a, gdk::ModifierType::CONTROL_MASK));
        assert_eq!(
            canvas.selection().len(),
            2,
            "the unknown object is not selectable"
        );
        assert!(canvas.key_pressed(gdk::Key::Right, gdk::ModifierType::empty()));
        pump(50);
        assert_eq!(
            canvas.history_info().undo_label.as_deref(),
            Some("Move 2 Objects")
        );
        // The canvas size stays available (it moves nothing).
        assert!(canvas
            .run(|d, _| annota::editing::image_ops::set_canvas_rect(
                d,
                annota::document::geometry::IRect::new(0, 0, 1800, 1000)
            ))
            .is_some());

        // A document without such objects: everything is available again.
        editor.open_file(&plain);
        pump(300);
        assert!(a.image.iter().all(|a| a.is_enabled()));
        assert!(a.flatten_all.is_enabled());
        assert!(w.crop_tool_button.is_sensitive());
        assert_eq!(
            w.crop_tool_button.tooltip_text().as_deref(),
            Some("Crop (C)")
        );
        for (button, tooltip) in &sidebar.image_op_buttons {
            assert!(button.is_sensitive());
            assert_eq!(button.tooltip_text(), *tooltip);
        }
        assert!(!editor.key_pressed(gdk::Key::r, gdk::ModifierType::CONTROL_MASK));
        let plain_canvas = editor.current_canvas().unwrap();
        let _ = WidgetExt::activate_action(&w.window, "win.rotate-right", None);
        pump(50);
        assert_eq!(
            plain_canvas.history_info().undo_label.as_deref(),
            Some("Rotate Right")
        );
        w.window.destroy();
        pump(50);
    }

    #[test]
    #[ignore = "opens a window; run under xvfb-run with GDK_BACKEND=x11"]
    fn editor_window_on_a_live_display() {
        let tmp = TempDir(
            std::env::temp_dir().join(format!("annota-editor-test-{}", std::process::id())),
        );
        let _ = std::fs::remove_dir_all(&tmp.0);
        std::fs::create_dir_all(tmp.0.join("captures")).unwrap();
        let app = environment(&tmp.0);
        let store = Store::new(tmp.0.join("autosave"));
        let editor = Editor::build(&app, store.clone(), style_paths(&tmp.0)).unwrap();
        // The application's editor window, as `get_or_create` would have registered it.
        EDITOR.with(|e| e.replace(Some(editor.inner.clone())));
        let settings = editor.inner.settings.clone();
        settings
            .set_string("save-folder", &tmp.0.join("captures").to_string_lossy())
            .unwrap();
        let w = &editor.inner.w;
        w.window.set_default_size(1280, 800);
        w.window.present();
        pump(200);

        // The zoom popover and the main menu show their shortcuts (mockups 07, 17).
        let zoom_button: gtk::MenuButton = w
            .zoom_level_label
            .ancestor(gtk::MenuButton::static_type())
            .and_downcast()
            .unwrap();
        let zoom_items = menu_accels(&zoom_button);
        for (label, accel) in [
            ("Fit to Window", "<Control>0"),
            ("100 %", "<Control>1"),
            ("Zoom In", "<Control>plus"),
            ("Zoom Out", "<Control>minus"),
        ] {
            assert!(
                zoom_items
                    .iter()
                    .any(|(l, a)| l == label && a.as_deref() == Some(accel)),
                "{label}: {zoom_items:?}"
            );
        }
        let main_items = menu_accels(&w.main_menu_button);
        for (label, accel) in [
            ("Flatten All", "<Shift><Control>t"),
            ("Rotate Right", "<Control>r"),
            ("Rotate Left", "<Shift><Control>r"),
        ] {
            assert!(
                main_items
                    .iter()
                    .any(|(l, a)| l == label && a.as_deref() == Some(accel)),
                "{label}: {main_items:?}"
            );
        }

        // Empty: title "Annota", no zoom OSD (mockup 14's window).
        assert_eq!(w.title.title(), "Annota");
        assert!(!w.zoom_osd.is_visible());

        // Open a saved document: title, clean subtitle, fit label, document sidebar.
        let file = tmp.0.join("captures/Checkout flow.annota");
        sample(&file);
        editor.open_file(&file);
        pump(300);
        assert_eq!(w.title.title(), "Checkout flow");
        assert_eq!(w.title.subtitle(), "1600 × 1000");
        assert!(w.zoom_level_label.text().starts_with("Fit · "));
        let sidebar = &editor.inner.sidebar;
        assert_eq!(
            sidebar.stack.visible_child_name().as_deref(),
            Some("document")
        );
        assert_eq!(sidebar.objects_row.title(), "2 editable objects");
        assert!(sidebar.properties_box.is_ancestor(&w.sidebar_scroller));
        // Tray: one open document; the open file is excluded from Recent as soon as it opens
        // (add_session refreshes Recent itself now — no manual refresh needed here).
        assert_eq!(editor.inner.tray.counts(), (1, 0));
        // Mockup 17: the image operations are live, with their shortcuts.
        assert!(editor.inner.actions.image.iter().all(|a| a.is_enabled()));

        // Select all images (the sidebar's per-type Select), then move them with X.
        let _ =
            WidgetExt::activate_action(&w.window, "win.select-type", Some(&"image".to_variant()));
        pump(50);
        assert_eq!(
            sidebar.stack.visible_child_name().as_deref(),
            Some("selection")
        );
        assert_eq!(sidebar.selection_heading.text(), "2 Images");
        sidebar.entries[0].set_text("100");
        sidebar.entries[0].emit_activate();
        pump(50);
        let canvas = editor.current_canvas().unwrap();
        assert_eq!(
            canvas.history_info().undo_label.as_deref(),
            Some("Move 2 Objects")
        );
        assert_eq!(w.title.subtitle(), "1600 × 1000 · Edited");
        assert!(editor.inner.actions.undo.is_enabled());
        assert!(!editor.inner.actions.redo.is_enabled());

        // Zoom popover: 200 % (mockup 07).
        let _ = WidgetExt::activate_action(&w.window, "win.zoom-preset", Some(&"200".to_variant()));
        pump(50);
        assert_eq!(w.zoom_level_label.text(), "200 %");
        assert_eq!(w.title.subtitle(), "1600 × 1000 · 200 %");
        assert_eq!(
            editor
                .inner
                .actions
                .zoom_preset
                .state()
                .and_then(|s| s.get::<String>())
                .as_deref(),
            Some("200")
        );
        let _ = WidgetExt::activate_action(&w.window, "win.zoom-fit", None);
        pump(50);
        assert!(canvas.fitted());

        // Narrow window (spec "Narrow window"): properties in a bottom sheet, no tray, no Copy.
        w.window.set_default_size(700, 800);
        pump(400);
        assert!(properties_in_sheet(&editor), "bottom sheet");
        assert!(w.sheet_revealer.reveals_child());
        assert!(!w.copy_button.is_visible());
        assert!(!editor.inner.tray.revealer.is_visible());
        w.window.set_default_size(1280, 800);
        pump(400);
        assert!(!properties_in_sheet(&editor), "back in the sidebar");
        assert!(w.split_view.shows_sidebar());
        assert!(w.copy_button.is_visible());

        // F9's action hides the sidebar and remembers it.
        let _ = WidgetExt::activate_action(&w.window, "win.show-properties", None);
        pump(50);
        assert!(!settings.boolean("show-properties"));
        assert!(!w.split_view.shows_sidebar());
        let _ = WidgetExt::activate_action(&w.window, "win.show-properties", None);

        // The sidebar switch and Ctrl+; are the same setting.
        assert!(canvas.snapping());
        assert!(canvas.key_pressed(gdk::Key::semicolon, gdk::ModifierType::CONTROL_MASK));
        pump(50);
        assert!(!settings.boolean("snap-to-guides"));
        assert!(!sidebar.snap_row.is_active());
        sidebar.snap_row.set_active(true);
        assert!(canvas.snapping());

        // A tray thumbnail dropped on the canvas: one more object, one undo step.
        let objects = canvas.with_document(|d| d.object_count()).unwrap();
        let uid = editor.current().unwrap().uid;
        assert!(editor.insert_from_tray(&TrayKey::Open(uid), 400.0, 300.0));
        assert_eq!(
            canvas.with_document(|d| d.object_count()),
            Some(objects + 1)
        );
        assert_eq!(
            canvas.history_info().undo_label.as_deref(),
            Some("Insert Image")
        );
        // Ctrl+Z takes it out again (spec "Picture in picture").
        assert!(canvas.key_pressed(gdk::Key::z, gdk::ModifierType::CONTROL_MASK));
        assert_eq!(canvas.with_document(|d| d.object_count()), Some(objects));
        assert!(canvas.key_pressed(
            gdk::Key::z,
            gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK
        ));
        assert_eq!(
            canvas.with_document(|d| d.object_count()),
            Some(objects + 1)
        );

        // Bug fix: tabbing through Arrange fields of a fractional box makes no undo step.
        let _ =
            WidgetExt::activate_action(&w.window, "win.select-type", Some(&"image".to_variant()));
        pump(50);
        sidebar.entries[2].set_text("123.45");
        sidebar.entries[2].emit_activate();
        pump(50);
        let steps = canvas.history_info().revision;
        for entry in &sidebar.entries {
            entry.grab_focus();
            pump(20);
            entry.emit_activate();
        }
        sidebar.rotation.grab_focus();
        pump(20);
        canvas.grab_focus();
        pump(50);
        assert_eq!(canvas.history_info().revision, steps, "no phantom edits");

        // B3: canvas extension from the keyboard, through the sidebar's Canvas entries.
        assert!(canvas.key_pressed(gdk::Key::Escape, gdk::ModifierType::empty()));
        pump(50);
        assert_eq!(
            sidebar.stack.visible_child_name().as_deref(),
            Some("document")
        );
        let before = canvas.with_document(|d| d.canvas().rect).unwrap();
        let texts = || {
            sidebar
                .canvas_entries
                .each_ref()
                .map(|e| e.text().to_string())
        };
        assert_eq!(texts(), ["0", "0", "0", "0"].map(String::from));
        // Tabbing through them changes nothing.
        let steps = canvas.history_info().revision;
        for entry in &sidebar.canvas_entries {
            entry.grab_focus();
            pump(20);
            entry.emit_activate();
        }
        canvas.grab_focus();
        pump(50);
        assert_eq!(
            canvas.history_info().revision,
            steps,
            "no phantom canvas edits"
        );
        // 200 px on the right: one undo step, the entry shows it, the canvas is wider.
        sidebar.canvas_entries[2].grab_focus();
        sidebar.canvas_entries[2].set_text("200");
        sidebar.canvas_entries[2].emit_activate();
        pump(50);
        assert_eq!(
            canvas.history_info().undo_label.as_deref(),
            Some("Extend Canvas")
        );
        assert_eq!(canvas.history_info().revision, steps + 1);
        let after = canvas.with_document(|d| d.canvas().rect).unwrap();
        assert_eq!((after.x, after.width), (before.x, before.width + 200));
        assert_eq!(texts(), ["0", "0", "200", "0"].map(String::from));
        // Into the image is refused and the real value comes back.
        sidebar.canvas_entries[0].set_text("-50");
        sidebar.canvas_entries[0].emit_activate();
        pump(50);
        assert_eq!(canvas.history_info().revision, steps + 1);
        assert_eq!(sidebar.canvas_entries[0].text(), "0");
        canvas.grab_focus();
        assert!(canvas.key_pressed(gdk::Key::z, gdk::ModifierType::CONTROL_MASK));
        pump(50);
        assert_eq!(canvas.with_document(|d| d.canvas().rect), Some(before));
        assert_eq!(texts(), ["0", "0", "0", "0"].map(String::from));

        // Context menu (mockup 05): right-clicking an unselected object selects it and opens
        // the menu; Ungroup is off without a group.
        let (first, centre) = canvas
            .with_document(|d| {
                let n = &d.scene().nodes()[0];
                (n.id, n.object.arrange().rect.centre())
            })
            .unwrap();
        canvas.key_pressed(gdk::Key::Escape, gdk::ModifierType::empty());
        assert!(canvas.selection().is_empty());
        canvas.context_menu_at(canvas.view_point(centre).unwrap());
        pump(100);
        assert_eq!(canvas.selection(), vec![first]);
        assert!(w.context_menu.is_visible());
        assert!(!editor.inner.actions.ungroup.is_enabled());
        w.context_menu.popdown();
        let _ =
            WidgetExt::activate_action(&w.window, "win.select-type", Some(&"image".to_variant()));
        pump(20);
        let _ = WidgetExt::activate_action(&w.window, "win.group", None);
        pump(50);
        assert_eq!(canvas.history_info().undo_label.as_deref(), Some("Group"));
        assert!(editor.inner.actions.ungroup.is_enabled());
        canvas.undo();
        pump(20);

        // Image operations (task 4.3): rotate right and back, one undo step each.
        let _ = WidgetExt::activate_action(&w.window, "win.rotate-right", None);
        pump(50);
        assert_eq!(w.title.subtitle(), "1000 × 1600 · Edited");
        assert_eq!(
            canvas.history_info().undo_label.as_deref(),
            Some("Rotate Right")
        );
        canvas.undo();
        pump(20);
        // Crop mode (mockup 08): C, the sidebar's crop page, a smaller frame, Enter.
        assert!(editor.key_pressed(gdk::Key::c, gdk::ModifierType::empty()));
        pump(50);
        assert!(canvas.cropping());
        assert!(w.crop_tool_button.is_active());
        assert_eq!(sidebar.stack.visible_child_name().as_deref(), Some("crop"));
        assert_eq!(w.title.subtitle(), "Crop · 1600 × 1000");
        assert!(!editor.inner.actions.crop_apply.is_enabled());
        sidebar.crop_entries[2].set_text("800");
        sidebar.crop_entries[2].emit_activate();
        pump(50);
        assert_eq!(w.title.subtitle(), "Crop · 800 × 1000");
        assert!(editor.inner.actions.crop_apply.is_enabled());
        assert!(canvas.key_pressed(gdk::Key::Return, gdk::ModifierType::empty()));
        pump(50);
        assert!(!canvas.cropping());
        assert!(w.select_tool_button.is_active());
        assert_eq!(w.title.subtitle(), "800 × 1000 · Edited");
        assert_eq!(canvas.history_info().undo_label.as_deref(), Some("Crop"));
        canvas.undo();
        pump(20);
        // Esc leaves crop mode without cropping.
        assert!(editor.key_pressed(gdk::Key::c, gdk::ModifierType::empty()));
        assert!(canvas.key_pressed(gdk::Key::Escape, gdk::ModifierType::empty()));
        pump(20);
        assert!(!canvas.cropping() && w.select_tool_button.is_active());
        // A palette swatch sets the canvas background.
        let coral = sidebar
            .swatches
            .iter()
            .find(|(s, _)| *s == sidebar::Swatch::Coral)
            .map(|(_, b)| b.clone())
            .unwrap();
        coral.emit_clicked();
        pump(20);
        assert_eq!(
            canvas.history_info().undo_label.as_deref(),
            Some("Canvas Background")
        );
        // Flatten All (mockup 18): no objects left, a toast with Undo.
        let _ = WidgetExt::activate_action(&w.window, "win.flatten-all", None);
        pump(50);
        assert_eq!(canvas.with_document(|d| d.object_count()), Some(0));
        assert_eq!(sidebar.objects_row.title(), "No editable objects");
        assert!(!editor.inner.actions.flatten_all.is_enabled());
        canvas.undo();
        canvas.undo();
        pump(20);

        // A recent capture in the save folder shows up and opens on click.
        let png = tmp.0.join("captures/Annota 2026-10-30.png");
        Bitmap::filled(320, 200, palette::TEAL)
            .unwrap()
            .surface()
            .write_to_png(&mut std::fs::File::create(&png).unwrap())
            .unwrap();
        editor.refresh_recent_tray();
        pump(300);
        assert_eq!(editor.inner.tray.counts(), (1, 1));
        editor.open_file(&png);
        pump(100);
        assert_eq!(editor.sessions().len(), 2);
        assert_eq!(w.title.subtitle(), "320 × 200 · Not saved");
        // Bug fix: the open image is no longer offered under Recent, and opening it again
        // switches to it instead of opening a duplicate.
        editor.refresh_recent_tray();
        pump(100);
        assert_eq!(editor.inner.tray.counts(), (2, 0));
        let first = editor.sessions()[0].uid;
        editor.switch_to(first);
        editor.open_file(&tmp.0.join("captures/./Annota 2026-10-30.png"));
        pump(50);
        assert_eq!(editor.sessions().len(), 2);
        assert_eq!(w.title.title(), "Annota 2026-10-30");

        // A new capture (task 4.5) opens as a new document, "Not saved", announced.
        assert!(open_capture(
            &app,
            Bitmap::filled(400, 300, palette::LIME).unwrap(),
            "Capture 2026-10-30 14-22-08",
            "Region captured · 400 × 300",
            Offer::Nothing,
        ));
        pump(100);
        // Nothing was left to recover when the editor opened, so no dialog (S2).
        assert!(w.window.visible_dialog().is_none());
        assert_eq!(editor.sessions().len(), 3);
        assert_eq!(w.title.title(), "Capture 2026-10-30 14-22-08");
        assert_eq!(w.title.subtitle(), "400 × 300 · Not saved");

        // Autosave: the edited document and the capture are written; the untouched image is
        // on disk already and is not.
        editor.autosave_now();
        let listing = store.list();
        assert_eq!(listing.entries.len(), 2);
        assert!(listing
            .entries
            .iter()
            .any(|e| e.name.as_deref() == Some("Checkout flow")
                && e.source.as_deref() == Some(file.as_path())
                && e.object_count == 3));

        // Restoring brings an autosave back as a document of its own.
        let entry = listing
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some("Checkout flow"))
            .unwrap()
            .clone();
        editor.restore(vec![entry]);
        pump(100);
        assert_eq!(editor.sessions().len(), 4);
        assert_eq!(w.title.title(), "Checkout flow");
        assert_eq!(w.title.subtitle(), "1600 × 1000 · Edited");

        // A newer major format: the alert of mockup 15, the file untouched.
        let newer = tmp.0.join("Release notes v2.annota");
        std::fs::copy(&file, &newer).unwrap();
        let bytes = rewrite_format(&newer, "2.0", "2.1");
        editor.open_file(&newer);
        pump(100);
        assert_eq!(editor.sessions().len(), 4);
        assert!(w.window.visible_dialog().is_some(), "alert shown");
        assert_eq!(std::fs::read(&newer).unwrap(), bytes);
        if let Some(dialog) = w.window.visible_dialog() {
            dialog.force_close();
        }

        // Closing keeps the autosaves (until Save exists): the restored document's slot and
        // the capture's are still there, and a change made after the last tick is written
        // when the window closes.
        let capture_canvas = editor
            .sessions()
            .iter()
            .find(|s| s.name() == "Capture 2026-10-30 14-22-08")
            .map(|s| s.canvas.clone())
            .unwrap();
        capture_canvas.run(|doc, _| image_ops::rotate(doc, Turn::Right));
        w.window.close();
        pump(100);
        let kept = store.list().entries;
        assert_eq!(kept.len(), 2, "closing keeps the autosaves");
        let capture = kept
            .iter()
            .find(|e| e.name.as_deref() == Some("Capture 2026-10-30 14-22-08"))
            .unwrap();
        let restored = store.restore(capture).unwrap();
        assert_eq!(
            restored.document.canvas().rect.width,
            300,
            "the last change"
        );
    }

    /// Rewrites `document.json`'s format fields of a `.annota` file; returns the new bytes.
    fn rewrite_format(path: &Path, format: &str, requires: &str) -> Vec<u8> {
        use std::io::{Read, Write};
        let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
        let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).unwrap();
            let name = entry.name().to_string();
            let mut data = Vec::new();
            entry.read_to_end(&mut data).unwrap();
            if name == "document.json" {
                let mut json: serde_json::Value = serde_json::from_slice(&data).unwrap();
                json["format"] = format.into();
                json["requires"] = requires.into();
                data = serde_json::to_vec(&json).unwrap();
            }
            out.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            out.write_all(&data).unwrap();
        }
        let bytes = out.finish().unwrap().into_inner();
        std::fs::write(path, &bytes).unwrap();
        bytes
    }

    #[test]
    #[ignore = "opens a window; run under xvfb-run with GDK_BACKEND=x11"]
    fn tool_strip_and_tool_keys_on_a_live_display() {
        use annota::document::annotation::PropertyValue;
        use annota::document::geometry::Point;

        let tmp =
            TempDir(std::env::temp_dir().join(format!("annota-tools-test-{}", std::process::id())));
        let _ = std::fs::remove_dir_all(&tmp.0);
        std::fs::create_dir_all(tmp.0.join("captures")).unwrap();
        let app = environment(&tmp.0);
        let editor = Editor::build(
            &app,
            Store::new(tmp.0.join("autosave")),
            style_paths(&tmp.0),
        )
        .unwrap();
        EDITOR.with(|e| e.replace(Some(editor.inner.clone())));
        let w = &editor.inner.w;
        w.window.set_default_size(1280, 800);
        w.window.present();
        pump(200);
        let first = tmp.0.join("captures/Checkout flow.annota");
        sample(&first);
        let second = tmp.0.join("captures/Second.annota");
        sample(&second);
        editor.open_file(&first);
        editor.open_file(&second);
        pump(300);
        let canvas = editor.current_canvas().unwrap();
        canvas.grab_focus();
        let none = gdk::ModifierType::empty();

        // D2's order, each tool with its key in the tooltip and its icon in the theme.
        let tools: Vec<Tool> = w.tool_buttons.iter().map(|(t, _)| *t).collect();
        assert_eq!(&tools, &Tool::ALL[1..]);
        let theme = gtk::IconTheme::for_display(&WidgetExt::display(&w.window));
        for (tool, button) in &w.tool_buttons {
            let tip = button.tooltip_text().unwrap_or_default();
            let key = tool.shortcut().to_ascii_uppercase();
            assert!(tip.ends_with(&format!("({key})")), "{tip}");
            assert!(button.is_visible() && button.is_sensitive());
            assert!(theme.has_icon(tool_icon(*tool)), "{}", tool_icon(*tool));
            assert_eq!(button.icon_name().as_deref(), Some(tool_icon(*tool)));
        }

        // Spec "Key switches tool": with the canvas focused, A makes the arrow tool active;
        // every tool's key likewise, and the strip follows.
        assert!(editor.key_pressed(gdk::Key::a, none));
        assert_eq!(canvas.tool(), Tool::Arrow);
        assert!(w.tool_buttons[0].1.is_active());
        for (tool, button) in &w.tool_buttons {
            let key = gdk::Key::from_name(tool.shortcut().to_string()).unwrap();
            assert!(editor.key_pressed(key, none), "{tool:?}");
            assert_eq!(canvas.tool(), *tool);
            assert!(button.is_active(), "{tool:?}");
        }
        assert!(editor.key_pressed(gdk::Key::v, none));
        assert!(w.select_tool_button.is_active());
        // Buttons switch too; Esc on the canvas goes back to Select.
        w.tool_buttons[1].1.set_active(true);
        assert_eq!(canvas.tool(), Tool::Line);
        assert!(editor.key_pressed(gdk::Key::Escape, none));
        assert_eq!(canvas.tool(), Tool::Select);
        assert!(w.select_tool_button.is_active());

        // The tool belongs to the window: another document draws with it too.
        assert!(editor.key_pressed(gdk::Key::p, none));
        let sessions = editor.sessions();
        editor.switch_to(sessions[0].uid);
        assert_eq!(editor.current_canvas().unwrap().tool(), Tool::Pen);
        editor.switch_to(sessions[1].uid);
        let canvas = editor.current_canvas().unwrap();

        // While typing in place, the tools' keys are letters.
        assert!(editor.key_pressed(gdk::Key::t, none));
        let at = canvas.view_point(Point::new(300.0, 300.0)).unwrap();
        canvas.pointer_pressed(at, Default::default());
        canvas.pointer_released();
        assert!(canvas.editing_text());
        assert!(
            !editor.key_pressed(gdk::Key::a, none),
            "goes on to the input method"
        );
        assert_eq!(canvas.tool(), Tool::Text);
        canvas.insert_text("a");
        assert!(editor.key_pressed(gdk::Key::Escape, none));
        assert!(!canvas.editing_text());
        assert!(w
            .tool_buttons
            .iter()
            .any(|(t, b)| *t == Tool::Text && b.is_active()));

        // Mockup 07: the subtitle says "Drawing" while a polygon is drawn.
        editor
            .inner
            .toolbox
            .borrow_mut()
            .set_property(
                Tool::Shape,
                "shape",
                &PropertyValue::Choice("polygon".into()),
            )
            .unwrap();
        assert!(editor.key_pressed(gdk::Key::u, none));
        let at = canvas.view_point(Point::new(500.0, 500.0)).unwrap();
        canvas.pointer_pressed(at, Default::default());
        canvas.pointer_released();
        pump(50);
        assert!(
            w.title.subtitle().ends_with("· Drawing"),
            "{}",
            w.title.subtitle()
        );
        assert!(editor.key_pressed(gdk::Key::Escape, none));
        pump(50);
        assert!(!w.title.subtitle().ends_with("· Drawing"));

        // Closing keeps text being typed (it is the document's).
        assert!(editor.key_pressed(gdk::Key::t, none));
        let at = canvas.view_point(Point::new(300.0, 600.0)).unwrap();
        canvas.pointer_pressed(at, Default::default());
        canvas.pointer_released();
        canvas.insert_text("kept");
        let texts = canvas
            .with_document(|d| d.scene().of_type("text").len())
            .unwrap();
        editor.close_all();
        assert_eq!(
            canvas
                .with_document(|d| d.scene().of_type("text").len())
                .unwrap(),
            texts + 1
        );
        w.window.destroy();
    }

    /// Saves `widget` as `<name>.png` into `$ANNOTA_SHOTS` (if set), for comparing with the
    /// mockups by eye.
    fn shot(widget: &impl IsA<gtk::Widget>, name: &str) {
        let Some(dir) = std::env::var_os("ANNOTA_SHOTS").filter(|d| !d.is_empty()) else {
            return;
        };
        pump(150);
        let widget = widget.as_ref();
        let paintable = gtk::WidgetPaintable::new(Some(widget));
        let snapshot = gtk::Snapshot::new();
        paintable.snapshot(
            &snapshot,
            f64::from(widget.width()),
            f64::from(widget.height()),
        );
        let (Some(node), Some(native)) = (snapshot.to_node(), widget.native()) else {
            return;
        };
        if let Some(renderer) = native.renderer() {
            let texture = renderer.render_texture(&node, None);
            let _ = texture.save_to_png(Path::new(&dir).join(format!("{name}.png")));
        }
    }

    #[test]
    #[ignore = "opens a window; run under xvfb-run with GDK_BACKEND=x11"]
    fn type_properties_on_a_live_display() {
        use annota::document::color::palette;
        use annota::document::geometry::Point;
        use annota::document::style::Shadow;
        use annota::editing::steps;
        use annota::tools::arrow::Arrow;
        use colour::Swatch;

        let tmp =
            TempDir(std::env::temp_dir().join(format!("annota-props-test-{}", std::process::id())));
        let _ = std::fs::remove_dir_all(&tmp.0);
        std::fs::create_dir_all(tmp.0.join("captures")).unwrap();
        let app = environment(&tmp.0);
        let editor = Editor::build(
            &app,
            Store::new(tmp.0.join("autosave")),
            style_paths(&tmp.0),
        )
        .unwrap();
        EDITOR.with(|e| e.replace(Some(editor.inner.clone())));
        let w = &editor.inner.w;
        w.window.set_default_size(1280, 800);
        w.window.present();
        pump(200);
        let file = tmp.0.join("captures/Backup settings.annota");
        sample(&file);
        editor.open_file(&file);
        pump(300);
        let canvas = editor.current_canvas().unwrap();
        canvas.grab_focus();
        let none = gdk::ModifierType::empty();
        let sidebar = &editor.inner.sidebar;
        let panel = &sidebar.panel;
        let at = |x: f64, y: f64| canvas.view_point(Point::new(x, y)).unwrap();
        let draw = |from: (f64, f64), to: (f64, f64)| {
            canvas.pointer_pressed(at(from.0, from.1), Default::default());
            canvas.pointer_moved(
                at((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0),
                Default::default(),
            );
            canvas.pointer_moved(at(to.0, to.1), Default::default());
            canvas.pointer_released();
            pump(30);
        };
        let click = |p: (f64, f64)| {
            canvas.pointer_pressed(at(p.0, p.1), Default::default());
            canvas.pointer_released();
            pump(30);
        };
        let revision = || canvas.history_info().revision;
        let undo_label = || canvas.history_info().undo_label;
        let first_object = |type_name: &str| {
            canvas
                .with_document(|d| {
                    let id = *d.scene().of_type(type_name).last().unwrap();
                    d.scene().get(id).unwrap().object.clone()
                })
                .unwrap()
        };
        let value = |object: &dyn annota::document::annotation::Annotation, key: &str| {
            ops::property(object, key)
        };
        let swatch = |key: &str, which: Swatch| {
            panel
                .colour_rows
                .iter()
                .find(|r| r.key == key)
                .and_then(|r| {
                    r.buttons
                        .borrow()
                        .iter()
                        .find(|(s, _)| *s == which)
                        .map(|(_, b)| b.clone())
                })
                .unwrap()
        };
        let spin = |key: &str| -> gtk::SpinButton {
            panel
                .spins
                .iter()
                .find(|(k, _, _)| *k == key)
                .map(|(_, s, _)| s.clone())
                .unwrap()
        };

        // Mockup 01: the arrow tool with nothing selected shows its defaults.
        assert!(editor.key_pressed(gdk::Key::a, none));
        pump(30);
        assert_eq!(
            sidebar.stack.visible_child_name().as_deref(),
            Some("selection")
        );
        // add-style-system: the tool's name moved to the quick styles' toggle; the heading of
        // the rows says "Properties" (mockup 01).
        assert_eq!(sidebar.selection_heading.text(), "Properties");
        assert_eq!(sidebar.quick.tool_label(), "Arrow");
        assert_eq!(sidebar.selection_caption.text(), "Tool defaults");
        assert!(!sidebar.arrange_box.is_visible());
        assert_eq!(
            panel.shown_rows(),
            [
                "row_color",
                "row_width",
                "row_ends",
                "row_line_style",
                "row_opacity",
                "row_shadow"
            ]
        );
        assert!(sidebar.select_all_type_button.is_visible());
        assert_eq!(
            sidebar.select_all_type_button.label().as_deref(),
            Some("Select All Arrows")
        );
        shot(&w.window, "01-tool-defaults");
        // A default changes no document.
        let before = revision();
        spin("width").set_value(12.0);
        assert_eq!(revision(), before);
        assert_eq!(
            editor
                .inner
                .toolbox
                .borrow()
                .properties(Tool::Arrow)
                .into_iter()
                .find(|p| p.key == "width")
                .map(|p| p.value),
            Some(PropertyValue::Number(12.0))
        );

        // Mockup 02: a drawn arrow has it, and is "1 selected".
        draw((400.0, 300.0), (900.0, 500.0));
        assert_eq!(sidebar.selection_caption.text(), "1 selected");
        assert!(sidebar.arrange_box.is_visible());
        assert_eq!(spin("width").value(), 12.0);
        assert_eq!(
            value(first_object("arrow").as_ref(), "width"),
            Some(PropertyValue::Number(12.0))
        );
        shot(&w.window, "02-arrow-selected");
        // A swatch: one "Change Arrow" step; the same swatch again: none.
        let before = revision();
        swatch("color", Swatch::Colour(palette::AZURE)).emit_clicked();
        pump(30);
        assert_eq!(revision(), before + 1);
        assert_eq!(undo_label().as_deref(), Some("Change Arrow"));
        assert_eq!(
            value(first_object("arrow").as_ref(), "color"),
            Some(PropertyValue::Color(palette::AZURE))
        );
        assert!(
            swatch("color", Swatch::Colour(palette::AZURE)).has_css_class("annota-swatch-current")
        );
        swatch("color", Swatch::Colour(palette::AZURE)).emit_clicked();
        pump(30);
        assert_eq!(revision(), before + 1, "no phantom step");
        // Leaving a spin re-reads its text: no step either.
        spin("width").update();
        sidebar.panel.end_tiles[1].1.set_active(true); // already Triangle
        pump(30);
        assert_eq!(revision(), before + 1);
        // Double-ended (spec scenario): start Triangle.
        panel.start_tiles[1].1.set_active(true);
        pump(30);
        assert_eq!(
            value(first_object("arrow").as_ref(), "start_style"),
            Some(PropertyValue::Choice("triangle".into()))
        );
        assert_eq!(revision(), before + 2);
        // A slider dragged: previewed on the canvas, one step when let go.
        let before = revision();
        panel.slide("opacity", &[90.0, 70.0, 50.0]);
        pump(30);
        assert_eq!(revision(), before + 1);
        assert_eq!(
            value(first_object("arrow").as_ref(), "opacity"),
            Some(PropertyValue::Number(0.5))
        );
        assert!(canvas.previewing().is_none());
        // A focused slider keeps its arrow keys (the selection is not nudged).
        panel.opacity.scale.grab_focus();
        assert!(!editor.routes_to_canvas(gdk::Key::Left));
        canvas.grab_focus();
        // Spec "One undo step per change": one undo brings back 100 %.
        canvas.undo();
        assert_eq!(
            value(first_object("arrow").as_ref(), "opacity"),
            Some(PropertyValue::Number(1.0))
        );
        canvas.redo();
        pump(30);
        // Let go where it started: nothing.
        panel.slide("opacity", &[60.0, 50.0]);
        assert_eq!(revision(), before + 1);
        // Mid-drag the canvas previews and the document is unchanged.
        panel.opacity.sliding.set(true);
        panel.opacity.scale.set_value(30.0);
        assert!(canvas.previewing().is_some());
        assert_eq!(revision(), before + 1);
        panel.slide("opacity", &[50.0]);
        assert!(canvas.previewing().is_none());
        assert_eq!(revision(), before + 1);

        // Shadow editor (mockup 18).
        let before = revision();
        panel.shadow.row.set_expanded(true);
        pump(30);
        assert!(panel.shadow.reset.is_visible());
        shot(&w.window, "18-shadow-editor");
        assert_eq!(panel.shadow.blur.scale.value(), 12.0, "blur 12 px shown");
        panel.slide("shadow_distance", &[20.0]);
        pump(30);
        let shadow = |o: &dyn annota::document::annotation::Annotation| match value(o, "shadow") {
            Some(PropertyValue::Shadow(s)) => s,
            other => panic!("{other:?}"),
        };
        assert_eq!(shadow(first_object("arrow").as_ref()).distance, 20.0);
        panel.shadow.angle.set_value(90.0);
        pump(30);
        assert_eq!(shadow(first_object("arrow").as_ref()).angle, 90.0);
        panel.shadow.reset.emit_clicked();
        pump(30);
        assert_eq!(shadow(first_object("arrow").as_ref()), Shadow::DEFAULT);
        panel.shadow.row.set_enable_expansion(false);
        pump(30);
        assert!(!shadow(first_object("arrow").as_ref()).enabled);
        assert_eq!(revision(), before + 4);
        // The slider cannot reach an out-of-range blur (B3).
        assert_eq!(panel.shadow.blur.scale.adjustment().upper(), 30.0);

        // The colour picker (mockup 16): a typed hex value previews, closing commits it and
        // remembers it as a recent colour.
        let before = revision();
        swatch("color", Swatch::Custom).emit_clicked();
        pump(50);
        assert!(panel.picker.popover.is_visible());
        panel.picker.hex.set_text("#2F6FD0");
        pump(30);
        assert!(canvas.previewing().is_some());
        assert_eq!(panel.picker.channels[2].text(), "208");
        shot(&panel.picker.popover, "16-colour-picker");
        panel.picker.popover.popdown();
        pump(50);
        let blue = Rgba::opaque(0x2F, 0x6F, 0xD0);
        assert_eq!(revision(), before + 1);
        assert_eq!(
            value(first_object("arrow").as_ref(), "color"),
            Some(PropertyValue::Color(blue))
        );
        assert_eq!(panel.recent(), vec![blue]);
        assert!(swatch("color", Swatch::Colour(blue)).has_css_class("annota-swatch-current"));

        // The eyedropper (mockup 17), with a stand-in for the portal: while it waits the
        // swatch is filled and the subtitle and chip say what to do; then the colour is used.
        let (send, receive) = futures_channel::oneshot::channel::<
            Result<Rgba, crate::capture::portal::CaptureError>,
        >();
        let receive = std::cell::RefCell::new(Some(receive));
        editor.set_screen_picker(Rc::new(move || {
            let receive = receive.borrow_mut().take();
            Box::pin(async move {
                match receive {
                    Some(r) => r
                        .await
                        .unwrap_or(Err(crate::capture::portal::CaptureError::Cancelled)),
                    None => Err(crate::capture::portal::CaptureError::Cancelled),
                }
            })
        }));
        let before = revision();
        swatch("color", Swatch::Eyedropper).emit_clicked();
        pump(50);
        assert!(w.pick_chip.is_visible());
        assert_eq!(w.title.subtitle(), "Pick a colour anywhere on the screen");
        assert!(swatch("color", Swatch::Eyedropper).has_css_class("annota-swatch-picking"));
        shot(&w.window, "17-eyedropper");
        let picked = Rgba::opaque(0x35, 0x84, 0xE4);
        send.send(Ok(picked)).unwrap();
        pump(100);
        assert!(!w.pick_chip.is_visible());
        assert_ne!(w.title.subtitle(), "Pick a colour anywhere on the screen");
        assert_eq!(revision(), before + 1);
        assert_eq!(
            value(first_object("arrow").as_ref(), "color"),
            Some(PropertyValue::Color(picked))
        );
        assert_eq!(panel.recent(), vec![picked, blue]);
        // Cancelled: nothing changes.
        editor.set_screen_picker(Rc::new(|| {
            Box::pin(async { Err(crate::capture::portal::CaptureError::Cancelled) })
        }));
        swatch("color", Swatch::Eyedropper).emit_clicked();
        pump(100);
        assert_eq!(revision(), before + 1);
        assert!(!w.pick_chip.is_visible());

        // Spec "Style exceptions": highlights have no shadow row, redactions neither opacity
        // nor shadow.
        assert!(editor.key_pressed(gdk::Key::h, none));
        draw((100.0, 700.0), (500.0, 760.0));
        assert!(!panel.shadow.row.is_visible() || !panel.shown_rows().contains(&"row_shadow"));
        assert_eq!(
            panel.shown_rows(),
            ["row_color", "row_opacity", "help_highlighter"]
        );
        assert!(editor.key_pressed(gdk::Key::b, none));
        draw((600.0, 700.0), (900.0, 760.0));
        assert_eq!(
            panel.shown_rows(),
            [
                "row_redact_type",
                "row_intensity",
                "row_redact_shape",
                "row_color",
                "warning_redact"
            ]
        );
        shot(&w.window, "15-redact");
        panel.redact_type[2].1.set_active(true);
        pump(30);
        assert!(!panel.shown_rows().contains(&"warning_redact"));
        assert!(!panel.rows["row_intensity"].is_sensitive());

        // The shape type reaches polygons (closing phase A's gap): Polygon in the tool defaults
        // makes the shape tool draw vertex by vertex.
        assert!(editor.key_pressed(gdk::Key::u, none));
        // A click (no drag) makes no shape and leaves nothing selected.
        click((1500.0, 950.0));
        assert_eq!(sidebar.selection_caption.text(), "Tool defaults");
        panel.shape_kind[3].1.set_active(true);
        pump(30);
        assert!(editor.inner.toolbox.borrow().draws_polygons());
        click((1100.0, 100.0));
        click((1400.0, 120.0));
        click((1300.0, 400.0));
        assert!(canvas.drawing_polygon());
        assert_eq!(sidebar.selection_caption.text(), "Drawing polygon");
        assert_eq!(panel.shown_rows().last(), Some(&"help_polygon"));
        shot(&w.window, "07-polygon");
        assert!(editor.key_pressed(gdk::Key::Return, none));
        assert_eq!(sidebar.selection_caption.text(), "1 selected");

        // B1: the Sequence of a selected marker recounts its whole sequence, one step.
        assert!(editor.key_pressed(gdk::Key::s, none));
        for x in [200.0, 300.0, 400.0] {
            click((x, 900.0));
        }
        pump(30);
        assert_eq!(panel.shown_rows()[..2], ["row_sequence", "row_step_shape"]);
        assert!(
            panel.shown_rows().contains(&"row_value"),
            "one marker selected"
        );
        shot(&w.window, "12-step-selected");
        let before = revision();
        panel.sequence.set_selected(1);
        pump(30);
        assert_eq!(revision(), before + 1);
        let labels = canvas
            .with_document(|d| {
                steps::markers(d.scene())
                    .iter()
                    .map(|s| s.label())
                    .collect::<Vec<_>>()
            })
            .unwrap();
        assert_eq!(labels, ["A", "B", "C"]);
        // The one marker's own value.
        panel.value_entry.set_text("7");
        panel.value_entry.emit_activate();
        pump(30);
        let labels = canvas
            .with_document(|d| {
                steps::markers(d.scene())
                    .iter()
                    .map(|s| s.label())
                    .collect::<Vec<_>>()
            })
            .unwrap();
        assert_eq!(labels, ["A", "B", "7"]);
        // Restart Sequence and Next value in the tool defaults (mockup 11). (Esc: back to
        // Select, then nothing selected.)
        assert!(editor.key_pressed(gdk::Key::Escape, none));
        assert!(editor.key_pressed(gdk::Key::Escape, none));
        assert!(editor.key_pressed(gdk::Key::s, none));
        pump(30);
        assert_eq!(sidebar.selection_caption.text(), "Tool defaults");
        assert_eq!(
            panel.sequence.selected(),
            1,
            "the current sequence counts A, B, C"
        );
        assert_eq!(panel.next_value.value(), 4.0);
        shot(&w.window, "11-step-defaults");
        panel.restart.emit_clicked();
        pump(30);
        assert_eq!(panel.next_value.value(), 1.0);
        click((600.0, 900.0));
        let labels = canvas
            .with_document(|d| {
                steps::markers(d.scene())
                    .iter()
                    .map(|s| s.label())
                    .collect::<Vec<_>>()
            })
            .unwrap();
        assert_eq!(labels, ["A", "B", "7", "1"]);
        // The tool defaults' Sequence is the current (restarted) sequence's: it recounts it as
        // one step and new markers follow; the first sequence keeps A, B.
        assert!(editor.key_pressed(gdk::Key::Escape, none));
        assert!(editor.key_pressed(gdk::Key::Escape, none));
        assert!(editor.key_pressed(gdk::Key::s, none));
        pump(30);
        assert_eq!(sidebar.selection_caption.text(), "Tool defaults");
        assert_eq!(panel.sequence.selected(), 0);
        let before = revision();
        panel.sequence.set_selected(3);
        pump(30);
        assert_eq!(revision(), before + 1);
        click((700.0, 900.0));
        let labels = canvas
            .with_document(|d| {
                steps::markers(d.scene())
                    .iter()
                    .map(|s| s.label())
                    .collect::<Vec<_>>()
            })
            .unwrap();
        assert_eq!(labels, ["A", "B", "7", "I", "II"]);

        // Text being typed (mockup 08): "Editing"; the sidebar's Bold is part of the one step.
        assert!(editor.key_pressed(gdk::Key::t, none));
        click((300.0, 600.0));
        assert!(canvas.editing_text());
        canvas.insert_text("Runs nightly");
        pump(30);
        assert_eq!(sidebar.selection_caption.text(), "Editing");
        assert_eq!(panel.shown_rows()[..2], ["row_font", "row_text_style"]);
        shot(&w.window, "08-text-editing");
        let before = revision();
        let bold = &panel.flags[0].1;
        assert!(bold.is_active(), "Text defaults to bold");
        bold.set_active(false);
        pump(30);
        assert_eq!(revision(), before, "no step while typing");
        assert!(editor.key_pressed(gdk::Key::Escape, none));
        assert_eq!(revision(), before + 1);
        assert_eq!(undo_label().as_deref(), Some("Add Text"));
        assert_eq!(
            value(first_object("text").as_ref(), "bold"),
            Some(PropertyValue::Bool(false))
        );
        let _ = Arrow::TYPE;
        window_done(&w.window);
    }

    const HARBOR: &str = r##"{"format": "1.0", "requires": "0.1", "name": "Harbor Corporate",
        "colours": ["#14324F", "#3FA7D6", "#F0C808", "#D1495B"],
        "fonts": {"heading": "DejaVu Serif", "body": "DejaVu Sans"}}"##;

    /// An editor window in `tmp` with one document open and Harbor Corporate installed "by an
    /// administrator" (the test's system theme directory).
    fn styles_editor(tmp: &Path) -> Editor {
        std::fs::create_dir_all(tmp.join("captures")).unwrap();
        std::fs::create_dir_all(tmp.join("system-themes")).unwrap();
        std::fs::write(tmp.join("system-themes/harbor.annota-theme"), HARBOR).unwrap();
        let app = environment(tmp);
        let editor =
            Editor::build(&app, Store::new(tmp.join("autosave")), style_paths(tmp)).unwrap();
        EDITOR.with(|e| e.replace(Some(editor.inner.clone())));
        let w = &editor.inner.w;
        w.window.set_default_size(1280, 800);
        w.window.present();
        pump(200);
        let file = tmp.join("captures/Checkout flow.annota");
        sample(&file);
        editor.open_file(&file);
        pump(300);
        editor
    }

    fn tool_default(editor: &Editor, tool: Tool, key: &str) -> Option<PropertyValue> {
        editor
            .inner
            .toolbox
            .borrow()
            .properties(tool)
            .into_iter()
            .find(|p| p.key == key)
            .map(|p| p.value)
    }

    /// add-style-system tasks 2.1, 2.2 and the theme switcher: the quick style grid, Save as
    /// Quick Style, the context menu and its keys, Favourites, switching themes.
    #[test]
    #[ignore = "opens a window; run under xvfb-run with GDK_BACKEND=x11"]
    fn quick_styles_on_a_live_display() {
        use annota::document::color::palette;
        use annota::document::geometry::Point;
        use annota::styles::library::Library;

        let tmp =
            TempDir(std::env::temp_dir().join(format!("annota-quick-test-{}", std::process::id())));
        let _ = std::fs::remove_dir_all(&tmp.0);
        let editor = styles_editor(&tmp.0);
        let w = &editor.inner.w;
        let canvas = editor.current_canvas().unwrap();
        canvas.grab_focus();
        let none = gdk::ModifierType::empty();
        let sidebar = &editor.inner.sidebar;
        let quick = &sidebar.quick;
        let at = |x: f64, y: f64| canvas.view_point(Point::new(x, y)).unwrap();
        let revision = || canvas.history_info().revision;
        let library = || {
            Library::parse(&std::fs::read_to_string(tmp.0.join("styles/styles.json")).unwrap())
                .unwrap()
        };

        // Mockup 01: the arrow tool: "Arrow | Favourites", eight theme styles, the first one
        // (the built-in default) selected, nothing modified, nothing to save.
        assert!(editor.key_pressed(gdk::Key::a, none));
        pump(50);
        assert_eq!(quick.tool_label(), "Arrow");
        let tiles = quick.tiles();
        assert_eq!(tiles.len(), 8, "no user styles yet");
        assert!(tiles[0].1 && tiles.iter().skip(1).all(|t| !t.1));
        assert_eq!(tiles[0].0, "Coral · 8 px · shadow");
        assert!(!quick.modified_shown() && !quick.save_enabled());
        assert_eq!(sidebar.selection_caption.text(), "Tool defaults");
        shot(&w.window, "s01-quick-style-grid");

        // Scenario "Save style" (mockups 02, 03): Teal and 12 px → Modified, Save enabled;
        // the new style is the last tile, selected, with the halo and a toast with Undo.
        canvas.set_property("color", PropertyValue::Color(palette::TEAL));
        canvas.set_property("width", PropertyValue::Number(12.0));
        pump(50);
        assert!(quick.modified_shown() && quick.save_enabled());
        assert!(quick.tiles().iter().all(|t| !t.1), "no tile matches");
        let (name, summary, place) = quick.open_save();
        assert_eq!(name, "Teal bold");
        assert_eq!(summary, "Teal · 12 px · shadow");
        assert_eq!(place, "Added to the end of Arrow");
        pump(50);
        shot(&w.window, "s02-save-as-quick-style");
        quick.save();
        pump(50);
        let tiles = quick.tiles();
        assert_eq!(tiles.len(), 9);
        assert!(tiles[8].0.starts_with("Teal bold\n"));
        assert!(tiles[8].1, "selected");
        assert!(tiles[8].2, "Add to Favourites was checked");
        assert!(quick.has_halo(8));
        assert!(!quick.modified_shown() && !quick.save_enabled());
        assert!(shows_label(
            w.toast_overlay.upcast_ref(),
            "Quick style “Teal bold” saved"
        ));
        assert_eq!(
            library().styles(Tool::Arrow).len(),
            1,
            "saved to styles.json"
        );
        shot(&w.window, "s03-style-saved");
        // Mockup 03: Delete in the destructive colour, the other items not.
        let delete = quick.menu_item_colour(8, "Delete").unwrap();
        let apply = quick.menu_item_colour(8, "Apply").unwrap();
        // Red (light: #C30000, dark: a light red), the others neutral.
        assert!(delete.red() - delete.green() > 0.3, "{delete}");
        assert!((apply.red() - apply.green()).abs() < 0.1, "{apply}");
        assert!(
            quick.menu_item_colour(0, "Delete").is_none(),
            "theme styles have no Delete"
        );
        // The Menu key and Shift+F10 open it on the focused tile.
        assert!(quick.press_on(8, gdk::Key::Menu, none));
        assert!(quick.menu_shown());
        assert!(quick.press_on(0, gdk::Key::F10, gdk::ModifierType::SHIFT_MASK));
        assert!(quick.menu_shown());
        pump(2100);
        assert!(!quick.has_halo(8), "the halo fades after 2 s");

        // A theme tile applies to the tool defaults; no document step.
        let before = revision();
        quick.activate(2);
        pump(50);
        assert_eq!(revision(), before);
        assert_eq!(
            tool_default(&editor, Tool::Arrow, "color"),
            Some(PropertyValue::Color(palette::LIME))
        );
        assert!(quick.tiles()[2].1);

        // On a selection: one undo step for all the style's properties.
        let draw = |from: (f64, f64), to: (f64, f64)| {
            canvas.pointer_pressed(at(from.0, from.1), Default::default());
            canvas.pointer_moved(at(to.0, to.1), Default::default());
            canvas.pointer_released();
            pump(30);
        };
        draw((400.0, 300.0), (900.0, 500.0));
        assert_eq!(sidebar.selection_caption.text(), "1 selected");
        let before = revision();
        quick.activate(8);
        pump(50);
        assert_eq!(revision(), before + 1);
        assert_eq!(canvas.undo_label().as_deref(), Some("Apply Style to Arrow"));
        let arrow = || {
            canvas
                .with_document(|d| {
                    let id = *d.scene().of_type("arrow").last().unwrap();
                    d.scene().get(id).unwrap().object.clone()
                })
                .unwrap()
        };
        assert_eq!(
            ops::property(arrow().as_ref(), "color"),
            Some(PropertyValue::Color(palette::TEAL))
        );
        assert_eq!(
            ops::property(arrow().as_ref(), "width"),
            Some(PropertyValue::Number(12.0))
        );
        // Already in use: no phantom step.
        quick.activate(8);
        pump(30);
        assert_eq!(revision(), before + 1);
        canvas.undo();
        pump(30);
        assert_eq!(
            ops::property(arrow().as_ref(), "color"),
            Some(PropertyValue::Color(palette::LIME)),
            "one undo restores every property"
        );

        // A second user style, then the keyboard (redline): F2 renames, Ctrl+← moves, Delete
        // deletes (toast with Undo) — and none of these keys reach the canvas's selection.
        quick.menu_action(8, "apply");
        pump(30);
        canvas.set_property("width", PropertyValue::Number(4.0));
        pump(30);
        let (name, _, _) = quick.open_save();
        assert_eq!(name, "Teal thin");
        quick.save();
        pump(50);
        assert_eq!(quick.tiles().len(), 10);
        let objects = || canvas.with_document(|d| d.scene().len()).unwrap();
        let count = objects();
        assert!(quick.press_on(9, gdk::Key::F2, none));
        quick.rename_to("Fine teal");
        pump(30);
        assert!(quick.tiles()[9].0.starts_with("Fine teal\n"));
        // Keys on a focused tile stay with the grid.
        assert!(!editor.routes_to_canvas(gdk::Key::Delete));
        assert!(!editor.routes_to_canvas(gdk::Key::Left));
        assert!(!editor.routes_to_canvas(gdk::Key::Menu));
        assert!(quick.press_on(9, gdk::Key::Left, gdk::ModifierType::CONTROL_MASK));
        pump(30);
        let names: Vec<String> = library()
            .styles(Tool::Arrow)
            .iter()
            .map(|s| s.name.clone())
            .collect();
        assert_eq!(names, ["Fine teal", "Teal bold"]);
        assert!(!quick.menu_action(8, "move-left"), "disabled at the start");
        assert!(quick.press_on(8, gdk::Key::Delete, none));
        pump(50);
        assert_eq!(objects(), count, "the selection was not deleted");
        assert_eq!(library().styles(Tool::Arrow).len(), 1);
        assert!(shows_label(
            w.toast_overlay.upcast_ref(),
            "Quick style “Fine teal” deleted"
        ));
        // The toast's Undo puts it back where it was.
        pump(300);
        assert!(click_button(w.toast_overlay.upcast_ref(), "Undo"));
        pump(50);
        let names: Vec<String> = library()
            .styles(Tool::Arrow)
            .iter()
            .map(|s| s.name.clone())
            .collect();
        assert_eq!(names, ["Fine teal", "Teal bold"]);
        assert_eq!(quick.tiles().len(), 10);
        // Delete again for what follows.
        assert!(quick.press_on(8, gdk::Key::Delete, none));
        pump(50);
        // Theme tiles have no Rename/Delete/Move; Update from Current Properties on a user one.
        assert!(!quick.menu_action(0, "delete"));
        assert!(!quick.menu_action(0, "rename"));
        assert!(!quick.press_on(0, gdk::Key::Delete, none));
        canvas.set_property("width", PropertyValue::Number(20.0));
        pump(30);
        assert!(quick.menu_action(8, "update"));
        pump(30);
        assert!(quick.tiles()[8].1 && !quick.modified_shown());

        // Favourites (mockups 04, 05): star a callout theme style; the view groups by tool.
        assert!(editor.key_pressed(gdk::Key::Escape, none));
        assert!(editor.key_pressed(gdk::Key::Escape, none));
        assert!(canvas.selection().is_empty());
        assert!(editor.key_pressed(gdk::Key::k, none));
        pump(30);
        assert_eq!(quick.tool_label(), "Callout");
        assert!(quick.menu_action(3, "star"));
        assert!(!quick.menu_action(3, "star"), "already starred");
        assert!(quick.tiles()[3].2);
        assert!(editor.key_pressed(gdk::Key::a, none));
        draw((300.0, 700.0), (600.0, 800.0));
        quick.show_favourites(true);
        pump(50);
        let (visible, groups) = quick.favourites();
        assert!(visible && !sidebar.stack.is_visible());
        assert_eq!(
            groups,
            [("Arrow".to_string(), 1), ("Callout".to_string(), 1)]
        );
        shot(&w.window, "s04-favourites");
        // Scenario "Use favourite": the callout tool becomes active with that style; the arrow
        // selection is let go.
        quick.activate_favourite(1);
        pump(50);
        assert_eq!(editor.inner.tool.get(), Tool::Callout);
        assert!(canvas.selection().is_empty());
        assert_eq!(quick.tool_label(), "Callout");
        assert!(sidebar.stack.is_visible(), "back to the tool's page");
        assert!(quick.tiles()[3].1, "the favourite's tile is selected");
        assert_eq!(
            tool_default(&editor, Tool::Callout, "fill"),
            Some(PropertyValue::Color(palette::TEAL))
        );
        shot(&w.window, "s05-favourite-applied");
        // Nothing starred: only the hint.
        let favourites: Vec<_> = editor
            .inner
            .styles
            .library()
            .favourites()
            .into_iter()
            .cloned()
            .collect();
        for f in favourites {
            editor.inner.styles.change(|l| l.set_favourite(&f, false));
        }
        quick.show_favourites(true);
        pump(30);
        let (visible, groups) = quick.favourites();
        assert!(visible && groups.is_empty());
        quick.show_favourites(false);

        // The theme switcher (mockup 07): built-in, the administrator's, a check on the current
        // one. Picking Harbor colours every tool's theme styles and the palette swatches, and
        // sets the tool defaults; user styles stay.
        assert_eq!(
            quick.theme_rows(),
            ["annota", "monochrome", "system/harbor"]
        );
        assert!(editor.key_pressed(gdk::Key::a, none));
        pump(30);
        quick.pick_theme(2);
        pump(50);
        let navy = annota::document::color::Rgba::opaque(0x14, 0x32, 0x4F);
        assert_eq!(editor.inner.styles.current_theme().id, "system/harbor");
        assert_eq!(
            tool_default(&editor, Tool::Arrow, "color"),
            Some(PropertyValue::Color(navy))
        );
        assert_eq!(
            tool_default(&editor, Tool::Step, "color"),
            Some(PropertyValue::Color(navy))
        );
        assert_eq!(
            tool_default(&editor, Tool::Text, "font_family"),
            Some(PropertyValue::Text("DejaVu Serif".into()))
        );
        assert_eq!(sidebar.panel.palette.borrow()[0], navy);
        let tiles = quick.tiles();
        assert!(tiles[0].1, "the new defaults are the theme's first style");
        assert_eq!(tiles.len(), 9, "the user's style stays");
        assert_eq!(library().theme, "system/harbor");
        shot(&w.window, "s07-theme-applied");
        window_done(&w.window);
    }

    /// add-style-system tasks 2.3, 2.4: the theme editor, managed themes, import/export.
    #[test]
    #[ignore = "opens a window; run under xvfb-run with GDK_BACKEND=x11"]
    fn theme_editor_on_a_live_display() {
        use annota::document::color::Rgba;
        use annota::styles::store;

        let tmp =
            TempDir(std::env::temp_dir().join(format!("annota-theme-test-{}", std::process::id())));
        let _ = std::fs::remove_dir_all(&tmp.0);
        let editor = styles_editor(&tmp.0);
        let w = &editor.inner.w;
        let styles = editor.inner.styles.clone();
        let themes = theme_editor::present(&w.window, styles.clone()).unwrap();
        pump(200);
        assert_eq!(themes.list(), ["annota", "monochrome", "system/harbor"]);
        assert_eq!(themes.selected(), "annota");
        // (banner, name editable, fonts, Add tile, More menu, Use Theme)
        assert_eq!(
            themes.detail(),
            (false, false, false, false, true, false),
            "Annota in use"
        );
        shot(&w.window, "s06-theme-editor");

        // Scenario "Managed theme" (mockup 08): read-only, banner, no Add, no More menu.
        themes.show_theme("system/harbor");
        pump(50);
        assert_eq!(themes.detail(), (true, false, false, false, false, true));
        shot(&w.window, "s08-managed-theme");
        // Duplicate makes an editable copy under My Themes.
        themes.duplicate_for_test();
        pump(50);
        assert_eq!(themes.selected(), "user/harbor-corporate-copy");
        assert_eq!(themes.detail(), (false, true, true, true, true, true));
        let theme = || styles.theme(&themes.selected()).unwrap();
        assert_eq!(theme().name, "Harbor Corporate Copy");
        themes.rename("Northwind Ferries");
        pump(30);
        assert_eq!(theme().name, "Northwind Ferries");
        // Palette: edit, add (up to 8: the Add tile goes), remove (down to 2), reorder.
        let blue = Rgba::opaque(0x0B, 0x5C, 0xAD);
        themes.edit_colour(0, blue);
        pump(30);
        assert_eq!(theme().colours[0], blue);
        for _ in 0..4 {
            themes.add_colour();
            pump(20);
        }
        assert_eq!(theme().colours.len(), 8);
        assert!(!themes.detail().3, "no Add tile at 8 colours");
        themes.move_colour(1, 1);
        pump(20);
        assert_eq!(theme().colours[2], Rgba::opaque(0x3F, 0xA7, 0xD6));
        for _ in 0..7 {
            themes.remove_colour(0);
            pump(20);
        }
        assert_eq!(theme().colours.len(), 2, "never fewer than 2");
        // Use Theme: the sidebar's quick styles follow; "In use" replaces the button.
        themes.use_theme();
        pump(50);
        assert_eq!(styles.current_theme().id, "user/harbor-corporate-copy");
        assert!(!themes.detail().5);
        // Export, then import: the same theme comes back as a new one.
        let out = tmp.0.join("northwind.annota-theme");
        themes.export(&theme(), &out);
        let exported = store::check(&out).unwrap();
        assert_eq!(exported.name, "Northwind Ferries");
        themes.import(&out);
        pump(50);
        // Its name is taken (the theme it was exported from): told apart by a number.
        assert_eq!(themes.selected(), "user/northwind-ferries-2");
        assert!(shows_label(
            themes.widget().upcast_ref(),
            "Theme “Northwind Ferries 2” imported"
        ));
        shot(&w.window, "s09-theme-imported");
        // Mockup 10: an invalid file gets the alert with its reason.
        let bad = tmp.0.join("brand-2024.annota-theme");
        let eleven: Vec<String> = (0..11).map(|i| format!("\"#1010{i:02}\"")).collect();
        std::fs::write(
            &bad,
            format!(
                r#"{{"format": "1.0", "name": "Brand", "colours": [{}], "fonts": {{"heading": "a", "body": "b"}}}}"#,
                eleven.join(", ")
            ),
        )
        .unwrap();
        themes.import(&bad);
        pump(100);
        let alert = w
            .window
            .visible_dialog()
            .and_downcast::<adw::AlertDialog>()
            .unwrap();
        assert_eq!(alert.heading().as_deref(), Some("Can’t Import Theme"));
        assert_eq!(
            alert.body().as_str(),
            "“brand-2024.annota-theme” is not a valid Annota theme. It contains 11 colours; a theme needs 2–8."
        );
        shot(&w.window, "s10-import-error");
        alert.force_close();
        pump(50);
        // New theme: from the shown theme, under My Themes.
        themes.new_theme_for_test();
        pump(30);
        assert!(themes.selected().starts_with("user/new-theme"));
        assert_eq!(themes.list().len(), 6);
        themes.close();
        pump(50);
        window_done(&w.window);
    }

    fn window_done(window: &adw::ApplicationWindow) {
        window.destroy();
        pump(50);
    }

    /// The shown button labelled `text` under `root`.
    fn find_button(root: &gtk::Widget, text: &str) -> Option<gtk::Button> {
        if let Some(button) = root.downcast_ref::<gtk::Button>() {
            if button.label().as_deref() == Some(text) && button.is_mapped() {
                return Some(button.clone());
            }
        }
        let mut child = root.first_child();
        while let Some(c) = child {
            if let Some(found) = find_button(&c, text) {
                return Some(found);
            }
            child = c.next_sibling();
        }
        None
    }

    /// The text of the first shown label under `root` that starts with `prefix`.
    fn label_starting(root: &gtk::Widget, prefix: &str) -> Option<String> {
        if let Some(label) = root.downcast_ref::<gtk::Label>() {
            if label.label().starts_with(prefix) && label.is_mapped() {
                return Some(label.label().to_string());
            }
        }
        let mut child = root.first_child();
        while let Some(c) = child {
            if let Some(found) = label_starting(&c, prefix) {
                return Some(found);
            }
            child = c.next_sibling();
        }
        None
    }

    /// add-export-share: the redaction warning before Export and Save (task 2.7, mockup 10),
    /// quick save into the save folder (task 2.2, scenario "Pattern", mockup 06), a save that
    /// fails, and the export dialog with its banners (mockups 02–05). Everything is written
    /// to a temporary folder; no file chooser or portal is opened. Run on its own like the
    /// tests above.
    #[test]
    #[ignore = "opens a window; run under xvfb-run with GDK_BACKEND=x11"]
    fn export_and_save_on_a_live_display() {
        use annota::document::annotation::Annotation;
        use annota::document::command::Command;
        use annota::document::geometry::Rect;
        use annota::document::scene::Node;
        use annota::tools::redaction::{Redaction, RedactionKind};
        use std::os::unix::fs::PermissionsExt;

        let tmp = TempDir(
            std::env::temp_dir().join(format!("annota-export-test-{}", std::process::id())),
        );
        let _ = std::fs::remove_dir_all(&tmp.0);
        std::fs::create_dir_all(&tmp.0).unwrap();
        // Not created: quick save and export create their folders.
        let captures = tmp.0.join("captures");
        let exports = tmp.0.join("exports");
        let app = environment(&tmp.0);
        let store = Store::new(tmp.0.join("autosave"));
        let editor = Editor::build(&app, store.clone(), style_paths(&tmp.0)).unwrap();
        EDITOR.with(|e| e.replace(Some(editor.inner.clone())));
        let settings = editor.inner.settings.clone();
        settings
            .set_string("save-folder", &captures.to_string_lossy())
            .unwrap();
        settings
            .set_string("export-folder", &exports.to_string_lossy())
            .unwrap();
        let w = &editor.inner.w;
        w.window.set_default_size(1280, 800);
        w.window.present();
        pump(200);
        let dialog = || w.window.visible_dialog();
        let toasts = || w.toast_overlay.clone().upcast::<gtk::Widget>();

        // A capture with a pixelate redaction, a solid one, and a group holding a blur and a
        // pixelate redaction.
        let redaction = |x: f64, kind: RedactionKind| -> Box<dyn Annotation> {
            let mut r = Redaction::new(Rect::new(x, 10.0, 40.0, 20.0));
            r.kind = kind;
            Box::new(r)
        };
        let mut doc = Document::with_title(
            Bitmap::filled(400, 250, palette::PAPER).unwrap(),
            "Chat with Jonas",
        );
        let ids = doc.scene().fresh_ids(5);
        let group = Group::new(vec![
            Node::new(ids[3], redaction(200.0, RedactionKind::Blur)),
            Node::new(ids[4], redaction(260.0, RedactionKind::Pixelate)),
        ]);
        let nodes = vec![
            (
                0,
                Node::new(ids[0], redaction(10.0, RedactionKind::Pixelate)),
            ),
            (1, Node::new(ids[1], redaction(60.0, RedactionKind::Solid))),
            (2, Node::new(ids[2], Box::new(group))),
        ];
        History::new()
            .execute(
                &mut doc,
                "Add",
                Command::Insert {
                    nodes,
                    resources: Vec::new(),
                },
            )
            .unwrap();
        let uid = editor.add_session(
            Loaded {
                document: doc,
                source: None,
                path: None,
                name: "Chat with Jonas".into(),
            },
            Origin::New,
            AutosaveId::generate(),
        );
        let session = editor.session(uid).unwrap();
        // Autosaved like every new capture.
        editor.autosave_now();
        assert_eq!(store.list().entries.len(), 1);

        // Ctrl+E: the warning comes first; Cancel exports nothing and changes nothing.
        WidgetExt::activate_action(&w.window, "win.export", None).unwrap();
        pump(150);
        let alert = dialog().expect("the redaction warning");
        assert!(shows_label(
            alert.upcast_ref(),
            "3 redactions use blur or pixelate. Blurred or pixelated text can sometimes be reconstructed. Solid boxes hide it for good."
        ));
        assert!(shows_label(alert.upcast_ref(), "Make Redactions Solid?"));
        assert!(find_button(alert.upcast_ref(), "Export Anyway").is_some());
        shot(&w.window, "es10-redaction-warning");
        assert!(click_button(alert.upcast_ref(), "Cancel"));
        pump(300);
        assert!(dialog().is_none(), "Cancel opens no export dialog");
        assert_eq!(session.canvas.history_info().undo_label, None);

        // Ctrl+S: the warning with "Save Anyway"; Convert to Solid is one undo step for all
        // three (the group's too), then the save goes on.
        WidgetExt::activate_action(&w.window, "win.save", None).unwrap();
        pump(150);
        let alert = dialog().expect("the redaction warning before saving");
        assert!(find_button(alert.upcast_ref(), "Save Anyway").is_some());
        assert!(click_button(alert.upcast_ref(), "Convert to Solid"));
        pump(300);
        assert_eq!(
            session.canvas.history_info().undo_label.as_deref(),
            Some("Convert Redactions to Solid")
        );
        assert!(session
            .canvas
            .with_document(redactions::reversible)
            .unwrap()
            .is_empty());

        // Scenario "Pattern": one file in the save folder, named after the pattern, no dialog.
        let saved: Vec<PathBuf> = std::fs::read_dir(&captures)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(saved.len(), 1, "{saved:?}");
        let file = saved[0].clone();
        let stem = session::file_stem(&file);
        assert!(stem.starts_with("Annota 20"), "{stem}");
        assert_eq!(file.extension().unwrap(), "annota");
        let back = annota::document::archive::load(&file).unwrap();
        assert_eq!(back.object_count(), 3);
        assert!(redactions::reversible(&back).is_empty());
        assert_eq!(back.metadata().title.as_deref(), Some(stem.as_str()));
        // Mockup 06: title, subtitle, toast; the autosave is gone.
        assert_eq!(w.title.title(), stem);
        assert_eq!(
            w.title.subtitle(),
            format!("{} · Saved", crate::preferences::display_path(&captures))
        );
        assert_eq!(session.status(), text::Status::Clean);
        assert!(shows_label(&toasts(), &format!("Saved as “{stem}.annota”")));
        assert!(find_button(&toasts(), "Show in Files").is_some());
        shot(&w.window, "es06-saved");
        assert!(
            store.list().entries.is_empty(),
            "a saved document has no autosave"
        );

        // An edit, then Ctrl+S again: no warning (nothing reversible), saved over its file.
        session
            .canvas
            .run(|doc, _| ops::add(doc, redaction(320.0, RedactionKind::Solid)));
        assert_eq!(session.status(), text::Status::Edited);
        assert!(w.title.subtitle().ends_with("Edited"));
        WidgetExt::activate_action(&w.window, "win.save", None).unwrap();
        pump(150);
        assert!(dialog().is_none(), "no warning without blur or pixelate");
        assert_eq!(std::fs::read_dir(&captures).unwrap().count(), 1);
        assert_eq!(
            annota::document::archive::load(&file)
                .unwrap()
                .object_count(),
            4
        );
        assert_eq!(session.status(), text::Status::Clean);

        // Ctrl+E: "Export Image" (mockup 02), PNG from the preference; JPG adds Quality.
        WidgetExt::activate_action(&w.window, "win.export", None).unwrap();
        pump(300);
        let export = dialog().expect("Export Image");
        let root: &gtk::Widget = export.upcast_ref();
        assert!(shows_label(root, "Export Image"));
        assert!(shows_label(root, "Pixel-identical to the canvas at 100 %"));
        assert!(shows_label(root, ".png"));
        assert!(!shows_label(root, "Quality"));
        assert!(click_button(root, "JPG"));
        pump(50);
        assert!(shows_label(root, ".jpg"));
        assert!(shows_label(root, "Quality"));
        assert!(shows_label(root, "100 % · 400 × 250 px"));
        // The estimate arrives a moment later.
        pump(800);
        let line = label_starting(root, "400 × 250 px · JPG").unwrap();
        assert!(line.contains("≈ ") && line.ends_with(" KB"), "{line}");
        shot(&w.window, "es03-export-jpg");
        assert!(click_button(root, "Export"));
        pump(300);
        assert!(dialog().is_none(), "the dialog closes after exporting");
        let jpg = exports.join(format!("{stem}.jpg"));
        let bytes = std::fs::read(&jpg).unwrap();
        assert_eq!(&bytes[..2], &[0xFF, 0xD8], "a JPG file");

        // Again: the file exists now (banner with Replace, Export disabled until replaced).
        WidgetExt::activate_action(&w.window, "win.export", None).unwrap();
        pump(300);
        let export = dialog().unwrap();
        // Ctrl+E and Ctrl+S do nothing while it is open.
        WidgetExt::activate_action(&w.window, "win.export", None).unwrap();
        WidgetExt::activate_action(&w.window, "win.save", None).unwrap();
        pump(100);
        assert_eq!(dialog().as_ref(), Some(&export));
        let root: &gtk::Widget = export.upcast_ref();
        assert!(click_button(root, "JPG"));
        pump(50);
        assert!(shows_label(root, "A file with this name already exists"));
        assert!(!find_button(root, "Export").unwrap().is_sensitive());
        shot(&w.window, "es04-exists");
        std::fs::write(&jpg, b"old").unwrap();
        assert!(click_button(root, "Replace"));
        pump(300);
        assert!(dialog().is_none());
        assert_eq!(
            &std::fs::read(&jpg).unwrap()[..2],
            &[0xFF, 0xD8],
            "replaced"
        );

        // Mockup 04: a folder without write permission (unless running as root, who may).
        let locked = tmp.0.join("locked");
        std::fs::create_dir_all(&locked).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o500)).unwrap();
        let root_user = std::fs::write(locked.join("probe"), b"").is_ok();
        settings
            .set_string("export-folder", &locked.to_string_lossy())
            .unwrap();
        WidgetExt::activate_action(&w.window, "win.export", None).unwrap();
        pump(300);
        let export = dialog().unwrap();
        let root: &gtk::Widget = export.upcast_ref();
        if !root_user {
            assert!(shows_label(root, "No permission to write to this folder"));
            assert!(find_button(root, "Choose Folder…").is_some());
            assert!(!find_button(root, "Export").unwrap().is_sensitive());
        }
        assert!(click_button(root, "WebP"));
        assert!(click_button(root, "Lossy"));
        shot(&w.window, "es04-permission-webp");
        assert!(click_button(root, "PDF"));
        shot(&w.window, "es05-pdf");
        assert!(click_button(root, "Cancel"));
        pump(300);
        assert!(dialog().is_none());

        // A save that fails (the save folder cannot be created): the same words in a toast,
        // the document stays unsaved and keeps its autosave.
        settings
            .set_string("save-folder", &locked.join("captures").to_string_lossy())
            .unwrap();
        editor.add_session(
            Loaded {
                document: Document::new(Bitmap::filled(40, 30, palette::PAPER).unwrap()),
                source: None,
                path: None,
                name: "Pasted Image".into(),
            },
            Origin::New,
            AutosaveId::generate(),
        );
        let pasted = editor.current().unwrap();
        editor.autosave_now();
        WidgetExt::activate_action(&w.window, "win.save", None).unwrap();
        pump(300);
        if !root_user {
            assert!(shows_label(
                &toasts(),
                "No permission to write to this folder"
            ));
            assert!(find_button(&toasts(), "Save As…").is_some());
            assert_eq!(pasted.status(), text::Status::NotSaved);
            assert_eq!(store.list().entries.len(), 1, "the autosave is kept");
        }
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).unwrap();
        window_done(&w.window);
    }
}
