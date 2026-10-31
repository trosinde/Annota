//! The canvas widget: draws a document with the shared renderer, with zoom and pan (spec
//! editor-canvas: View control), and edits it with the Select tool: selection, moving,
//! resizing, rotating, rubber band, snapping guides, nudging, z-order and grouping (spec
//! editor-canvas: Object selection and manipulation, Z-order and grouping, Snapping and
//! guides).
//!
//! Built imperatively, as CLAUDE.md §7 allows for the canvas (its measurement and snap chips
//! included). All geometry and editing logic comes from `annota::render::view` and
//! `annota::editing`; this module only wires GTK events to them, executes the resulting
//! `Edit`s through the document's `History` (one undo step each) and draws
//! `annota::render::overlay`. The editor window (`super::editor`) puts the zoom OSD, header
//! bar, sidebar and tray around it, follows it through [`Canvas::connect_changed`] and
//! [`Canvas::connect_view_changed`], and routes key presses here unless a text field or a
//! dialog has the focus.
//!
//! **Viewport-sized, not content-sized.** [`CanvasView`] implements `GtkScrollable`: inside a
//! `GtkScrolledWindow` it is allocated exactly the visible area and *owns* the scroll
//! adjustments, whose range it sets to the zoomed content. Each frame records only the visible
//! area (the cairo clip is the viewport), so the renderer's culling skips everything outside
//! it, and the widget shrinks with the window, so "Fit" stays centred. (A `GtkDrawingArea`
//! sized to the zoomed content made GTK record the whole content at every zoom level — at
//! 1600 % a 25 000 px wide surface — and kept the window from shrinking the canvas.)

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, glib, graphene};

use annota::document::annotation::{Annotation, HandleId, Modifiers};
use annota::document::bitmap::Bitmap;
use annota::document::color::Rgba;
use annota::document::command::{Command, History};
use annota::document::geometry::{Arrange, IRect, Point};
use annota::document::scene::ObjectId;
use annota::document::Document;
use annota::editing::canvas_edit::{self, CanvasDrag, Side, Target as CanvasTarget};
use annota::editing::crop::{Aspect, Crop};
use annota::editing::drag::{self, Drag, Feedback, Hover};
use annota::editing::image_ops;
use annota::editing::ops::{self, Action, Edit, Stacking, Subject};
use annota::editing::selection::{selection_frame, Selection};
use annota::editing::snap::{Axis, Snap};
use annota::editing::steps;
use annota::editing::text_edit::{Motion, TextEdit};
use annota::editing::tool::{
    route_press, Draw, DrawFeedback, Drawn, PolygonDraft, Press, StepPlace, Tool, Toolbox,
};
use annota::render::{self, overlay, view, Options, Preview, Viewport};
use annota::tools::text::editable;

use super::labels;

/// Kit `--canvas-bg`, light and dark.
const CANVAS_BG_LIGHT: Rgba = Rgba::opaque(0xDE, 0xDD, 0xDA);
const CANVAS_BG_DARK: Rgba = Rgba::opaque(0x18, 0x18, 0x1B);

/// The canvas background (kit `--canvas-bg`) for the current light or dark style.
pub fn background() -> Rgba {
    if adw::StyleManager::default().is_dark() {
        CANVAS_BG_DARK
    } else {
        CANVAS_BG_LIGHT
    }
}

/// Fraction of the view one scroll-bar step / page moves.
const STEP: f64 = 0.1;
const PAGE: f64 = 0.9;

struct State {
    doc: Document,
    history: History,
    selection: Selection,
    /// The pointer gesture in progress (Select tool).
    drag: Option<Drag>,
    /// A canvas edge or the corner-radius handle being dragged (Select tool).
    canvas_drag: Option<CanvasDrag>,
    /// The canvas edge or radius handle under the pointer: its grabber is shown.
    hover: Option<CanvasTarget>,
    /// Crop mode (tool C), if active.
    crop: Option<Crop>,
    /// The tool the window's tool strip has selected (Select or a drawing tool).
    tool: Tool,
    /// The tools' defaults, shared by the window's documents.
    toolbox: Rc<RefCell<Toolbox>>,
    /// A drawing tool's gesture in progress.
    draw: Option<Draw>,
    /// A polygon being drawn with the shape tool, vertex by vertex.
    polygon: Option<PolygonDraft>,
    /// Text being edited in place.
    text: Option<TextEdit>,
    /// The pointer is selecting text in the text being edited.
    text_drag: bool,
    /// The sequence the next step marker joins (Restart Sequence); `None`: the newest.
    step_sequence: Option<u32>,
    /// The number the next step marker gets ("Next value"); `None`: after the highest.
    step_next: Option<u32>,
    /// Presses of the current click (2 for a double-click).
    presses: i32,
    /// The pointer is over the canvas (the step tool shows where a marker would go).
    hovering: bool,
    /// An edit made inside a state borrow, executed right after it.
    pending: Option<Edit>,
    /// Snap to guides (sidebar switch, `Ctrl+;`); Alt suspends it while held.
    snapping: bool,
    zoom: f64,
    /// Follows the window size ("Fit · 50 %") until the user zooms.
    fitted: bool,
    /// Space is held: primary-button drags pan.
    space: bool,
    /// Last pointer position in view coordinates, the anchor for Ctrl+scroll.
    pointer: (f64, f64),
    pan_start: Option<(f64, f64)>,
}

/// Called with (zoom, fitted).
type ViewListener = Box<dyn Fn(f64, bool)>;
/// Called after the document, its history, the selection or the snapping switch may have
/// changed.
type ChangeListener = Box<dyn Fn()>;
/// Called with a point in view coordinates where the context menu should open.
type MenuListener = Box<dyn Fn(f64, f64)>;
/// Called once with the colour the eyedropper picked, or `None` if it was cancelled.
type Picker = Box<dyn FnOnce(Option<Rgba>)>;
/// Called with things the window announces in a toast.
type NoticeListener = Box<dyn Fn(Notice)>;

/// Something the window announces (a toast).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    /// Step markers were deleted and others renumbered (mockup 13: "Step 2 deleted · 2 steps
    /// renumbered"): the deleted marker's label if it was one marker, how many were renumbered.
    StepsRenumbered {
        deleted: Option<String>,
        renumbered: usize,
        done: Done,
    },
}

/// What an executed edit did, for toasts that offer Undo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Done {
    pub action: Action,
    pub subject: Subject,
    /// The history revision right after the edit: Undo applies only if nothing happened since.
    pub revision: u64,
}

/// What the header bar shows of the history (Undo/Redo buttons and tooltips, "Edited").
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HistoryInfo {
    pub undo_label: Option<String>,
    pub redo_label: Option<String>,
    pub revision: u64,
    pub dirty: bool,
}

mod imp {
    use std::sync::OnceLock;

    use super::*;

    pub struct CanvasView {
        pub(super) state: RefCell<Option<State>>,
        pub(super) listeners: RefCell<Vec<ViewListener>>,
        pub(super) changes: RefCell<Vec<ChangeListener>>,
        pub(super) menus: RefCell<Vec<MenuListener>>,
        /// The eyedropper: called with the colour under the next click (`None`: cancelled).
        pub(super) picker: RefCell<Option<Picker>>,
        pub(super) notices: RefCell<Vec<NoticeListener>>,
        /// Input method for typing text in place (dead keys, compose, CJK) and the key
        /// controller it is attached to while text is edited.
        pub(super) im: RefCell<Option<(gtk::IMMulticontext, gtk::EventControllerKey)>>,
        /// Time and place of the last primary press, for double-clicks.
        pub(super) last_press: Cell<Option<(u32, Point)>>,
        hadjustment: RefCell<Option<(gtk::Adjustment, glib::SignalHandlerId)>>,
        vadjustment: RefCell<Option<(gtk::Adjustment, glib::SignalHandlerId)>>,
        hscroll_policy: Cell<gtk::ScrollablePolicy>,
        vscroll_policy: Cell<gtk::ScrollablePolicy>,
    }

    impl Default for CanvasView {
        fn default() -> Self {
            Self {
                state: RefCell::new(None),
                listeners: RefCell::new(Vec::new()),
                changes: RefCell::new(Vec::new()),
                menus: RefCell::new(Vec::new()),
                picker: RefCell::new(None),
                notices: RefCell::new(Vec::new()),
                im: RefCell::new(None),
                last_press: Cell::new(None),
                hadjustment: RefCell::new(None),
                vadjustment: RefCell::new(None),
                hscroll_policy: Cell::new(gtk::ScrollablePolicy::Minimum),
                vscroll_policy: Cell::new(gtk::ScrollablePolicy::Minimum),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CanvasView {
        const NAME: &'static str = "AnnotaCanvasView";
        type Type = super::CanvasView;
        type ParentType = gtk::Widget;
        type Interfaces = (gtk::Scrollable,);
    }

    impl ObjectImpl for CanvasView {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();
            PROPERTIES.get_or_init(|| {
                [
                    "hadjustment",
                    "vadjustment",
                    "hscroll-policy",
                    "vscroll-policy",
                ]
                .into_iter()
                .map(glib::ParamSpecOverride::for_interface::<gtk::Scrollable>)
                .collect()
            })
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            match pspec.name() {
                "hadjustment" => {
                    self.set_adjustment(gtk::Orientation::Horizontal, value.get().ok().flatten())
                }
                "vadjustment" => {
                    self.set_adjustment(gtk::Orientation::Vertical, value.get().ok().flatten())
                }
                "hscroll-policy" => {
                    if let Ok(policy) = value.get() {
                        self.hscroll_policy.set(policy);
                    }
                }
                "vscroll-policy" => {
                    if let Ok(policy) = value.get() {
                        self.vscroll_policy.set(policy);
                    }
                }
                _ => {}
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                "hadjustment" => self.adjustment(gtk::Orientation::Horizontal).to_value(),
                "vadjustment" => self.adjustment(gtk::Orientation::Vertical).to_value(),
                "hscroll-policy" => self.hscroll_policy.get().to_value(),
                "vscroll-policy" => self.vscroll_policy.get().to_value(),
                _ => pspec.default_value().clone(),
            }
        }

        fn dispose(&self) {
            for slot in [&self.hadjustment, &self.vadjustment] {
                if let Some((adjustment, handler)) = slot.borrow_mut().take() {
                    adjustment.disconnect(handler);
                }
            }
        }
    }

    impl WidgetImpl for CanvasView {
        /// No minimum and no natural size: the scrolled window gives it the visible area.
        fn measure(&self, _: gtk::Orientation, _: i32) -> (i32, i32, i32, i32) {
            (0, 0, -1, -1)
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            self.parent_size_allocate(width, height, baseline);
            let size = (f64::from(width), f64::from(height));
            // Keep "Fit" fitted while the window is resized.
            let refit = self
                .state
                .borrow_mut()
                .as_mut()
                .filter(|s| s.fitted && size.0 >= 1.0 && size.1 >= 1.0)
                .and_then(|s| {
                    let zoom = view::fit_zoom(s.doc.canvas().rect, size);
                    let changed = (zoom - s.zoom).abs() > 1e-9;
                    s.zoom = zoom;
                    changed.then_some(zoom)
                });
            self.configure_adjustments(None);
            if let Some(zoom) = refit {
                self.notify_view(zoom, true);
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let (width, height) = (widget.width(), widget.height());
            if width <= 0 || height <= 0 {
                return;
            }
            // The recording surface is the viewport, nothing more.
            let bounds = graphene::Rect::new(0.0, 0.0, width as f32, height as f32);
            let cr = snapshot.append_cairo(&bounds);
            let state = self.state.borrow();
            if let Some(state) = state.as_ref() {
                draw(
                    state,
                    &cr,
                    (f64::from(width), f64::from(height)),
                    self.scroll(),
                );
            }
        }
    }

    impl ScrollableImpl for CanvasView {}

    impl CanvasView {
        pub(super) fn adjustment(&self, orientation: gtk::Orientation) -> Option<gtk::Adjustment> {
            let slot = match orientation {
                gtk::Orientation::Horizontal => &self.hadjustment,
                _ => &self.vadjustment,
            };
            slot.borrow().as_ref().map(|(a, _)| a.clone())
        }

        fn set_adjustment(
            &self,
            orientation: gtk::Orientation,
            adjustment: Option<gtk::Adjustment>,
        ) {
            let slot = match orientation {
                gtk::Orientation::Horizontal => &self.hadjustment,
                _ => &self.vadjustment,
            };
            if let Some((old, handler)) = slot.borrow_mut().take() {
                old.disconnect(handler);
            }
            let adjustment = adjustment.unwrap_or_default();
            let widget = self.obj().downgrade();
            let handler = adjustment.connect_value_changed(move |_| {
                if let Some(widget) = widget.upgrade() {
                    widget.queue_draw();
                }
            });
            slot.replace(Some((adjustment, handler)));
            self.configure_adjustments(None);
            self.obj().notify(match orientation {
                gtk::Orientation::Horizontal => "hadjustment",
                _ => "vadjustment",
            });
        }

        /// Current scroll offset in content coordinates.
        pub(super) fn scroll(&self) -> (f64, f64) {
            let value = |o| self.adjustment(o).map_or(0.0, |a| a.value());
            (
                value(gtk::Orientation::Horizontal),
                value(gtk::Orientation::Vertical),
            )
        }

        pub(super) fn view_size(&self) -> (f64, f64) {
            let widget = self.obj();
            (f64::from(widget.width()), f64::from(widget.height()))
        }

        /// Sets both scroll ranges to the zoomed content (at least the view) and the page
        /// size to the view; `scroll` moves to a new position, else the current one is kept
        /// (clamped to the new range).
        pub(super) fn configure_adjustments(&self, scroll: Option<(f64, f64)>) {
            let view_size = self.view_size();
            let content = {
                let state = self.state.borrow();
                let Some(state) = state.as_ref() else {
                    return;
                };
                view::content_size(state.doc.canvas().rect, state.zoom)
            };
            let current = self.scroll();
            let scroll = scroll.unwrap_or(current);
            for (orientation, page, size, value) in [
                (
                    gtk::Orientation::Horizontal,
                    view_size.0,
                    content.0,
                    scroll.0,
                ),
                (gtk::Orientation::Vertical, view_size.1, content.1, scroll.1),
            ] {
                if let Some(adjustment) = self.adjustment(orientation) {
                    let upper = size.max(page);
                    let value = value.clamp(0.0, (upper - page).max(0.0));
                    adjustment.configure(value, 0.0, upper, page * STEP, page * PAGE, page);
                }
            }
            self.obj().queue_draw();
        }

        pub(super) fn notify_view(&self, zoom: f64, fitted: bool) {
            for listener in self.listeners.borrow().iter() {
                listener(zoom, fitted);
            }
        }

        pub(super) fn notify_changed(&self) {
            for listener in self.changes.borrow().iter() {
                listener();
            }
        }
    }
}

glib::wrapper! {
    /// The scrollable drawing surface of the canvas; see the module documentation.
    pub struct CanvasView(ObjectSubclass<imp::CanvasView>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Scrollable;
}

/// The canvas: a [`CanvasView`] in a `GtkScrolledWindow` (scroll bars, kinetic scrolling),
/// with zoom and pan, in an overlay that holds the chips.
#[derive(Clone)]
pub struct Canvas {
    overlay: gtk::Overlay,
    scrolled: gtk::ScrolledWindow,
    view: CanvasView,
    /// Position, size or angle of what is being dragged (mockups 02, 03, 06).
    measure_chip: gtk::Label,
    /// Names the snap target (mockup 02: "Canvas centre").
    snap_chip: gtk::Label,
}

/// Redline "Canvas": measurement chips sit 10 px right of the cursor.
const CHIP_OFFSET: f64 = 10.0;
/// Gap between a chip and the selection box or guide it belongs to.
const CHIP_GAP: f64 = 6.0;

fn chip(classes: &[&str]) -> gtk::Label {
    gtk::Label::builder()
        .css_classes(classes.iter().map(|c| c.to_string()).collect::<Vec<_>>())
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Start)
        .can_target(false)
        .visible(false)
        .build()
}

fn modifiers(state: gdk::ModifierType) -> Modifiers {
    Modifiers {
        shift: state.contains(gdk::ModifierType::SHIFT_MASK),
        ctrl: state.contains(gdk::ModifierType::CONTROL_MASK),
        alt: state.contains(gdk::ModifierType::ALT_MASK),
    }
}

impl Canvas {
    pub fn new(doc: Document) -> Self {
        let view: CanvasView = glib::Object::builder()
            .property("hexpand", true)
            .property("vexpand", true)
            // Takes the keyboard focus when clicked, so Space pans instead of activating the
            // button clicked last.
            .property("focusable", true)
            .build();
        view.imp().state.replace(Some(State {
            doc,
            history: History::new(),
            selection: Selection::new(),
            drag: None,
            canvas_drag: None,
            hover: None,
            crop: None,
            tool: Tool::Select,
            toolbox: Rc::new(RefCell::new(Toolbox::default())),
            draw: None,
            polygon: None,
            text: None,
            text_drag: false,
            step_sequence: None,
            step_next: None,
            presses: 1,
            hovering: false,
            pending: None,
            snapping: true,
            zoom: 1.0,
            fitted: true,
            space: false,
            pointer: (0.0, 0.0),
            pan_start: None,
        }));
        let scrolled = gtk::ScrolledWindow::builder()
            .child(&view)
            .hexpand(true)
            .vexpand(true)
            .build();
        let overlay = gtk::Overlay::builder().child(&scrolled).build();
        let measure_chip = chip(&["annota-chip", "annota-canvas-chip", "numeric"]);
        let snap_chip = chip(&["annota-chip", "annota-snap-chip", "annota-canvas-chip"]);
        overlay.add_overlay(&measure_chip);
        overlay.add_overlay(&snap_chip);
        let canvas = Self {
            overlay,
            scrolled,
            view,
            measure_chip,
            snap_chip,
        };
        canvas.setup();
        canvas
    }

    pub fn widget(&self) -> &gtk::Overlay {
        &self.overlay
    }

    /// Where a document point is in the view (tests drive the canvas in view coordinates).
    #[cfg(test)]
    pub fn view_point(&self, doc: Point) -> Option<Point> {
        Some(self.viewport()?.to_device(doc))
    }

    #[cfg(test)]
    pub fn scrolled(&self) -> &gtk::ScrolledWindow {
        &self.scrolled
    }

    pub fn zoom(&self) -> f64 {
        self.with_state(|s| s.zoom).unwrap_or(1.0)
    }

    /// Called with (zoom, fitted) whenever the view changes.
    pub fn connect_view_changed(&self, f: impl Fn(f64, bool) + 'static) {
        self.view.imp().listeners.borrow_mut().push(Box::new(f));
    }

    /// Called after anything the window shows may have changed: the document, its history,
    /// the selection or the snapping switch. (Not during a drag: the gesture commits once.)
    pub fn connect_changed(&self, f: impl Fn() + 'static) {
        self.view.imp().changes.borrow_mut().push(Box::new(f));
    }

    fn notify_changed(&self) {
        self.view.imp().notify_changed();
    }

    /// Called with a point in [`Canvas::widget`] coordinates where the context menu opens
    /// (right-click, Shift+F10, the Menu key).
    pub fn connect_context_menu(&self, f: impl Fn(f64, f64) + 'static) {
        self.view.imp().menus.borrow_mut().push(Box::new(f));
    }

    fn open_context_menu(&self, at: Point) {
        let point = self
            .view
            .compute_point(
                &self.overlay,
                &graphene::Point::new(at.x as f32, at.y as f32),
            )
            .map_or((at.x, at.y), |p| (f64::from(p.x()), f64::from(p.y())));
        for listener in self.view.imp().menus.borrow().iter() {
            listener(point.0, point.1);
        }
    }

    /// Right-click at `at` (view coordinates): selects the object there unless it is already
    /// selected (nothing there: the selection stays), then opens the context menu.
    pub fn context_menu_at(&self, at: Point) {
        if self.cropping() {
            return;
        }
        self.cancel_drag();
        self.finish_text();
        self.with_state(|s| s.polygon = None);
        if let Some(viewport) = self.viewport() {
            let changed = self
                .with_state(|s| {
                    let hit = drag::hit(&s.doc, &viewport, at)?;
                    (!s.selection.contains(hit)).then(|| s.selection.set(s.doc.scene(), [hit]))
                })
                .flatten()
                .is_some();
            if changed {
                self.view.queue_draw();
                self.notify_changed();
            }
        }
        self.open_context_menu(at);
    }

    /// Shift+F10 or the Menu key: the context menu at the centre of the selection (or of the
    /// view when nothing is selected).
    fn context_menu_from_keyboard(&self) {
        let Some(viewport) = self.viewport() else {
            return;
        };
        let size = self.view.imp().view_size();
        let at = self
            .with_state(|s| {
                let frame = selection_frame(s.doc.scene(), &s.selection, &[])?;
                Some(viewport.to_device(frame.frame.bounds().centre()))
            })
            .flatten()
            .map_or(Point::new(size.0 / 2.0, size.1 / 2.0), |p| {
                Point::new(p.x.clamp(0.0, size.0), p.y.clamp(0.0, size.1))
            });
        self.open_context_menu(at);
    }

    /// Whether the view follows the window size ("Fit · 50 %").
    pub fn fitted(&self) -> bool {
        self.with_state(|s| s.fitted).unwrap_or(true)
    }

    pub fn history_info(&self) -> HistoryInfo {
        self.with_state(|s| HistoryInfo {
            undo_label: s.history.undo_label().map(str::to_string),
            redo_label: s.history.redo_label().map(str::to_string),
            revision: s.history.revision(),
            dirty: s.history.is_dirty(),
        })
        .unwrap_or_default()
    }

    /// Records that the current state is on disk (a document opened from a file).
    pub fn mark_saved(&self) {
        self.with_state(|s| s.history.mark_saved());
        self.notify_changed();
    }

    /// Takes the keyboard focus (after switching documents).
    pub fn grab_focus(&self) {
        self.view.grab_focus();
    }

    /// Where a point of [`Canvas::widget`] lies in the document.
    pub fn document_point(&self, x: f64, y: f64) -> Option<Point> {
        let viewport = self.viewport()?;
        let at = self
            .overlay
            .compute_point(&self.view, &graphene::Point::new(x as f32, y as f32))?;
        Some(viewport.to_document(Point::new(f64::from(at.x()), f64::from(at.y()))))
    }

    fn with_state<T>(&self, f: impl FnOnce(&mut State) -> T) -> Option<T> {
        self.view.imp().state.borrow_mut().as_mut().map(f)
    }

    fn setup(&self) {
        let motion = gtk::EventControllerMotion::new();
        let weak = self.downgrade();
        motion.connect_motion(move |_, x, y| {
            if let Some(canvas) = weak.upgrade() {
                canvas.pointer_hovered(Point::new(x, y));
                canvas.update_hover();
                canvas.update_cursor();
            }
        });
        let weak = self.downgrade();
        motion.connect_leave(move |_| {
            if let Some(canvas) = weak.upgrade() {
                let changed = canvas
                    .with_state(|s| {
                        let ghost = s.hovering && s.tool == Tool::Step;
                        s.hovering = false;
                        (s.canvas_drag.is_none() && s.hover.take().is_some()) || ghost
                    })
                    .unwrap_or(false);
                if changed {
                    canvas.view.queue_draw();
                    canvas.update_chips();
                }
            }
        });
        self.view.add_controller(motion);

        // Right-click: the context menu (redline "Context menu"), on the object under the
        // pointer — selected first if it is not part of the selection.
        let menu = gtk::GestureClick::new();
        menu.set_button(gdk::BUTTON_SECONDARY);
        let weak = self.downgrade();
        menu.connect_pressed(move |gesture, _, x, y| {
            if let Some(canvas) = weak.upgrade() {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                canvas.view.grab_focus();
                canvas.context_menu_at(Point::new(x, y));
            }
        });
        self.view.add_controller(menu);

        // Select tool: press, drag, release with the primary button.
        let select = gtk::GestureDrag::new();
        select.set_button(gdk::BUTTON_PRIMARY);
        let weak = self.downgrade();
        select.connect_drag_begin(move |gesture, x, y| {
            let Some(canvas) = weak.upgrade() else {
                return;
            };
            if canvas.with_state(|s| s.space).unwrap_or(false) {
                // Space + drag pans (the scrolled window's gesture).
                gesture.set_state(gtk::EventSequenceState::Denied);
                return;
            }
            gesture.set_state(gtk::EventSequenceState::Claimed);
            canvas.view.grab_focus();
            canvas.count_press(gesture.current_event_time(), Point::new(x, y));
            canvas.pointer_pressed(Point::new(x, y), modifiers(gesture.current_event_state()));
        });
        let weak = self.downgrade();
        select.connect_drag_update(move |gesture, dx, dy| {
            let (Some(canvas), Some((x, y))) = (weak.upgrade(), gesture.start_point()) else {
                return;
            };
            canvas.pointer_moved(
                Point::new(x + dx, y + dy),
                modifiers(gesture.current_event_state()),
            );
        });
        let weak = self.downgrade();
        select.connect_drag_end(move |_, _, _| {
            if let Some(canvas) = weak.upgrade() {
                canvas.pointer_released();
            }
        });
        let weak = self.downgrade();
        select.connect_cancel(move |_, _| {
            if let Some(canvas) = weak.upgrade() {
                canvas.cancel_drag();
            }
        });
        self.view.add_controller(select);

        // Typing text in place goes through an input method (dead keys, compose sequences):
        // a key controller on the canvas that holds it while text is edited.
        let im = gtk::IMMulticontext::new();
        im.set_client_widget(Some(&self.view));
        let weak = self.downgrade();
        im.connect_commit(move |_, text| {
            if let Some(canvas) = weak.upgrade() {
                canvas.insert_text(text);
            }
        });
        let typing = gtk::EventControllerKey::new();
        self.view.add_controller(typing.clone());
        self.view.imp().im.replace(Some((im, typing)));

        // Ctrl+scroll zooms at the pointer; plain scrolling scrolls (the scrolled window).
        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        scroll.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = self.downgrade();
        scroll.connect_scroll(move |controller, _, dy| {
            let ctrl = controller
                .current_event_state()
                .contains(gdk::ModifierType::CONTROL_MASK);
            match weak.upgrade() {
                Some(canvas) if ctrl => {
                    if let Some((zoom, anchor)) = canvas.with_state(|s| (s.zoom, s.pointer)) {
                        canvas.zoom_to(zoom * view::SCROLL_FACTOR.powf(-dy), Some(anchor));
                    }
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        self.scrolled.add_controller(scroll);

        // Pan with the middle button, or space + primary button.
        let drag = gtk::GestureDrag::new();
        drag.set_button(0);
        let weak = self.downgrade();
        drag.connect_drag_begin(move |gesture, _, _| {
            let Some(canvas) = weak.upgrade() else {
                return;
            };
            let button = gesture.current_button();
            let space = canvas.with_state(|s| s.space).unwrap_or(false);
            if button == gdk::BUTTON_MIDDLE || (button == gdk::BUTTON_PRIMARY && space) {
                let start = canvas.view.imp().scroll();
                canvas.with_state(|s| s.pan_start = Some(start));
                canvas.scrolled.set_cursor_from_name(Some("grabbing"));
                gesture.set_state(gtk::EventSequenceState::Claimed);
            } else {
                gesture.set_state(gtk::EventSequenceState::Denied);
            }
        });
        let weak = self.downgrade();
        drag.connect_drag_update(move |_, dx, dy| {
            let Some(canvas) = weak.upgrade() else {
                return;
            };
            if let Some(Some((sx, sy))) = canvas.with_state(|s| s.pan_start) {
                let imp = canvas.view.imp();
                if let Some(h) = imp.adjustment(gtk::Orientation::Horizontal) {
                    h.set_value(sx - dx);
                }
                if let Some(v) = imp.adjustment(gtk::Orientation::Vertical) {
                    v.set_value(sy - dy);
                }
            }
        });
        let weak = self.downgrade();
        drag.connect_drag_end(move |_, _, _| {
            let Some(canvas) = weak.upgrade() else {
                return;
            };
            let space = canvas
                .with_state(|s| {
                    s.pan_start = None;
                    s.space
                })
                .unwrap_or(false);
            canvas
                .scrolled
                .set_cursor_from_name(space.then_some("grab"));
        });
        self.scrolled.add_controller(drag);
    }

    fn downgrade(&self) -> WeakCanvas {
        WeakCanvas {
            overlay: self.overlay.downgrade(),
            scrolled: self.scrolled.downgrade(),
            view: self.view.downgrade(),
            measure_chip: self.measure_chip.downgrade(),
            snap_chip: self.snap_chip.downgrade(),
        }
    }

    /// The current mapping of document space into the view.
    fn viewport(&self) -> Option<Viewport> {
        let imp = self.view.imp();
        let (size, scroll) = (imp.view_size(), imp.scroll());
        self.with_state(|s| {
            Viewport::new(
                s.zoom,
                view::view_origin(s.doc.canvas().rect, s.zoom, size, scroll),
            )
        })
    }

    // ---- Select tool, drawing tools and crop mode ----

    /// Counts presses for double-clicks, by GTK's double-click time and distance.
    fn count_press(&self, time: u32, at: Point) {
        let (max_time, max_distance) = gtk::Settings::default().map_or((400, 5), |s| {
            (s.gtk_double_click_time(), s.gtk_double_click_distance())
        });
        let imp = self.view.imp();
        let double = imp.last_press.get().is_some_and(|(t, p)| {
            time.wrapping_sub(t) <= u32::try_from(max_time).unwrap_or(400)
                && p.distance(at) <= f64::from(max_distance)
        });
        self.with_state(|s| s.presses = if double { s.presses + 1 } else { 1 });
        imp.last_press.set(Some((time, at)));
    }

    /// The pointer moved over the canvas without a button: the polygon's rubber segment and
    /// the step tool's placement preview follow it.
    fn pointer_hovered(&self, at: Point) {
        let Some(viewport) = self.viewport() else {
            return;
        };
        let redraw = self
            .with_state(|s| {
                s.pointer = (at.x, at.y);
                s.hovering = true;
                if let Some(polygon) = s.polygon.as_mut() {
                    polygon.set_pointer(viewport.to_document(at));
                    return true;
                }
                s.tool == Tool::Step && s.text.is_none()
            })
            .unwrap_or(false);
        if redraw {
            self.view.queue_draw();
            self.update_chips();
        }
    }

    /// Where the next step marker goes: its sequence, number and counting.
    fn step_place(s: &State) -> StepPlace {
        let scene = s.doc.scene();
        let sequence = steps::current_sequence(scene, s.step_sequence);
        StepPlace {
            sequence,
            number: s
                .step_next
                .unwrap_or_else(|| steps::next_number(scene, sequence)),
            counting: steps::counting(scene, sequence),
        }
    }

    /// Starts editing the text of object `id` (a text or callout), the caret at `at` (document
    /// space) or at the end; a double-click (`word`) selects the word there.
    fn start_text_edit(s: &mut State, id: ObjectId, at: Option<Point>, word: bool) -> bool {
        let Some(mut edit) = TextEdit::existing(&s.doc, id) else {
            return false;
        };
        if let Some(at) = at {
            edit.click(at, false);
            if word {
                edit.select_word(edit.caret());
            }
        }
        s.selection.set(s.doc.scene(), [id]);
        s.text = Some(edit);
        true
    }

    /// Primary button pressed at `at` (view coordinates).
    pub fn pointer_pressed(&self, at: Point, modifiers: Modifiers) {
        let Some(viewport) = self.viewport() else {
            return;
        };
        if self.view.imp().picker.borrow().is_some() {
            let colour = self.colour_at(viewport.to_document(at));
            self.finish_picking(colour);
            return;
        }
        // Text being edited: a press inside it places the caret or selects; a press outside
        // finishes editing and then does what it would have done.
        let doc_at = viewport.to_document(at);
        let tolerance = drag::HIT_TOLERANCE / viewport.zoom;
        let inside_text = self
            .with_state(|s| {
                s.pointer = (at.x, at.y);
                let presses = s.presses;
                let edit = s.text.as_mut()?;
                if !edit.object().hit_test(doc_at, tolerance) {
                    return Some(false);
                }
                if presses >= 2 {
                    edit.click(doc_at, false);
                    edit.select_word(edit.caret());
                } else {
                    edit.click(doc_at, modifiers.shift);
                    s.text_drag = true;
                }
                Some(true)
            })
            .flatten();
        match inside_text {
            Some(true) => {
                self.view.queue_draw();
                self.update_chips();
                return;
            }
            Some(false) => {
                self.finish_text();
            }
            None => {}
        }
        let begin_text = self
            .with_state(|s| {
                if let Some(crop) = s.crop.as_mut() {
                    crop.press(&viewport, at);
                    return false;
                }
                // A polygon in progress: a vertex per click; a double-click or a click on the
                // first vertex finishes it.
                if let Some(mut polygon) = s.polygon.take() {
                    if s.presses >= 2 || polygon.closes(&viewport, at) {
                        let toolbox = s.toolbox.borrow().clone();
                        if let Some(edit) = polygon.finish(&s.doc, &toolbox) {
                            s.pending = Some(edit);
                        }
                    } else {
                        polygon.click(&viewport, at);
                        s.polygon = Some(polygon);
                    }
                    return false;
                }
                let hit = drag::hit(&s.doc, &viewport, at);
                let on_handle = matches!(
                    drag::hover(&s.doc, &s.selection, &viewport, at),
                    Hover::Handle { .. }
                );
                let editable_hit = hit.filter(|id| {
                    s.doc
                        .scene()
                        .get(*id)
                        .is_some_and(|n| editable(n.object.as_ref()).is_some())
                });
                match s.tool {
                    Tool::Select => {
                        // A double-click on a text or callout edits its text in place.
                        if s.presses >= 2 && !on_handle {
                            if let Some(id) = editable_hit {
                                return Self::start_text_edit(s, id, Some(doc_at), true);
                            }
                        }
                        // A handle of the selection first, then a canvas edge or the
                        // corner-radius handle, then objects and the rubber band.
                        if !on_handle {
                            s.canvas_drag =
                                CanvasDrag::press(&s.doc, &viewport, at, s.selection.is_empty());
                            if s.canvas_drag.is_some() {
                                return false;
                            }
                        }
                        s.drag = Drag::press(&s.doc, &mut s.selection, &viewport, at, modifiers);
                        false
                    }
                    tool => {
                        // Drawing tools always draw, except on a handle of the selection
                        // (adjust what was just drawn) and, for the text and callout tools, on
                        // the selected text or callout (edit its text).
                        match route_press(&s.doc, &s.selection, &viewport, at, tool) {
                            Press::Handle => {
                                s.drag =
                                    Drag::press(&s.doc, &mut s.selection, &viewport, at, modifiers);
                                return false;
                            }
                            Press::EditText(id) => {
                                return Self::start_text_edit(s, id, Some(doc_at), false);
                            }
                            Press::Draw => {}
                        }
                        // A new object, which becomes the selection.
                        s.selection.clear();
                        let toolbox = s.toolbox.borrow().clone();
                        if tool == Tool::Shape && toolbox.draws_polygons() {
                            s.polygon = Some(PolygonDraft::new(doc_at));
                            return false;
                        }
                        let place = Self::step_place(s);
                        s.draw = Draw::press(&toolbox, tool, place, &viewport, at);
                        false
                    }
                }
            })
            .unwrap_or(false);
        if let Some(edit) = self.with_state(|s| s.pending.take()).flatten() {
            self.execute(edit);
        }
        if begin_text {
            self.text_started();
        }
        self.update_cursor();
        self.update_chips();
        self.view.queue_draw();
        self.notify_changed();
    }

    /// Pointer moved to `at` with the button held.
    pub fn pointer_moved(&self, at: Point, modifiers: Modifiers) {
        let Some(viewport) = self.viewport() else {
            return;
        };
        let cropping = self
            .with_state(|s| {
                s.pointer = (at.x, at.y);
                if let Some(crop) = s.crop.as_mut() {
                    crop.motion(&viewport, at);
                    return true;
                }
                if s.text_drag {
                    if let Some(edit) = s.text.as_mut() {
                        edit.drag_to(viewport.to_document(at));
                    }
                    return false;
                }
                if let Some(draw) = s.draw.as_mut() {
                    draw.motion(&viewport, at, modifiers);
                    return false;
                }
                if let Some(canvas_drag) = s.canvas_drag.as_mut() {
                    canvas_drag.motion(&viewport, at);
                    return false;
                }
                let State {
                    doc,
                    selection,
                    drag,
                    snapping,
                    ..
                } = s;
                if let Some(drag) = drag.as_mut() {
                    drag.motion(doc, selection, &viewport, at, modifiers, *snapping);
                }
                false
            })
            .unwrap_or(false);
        self.update_hover();
        self.update_chips();
        self.view.queue_draw();
        if cropping {
            // The sidebar's X/Y/W/H and the subtitle follow the frame.
            self.notify_changed();
        }
    }

    /// Button released: the gesture's change becomes one undo step.
    pub fn pointer_released(&self) {
        enum Outcome {
            Edit(Edit),
            Text(TextEdit),
            Nothing,
        }
        let outcome = self
            .with_state(|s| {
                if let Some(crop) = s.crop.as_mut() {
                    crop.release();
                    return Outcome::Nothing;
                }
                if std::mem::take(&mut s.text_drag) {
                    return Outcome::Nothing;
                }
                if let Some(draw) = s.draw.take() {
                    let step = draw.tool() == Tool::Step;
                    return match draw.release(&s.doc) {
                        Some(Drawn::Edit(edit)) => {
                            if step {
                                // "Next value" applies to one marker; then numbering goes on.
                                s.step_next = None;
                            }
                            Outcome::Edit(edit)
                        }
                        Some(Drawn::Text(text)) => Outcome::Text(text),
                        None => Outcome::Nothing,
                    };
                }
                if let Some(canvas_drag) = s.canvas_drag.take() {
                    return canvas_drag
                        .release(&s.doc)
                        .map_or(Outcome::Nothing, Outcome::Edit);
                }
                let Some(drag) = s.drag.take() else {
                    return Outcome::Nothing;
                };
                drag.release(&s.doc, &mut s.selection)
                    .map_or(Outcome::Nothing, Outcome::Edit)
            })
            .unwrap_or(Outcome::Nothing);
        match outcome {
            Outcome::Edit(edit) => {
                self.execute(edit);
            }
            Outcome::Text(text) => {
                self.with_state(|s| {
                    s.selection.clear();
                    s.text = Some(text);
                });
                self.text_started();
            }
            Outcome::Nothing => {}
        }
        self.update_hover();
        self.update_chips();
        self.update_cursor();
        self.view.queue_draw();
        self.notify_changed();
    }

    /// Drops the gesture in progress (Escape, or the grab was broken).
    fn cancel_drag(&self) -> bool {
        let cancelled = self
            .with_state(|s| {
                if let Some(crop) = s.crop.as_mut() {
                    let dragging = crop.is_dragging();
                    crop.release();
                    return dragging;
                }
                s.text_drag = false;
                if s.draw.take().is_some() {
                    return true;
                }
                if s.canvas_drag.take().is_some() {
                    return true;
                }
                match s.drag.take() {
                    Some(drag) => {
                        drag.cancel(&mut s.selection);
                        true
                    }
                    None => false,
                }
            })
            .unwrap_or(false);
        self.update_chips();
        self.view.queue_draw();
        cancelled
    }

    // ---- tools ----

    /// The tool the canvas draws with.
    pub fn tool(&self) -> Tool {
        self.with_state(|s| s.tool).unwrap_or_default()
    }

    /// Switches the tool (the tool strip, the tools' keys, Esc). Anything in progress ends:
    /// text being edited is kept, a polygon or a draw is dropped, crop mode ends.
    pub fn set_tool(&self, tool: Tool) {
        if self.tool() == tool && !self.cropping() {
            return;
        }
        self.cancel_drag();
        self.finish_text();
        self.with_state(|s| s.polygon = None);
        self.end_crop();
        self.with_state(|s| s.tool = tool);
        self.update_cursor();
        self.update_chips();
        self.view.queue_draw();
        self.notify_changed();
    }

    /// The tools' defaults; the window shares one set between its documents.
    pub fn set_toolbox(&self, toolbox: Rc<RefCell<Toolbox>>) {
        self.with_state(|s| s.toolbox = toolbox);
    }

    /// Restart Sequence: the next step marker starts a new sequence at 1 (or A). The step
    /// properties (add-core-annotation-tools task 3.1) will call this and `set_next_step`.
    #[allow(dead_code)]
    pub fn restart_sequence(&self) {
        self.with_state(|s| {
            s.step_sequence = Some(steps::fresh_sequence(s.doc.scene()));
            s.step_next = None;
        });
        self.view.queue_draw();
        self.update_chips();
    }

    /// "Next value": the number the next step marker gets (`None`: after the highest).
    #[allow(dead_code)]
    pub fn set_next_step(&self, number: Option<u32>) {
        self.with_state(|s| s.step_next = number.filter(|n| *n > 0));
        self.view.queue_draw();
        self.update_chips();
    }

    /// Whether a polygon is being drawn (header subtitle "Drawing", mockup 07).
    pub fn drawing_polygon(&self) -> bool {
        self.with_state(|s| s.polygon.is_some()).unwrap_or(false)
    }

    /// Whether text is being edited in place (the tools' keys type instead).
    pub fn editing_text(&self) -> bool {
        self.with_state(|s| s.text.is_some()).unwrap_or(false)
    }

    /// The text being edited.
    #[cfg(test)]
    pub fn edited_text(&self) -> Option<String> {
        self.with_state(|s| s.text.as_ref().map(|t| t.text().to_string()))
            .flatten()
    }

    /// Starts editing the text of the selected text or callout (Enter).
    fn edit_selected_text(&self) -> bool {
        let started = self
            .with_state(|s| match s.selection.ids() {
                [id] => {
                    let id = *id;
                    Self::start_text_edit(s, id, None, false)
                }
                _ => false,
            })
            .unwrap_or(false);
        if started {
            self.text_started();
        }
        started
    }

    /// The input method takes the keys while text is edited.
    fn text_started(&self) {
        if let Some((im, typing)) = self.view.imp().im.borrow().as_ref() {
            typing.set_im_context(Some(im));
            im.focus_in();
        }
        self.view.grab_focus();
        self.update_cursor();
        self.update_chips();
        self.view.queue_draw();
        self.notify_changed();
    }

    /// Typed text (the input method's commit) goes into the text being edited.
    pub fn insert_text(&self, text: &str) {
        let typed = self
            .with_state(|s| s.text.as_mut().map(|t| t.insert(text)).is_some())
            .unwrap_or(false);
        if typed {
            self.view.queue_draw();
            self.update_chips();
        }
    }

    /// Ends editing text in place (Esc, a click outside, another tool, undo …): the change is
    /// one undo step. Returns whether text was being edited.
    pub fn finish_text(&self) -> bool {
        let Some(edit) = self
            .with_state(|s| {
                s.text_drag = false;
                s.text.take()
            })
            .flatten()
        else {
            return false;
        };
        if let Some((im, typing)) = self.view.imp().im.borrow().as_ref() {
            im.focus_out();
            im.reset();
            typing.set_im_context(None::<&gtk::IMContext>);
        }
        let change = self.with_state(|s| edit.finish(&s.doc)).flatten();
        match change {
            Some(change) => {
                self.execute(change);
            }
            None => self.notify_changed(),
        }
        self.update_cursor();
        self.update_chips();
        self.view.queue_draw();
        true
    }

    /// Edits the text being edited and redraws.
    fn edit_text(&self, f: impl FnOnce(&mut TextEdit)) -> bool {
        self.with_state(|s| s.text.as_mut().map(f));
        self.view.queue_draw();
        self.update_chips();
        true
    }

    /// Keys while text is edited in place (mockup 08). Printable keys are not handled here:
    /// they go to the input method.
    fn text_key(&self, key: gdk::Key, modifiers: gdk::ModifierType) -> bool {
        let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
        let shift = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
        let step = |word: Motion, plain: Motion| if ctrl { word } else { plain };
        match key {
            gdk::Key::Escape => {
                self.finish_text();
                true
            }
            gdk::Key::BackSpace => self.edit_text(|t| t.backspace(ctrl)),
            gdk::Key::Delete | gdk::Key::KP_Delete => self.edit_text(|t| t.delete(ctrl)),
            gdk::Key::Left | gdk::Key::KP_Left => {
                self.edit_text(|t| t.move_caret(step(Motion::WordLeft, Motion::Left), shift))
            }
            gdk::Key::Right | gdk::Key::KP_Right => {
                self.edit_text(|t| t.move_caret(step(Motion::WordRight, Motion::Right), shift))
            }
            gdk::Key::Up | gdk::Key::KP_Up => self.edit_text(|t| t.move_caret(Motion::Up, shift)),
            gdk::Key::Down | gdk::Key::KP_Down => {
                self.edit_text(|t| t.move_caret(Motion::Down, shift))
            }
            gdk::Key::Home | gdk::Key::KP_Home => {
                self.edit_text(|t| t.move_caret(step(Motion::Start, Motion::LineStart), shift))
            }
            gdk::Key::End | gdk::Key::KP_End => {
                self.edit_text(|t| t.move_caret(step(Motion::End, Motion::LineEnd), shift))
            }
            gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter => {
                self.edit_text(|t| t.insert("\n"))
            }
            _ if ctrl => match key.to_lower() {
                gdk::Key::a => self.edit_text(TextEdit::select_all),
                gdk::Key::c | gdk::Key::x => {
                    let cut = key.to_lower() == gdk::Key::x;
                    let selected = self
                        .with_state(|s| s.text.as_ref().map(|t| t.selected_text().to_string()))
                        .flatten()
                        .filter(|t| !t.is_empty());
                    if let Some(selected) = selected {
                        self.view.clipboard().set_text(&selected);
                        if cut {
                            self.edit_text(|t| t.insert(""));
                        }
                    }
                    true
                }
                gdk::Key::v => {
                    let weak = self.downgrade();
                    self.view.clipboard().read_text_async(
                        None::<&gtk::gio::Cancellable>,
                        move |result| {
                            if let (Some(canvas), Ok(Some(text))) = (weak.upgrade(), result) {
                                canvas.insert_text(&text);
                            }
                        },
                    );
                    true
                }
                gdk::Key::b => self.edit_text(|t| {
                    t.toggle("bold");
                }),
                gdk::Key::i => self.edit_text(|t| {
                    t.toggle("italic");
                }),
                gdk::Key::u => self.edit_text(|t| {
                    t.toggle("underline");
                }),
                // Undo and redo wait until editing ends (it is one step then).
                gdk::Key::z => true,
                _ => false,
            },
            _ => false,
        }
    }

    fn canvas_rect(&self) -> Option<IRect> {
        self.with_state(|s| s.doc.canvas().rect)
    }

    /// After the canvas changed size (crop, rotate, resize, canvas edges, undo of those): a
    /// fitted view fits the new canvas, any other view gets the new scroll range.
    fn canvas_changed(&self, before: Option<IRect>) {
        let after = self.canvas_rect();
        if before == after {
            return;
        }
        let imp = self.view.imp();
        let size = imp.view_size();
        let refit = self
            .with_state(|s| {
                (s.fitted && size.0 >= 1.0 && size.1 >= 1.0).then(|| {
                    s.zoom = view::fit_zoom(s.doc.canvas().rect, size);
                    s.zoom
                })
            })
            .flatten();
        imp.configure_adjustments(None);
        if let Some(zoom) = refit {
            imp.notify_view(zoom, true);
        }
    }

    /// Called with things the window announces in a toast (steps renumbered).
    pub fn connect_notice(&self, f: impl Fn(Notice) + 'static) {
        self.view.imp().notices.borrow_mut().push(Box::new(f));
    }

    /// Executes `edit` as one undo step and updates the selection.
    fn execute(&self, edit: Edit) -> Option<Done> {
        let before = self.canvas_rect();
        let label = labels::undo_label(&edit);
        let (action, subject) = (edit.action, edit.subject);
        // Deleting step markers renumbers others (mockup 13 announces it).
        let removed = removed_ids(&edit.command);
        let renumbering = self
            .with_state(|s| {
                (action == Action::Delete && !removed.is_empty())
                    .then(|| steps::deletion_summary(s.doc.scene(), &removed))
            })
            .flatten()
            .filter(|(_, n)| *n > 0);
        let done = self
            .with_state(
                |s| match s.history.execute(&mut s.doc, label, edit.command) {
                    Ok(()) => {
                        match edit.select {
                            Some(ids) => s.selection.set(s.doc.scene(), ids),
                            None => s.selection.sync(s.doc.scene()),
                        }
                        Some(Done {
                            action,
                            subject,
                            revision: s.history.revision(),
                        })
                    }
                    Err(err) => {
                        eprintln!("annota: canvas: {err}");
                        None
                    }
                },
            )
            .flatten();
        self.canvas_changed(before);
        self.view.queue_draw();
        self.notify_changed();
        if let (Some(done), Some((deleted, renumbered))) = (done, renumbering) {
            for listener in self.view.imp().notices.borrow().iter() {
                listener(Notice::StepsRenumbered {
                    deleted: deleted.clone(),
                    renumbered,
                    done,
                });
            }
        }
        done
    }

    /// Ends whatever is in progress before an operation on the whole document: a gesture,
    /// text editing (kept), a polygon (dropped), crop mode.
    fn end_modes(&self) {
        self.cancel_drag();
        self.finish_text();
        self.with_state(|s| s.polygon = None);
        self.end_crop();
    }

    /// Runs a one-shot operation on the document and the selection as one undo step (ends crop
    /// mode and any gesture first). Returns what was done, or `None` if nothing changed.
    pub fn run(&self, op: impl FnOnce(&Document, &Selection) -> Option<Edit>) -> Option<Done> {
        self.end_modes();
        let edit = self.with_state(|s| op(&s.doc, &s.selection)).flatten()?;
        self.execute(edit)
    }

    /// Runs a one-shot operation on the selection; returns whether it changed anything.
    fn edit(&self, op: impl FnOnce(&Document, &Selection) -> Option<Edit>) -> bool {
        match self.with_state(|s| op(&s.doc, &s.selection)).flatten() {
            Some(edit) => self.execute(edit).is_some(),
            None => false,
        }
    }

    pub fn undo(&self) {
        self.end_modes();
        let before = self.canvas_rect();
        self.with_state(|s| {
            if let Err(err) = s.history.undo(&mut s.doc) {
                eprintln!("annota: canvas: {err}");
            }
            s.selection.sync(s.doc.scene());
        });
        self.canvas_changed(before);
        self.view.queue_draw();
        self.notify_changed();
    }

    pub fn redo(&self) {
        self.end_modes();
        let before = self.canvas_rect();
        self.with_state(|s| {
            if let Err(err) = s.history.redo(&mut s.doc) {
                eprintln!("annota: canvas: {err}");
            }
            s.selection.sync(s.doc.scene());
        });
        self.canvas_changed(before);
        self.view.queue_draw();
        self.notify_changed();
    }

    /// Undoes the step `done` made, if it is still the last one (a toast's Undo button).
    pub fn undo_if_last(&self, done: &Done) {
        if self.history_info().revision == done.revision {
            self.undo();
        }
    }

    // ---- eyedropper ----

    /// The eyedropper (sidebar: Canvas › Pick Colour From Image): the next click on the canvas
    /// picks the colour shown there and hands it to `f`; Escape hands `None`.
    pub fn pick_colour(&self, f: impl FnOnce(Option<Rgba>) + 'static) {
        self.end_modes();
        if let Some(previous) = self.view.imp().picker.replace(Some(Box::new(f))) {
            previous(None);
        }
        self.view.set_cursor_from_name(Some("crosshair"));
        self.view.grab_focus();
    }

    fn finish_picking(&self, colour: Option<Rgba>) -> bool {
        let picker = self.view.imp().picker.take();
        self.update_cursor();
        match picker {
            Some(f) => {
                f(colour);
                true
            }
            None => false,
        }
    }

    /// The exported colour of the document pixel at `at` (one pixel rendered: only the objects
    /// on it are drawn); `None` outside the canvas or where it is transparent.
    fn colour_at(&self, at: Point) -> Option<Rgba> {
        let (x, y) = (at.x.floor(), at.y.floor());
        self.with_state(|s| {
            let canvas = s.doc.canvas().rect;
            let inside = x >= f64::from(canvas.x)
                && y >= f64::from(canvas.y)
                && x < f64::from(canvas.right())
                && y < f64::from(canvas.bottom());
            if !inside {
                return None;
            }
            let pixel = Bitmap::draw(1, 1, |cr| {
                render::render(
                    cr,
                    &s.doc,
                    &Viewport::new(1.0, Point::new(-x, -y)),
                    &Options::EXPORT,
                )
            })
            .ok()?
            .pixel(0, 0)?;
            (pixel.a > 0).then_some(Rgba::opaque(pixel.r, pixel.g, pixel.b))
        })
        .flatten()
    }

    // ---- crop mode ----

    /// Enters crop mode (tool C): the frame starts as the whole canvas; the selection is cleared.
    /// Not in a document with objects of a newer Annota, which could not follow a crop.
    pub fn start_crop(&self) {
        if self
            .with_document(image_ops::has_unknown_objects)
            .unwrap_or(true)
        {
            self.notify_changed();
            return;
        }
        self.cancel_drag();
        self.finish_text();
        self.with_state(|s| s.polygon = None);
        self.with_state(|s| {
            if s.crop.is_none() {
                s.crop = Some(Crop::new(&s.doc));
                s.selection.clear();
                s.hover = None;
            }
        });
        self.update_chips();
        self.update_cursor();
        self.view.queue_draw();
        self.notify_changed();
    }

    /// Leaves crop mode without cropping (Esc, Cancel, another tool).
    pub fn end_crop(&self) {
        let ended = self
            .with_state(|s| s.crop.take().is_some())
            .unwrap_or(false);
        if ended {
            self.update_chips();
            self.update_cursor();
            self.view.queue_draw();
            self.notify_changed();
        }
    }

    pub fn cropping(&self) -> bool {
        self.with_state(|s| s.crop.is_some()).unwrap_or(false)
    }

    /// The crop frame (crop mode only).
    pub fn crop_frame(&self) -> Option<IRect> {
        self.with_state(|s| s.crop.as_ref().map(Crop::frame))
            .flatten()
    }

    pub fn crop_aspect(&self) -> Option<Aspect> {
        self.with_state(|s| s.crop.as_ref().map(Crop::aspect))
            .flatten()
    }

    /// Whether Crop (Enter) would change anything.
    pub fn can_apply_crop(&self) -> bool {
        self.with_state(|s| s.crop.as_ref().is_some_and(|c| c.can_apply(&s.doc)))
            .unwrap_or(false)
    }

    /// The sidebar's X/Y/W/H in crop mode.
    pub fn set_crop_frame(&self, rect: IRect) {
        self.with_state(|s| {
            if let Some(crop) = s.crop.as_mut() {
                crop.set_frame(rect);
            }
        });
        self.update_chips();
        self.view.queue_draw();
        self.notify_changed();
    }

    /// The sidebar's aspect-ratio choice in crop mode.
    pub fn set_crop_aspect(&self, aspect: Aspect) {
        self.with_state(|s| {
            let State { crop, doc, .. } = s;
            if let Some(crop) = crop.as_mut() {
                crop.set_aspect(aspect, doc);
            }
        });
        self.update_chips();
        self.view.queue_draw();
        self.notify_changed();
    }

    /// Crop (Enter): crops to the frame as one undo step and leaves crop mode. Returns whether
    /// the document changed; without a change crop mode stays on.
    pub fn apply_crop(&self) -> bool {
        let Some(frame) = self.crop_frame() else {
            return false;
        };
        if !self.can_apply_crop() {
            return false;
        }
        self.run(|doc, _| image_ops::crop(doc, frame)).is_some()
    }

    #[cfg(test)]
    /// Label of the step Ctrl+Z would undo (Undo tooltip, mockup 06).
    pub fn undo_label(&self) -> Option<String> {
        self.with_state(|s| s.history.undo_label().map(str::to_string))
            .flatten()
    }

    /// The selected objects, in stacking order.
    pub fn selection(&self) -> Vec<ObjectId> {
        self.with_state(|s| s.selection.ids().to_vec())
            .unwrap_or_default()
    }

    /// "Select All <Type>" (sidebar) and the per-type "Select" buttons (mockup 16).
    pub fn select_type(&self, type_name: &str) {
        self.end_modes();
        self.with_state(|s| s.selection.select_type(s.doc.scene(), type_name));
        self.view.queue_draw();
        self.notify_changed();
    }

    /// The sidebar's Arrange entries; returns whether the document changed.
    pub fn set_arrange(&self, arrange: Arrange) -> bool {
        self.edit(|doc, sel| ops::arrange(doc, sel, arrange))
    }

    /// The sidebar's Ungroup button (Ctrl+Shift+G).
    pub fn ungroup(&self) -> bool {
        self.edit(ops::ungroup)
    }

    /// Inserts `bitmap` as an image object centred on `centre` (document space); returns the
    /// revision after the insertion, for the toast's Undo.
    pub fn insert_image(&self, bitmap: Bitmap, centre: Point) -> Option<u64> {
        self.end_modes();
        let inserted = self.edit(|doc, _| ops::insert_image(doc, bitmap, centre));
        inserted.then(|| self.history_info().revision)
    }

    /// Snap to guides on or off (`Ctrl+;`, the sidebar switch).
    pub fn set_snapping(&self, on: bool) {
        self.with_state(|s| s.snapping = on);
    }

    pub fn snapping(&self) -> bool {
        self.with_state(|s| s.snapping).unwrap_or(true)
    }

    /// Runs `f` on the document (read-only).
    pub fn with_document<T>(&self, f: impl FnOnce(&Document) -> T) -> Option<T> {
        self.with_state(|s| f(&s.doc))
    }

    /// Shows the grabber of the canvas edge (or the corner-radius handle) under the pointer.
    fn update_hover(&self) {
        let Some(viewport) = self.viewport() else {
            return;
        };
        let changed = self
            .with_state(|s| {
                let pointer = Point::new(s.pointer.0, s.pointer.1);
                let busy = s.crop.is_some()
                    || s.space
                    || s.pan_start.is_some()
                    || s.drag.as_ref().is_some_and(Drag::is_dragging);
                let next = match &s.canvas_drag {
                    Some(d) => Some(d.target()),
                    None if busy => None,
                    None => {
                        let on_handle = matches!(
                            drag::hover(&s.doc, &s.selection, &viewport, pointer),
                            Hover::Handle { .. }
                        );
                        (!on_handle)
                            .then(|| {
                                canvas_edit::target_at(
                                    &s.doc,
                                    &viewport,
                                    pointer,
                                    s.selection.is_empty(),
                                )
                            })
                            .flatten()
                    }
                };
                let changed = s.hover != next;
                s.hover = next;
                changed
            })
            .unwrap_or(false);
        if changed {
            self.view.queue_draw();
        }
    }

    /// Cursor for what is under the pointer (system cursor theme; redline "Kit additions").
    fn update_cursor(&self) {
        let Some(viewport) = self.viewport() else {
            return;
        };
        if self.view.imp().picker.borrow().is_some() {
            self.view.set_cursor_from_name(Some("crosshair"));
            return;
        }
        let name = self
            .with_state(|s| {
                if s.space || s.pan_start.is_some() {
                    return None;
                }
                let pointer = Point::new(s.pointer.0, s.pointer.1);
                if let Some(crop) = &s.crop {
                    return match crop.grip_at(&viewport, pointer) {
                        Some(annota::editing::crop::Grip::Handle(compass)) => {
                            Some(drag::resize_cursor(compass, 0.0))
                        }
                        Some(annota::editing::crop::Grip::Inside) => Some("move"),
                        None => None,
                    };
                }
                // Typing in place: an I-beam over the text, elsewhere a click finishes.
                if let Some(text) = &s.text {
                    let at = viewport.to_document(pointer);
                    let over = text
                        .object()
                        .hit_test(at, drag::HIT_TOLERANCE / viewport.zoom);
                    return over.then_some("text");
                }
                if s.polygon.is_some() || s.draw.is_some() {
                    return Some("crosshair");
                }
                match s.hover {
                    Some(CanvasTarget::Edge(side)) if side.horizontal() => {
                        return Some("ew-resize")
                    }
                    Some(CanvasTarget::Edge(_)) => return Some("ns-resize"),
                    Some(CanvasTarget::Radius) => return Some("nwse-resize"),
                    None => {}
                }
                let hover = match &s.drag {
                    Some(d) if d.is_dragging() => match d.feedback() {
                        Feedback::Move { .. } => Hover::Object,
                        Feedback::Rotate { .. } => {
                            return Some("grabbing");
                        }
                        _ => drag::hover(&s.doc, &s.selection, &viewport, pointer),
                    },
                    _ => drag::hover(&s.doc, &s.selection, &viewport, pointer),
                };
                // Drawing tools draw wherever they do not grab a handle of the selection
                // (mockup 03; `route_press`), also over objects.
                let dragging = s.drag.as_ref().is_some_and(Drag::is_dragging);
                match s.tool {
                    Tool::Select => {}
                    _ if dragging => {}
                    tool => match route_press(&s.doc, &s.selection, &viewport, pointer, tool) {
                        Press::Handle => {}
                        Press::EditText(_) => return Some("text"),
                        Press::Draw if tool == Tool::Text => return Some("text"),
                        Press::Draw => return Some("crosshair"),
                    },
                }
                match hover {
                    Hover::Handle {
                        handle: HandleId::Resize(compass),
                        rotation,
                    } => Some(drag::resize_cursor(compass, rotation)),
                    Hover::Handle {
                        handle: HandleId::Rotate,
                        ..
                    } => Some("grab"),
                    Hover::Handle { .. } => Some("crosshair"),
                    Hover::Object => Some("move"),
                    Hover::Nothing => None,
                }
            })
            .flatten();
        self.view.set_cursor_from_name(name);
    }

    /// The chip's own size: the measured size includes the margins that place it.
    fn chip_size(label: &gtk::Label) -> (f64, f64) {
        let (_, natural) = label.preferred_size();
        (
            f64::from(natural.width() - label.margin_start() - label.margin_end()),
            f64::from(natural.height() - label.margin_top() - label.margin_bottom()),
        )
    }

    /// Places `label` at (x, y) in the view, kept inside it.
    fn place_chip(&self, label: &gtk::Label, x: f64, y: f64) {
        let view_size = self.view.imp().view_size();
        let (w, h) = Self::chip_size(label);
        let x = x.clamp(0.0, (view_size.0 - w).max(0.0));
        let y = y.clamp(0.0, (view_size.1 - h).max(0.0));
        label.set_margin_start(x.round() as i32);
        label.set_margin_top(y.round() as i32);
    }

    /// The crop size chip under the frame (mockup 08), or the canvas-edge / corner-radius chip
    /// (mockups 09, 10). Returns whether one of them is shown.
    fn update_canvas_chips(&self, viewport: &Viewport) -> bool {
        enum Chip {
            Crop(IRect),
            Canvas(canvas_edit::Feedback, IRect),
        }
        let chip = self
            .with_state(|s| {
                if let Some(crop) = &s.crop {
                    return Some(Chip::Crop(crop.frame()));
                }
                let d = s.canvas_drag.as_ref().filter(|d| d.is_dragging())?;
                Some(Chip::Canvas(
                    d.feedback(),
                    d.preview_canvas().unwrap_or(s.doc.canvas().rect),
                ))
            })
            .flatten();
        let chip_label = &self.measure_chip;
        match chip {
            Some(Chip::Crop(frame)) => {
                chip_label.set_label(&labels::crop_chip(frame.width, frame.height));
                chip_label.set_visible(true);
                let r = frame.to_rect();
                let a = viewport.to_device(Point::new(r.x, r.y));
                let b = viewport.to_device(Point::new(r.right(), r.bottom()));
                let (w, _) = Self::chip_size(chip_label);
                // Centred under the frame, 12 px below (mockup 08).
                self.place_chip(chip_label, (a.x + b.x) / 2.0 - w / 2.0, b.y + 12.0);
                true
            }
            Some(Chip::Canvas(feedback, canvas)) => {
                let Some(text) = labels::canvas_chip(feedback) else {
                    chip_label.set_visible(false);
                    return true;
                };
                chip_label.set_label(&text);
                chip_label.set_visible(true);
                let (w, h) = Self::chip_size(chip_label);
                let r = canvas.to_rect();
                let a = viewport.to_device(Point::new(r.x, r.y));
                let b = viewport.to_device(Point::new(r.right(), r.bottom()));
                let (mx, my) = ((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
                let (x, y) = match feedback {
                    // Mockup 10: inside the canvas next to the edge, below the grabber.
                    canvas_edit::Feedback::Extend { side, .. } => match side {
                        Side::Right => (b.x - 14.0 - w, my + 26.0),
                        Side::Left => (a.x + 14.0, my + 26.0),
                        Side::Top => (mx + 26.0, a.y + 14.0),
                        Side::Bottom => (mx + 26.0, b.y - 14.0 - h),
                    },
                    // Mockup 09: above the image's top-left corner.
                    _ => {
                        let corner = viewport.to_device(Point::new(0.0, 0.0));
                        (corner.x - 4.0, corner.y - 30.0)
                    }
                };
                self.place_chip(chip_label, x, y);
                true
            }
            None => false,
        }
    }

    /// Shows, fills and places the measurement and snap chips for the gesture in progress.
    /// The chip of a drawing tool, if one shows (redline "On-canvas handles"): typing in place,
    /// drawing a polygon, Shift while drawing an arrow or line, placing a step marker. Returns
    /// whether one is shown.
    fn update_drawing_chip(&self, viewport: &Viewport) -> bool {
        enum Place {
            /// Right of the text's editing frame, level with its top (mockup 08).
            RightOf(Point),
            /// Right of and below the pointer (mockup 07).
            Pointer,
            /// Centred below a point (mockup 03).
            Below(Point),
            /// Right of the placement preview (mockup 11).
            Beside(Point, f64),
        }
        let chip = self
            .with_state(|s| {
                let pointer = Point::new(s.pointer.0, s.pointer.1);
                if let Some(text) = &s.text {
                    let frame = text.block()?.bounds();
                    let corners = frame
                        .rect
                        .corners()
                        .map(|p| viewport.to_device(frame.to_document(p)));
                    let right = corners.iter().map(|p| p.x).fold(f64::MIN, f64::max);
                    let top = corners.iter().map(|p| p.y).fold(f64::MAX, f64::min);
                    let gap = overlay::TEXT_FRAME_OFFSET + 6.0;
                    return Some((
                        labels::text_edit_chip(),
                        Place::RightOf(Point::new(right + gap, top - 3.0)),
                    ));
                }
                if let Some(polygon) = &s.polygon {
                    return Some((
                        labels::polygon_chip(polygon.vertices().len()),
                        Place::Pointer,
                    ));
                }
                if let Some(draw) = &s.draw {
                    let DrawFeedback::Angle { degrees } = draw.feedback() else {
                        return None;
                    };
                    let line = draw.preview()?.arrange().rect;
                    let mid = viewport.to_device(Point::new(line.centre().x, line.bottom()));
                    return Some((labels::angle_chip(degrees), Place::Below(mid)));
                }
                let ghost = step_ghost(s, viewport)?;
                Some((
                    labels::step_chip(&ghost.label()),
                    Place::Beside(pointer, ghost.radius * viewport.zoom),
                ))
            })
            .flatten();
        let Some((text, place)) = chip else {
            return false;
        };
        let pointer = self
            .with_state(|s| Point::new(s.pointer.0, s.pointer.1))
            .unwrap_or_default();
        let label = &self.measure_chip;
        label.set_label(&text);
        label.set_visible(true);
        let (w, h) = Self::chip_size(label);
        let (x, y) = match place {
            Place::RightOf(p) => (p.x, p.y),
            Place::Pointer => (pointer.x + 16.0, pointer.y + 11.0),
            Place::Below(p) => (p.x - w / 2.0, p.y + 18.0),
            Place::Beside(p, r) => (p.x + r + 12.0, p.y - h / 2.0),
        };
        self.place_chip(label, x, y);
        true
    }

    fn update_chips(&self) {
        let Some(viewport) = self.viewport() else {
            return;
        };
        if self.update_canvas_chips(&viewport) || self.update_drawing_chip(&viewport) {
            self.snap_chip.set_visible(false);
            return;
        }
        let info = self.with_state(|s| {
            let drag = s.drag.as_ref().filter(|d| d.is_dragging())?;
            let frame = drag
                .selection_frame(&s.doc, &s.selection)
                .map(|f| f.frame.bounds());
            Some((
                drag.feedback(),
                drag.guides().to_vec(),
                frame,
                Point::new(s.pointer.0, s.pointer.1),
                s.doc.canvas().rect,
            ))
        });
        let Some(Some((feedback, guides, frame, pointer, canvas))) = info else {
            self.measure_chip.set_visible(false);
            self.snap_chip.set_visible(false);
            return;
        };

        match labels::measure_chip(feedback) {
            Some(text) => {
                self.measure_chip.set_label(&text);
                self.measure_chip.set_visible(true);
                let (w, h) = Self::chip_size(&self.measure_chip);
                let (x, y) = match feedback {
                    // Mockup 03: centred above the pointer.
                    Feedback::Rotate { .. } => (pointer.x - w / 2.0, pointer.y - 18.0 - h),
                    // Mockups 02, 06: right of the pointer, below the box being moved.
                    _ => {
                        let below = frame.map_or(pointer.y, |f| {
                            viewport.to_device(Point::new(f.x, f.bottom())).y
                        });
                        (
                            pointer.x + CHIP_OFFSET,
                            (pointer.y + CHIP_OFFSET).max(below + CHIP_GAP),
                        )
                    }
                };
                self.place_chip(&self.measure_chip, x, y);
            }
            None => self.measure_chip.set_visible(false),
        }

        match labels::snap_chip(&Snap::targets(&guides)) {
            Some(text) => {
                self.snap_chip.set_label(&text);
                self.snap_chip.set_visible(true);
                let r = canvas.to_rect();
                let top_left = viewport.to_device(Point::new(r.x, r.y));
                let (_, h) = Self::chip_size(&self.snap_chip);
                // Mockup 02: beside the top end of a vertical guide, else beside the left end
                // of a horizontal one.
                let (x, y) = match guides.iter().find(|g| g.axis == Axis::Vertical) {
                    Some(g) => (
                        viewport.to_device(Point::new(g.pos, 0.0)).x + CHIP_GAP,
                        top_left.y - overlay::GUIDE_OVERSHOOT + 4.0,
                    ),
                    None => (
                        top_left.x - overlay::GUIDE_OVERSHOOT + 4.0,
                        guides
                            .first()
                            .map_or(0.0, |g| viewport.to_device(Point::new(0.0, g.pos)).y)
                            - CHIP_GAP
                            - h,
                    ),
                };
                self.place_chip(&self.snap_chip, x, y);
            }
            None => self.snap_chip.set_visible(false),
        }
    }

    /// Zoom to fit (Ctrl+0): the whole canvas visible and centred, at most 100 %.
    pub fn zoom_fit(&self) {
        let view_size = self.view.imp().view_size();
        if view_size.0 < 1.0 || view_size.1 < 1.0 {
            // Not allocated yet: the first allocation fits.
            self.with_state(|s| s.fitted = true);
            return;
        }
        if let Some(canvas) = self.with_state(|s| s.doc.canvas().rect) {
            self.set_zoom(view::fit_zoom(canvas, view_size), None, true);
        }
    }

    /// Zoom to `zoom` keeping the document point under `anchor` (view coordinates; default
    /// the view centre) in place. Leaves fit mode.
    pub fn zoom_to(&self, zoom: f64, anchor: Option<(f64, f64)>) {
        self.set_zoom(zoom, anchor, false);
    }

    /// "Fit Width" (zoom popover): the canvas width fills the view.
    pub fn zoom_fit_width(&self) {
        let view_size = self.view.imp().view_size();
        if let Some(canvas) = self.with_state(|s| s.doc.canvas().rect) {
            self.zoom_to(view::fit_width_zoom(canvas, view_size), None);
        }
    }

    pub fn zoom_in(&self) {
        self.zoom_to(view::zoom_in(self.zoom()), None);
    }

    pub fn zoom_out(&self) {
        self.zoom_to(view::zoom_out(self.zoom()), None);
    }

    fn set_zoom(&self, zoom: f64, anchor: Option<(f64, f64)>, fitted: bool) {
        let zoom = view::clamp_zoom(zoom);
        let imp = self.view.imp();
        let view_size = imp.view_size();
        let anchor = anchor.unwrap_or((view_size.0 / 2.0, view_size.1 / 2.0));
        let Some((canvas, old_zoom, was_fitted)) =
            self.with_state(|s| (s.doc.canvas().rect, s.zoom, s.fitted))
        else {
            return;
        };
        if (zoom - old_zoom).abs() < 1e-9 && fitted == was_fitted {
            return;
        }
        let scroll = view::scroll_for_zoom(canvas, view_size, old_zoom, imp.scroll(), zoom, anchor);
        self.with_state(|s| {
            s.zoom = zoom;
            s.fitted = fitted;
        });
        // The adjustments are ours: range and position change together, nothing waits for a
        // new allocation.
        imp.configure_adjustments(Some(scroll));
        imp.notify_view(zoom, fitted);
    }

    /// Keyboard zoom, space-to-pan and the Select tool's keys; returns whether the key was
    /// used. (Window-level shortcuts, actions and menus arrive with the editor window, task
    /// 4.1; the keys follow the redlines: context menu, "Object selection" and "Snap to
    /// guides".)
    pub fn key_pressed(&self, key: gdk::Key, modifiers: gdk::ModifierType) -> bool {
        let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
        let shift = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
        let alt = modifiers.contains(gdk::ModifierType::ALT_MASK);
        if key == gdk::Key::Escape && self.finish_picking(None) {
            return true;
        }
        if self.editing_text() {
            return self.text_key(key, modifiers);
        }
        let dragging = self
            .with_state(|s| {
                s.drag.as_ref().is_some_and(Drag::is_dragging)
                    || s.draw.as_ref().is_some_and(Draw::is_dragging)
                    || s.canvas_drag.as_ref().is_some_and(CanvasDrag::is_dragging)
                    || s.crop.as_ref().is_some_and(Crop::is_dragging)
            })
            .unwrap_or(false);
        // A polygon in progress (mockup 07): Enter finishes, Backspace removes the last point,
        // Esc cancels.
        if self.drawing_polygon() {
            let handled = match key {
                gdk::Key::Escape => {
                    self.with_state(|s| s.polygon = None);
                    true
                }
                gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter => {
                    let edit = self
                        .with_state(|s| {
                            let polygon = s.polygon.take()?;
                            let toolbox = s.toolbox.borrow().clone();
                            polygon.finish(&s.doc, &toolbox)
                        })
                        .flatten();
                    if let Some(edit) = edit {
                        self.execute(edit);
                    }
                    true
                }
                gdk::Key::BackSpace => {
                    self.with_state(|s| {
                        if !s.polygon.as_mut().is_some_and(PolygonDraft::remove_last) {
                            s.polygon = None;
                        }
                    });
                    true
                }
                _ => false,
            };
            if handled {
                self.update_chips();
                self.view.queue_draw();
                self.notify_changed();
                return true;
            }
        }
        // Crop mode (mockup 08): Esc cancels, Enter crops; the editing keys rest.
        if self.cropping() {
            match key {
                gdk::Key::Escape => {
                    if !self.cancel_drag() {
                        self.end_crop();
                    }
                    return true;
                }
                gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter if !dragging => {
                    self.apply_crop();
                    return true;
                }
                _ => {}
            }
        }
        let menu_key = key == gdk::Key::Menu || (key == gdk::Key::F10 && shift && !ctrl);
        if menu_key && !dragging && !self.cropping() {
            self.context_menu_from_keyboard();
            return true;
        }
        if key == gdk::Key::Escape {
            if self.cancel_drag() {
                return true;
            }
            // After drawing, the tool stays until Esc (spec "Tool switching").
            if self.tool() != Tool::Select {
                self.set_tool(Tool::Select);
                return true;
            }
            let had = self
                .with_state(|s| {
                    let had = !s.selection.is_empty();
                    s.selection.clear();
                    had
                })
                .unwrap_or(false);
            self.view.queue_draw();
            if had {
                self.notify_changed();
            }
            return had;
        }
        // The document does not change under a gesture in progress, nor in crop mode (only
        // its own keys, the zoom keys and undo/redo, which end it).
        if dragging {
            return false;
        }
        let cropping = self.cropping();
        let step = if shift { ops::NUDGE_LARGE } else { ops::NUDGE };
        let nudge = |dx: f64, dy: f64| self.edit(|doc, sel| ops::nudge(doc, sel, dx, dy));
        let handled = match key.to_lower() {
            gdk::Key::z if ctrl && shift => {
                self.redo();
                true
            }
            gdk::Key::z if ctrl => {
                self.undo();
                true
            }
            _ if cropping => false,
            gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter if !ctrl && !alt => {
                self.edit_selected_text()
            }
            gdk::Key::Left | gdk::Key::KP_Left if !ctrl && !alt => nudge(-step, 0.0),
            gdk::Key::Right | gdk::Key::KP_Right if !ctrl && !alt => nudge(step, 0.0),
            gdk::Key::Up | gdk::Key::KP_Up if !ctrl && !alt => nudge(0.0, -step),
            gdk::Key::Down | gdk::Key::KP_Down if !ctrl && !alt => nudge(0.0, step),
            gdk::Key::Delete | gdk::Key::KP_Delete if !ctrl => self.edit(ops::delete),
            gdk::Key::d if ctrl => self.edit(ops::duplicate),
            gdk::Key::a if ctrl => {
                self.with_state(|s| s.selection.select_all(s.doc.scene()));
                self.view.queue_draw();
                true
            }
            gdk::Key::g if ctrl && shift => self.edit(ops::ungroup),
            gdk::Key::g if ctrl => self.edit(ops::group),
            gdk::Key::bracketright | gdk::Key::braceright if ctrl => {
                let how = if shift {
                    Stacking::ToFront
                } else {
                    Stacking::Forward
                };
                self.edit(|doc, sel| ops::restack(doc, sel, how))
            }
            gdk::Key::bracketleft | gdk::Key::braceleft if ctrl => {
                let how = if shift {
                    Stacking::ToBack
                } else {
                    Stacking::Backward
                };
                self.edit(|doc, sel| ops::restack(doc, sel, how))
            }
            gdk::Key::semicolon if ctrl => {
                self.set_snapping(!self.snapping());
                true
            }
            _ => false,
        };
        if handled {
            self.update_cursor();
            self.notify_changed();
            return true;
        }
        match key {
            gdk::Key::plus | gdk::Key::equal | gdk::Key::KP_Add if ctrl => self.zoom_in(),
            gdk::Key::minus | gdk::Key::KP_Subtract if ctrl => self.zoom_out(),
            gdk::Key::_0 | gdk::Key::KP_0 if ctrl => self.zoom_fit(),
            gdk::Key::_1 | gdk::Key::KP_1 if ctrl => self.zoom_to(1.0, None),
            gdk::Key::space if !ctrl => {
                self.with_state(|s| s.space = true);
                self.view.set_cursor(None);
                self.scrolled.set_cursor_from_name(Some("grab"));
            }
            _ => return false,
        }
        true
    }

    pub fn key_released(&self, key: gdk::Key) {
        if key == gdk::Key::space {
            let panning = self
                .with_state(|s| {
                    s.space = false;
                    s.pan_start.is_some()
                })
                .unwrap_or(false);
            if !panning {
                self.scrolled.set_cursor(None);
                self.update_cursor();
            }
        }
    }
}

/// The objects a command removes (a `Remove`, or the first step of a `Batch`).
fn removed_ids(command: &Command) -> Vec<ObjectId> {
    match command {
        Command::Remove { ids } => ids.clone(),
        Command::Batch(steps) => steps.first().map(removed_ids).unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Event handlers hold the canvas weakly; the widgets own the handlers.
struct WeakCanvas {
    overlay: glib::WeakRef<gtk::Overlay>,
    scrolled: glib::WeakRef<gtk::ScrolledWindow>,
    view: glib::WeakRef<CanvasView>,
    measure_chip: glib::WeakRef<gtk::Label>,
    snap_chip: glib::WeakRef<gtk::Label>,
}

impl WeakCanvas {
    fn upgrade(&self) -> Option<Canvas> {
        Some(Canvas {
            overlay: self.overlay.upgrade()?,
            scrolled: self.scrolled.upgrade()?,
            view: self.view.upgrade()?,
            measure_chip: self.measure_chip.upgrade()?,
            snap_chip: self.snap_chip.upgrade()?,
        })
    }
}

/// Draws the visible part: `size` is the view, `scroll` the scroll offset.
fn draw(state: &State, cr: &gtk::cairo::Context, size: (f64, f64), scroll: (f64, f64)) {
    let dark = adw::StyleManager::default().is_dark();
    let (background, options) = if dark {
        (CANVAS_BG_DARK, Options::SCREEN_DARK)
    } else {
        (CANVAS_BG_LIGHT, Options::SCREEN_LIGHT)
    };
    background.set_source(cr);
    if let Err(err) = cr.paint() {
        eprintln!("annota: canvas: {err}");
        return;
    }
    // The view is laid out for the document's canvas; a dragged canvas edge only previews.
    let viewport = Viewport::new(
        state.zoom,
        view::view_origin(state.doc.canvas().rect, state.zoom, size, scroll),
    );
    let canvas_drag = state.canvas_drag.as_ref();
    let canvas = canvas_drag
        .and_then(CanvasDrag::preview_canvas)
        .unwrap_or(state.doc.canvas().rect);
    let corner = viewport.to_device(Point::new(f64::from(canvas.x), f64::from(canvas.y)));
    let (w, h) = (
        f64::from(canvas.width) * state.zoom,
        f64::from(canvas.height) * state.zoom,
    );
    if let Err(err) = draw_canvas_shadow(cr, corner, w, h) {
        eprintln!("annota: canvas: {err}");
    }
    // Text being edited is drawn as edited so far: in place of its object, or on top if it is
    // new; a draw in progress on top; the step tool's placement preview faded (mockup 11).
    let edited: Vec<(ObjectId, Box<dyn Annotation>)> = state
        .text
        .as_ref()
        .and_then(|t| Some((t.id()?, t.object().clone_box())))
        .into_iter()
        .collect();
    let ghost = step_ghost(state, &viewport);
    let mut preview = match &state.drag {
        Some(d) => Preview::objects(d.preview(), d.shows_ghost()),
        None => Preview::objects(&edited, false),
    };
    preview.added = state.draw.as_ref().and_then(Draw::preview).or_else(|| {
        state
            .text
            .as_ref()
            .filter(|t| t.id().is_none())
            .map(TextEdit::object)
    });
    preview.ghost_object = ghost
        .as_ref()
        .map(|g| (g as &dyn Annotation, STEP_GHOST_OPACITY));
    preview.canvas = canvas_drag.and_then(CanvasDrag::preview_canvas);
    preview.corner_radius = canvas_drag.and_then(CanvasDrag::preview_radius);
    if let Err(err) = render::render_preview(cr, &state.doc, &viewport, &options, Some(&preview)) {
        eprintln!("annota: canvas: {err}");
    }
    if let Err(err) = draw_editing_aids(state, cr, &viewport, canvas) {
        eprintln!("annota: canvas: {err}");
    }
}

/// Opacity of the step tool's placement preview (redline: 60 %).
const STEP_GHOST_OPACITY: f64 = 0.6;

/// The marker a click with the step tool would place under the pointer, while nothing else is
/// going on (mockup 11).
fn step_ghost(state: &State, viewport: &Viewport) -> Option<annota::tools::step::Step> {
    let idle = state.tool == Tool::Step
        && state.hovering
        && state.text.is_none()
        && state.draw.is_none()
        && state.drag.is_none()
        && state.crop.is_none()
        && state.canvas_drag.is_none();
    if !idle {
        return None;
    }
    let at = viewport.to_document(Point::new(state.pointer.0, state.pointer.1));
    let place = Canvas::step_place(state);
    Some(state.toolbox.borrow().step(at, place))
}

/// Selection, guides and rubber band over the document; in crop mode the crop frame; the
/// grabber of the canvas edge under the pointer and the corner-radius handle; the editing
/// frame, caret and selection of text being edited; a polygon being drawn.
fn draw_editing_aids(
    state: &State,
    cr: &gtk::cairo::Context,
    viewport: &Viewport,
    canvas: IRect,
) -> Result<(), gtk::cairo::Error> {
    if let Some(crop) = &state.crop {
        return overlay::draw_crop(cr, viewport, state.doc.canvas().rect, crop.frame());
    }
    if let Some(text) = &state.text {
        if let Some(block) = text.block() {
            let (start, end) = text.selection();
            let selection = if start < end {
                block.selection(start, end)
            } else {
                Vec::new()
            };
            let caret = block.caret(text.caret());
            overlay::draw_text_edit(cr, viewport, &block.bounds(), caret, &selection)?;
        }
        return Ok(());
    }
    if let Some(polygon) = &state.polygon {
        let toolbox = state.toolbox.borrow();
        let style = toolbox.shape_style();
        let fill = style.fill;
        let alpha = (f64::from(fill.a) * style.fill_opacity.clamp(0.0, 1.0)).round() as u8;
        overlay::draw_polygon_draft(
            cr,
            viewport,
            polygon.vertices(),
            polygon.pointer(),
            (style.outline_color, style.outline_width),
            Some(Rgba::new(fill.r, fill.g, fill.b, alpha)),
        )?;
        return Ok(());
    }
    // An arrow or line being drawn shows its ends and curve handle (mockup 03).
    if let Some(object) = state.draw.as_ref().and_then(Draw::preview) {
        if matches!(object.type_name(), "arrow" | "line") {
            overlay::draw_handles(cr, viewport, &object.handles())?;
        }
    }
    let frame = match &state.drag {
        Some(drag) => drag.selection_frame(&state.doc, &state.selection),
        None => selection_frame(state.doc.scene(), &state.selection, &[]),
    };
    if let Some(frame) = frame {
        overlay::draw_selection(cr, viewport, &frame)?;
    }
    if let Some(drag) = &state.drag {
        overlay::draw_guides(cr, viewport, state.doc.canvas().rect, drag.guides())?;
        if let Feedback::Band(rect) = drag.feedback() {
            overlay::draw_band(cr, viewport, rect)?;
        }
    }
    let dragging = state.canvas_drag.as_ref().filter(|d| d.is_dragging());
    match state.hover {
        Some(CanvasTarget::Edge(side)) => {
            if let Some(canvas_edit::Feedback::Extend { extra, .. }) =
                dragging.map(CanvasDrag::feedback)
            {
                if extra > 0 {
                    let image = image_ops::image_rect(&state.doc);
                    overlay::draw_image_edge(cr, viewport, image, canvas, side)?;
                }
            }
            let rect = canvas_edit::grabber_rect(viewport, canvas, side);
            overlay::draw_canvas_grabber(
                cr,
                viewport,
                canvas,
                rect,
                dragging.is_some().then_some(side),
            )?;
        }
        Some(CanvasTarget::Radius) => {
            let radius = dragging
                .and_then(CanvasDrag::preview_radius)
                .unwrap_or(state.doc.image().corner_radius);
            overlay::draw_handles(cr, viewport, &[canvas_edit::radius_handle(radius)])?;
        }
        None => {}
    }
    Ok(())
}

/// Kit `.canvas-image`: `0 0 0 1px rgb(0 0 6 / 10%), 0 2px 10px rgb(0 0 6 / 18%)`.
fn draw_canvas_shadow(
    cr: &gtk::cairo::Context,
    corner: Point,
    w: f64,
    h: f64,
) -> Result<(), gtk::cairo::Error> {
    let ink = Rgba::opaque(0, 0, 6);
    cr.save()?;
    cr.translate(0.0, 2.0);
    let shadow = render::shadow::paint(cr, ink, 0.18, 5.0, |cr| {
        cr.rectangle(corner.x, corner.y, w, h)
    });
    cr.restore()?;
    shadow?;
    cr.set_source_rgba(0.0, 0.0, 6.0 / 255.0, 0.10);
    cr.set_line_width(1.0);
    cr.rectangle(corner.x - 0.5, corner.y - 0.5, w + 1.0, h + 1.0);
    cr.stroke()
}

/// These open a window, so they are opt-in (they must never pop up on a desktop during a plain
/// `cargo test`). Run each in its own process, headless (see `crate::ui::test_support`).
#[cfg(test)]
mod tests {
    use gtk::gsk;

    use super::*;
    use crate::ui::test_support::{init, pump};
    use annota::document::bitmap::Bitmap;
    use annota::document::color::palette;
    use annota::document::geometry::IRect;

    /// Sizes of the cairo recording nodes in `node`.
    fn recordings(node: &gsk::RenderNode, out: &mut Vec<(f32, f32)>) {
        use gsk::RenderNodeType as T;
        let children: Vec<gsk::RenderNode> = match node.node_type() {
            T::CairoNode => {
                out.push((node.bounds().width(), node.bounds().height()));
                Vec::new()
            }
            T::ContainerNode => {
                let c = node.downcast_ref::<gsk::ContainerNode>().unwrap();
                (0..c.n_children()).map(|i| c.child(i)).collect()
            }
            T::TransformNode => vec![node.downcast_ref::<gsk::TransformNode>().unwrap().child()],
            T::ClipNode => vec![node.downcast_ref::<gsk::ClipNode>().unwrap().child()],
            T::DebugNode => vec![node.downcast_ref::<gsk::DebugNode>().unwrap().child()],
            T::OpacityNode => vec![node.downcast_ref::<gsk::OpacityNode>().unwrap().child()],
            T::RoundedClipNode => {
                vec![node.downcast_ref::<gsk::RoundedClipNode>().unwrap().child()]
            }
            _ => Vec::new(),
        };
        children.iter().for_each(|c| recordings(c, out));
    }

    fn render_node(widget: &gtk::Widget) -> gsk::RenderNode {
        let paintable = gtk::WidgetPaintable::new(Some(widget));
        let snapshot = gtk::Snapshot::new();
        paintable.snapshot(
            &snapshot,
            f64::from(widget.width()),
            f64::from(widget.height()),
        );
        snapshot.to_node().unwrap()
    }

    /// Bounding box of the pixels with the colour at the centre of the widget.
    fn centre_colour_bbox(widget: &gtk::Widget) -> (i32, i32, i32, i32) {
        let renderer = widget.native().unwrap().renderer().unwrap();
        let (w, h) = (widget.width(), widget.height());
        let area = graphene::Rect::new(0.0, 0.0, w as f32, h as f32);
        let texture = renderer.render_texture(render_node(widget), Some(&area));
        let stride = w as usize * 4;
        let mut px = vec![0u8; stride * h as usize];
        texture.download(&mut px, stride);
        let at = |x: i32, y: i32| y as usize * stride + x as usize * 4;
        let c = at(w / 2, h / 2);
        let reference = [px[c], px[c + 1], px[c + 2]];
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, -1, -1);
        for y in 0..h {
            for x in 0..w {
                let o = at(x, y);
                if (0..3).all(|k| (i32::from(px[o + k]) - i32::from(reference[k])).abs() <= 2) {
                    (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
                }
            }
        }
        (x0, y0, x1 - x0 + 1, y1 - y0 + 1)
    }

    #[test]
    #[ignore = "opens a window; run under xvfb-run with GDK_BACKEND=x11"]
    fn records_only_the_viewport_and_keeps_fit_centred() {
        init();
        let window = gtk::Window::new();
        window.set_default_size(1280, 800);
        let doc = Document::new(Bitmap::filled(2000, 1000, palette::AZURE).unwrap());
        let canvas = Canvas::new(doc);
        window.set_child(Some(canvas.widget()));
        window.present();
        pump(300);
        let widget: gtk::Widget = canvas.widget().clone().upcast();
        let view = (widget.width(), widget.height());
        assert!(view.0 > 0 && view.1 > 0);

        // Reviewer finding B1: at 1600 % GTK recorded the whole zoomed document (25 000 px
        // wide), so culling never culled. Now the recording is the viewport.
        canvas.zoom_to(16.0, None);
        pump(100);
        let mut sizes = Vec::new();
        recordings(&render_node(&widget), &mut sizes);
        assert_eq!(sizes, vec![(view.0 as f32, view.1 as f32)]);
        let h = canvas.scrolled().hadjustment();
        assert!(h.upper() > 30_000.0 && (h.page_size() - f64::from(view.0)).abs() < 1.0);

        // Reviewer finding B2: shrinking the window in fit mode left the canvas off-centre
        // and cut off. Fit, shrink 1280 × 800 → 900 × 600, and look at the pixels.
        canvas.zoom_fit();
        window.set_default_size(900, 600);
        pump(300);
        let (w, h) = (widget.width(), widget.height());
        assert!(w < view.0 && h < view.1, "the window shrank");
        let zoom = canvas.zoom();
        let fit = view::fit_zoom(IRect::new(0, 0, 2000, 1000), (f64::from(w), f64::from(h)));
        assert_eq!(zoom, fit);
        let (x, y, bw, bh) = centre_colour_bbox(&widget);
        let expected = (
            (2000.0 * zoom).round() as i32,
            (1000.0 * zoom).round() as i32,
        );
        assert!(
            (bw - expected.0).abs() <= 1 && (bh - expected.1).abs() <= 1,
            "whole canvas"
        );
        assert!(
            (x - (w - x - bw)).abs() <= 1,
            "centred horizontally: {x} {w} {bw}"
        );
        assert!(
            (y - (h - y - bh)).abs() <= 1,
            "centred vertically: {y} {h} {bh}"
        );
        window.destroy();
    }

    /// Pixels of `widget` as RGBA rows.
    fn pixels(widget: &gtk::Widget) -> (Vec<u8>, usize, i32, i32) {
        let renderer = widget.native().unwrap().renderer().unwrap();
        let (w, h) = (widget.width(), widget.height());
        let area = graphene::Rect::new(0.0, 0.0, w as f32, h as f32);
        let texture = renderer.render_texture(render_node(widget), Some(&area));
        let stride = w as usize * 4;
        let mut px = vec![0u8; stride * h as usize];
        texture.download(&mut px, stride);
        (px, stride, w, h)
    }

    #[test]
    #[ignore = "opens a window; run under xvfb-run with GDK_BACKEND=x11"]
    fn select_move_snap_nudge_undo_and_group_on_a_live_canvas() {
        use annota::document::command::History;
        use annota::document::geometry::Rect;
        use annota::tools::image::Image;

        init();
        // Two image objects on a 1600 × 1000 capture (mockup 02's document size).
        let mut doc = Document::new(Bitmap::filled(1600, 1000, palette::PAPER).unwrap());
        let mut history = History::new();
        for centre in [Point::new(300.0, 300.0), Point::new(1200.0, 700.0)] {
            let pip = Bitmap::filled(440, 66, palette::CORAL).unwrap();
            let cmd = Image::insert_command(&doc, pip, centre);
            history.execute(&mut doc, "Insert", cmd).unwrap();
        }
        let window = gtk::Window::new();
        window.set_default_size(1280, 800);
        let canvas = Canvas::new(doc);
        window.set_child(Some(canvas.widget()));
        window.present();
        pump(300);
        let vp = canvas.viewport().unwrap();
        let rect_of = |canvas: &Canvas, i: usize| {
            canvas
                .with_document(|d| {
                    let id = d.scene().ids()[i];
                    d.scene().get(id).unwrap().object.arrange().rect
                })
                .unwrap()
        };
        let start = rect_of(&canvas, 0).centre();

        // Drag the first object near the canvas centre: guides and chips show.
        canvas.pointer_pressed(vp.to_device(start), Modifiers::default());
        assert_eq!(canvas.selection().len(), 1);
        let near_centre = Point::new(803.0, 498.0);
        canvas.pointer_moved(
            vp.to_device(Point::new(near_centre.x, near_centre.y)),
            Modifiers::default(),
        );
        pump(100);
        assert!(canvas.snap_chip.is_visible());
        assert_eq!(canvas.snap_chip.label(), "Canvas centre");
        assert!(canvas.measure_chip.is_visible());
        assert!(canvas.measure_chip.label().starts_with("X 580 · Y 467"));
        // A violet guide is on screen.
        let widget: gtk::Widget = canvas.widget().clone().upcast();
        let (px, stride, w, h) = pixels(&widget);
        let violet = (0..h).any(|y| {
            (0..w).any(|x| {
                let o = y as usize * stride + x as usize * 4;
                let v = overlay::VIOLET;
                // Texture bytes are B, G, R, A or R, G, B, A depending on format; accept both.
                let rgb = [px[o], px[o + 1], px[o + 2]];
                rgb == [v.r, v.g, v.b] || rgb == [v.b, v.g, v.r]
            })
        });
        assert!(violet, "snap guide drawn");
        // The document is untouched until release; then it is one undo step.
        assert_eq!(rect_of(&canvas, 0).centre(), start);
        canvas.pointer_released();
        assert!(!canvas.snap_chip.is_visible() && !canvas.measure_chip.is_visible());
        assert_eq!(rect_of(&canvas, 0).centre(), Point::new(800.0, 500.0));
        assert_eq!(canvas.undo_label().as_deref(), Some("Move Image"));

        // Shift+→ nudges 10 px (spec "Keyboard nudge"), Ctrl+Z undoes it.
        assert!(canvas.key_pressed(gdk::Key::Right, gdk::ModifierType::SHIFT_MASK));
        assert_eq!(rect_of(&canvas, 0).x, 590.0);
        assert!(canvas.key_pressed(gdk::Key::z, gdk::ModifierType::CONTROL_MASK));
        assert_eq!(rect_of(&canvas, 0).x, 580.0);

        // Ctrl+A, Ctrl+G: one group; dragging one member moves both.
        let ctrl = gdk::ModifierType::CONTROL_MASK;
        assert!(canvas.key_pressed(gdk::Key::a, ctrl));
        assert!(canvas.key_pressed(gdk::Key::g, ctrl));
        assert_eq!(canvas.undo_label().as_deref(), Some("Group"));
        assert_eq!(canvas.with_document(|d| d.scene().len()), Some(1));
        let second = Point::new(1200.0, 700.0);
        canvas.pointer_pressed(vp.to_device(second), Modifiers::default());
        canvas.pointer_moved(
            vp.to_device(Point::new(second.x - 100.0, second.y)),
            Modifiers {
                alt: true,
                ..Modifiers::default()
            },
        );
        canvas.pointer_released();
        assert_eq!(canvas.undo_label().as_deref(), Some("Move Group"));
        let members: Vec<Rect> = canvas
            .with_document(|d| {
                d.scene().nodes()[0]
                    .object
                    .children()
                    .iter()
                    .map(|c| c.object.arrange().rect)
                    .collect()
            })
            .unwrap();
        assert_eq!(members[0].x, 480.0);
        assert_eq!(members[1].x, 980.0 - 100.0);

        // Rotate the group with Shift (mockup 03): the angle chip shows 15° steps and sits
        // centred above the pointer (its own margins must not count as its size).
        let frame = canvas
            .with_state(|s| selection_frame(s.doc.scene(), &s.selection, &[]))
            .flatten()
            .unwrap();
        assert!(frame.dashed, "a group's box is dashed");
        let vp = canvas.viewport().unwrap();
        let grab = frame
            .handles
            .iter()
            .find(|h| h.id == HandleId::Rotate)
            .unwrap()
            .screen_position(vp.zoom, vp.origin);
        let shift = Modifiers {
            shift: true,
            ..Modifiers::default()
        };
        canvas.pointer_pressed(grab, shift);
        let pointer = Point::new(grab.x - 60.0, grab.y + 5.0);
        canvas.pointer_moved(pointer, shift);
        pump(100);
        assert_eq!(
            canvas.measure_chip.label(),
            "\u{2212}30° · Shift: 15° steps"
        );
        let chip = &canvas.measure_chip;
        let (x, w) = (f64::from(chip.margin_start()), f64::from(chip.width()));
        assert!((x + w / 2.0 - pointer.x).abs() <= 1.0, "centred: {x} {w}");
        assert!(
            f64::from(chip.margin_top() + chip.height()) < pointer.y,
            "above"
        );
        canvas.pointer_released();
        assert_eq!(canvas.undo_label().as_deref(), Some("Rotate Group"));
        window.destroy();
    }

    #[test]
    #[ignore = "opens a window; run under xvfb-run with GDK_BACKEND=x11"]
    fn canvas_edges_corner_radius_crop_and_eyedropper_on_a_live_canvas() {
        init();
        let doc = Document::new(Bitmap::filled(1600, 1000, palette::PAPER).unwrap());
        let window = gtk::Window::new();
        window.set_default_size(1280, 800);
        let canvas = Canvas::new(doc);
        window.set_child(Some(canvas.widget()));
        window.present();
        pump(300);
        let vp = canvas.viewport().unwrap();
        let none = Modifiers::default();
        let undo_label = |c: &Canvas| c.history_info().undo_label;

        // Mockup 10: near the right edge its grabber appears; dragging it 200 px out extends
        // the canvas, with a "+200 px" chip, as one undo step.
        let edge = vp.to_device(Point::new(1600.0, 500.0));
        canvas.with_state(|s| s.pointer = (edge.x + 4.0, edge.y));
        canvas.update_hover();
        assert_eq!(
            canvas.with_state(|s| s.hover).flatten(),
            Some(CanvasTarget::Edge(Side::Right))
        );
        canvas.pointer_pressed(Point::new(edge.x + 4.0, edge.y), none);
        canvas.pointer_moved(vp.to_device(Point::new(1800.0, 510.0)), none);
        pump(50);
        assert!(canvas.measure_chip.is_visible());
        assert_eq!(canvas.measure_chip.label(), "+200 px");
        assert_eq!(canvas.with_document(|d| d.canvas().rect.width), Some(1600));
        let fitted_zoom = canvas.zoom();
        canvas.pointer_released();
        assert_eq!(canvas.with_document(|d| d.canvas().rect.width), Some(1800));
        assert_eq!(undo_label(&canvas).as_deref(), Some("Extend Canvas"));
        // The fitted view fits the wider canvas (mockup 10: "Fit · 42 %").
        assert!(canvas.fitted() && canvas.zoom() < fitted_zoom);
        canvas.undo();
        pump(100);

        // Mockup 09: the corner-radius handle at the image's top-left inner corner.
        let vp = canvas.viewport().unwrap();
        let corner = vp.to_device(Point::new(0.0, 0.0));
        canvas.pointer_pressed(corner, none);
        canvas.pointer_moved(vp.to_device(Point::new(40.0, 40.0)), none);
        pump(50);
        assert_eq!(canvas.measure_chip.label(), "Corner radius 40 px");
        canvas.pointer_released();
        assert_eq!(
            canvas.with_document(|d| d.image().corner_radius),
            Some(40.0)
        );
        assert_eq!(undo_label(&canvas).as_deref(), Some("Corner Radius"));
        canvas.undo();

        // Mockup 08: crop mode, a grip dragged, the size chip, Enter.
        canvas.start_crop();
        pump(50);
        assert_eq!(canvas.measure_chip.label(), "1600 × 1000 px");
        let vp = canvas.viewport().unwrap();
        canvas.pointer_pressed(vp.to_device(Point::new(1600.0, 1000.0)), none);
        canvas.pointer_moved(vp.to_device(Point::new(970.0, 970.0)), none);
        canvas.pointer_released();
        pump(50);
        assert_eq!(canvas.crop_frame(), Some(IRect::new(0, 0, 970, 970)));
        assert_eq!(canvas.measure_chip.label(), "970 × 970 px");
        assert!(canvas.key_pressed(gdk::Key::Return, gdk::ModifierType::empty()));
        assert!(!canvas.cropping());
        assert_eq!(
            canvas.with_document(|d| d.canvas().rect),
            Some(IRect::new(0, 0, 970, 970))
        );
        assert_eq!(undo_label(&canvas).as_deref(), Some("Crop"));
        assert!(!canvas.measure_chip.is_visible());

        // The eyedropper picks the colour under the next click.
        let picked = std::rc::Rc::new(std::cell::Cell::new(None));
        let into = picked.clone();
        canvas.pick_colour(move |c| into.set(c));
        let vp = canvas.viewport().unwrap();
        canvas.pointer_pressed(vp.to_device(Point::new(100.0, 100.0)), none);
        assert_eq!(picked.get(), Some(palette::PAPER));
        // Escape cancels a pick.
        let into = picked.clone();
        canvas.pick_colour(move |c| into.set(c));
        assert!(canvas.key_pressed(gdk::Key::Escape, gdk::ModifierType::empty()));
        assert_eq!(picked.get(), None);
        window.destroy();
    }

    /// A press and release at the same place (a click), `presses` 2 for a double-click.
    fn click(canvas: &Canvas, at: Point, presses: i32) {
        canvas.with_state(|s| s.presses = presses);
        canvas.pointer_pressed(at, Modifiers::default());
        canvas.pointer_released();
    }

    /// Press at `from`, move to `to` (view points), release.
    fn drag(canvas: &Canvas, from: Point, to: Point, modifiers: Modifiers) {
        canvas.with_state(|s| s.presses = 1);
        canvas.pointer_pressed(from, modifiers);
        canvas.pointer_moved(
            Point::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0),
            modifiers,
        );
        canvas.pointer_moved(to, modifiers);
        canvas.pointer_released();
    }

    #[test]
    #[ignore = "opens a window; run under xvfb-run with GDK_BACKEND=x11"]
    fn drawing_tools_on_a_live_canvas() {
        use annota::document::color::palette;
        use annota::tools::arrow::{Arrow, EndStyle};
        use annota::tools::callout::{Callout, ADD_TAIL};
        use annota::tools::shape::{Shape, ShapeKind};
        use annota::tools::step::Step;

        init();
        let doc = Document::new(Bitmap::filled(1600, 1000, palette::PAPER).unwrap());
        let window = gtk::Window::new();
        window.set_default_size(1280, 800);
        let canvas = Canvas::new(doc);
        window.set_child(Some(canvas.widget()));
        window.present();
        pump(300);
        let vp = canvas.viewport().unwrap();
        let at = |x: f64, y: f64| vp.to_device(Point::new(x, y));
        let none = Modifiers::default();
        let shift = Modifiers {
            shift: true,
            ..none
        };
        let count = |c: &Canvas, t: &str| c.with_document(|d| d.scene().of_type(t).len()).unwrap();
        let undo_label = |c: &Canvas| c.history_info().undo_label;

        // Arrow (A): drawn end to end, selected, one "Add Arrow" step; the tool stays.
        canvas.set_tool(Tool::Arrow);
        drag(&canvas, at(100.0, 100.0), at(500.0, 300.0), none);
        assert_eq!(count(&canvas, "arrow"), 1);
        assert_eq!(undo_label(&canvas).as_deref(), Some("Add Arrow"));
        assert_eq!(canvas.selection().len(), 1);
        assert_eq!(canvas.tool(), Tool::Arrow, "the tool stays active");
        // While drawing with Shift: constrained, with the chip (redline).
        canvas.with_state(|s| s.presses = 1);
        canvas.pointer_pressed(at(100.0, 600.0), shift);
        canvas.pointer_moved(at(300.0, 610.0), shift);
        canvas.pointer_moved(at(600.0, 612.0), shift);
        pump(50);
        assert!(canvas.measure_chip.is_visible());
        assert_eq!(canvas.measure_chip.label(), "0° · Shift: 0/45/90°");
        canvas.pointer_released();
        assert_eq!(
            count(&canvas, "arrow"),
            2,
            "a second arrow without picking the tool again"
        );
        let second = canvas
            .with_document(|d| {
                let id = *d.scene().of_type("arrow").last().unwrap();
                d.scene()
                    .get(id)
                    .unwrap()
                    .object
                    .as_any()
                    .downcast_ref::<Arrow>()
                    .unwrap()
                    .clone()
            })
            .unwrap();
        assert_eq!(second.end.y, 600.0);
        // Esc goes back to Select (spec "Tool switching").
        assert!(canvas.key_pressed(gdk::Key::Escape, gdk::ModifierType::empty()));
        assert_eq!(canvas.tool(), Tool::Select);

        // Line (L): no heads (spec "Plain line").
        canvas.set_tool(Tool::Line);
        drag(&canvas, at(700.0, 100.0), at(1000.0, 100.0), none);
        let line = canvas
            .with_document(|d| {
                let id = d.scene().of_type("line")[0];
                d.scene()
                    .get(id)
                    .unwrap()
                    .object
                    .as_any()
                    .downcast_ref::<Arrow>()
                    .unwrap()
                    .clone()
            })
            .unwrap();
        assert_eq!(
            (line.start_style, line.end_style),
            (EndStyle::None, EndStyle::None)
        );

        // Pen (P), shape (U), highlighter (H), redaction (B).
        for (tool, type_name, from, to) in [
            (Tool::Pen, "pen", (100.0, 700.0), (300.0, 760.0)),
            (Tool::Shape, "shape", (400.0, 700.0), (600.0, 800.0)),
            (
                Tool::Highlighter,
                "highlighter",
                (700.0, 700.0),
                (900.0, 740.0),
            ),
            (Tool::Redact, "redaction", (1000.0, 700.0), (1200.0, 740.0)),
        ] {
            canvas.set_tool(tool);
            drag(&canvas, at(from.0, from.1), at(to.0, to.1), none);
            assert_eq!(count(&canvas, type_name), 1, "{type_name}");
        }

        // Polygon: a vertex per click, the chip counts them, Enter finishes.
        canvas
            .with_state(|s| {
                s.toolbox.borrow_mut().set_property(
                    Tool::Shape,
                    "shape",
                    &annota::document::annotation::PropertyValue::Choice("polygon".into()),
                )
            })
            .unwrap()
            .unwrap();
        canvas.set_tool(Tool::Shape);
        for p in [(1300.0, 100.0), (1500.0, 120.0), (1450.0, 300.0)] {
            click(&canvas, at(p.0, p.1), 1);
        }
        canvas.pointer_hovered(at(1350.0, 280.0));
        pump(50);
        assert!(canvas.drawing_polygon());
        assert_eq!(
            canvas.measure_chip.label(),
            "3 points · Enter or double-click to finish"
        );
        assert!(canvas.key_pressed(gdk::Key::Return, gdk::ModifierType::empty()));
        assert!(!canvas.drawing_polygon());
        let polygon = canvas
            .with_document(|d| {
                d.scene()
                    .nodes()
                    .iter()
                    .rev()
                    .find_map(|n| n.object.as_any().downcast_ref::<Shape>().cloned())
            })
            .flatten()
            .unwrap();
        assert_eq!(
            (polygon.kind, polygon.points.len()),
            (ShapeKind::Polygon, 3)
        );
        // A double-click finishes too; Esc cancels.
        for p in [(1300.0, 400.0), (1500.0, 420.0), (1450.0, 600.0)] {
            click(&canvas, at(p.0, p.1), 1);
        }
        click(&canvas, at(1450.0, 600.0), 2);
        assert!(!canvas.drawing_polygon());
        assert_eq!(count(&canvas, "shape"), 3);
        click(&canvas, at(1300.0, 700.0), 1);
        assert!(canvas.key_pressed(gdk::Key::Escape, gdk::ModifierType::empty()));
        assert!(!canvas.drawing_polygon());
        assert_eq!(count(&canvas, "shape"), 3);

        // Text (T): a click starts typing in place; letters go to the text; Esc adds it.
        canvas.set_tool(Tool::Text);
        click(&canvas, at(200.0, 900.0), 1);
        assert!(canvas.editing_text());
        pump(50);
        assert_eq!(
            canvas.measure_chip.label(),
            "Esc or click outside to finish"
        );
        canvas.insert_text("Runs nightly");
        assert!(canvas.key_pressed(gdk::Key::Return, gdk::ModifierType::empty()));
        canvas.insert_text("on mains");
        assert!(
            !canvas.key_pressed(gdk::Key::a, gdk::ModifierType::empty()),
            "letters are typed"
        );
        assert!(canvas.key_pressed(gdk::Key::Escape, gdk::ModifierType::empty()));
        assert!(!canvas.editing_text());
        assert_eq!(count(&canvas, "text"), 1);
        assert_eq!(undo_label(&canvas).as_deref(), Some("Add Text"));
        // Spec "Edit in place": with Select, a double-click edits the text on the canvas.
        canvas.set_tool(Tool::Select);
        let text_box = canvas
            .with_document(|d| {
                let id = d.scene().of_type("text")[0];
                d.scene().get(id).unwrap().object.arrange().rect
            })
            .unwrap();
        let inside = vp.to_device(text_box.centre());
        click(&canvas, inside, 1);
        click(&canvas, inside, 2);
        assert!(canvas.editing_text());
        assert!(canvas.key_pressed(gdk::Key::End, gdk::ModifierType::CONTROL_MASK));
        canvas.insert_text(" only");
        assert_eq!(
            canvas.edited_text().as_deref(),
            Some("Runs nightly\non mains only")
        );
        // A click outside finishes: one "Edit Text" step.
        click(&canvas, at(1500.0, 950.0), 1);
        assert!(!canvas.editing_text());
        assert_eq!(undo_label(&canvas).as_deref(), Some("Edit Text"));

        // Callout (K): a drag makes the body, typing follows; the add-tail handle adds a tail.
        canvas.set_tool(Tool::Callout);
        drag(&canvas, at(800.0, 400.0), at(1100.0, 480.0), none);
        assert!(canvas.editing_text());
        canvas.insert_text("Both paths");
        assert!(canvas.key_pressed(gdk::Key::Escape, gdk::ModifierType::empty()));
        assert_eq!(count(&canvas, "callout"), 1);
        canvas.set_tool(Tool::Select);
        let (id, add) = canvas
            .with_document(|d| {
                let id = d.scene().of_type("callout")[0];
                let add = d
                    .scene()
                    .get(id)
                    .unwrap()
                    .object
                    .handles()
                    .into_iter()
                    .find(|h| h.id == ADD_TAIL)
                    .unwrap();
                (id, add)
            })
            .unwrap();
        canvas.with_state(|s| s.selection.set(s.doc.scene(), [id]));
        let grab = add.screen_position(vp.zoom, vp.origin);
        drag(
            &canvas,
            grab,
            at(1300.0, 650.0),
            Modifiers { alt: true, ..none },
        );
        let tails = canvas
            .with_document(|d| {
                d.scene()
                    .get(id)
                    .unwrap()
                    .object
                    .as_any()
                    .downcast_ref::<Callout>()
                    .unwrap()
                    .tails
                    .len()
            })
            .unwrap();
        assert_eq!(tails, 2, "spec \"Two tails\"");

        // Step (S): the placement preview's chip, three clicks give 1, 2, 3.
        canvas.set_tool(Tool::Step);
        canvas.pointer_hovered(at(300.0, 200.0));
        pump(50);
        assert_eq!(canvas.measure_chip.label(), "Click to place 1");
        let spots = [
            (300.0, 200.0),
            (300.0, 300.0),
            (300.0, 400.0),
            (300.0, 500.0),
        ];
        for p in spots {
            click(&canvas, at(p.0, p.1), 1);
        }
        let labels = |c: &Canvas| {
            c.with_document(|d| {
                annota::editing::steps::markers(d.scene())
                    .iter()
                    .map(|s| s.label())
                    .collect::<Vec<_>>()
            })
            .unwrap()
        };
        assert_eq!(labels(&canvas), ["1", "2", "3", "4"]);
        // Delete marker 2: 1, 2, 3, with the toast's notice.
        let notices = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let into = notices.clone();
        canvas.connect_notice(move |n| into.borrow_mut().push(n));
        canvas.set_tool(Tool::Select);
        click(&canvas, at(300.0, 300.0), 1);
        assert!(canvas.key_pressed(gdk::Key::Delete, gdk::ModifierType::empty()));
        assert_eq!(labels(&canvas), ["1", "2", "3"]);
        assert!(matches!(
            notices.borrow().first(),
            Some(Notice::StepsRenumbered { deleted: Some(label), renumbered: 2, .. }) if label == "2"
        ));
        // Restart Sequence: the next marker is 1 again; the others keep their numbers.
        canvas.set_tool(Tool::Step);
        canvas.restart_sequence();
        click(&canvas, at(500.0, 200.0), 1);
        assert_eq!(labels(&canvas), ["1", "2", "3", "1"]);
        canvas.set_next_step(Some(7));
        click(&canvas, at(500.0, 300.0), 1);
        assert_eq!(labels(&canvas), ["1", "2", "3", "1", "7"]);
        let last = canvas
            .with_document(|d| {
                let id = *d.scene().of_type("step").last().unwrap();
                d.scene()
                    .get(id)
                    .unwrap()
                    .object
                    .as_any()
                    .downcast_ref::<Step>()
                    .unwrap()
                    .sequence
            })
            .unwrap();
        assert_eq!(last, 2);
        // Undo takes back one marker at a time.
        canvas.undo();
        assert_eq!(labels(&canvas).len(), 4);

        // Phase-A review repros: drawing tools always draw over existing objects; only a
        // handle of the selection is still grabbed.
        let object_json = |c: &Canvas, id| {
            c.with_document(|d| d.scene().get(id).unwrap().object.to_json().unwrap())
                .unwrap()
        };
        canvas.set_tool(Tool::Arrow);
        drag(&canvas, at(1250.0, 850.0), at(1550.0, 850.0), none);
        let first = canvas.selection()[0];
        drag(&canvas, at(1250.0, 950.0), at(1550.0, 950.0), none);
        let before = object_json(&canvas, first);
        let arrows = count(&canvas, "arrow");
        drag(&canvas, at(1400.0, 850.0), at(1400.0, 780.0), none);
        assert_eq!(count(&canvas, "arrow"), arrows + 1, "a new arrow");
        assert_eq!(
            object_json(&canvas, first),
            before,
            "the old arrow is not bent"
        );
        // The arrow just drawn still bends through its curve handle.
        let just_drawn = canvas.selection()[0];
        let bent_before = object_json(&canvas, just_drawn);
        drag(&canvas, at(1400.0, 815.0), at(1450.0, 815.0), none);
        assert_eq!(count(&canvas, "arrow"), arrows + 1, "no new arrow");
        assert_ne!(object_json(&canvas, just_drawn), bent_before, "bent");
        assert_eq!(undo_label(&canvas).as_deref(), Some("Edit Arrow"));

        canvas.set_tool(Tool::Highlighter);
        drag(&canvas, at(700.0, 800.0), at(900.0, 880.0), none);
        let highlight = canvas.selection()[0];
        let before = object_json(&canvas, highlight);
        let highlights = count(&canvas, "highlighter");
        drag(&canvas, at(760.0, 840.0), at(880.0, 870.0), none);
        assert_eq!(count(&canvas, "highlighter"), highlights + 1);
        assert_eq!(object_json(&canvas, highlight), before, "not moved");
        window.destroy();
    }
}
