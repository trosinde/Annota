//! The canvas widget: draws a document with the shared renderer, with zoom and pan (spec
//! editor-canvas: View control).
//!
//! Built imperatively, as CLAUDE.md §7 allows for the canvas. All geometry comes from
//! `annota::render::view`; this module only wires GTK to it. Selection, handles and the zoom
//! OSD arrive with tasks 3.2 and 4.1.
//!
//! **Viewport-sized, not content-sized.** [`CanvasView`] implements `GtkScrollable`: inside a
//! `GtkScrolledWindow` it is allocated exactly the visible area and *owns* the scroll
//! adjustments, whose range it sets to the zoomed content. Each frame records only the visible
//! area (the cairo clip is the viewport), so the renderer's culling skips everything outside
//! it, and the widget shrinks with the window, so "Fit" stays centred. (A `GtkDrawingArea`
//! sized to the zoomed content made GTK record the whole content at every zoom level — at
//! 1600 % a 25 000 px wide surface — and kept the window from shrinking the canvas.)

use std::cell::{Cell, RefCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, glib, graphene};

use annota::document::color::Rgba;
use annota::document::geometry::Point;
use annota::document::Document;
use annota::render::{self, view, Options, Viewport};

/// Kit `--canvas-bg`, light and dark.
const CANVAS_BG_LIGHT: Rgba = Rgba::opaque(0xDE, 0xDD, 0xDA);
const CANVAS_BG_DARK: Rgba = Rgba::opaque(0x18, 0x18, 0x1B);

/// Fraction of the view one scroll-bar step / page moves.
const STEP: f64 = 0.1;
const PAGE: f64 = 0.9;

struct State {
    doc: Document,
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

mod imp {
    use std::sync::OnceLock;

    use super::*;

    pub struct CanvasView {
        pub(super) state: RefCell<Option<State>>,
        pub(super) listeners: RefCell<Vec<ViewListener>>,
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
    }
}

glib::wrapper! {
    /// The scrollable drawing surface of the canvas; see the module documentation.
    pub struct CanvasView(ObjectSubclass<imp::CanvasView>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Scrollable;
}

/// The canvas: a [`CanvasView`] in a `GtkScrolledWindow` (scroll bars, kinetic scrolling),
/// with zoom and pan.
#[derive(Clone)]
pub struct Canvas {
    scrolled: gtk::ScrolledWindow,
    view: CanvasView,
}

impl Canvas {
    pub fn new(doc: Document) -> Self {
        let view: CanvasView = glib::Object::builder()
            .property("hexpand", true)
            .property("vexpand", true)
            .build();
        view.imp().state.replace(Some(State {
            doc,
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
        let canvas = Self { scrolled, view };
        canvas.setup();
        canvas
    }

    pub fn widget(&self) -> &gtk::ScrolledWindow {
        &self.scrolled
    }

    pub fn zoom(&self) -> f64 {
        self.with_state(|s| s.zoom).unwrap_or(1.0)
    }

    /// Called with (zoom, fitted) whenever the view changes.
    pub fn connect_view_changed(&self, f: impl Fn(f64, bool) + 'static) {
        self.view.imp().listeners.borrow_mut().push(Box::new(f));
    }

    fn with_state<T>(&self, f: impl FnOnce(&mut State) -> T) -> Option<T> {
        self.view.imp().state.borrow_mut().as_mut().map(f)
    }

    fn setup(&self) {
        let motion = gtk::EventControllerMotion::new();
        let weak = self.view.downgrade();
        motion.connect_motion(move |_, x, y| {
            if let Some(view) = weak.upgrade() {
                if let Some(state) = view.imp().state.borrow_mut().as_mut() {
                    state.pointer = (x, y);
                }
            }
        });
        self.view.add_controller(motion);

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
            scrolled: self.scrolled.downgrade(),
            view: self.view.downgrade(),
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

    /// Keyboard zoom and space-to-pan; returns whether the key was used.
    /// (Window-level shortcuts and actions arrive with the editor window, task 4.1.)
    pub fn key_pressed(&self, key: gdk::Key, modifiers: gdk::ModifierType) -> bool {
        let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
        match key {
            gdk::Key::plus | gdk::Key::equal | gdk::Key::KP_Add if ctrl => self.zoom_in(),
            gdk::Key::minus | gdk::Key::KP_Subtract if ctrl => self.zoom_out(),
            gdk::Key::_0 | gdk::Key::KP_0 if ctrl => self.zoom_fit(),
            gdk::Key::_1 | gdk::Key::KP_1 if ctrl => self.zoom_to(1.0, None),
            gdk::Key::space if !ctrl => {
                self.with_state(|s| s.space = true);
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
            }
        }
    }
}

/// Event handlers hold the canvas weakly; the widgets own the handlers.
struct WeakCanvas {
    scrolled: glib::WeakRef<gtk::ScrolledWindow>,
    view: glib::WeakRef<CanvasView>,
}

impl WeakCanvas {
    fn upgrade(&self) -> Option<Canvas> {
        Some(Canvas {
            scrolled: self.scrolled.upgrade()?,
            view: self.view.upgrade()?,
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
    let canvas = state.doc.canvas().rect;
    let viewport = Viewport::new(
        state.zoom,
        view::view_origin(canvas, state.zoom, size, scroll),
    );
    let corner = viewport.to_device(Point::new(f64::from(canvas.x), f64::from(canvas.y)));
    let (w, h) = (
        f64::from(canvas.width) * state.zoom,
        f64::from(canvas.height) * state.zoom,
    );
    if let Err(err) = draw_canvas_shadow(cr, corner, w, h) {
        eprintln!("annota: canvas: {err}");
    }
    if let Err(err) = render::render(cr, &state.doc, &viewport, &options) {
        eprintln!("annota: canvas: {err}");
    }
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
/// `cargo test`). Run headless with
/// `GDK_BACKEND=x11 xvfb-run -a cargo test --bin annota -- --ignored`.
#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use gtk::gsk;

    use super::*;
    use annota::document::bitmap::Bitmap;
    use annota::document::color::palette;
    use annota::document::geometry::IRect;

    fn pump(ms: u64) {
        let end = Instant::now() + Duration::from_millis(ms);
        let ctx = glib::MainContext::default();
        while Instant::now() < end {
            while ctx.iteration(false) {}
            std::thread::sleep(Duration::from_millis(2));
        }
    }

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
        if gtk::init().is_err() || adw::init().is_err() {
            eprintln!("skipped: no display");
            return;
        }
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
        let h = canvas.widget().hadjustment();
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
}
