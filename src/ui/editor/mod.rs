//! The editor window (add-annotation-editor tasks 4.1 and 4.2; spec editor-canvas: Editor
//! layout, View control, Recent captures tray; spec document-format: Format versions, Autosave
//! and recovery).
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
//! **Placeholders.** Copy, Export, Save and Save As belong to add-export-share, Open Library
//! to add-capture-library: they show a "… arrives in milestone 0.1" toast. Share (D1) and the
//! image operations of task 4.3 (Resize, Rotate, Flip, Flatten All, canvas background) are
//! present but insensitive.

mod dialogs;
mod session;
mod sidebar;
mod text;
mod tray;

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::time::Duration;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, gio, glib};

use annota::document::archive::ArchiveError;
use annota::document::autosave::{self, AutosaveId, RecoveryEntry, Schedule, Store};
use annota::document::Document;
use annota::render::{self, view};

use self::session::{LoadError, Loaded, Origin, Session};
use self::sidebar::Sidebar;
use self::tray::{OpenTile, Tray, TrayKey};
use crate::cli::CaptureMode;
use crate::config;
use crate::ui::canvas::Canvas;

const UI_EDITOR: &str = "/io/github/trosinde/Annota/ui/editor-window.ui";
/// Redline: below 900 px the sidebar becomes a bottom sheet and the tray and Copy go.
const NARROW: &str = "max-width: 899px";
/// After a change, the document's tray thumbnail is rendered again this much later.
const THUMBNAIL_DELAY: Duration = Duration::from_millis(400);

thread_local! {
    /// The application's editor window. Owned here (the widgets only hold it weakly) from
    /// creation until the window closes.
    static EDITOR: RefCell<Option<Rc<Inner>>> = const { RefCell::new(None) };
}

/// Opens `path` (an image or `.annota` file) as a new document in the editor window and
/// presents the window (the `--dev-canvas FILE` debug option; `--open` follows with task 4.4).
pub fn open_path(app: &adw::Application, path: &Path) {
    if let Some(editor) = Editor::get_or_create(app) {
        editor.open_file(path);
        editor.inner.w.window.present();
    }
}

/// If a previous run left autosaves, presents the editor window with "Restore Unsaved
/// Captures?" (mockup 14) and returns `true`.
pub fn offer_recovery(app: &adw::Application) -> bool {
    let store = default_store();
    let listing = store.list();
    for path in &listing.unreadable {
        eprintln!("annota: damaged autosave left in place: {}", path.display());
    }
    if listing.entries.is_empty() {
        return false;
    }
    let Some(editor) = Editor::get_or_create(app) else {
        return false;
    };
    editor.inner.w.window.present();
    editor.offer_recovery(listing.entries);
    true
}

fn default_store() -> Store {
    Store::new(autosave::default_dir(&glib::user_cache_dir()))
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
    select_tool_button: gtk::ToggleButton,
    canvas_empty: gtk::DrawingArea,
    main_menu: gio::MenuModel,
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
            select_tool_button: obj!("select_tool_button"),
            canvas_empty: obj!("canvas_empty"),
            main_menu: obj!("main_menu"),
        })
    }
}

/// Window actions whose state follows the current document.
struct Actions {
    undo: gio::SimpleAction,
    redo: gio::SimpleAction,
    document: Vec<gio::SimpleAction>,
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
        let editor = Self::build(app, default_store())?;
        EDITOR.with(|e| e.replace(Some(editor.inner.clone())));
        Some(editor)
    }

    fn build(app: &adw::Application, store: Store) -> Option<Self> {
        let builder = gtk::Builder::from_resource(UI_EDITOR);
        let w = Widgets::new(&builder)?;
        let sidebar = Sidebar::new(&builder)?;
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

        self.setup_actions();
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
                editor.close_all();
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

    /// Opens a file as a new document; a file that needs a newer Annota gets the alert of
    /// mockup 15, other failures a toast.
    fn open_file(&self, path: &Path) {
        if let Some(open) = self
            .sessions()
            .into_iter()
            .find(|s| s.source.as_deref() == Some(path))
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
                    Origin::New
                };
                self.add_session(loaded, origin, AutosaveId::generate());
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

    fn add_session(&self, loaded: Loaded, origin: Origin, autosave: AutosaveId) -> u64 {
        let uid = self.inner.next_uid.get();
        self.inner.next_uid.set(uid + 1);
        let canvas = Canvas::new(loaded.document);
        canvas.set_snapping(self.inner.settings.boolean("snap-to-guides"));
        if origin == Origin::File {
            canvas.mark_saved();
        }
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
            origin,
            source: loaded.source,
            name,
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
        uid
    }

    fn switch_to(&self, uid: u64) {
        let Some(session) = self.session(uid) else {
            return;
        };
        self.inner.current.set(Some(uid));
        self.inner
            .w
            .canvas_stack
            .set_visible_child_name(&session.page_name());
        session.canvas.grab_focus();
        self.refresh();
    }

    /// The window closes: every document's autosave goes (spec: removed when closed).
    fn close_all(&self) {
        let sessions = self.inner.sessions.take();
        for session in &sessions {
            session.close(&self.inner.store);
        }
        self.inner.current.set(None);
    }

    fn document_changed(&self, uid: u64) {
        if self.inner.current.get() == Some(uid) {
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
        w.title.set_title(&session.name);
        w.window.set_title(Some(&session.name));
        w.title.set_subtitle(&text::subtitle(
            size,
            session::status(session.origin, &info),
            (!fitted).then_some(zoom),
            None,
        ));
        w.zoom_level_label.set_text(&text::zoom_label(zoom, fitted));
        let preset = view::preset(zoom)
            .filter(|_| !fitted)
            .map(|p| view::percent(p).to_string())
            .unwrap_or_default();
        a.zoom_preset.set_state(&preset.to_variant());
    }

    fn refresh_sidebar(&self) {
        let state = match self.current() {
            Some(session) => {
                let selection = session.canvas.selection();
                session
                    .canvas
                    .with_document(|doc| sidebar::state(doc, &selection))
                    .unwrap_or(sidebar::State::Empty)
            }
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
                name: s.name.clone(),
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
        let open: Vec<PathBuf> = self
            .sessions()
            .iter()
            .filter_map(|s| s.source.clone())
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
        // Placeholders until add-export-share.
        entry("copy", |e| {
            e.toast(&gettext("Copying the image arrives in milestone 0.1"))
        });
        entry("export", |e| {
            e.toast(&gettext("Exporting arrives in milestone 0.1"))
        });
        entry("save", |e| {
            e.toast(&gettext("Saving arrives in milestone 0.1"))
        });
        entry("save-as", |e| {
            e.toast(&gettext("Saving arrives in milestone 0.1"))
        });

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

    fn toast(&self, text: &str) {
        let toast = adw::Toast::new(text);
        toast.set_use_markup(false);
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
        on_canvas || !activates
    }

    fn key_pressed(&self, key: gdk::Key, modifiers: gdk::ModifierType) -> bool {
        if !self.routes_to_canvas(key) {
            return false;
        }
        let Some(canvas) = self.current_canvas() else {
            return false;
        };
        if canvas.key_pressed(key, modifiers) {
            return true;
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
            // Select tool (V); the only tool until add-core-annotation-tools ships.
            gdk::Key::v if plain => {
                self.inner.w.select_tool_button.set_active(true);
                true
            }
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
        self.inner.sidebar.connect_arrange(move |arrange| {
            if let Some(canvas) = weak.upgrade().and_then(|e| e.current_canvas()) {
                if !canvas.set_arrange(arrange) {
                    // Refused or unchanged: show the real values again.
                    if let Some(editor) = weak.upgrade() {
                        editor.refresh_sidebar();
                    }
                }
            }
        });
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

/// Creates the window actions. Those of later tasks and changes exist but stay disabled; the
/// others get their handlers in [`Editor::setup_actions`].
fn install_actions(window: &adw::ApplicationWindow, settings: &gio::Settings) -> Actions {
    let simple = |name: &str| {
        let action = gio::SimpleAction::new(name, None);
        window.add_action(&action);
        action
    };
    // Task 4.3 (image operations) and decision D1's Share menu (add-export-share).
    for name in [
        "resize-image",
        "rotate-left",
        "rotate-right",
        "flip-horizontal",
        "flip-vertical",
        "flatten-all",
        "share-email",
        "share-open-with",
        "share-copy-to-folder",
    ] {
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
    Actions {
        undo: simple("undo"),
        redo: simple("redo"),
        document,
        zoom_preset,
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
    #[ignore = "opens a window; run under xvfb-run with GDK_BACKEND=x11"]
    fn editor_window_on_a_live_display() {
        let tmp = TempDir(
            std::env::temp_dir().join(format!("annota-editor-test-{}", std::process::id())),
        );
        let _ = std::fs::remove_dir_all(&tmp.0);
        std::fs::create_dir_all(tmp.0.join("captures")).unwrap();
        let app = environment(&tmp.0);
        let store = Store::new(tmp.0.join("autosave"));
        let editor = Editor::build(&app, store.clone()).unwrap();
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
        assert!(main_items
            .iter()
            .any(|(l, a)| l == "Flatten All" && a.as_deref() == Some("<Shift><Control>t")));

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
        // Tray: one open document; the open file is not listed under Recent.
        editor.refresh_recent_tray();
        assert_eq!(editor.inner.tray.counts(), (1, 0));

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

        // Autosave: both unsaved documents are written; closing removes them.
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
        assert_eq!(editor.sessions().len(), 3);
        assert_eq!(w.title.title(), "Checkout flow");
        assert_eq!(w.title.subtitle(), "1600 × 1000 · Edited");

        // A newer major format: the alert of mockup 15, the file untouched.
        let newer = tmp.0.join("Release notes v2.annota");
        std::fs::copy(&file, &newer).unwrap();
        let bytes = rewrite_format(&newer, "2.0", "2.1");
        editor.open_file(&newer);
        pump(100);
        assert_eq!(editor.sessions().len(), 3);
        assert!(w.window.visible_dialog().is_some(), "alert shown");
        assert_eq!(std::fs::read(&newer).unwrap(), bytes);
        if let Some(dialog) = w.window.visible_dialog() {
            dialog.force_close();
        }

        w.window.close();
        pump(100);
        assert!(store.list().entries.is_empty(), "closing removes autosaves");
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
}
