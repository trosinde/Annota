//! The canvas widget: a `GtkDrawingArea` in a `GtkScrolledWindow` that draws a document with
//! the shared renderer, with zoom and pan (spec editor-canvas: View control).
//!
//! Built imperatively, as CLAUDE.md §7 allows for the canvas. All geometry comes from
//! `annota::render::view`; this module only wires GTK events to it. Selection, handles and
//! the zoom OSD arrive with tasks 3.2 and 4.1.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gdk, glib};

use annota::document::color::Rgba;
use annota::document::geometry::Point;
use annota::document::Document;
use annota::render::{self, view, Options, Viewport};

/// Kit `--canvas-bg`, light and dark.
const CANVAS_BG_LIGHT: Rgba = Rgba::opaque(0xDE, 0xDD, 0xDA);
const CANVAS_BG_DARK: Rgba = Rgba::opaque(0x18, 0x18, 0x1B);

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

struct Inner {
    scrolled: gtk::ScrolledWindow,
    area: gtk::DrawingArea,
    state: RefCell<State>,
    listeners: RefCell<Vec<ViewListener>>,
}

#[derive(Clone)]
pub struct Canvas {
    inner: Rc<Inner>,
}

impl Canvas {
    pub fn new(doc: Document) -> Self {
        let area = gtk::DrawingArea::builder()
            .hexpand(true)
            .vexpand(true)
            .build();
        let scrolled = gtk::ScrolledWindow::builder()
            .child(&area)
            .hexpand(true)
            .vexpand(true)
            .build();
        let inner = Rc::new(Inner {
            scrolled,
            area,
            state: RefCell::new(State {
                doc,
                zoom: 1.0,
                fitted: true,
                space: false,
                pointer: (0.0, 0.0),
                pan_start: None,
            }),
            listeners: RefCell::new(Vec::new()),
        });
        let canvas = Self { inner };
        canvas.setup();
        canvas
    }

    pub fn widget(&self) -> &gtk::ScrolledWindow {
        &self.inner.scrolled
    }

    pub fn zoom(&self) -> f64 {
        self.inner.state.borrow().zoom
    }

    /// Called with (zoom, fitted) whenever the view changes.
    pub fn connect_view_changed(&self, f: impl Fn(f64, bool) + 'static) {
        self.inner.listeners.borrow_mut().push(Box::new(f));
    }

    fn setup(&self) {
        let weak = Rc::downgrade(&self.inner);
        self.inner.area.set_draw_func(glib::clone!(
            #[strong]
            weak,
            move |_, cr, width, height| {
                if let Some(inner) = weak.upgrade() {
                    draw(&inner, cr, f64::from(width), f64::from(height));
                }
            }
        ));

        // Keep "Fit" fitted while the window is resized.
        self.inner.area.connect_resize(glib::clone!(
            #[strong]
            weak,
            move |_, _, _| {
                if let Some(canvas) = upgrade(&weak) {
                    if canvas.inner.state.borrow().fitted {
                        canvas.zoom_fit();
                    }
                }
            }
        ));

        let motion = gtk::EventControllerMotion::new();
        motion.connect_motion(glib::clone!(
            #[strong]
            weak,
            move |_, x, y| {
                if let Some(inner) = weak.upgrade() {
                    inner.state.borrow_mut().pointer = (x, y);
                }
            }
        ));
        self.inner.scrolled.add_controller(motion);

        // Ctrl+scroll zooms at the pointer; plain scrolling scrolls.
        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        scroll.set_propagation_phase(gtk::PropagationPhase::Capture);
        scroll.connect_scroll(glib::clone!(
            #[strong]
            weak,
            move |controller, _, dy| {
                let ctrl = controller
                    .current_event_state()
                    .contains(gdk::ModifierType::CONTROL_MASK);
                match upgrade(&weak) {
                    Some(canvas) if ctrl => {
                        let (zoom, anchor) = {
                            let state = canvas.inner.state.borrow();
                            (state.zoom, state.pointer)
                        };
                        canvas.zoom_to(zoom * view::SCROLL_FACTOR.powf(-dy), Some(anchor));
                        glib::Propagation::Stop
                    }
                    _ => glib::Propagation::Proceed,
                }
            }
        ));
        self.inner.scrolled.add_controller(scroll);

        // Pan with the middle button, or space + primary button.
        let drag = gtk::GestureDrag::new();
        drag.set_button(0);
        drag.connect_drag_begin(glib::clone!(
            #[strong]
            weak,
            move |gesture, _, _| {
                let Some(inner) = weak.upgrade() else {
                    return;
                };
                let button = gesture.current_button();
                let space = inner.state.borrow().space;
                if button == gdk::BUTTON_MIDDLE || (button == gdk::BUTTON_PRIMARY && space) {
                    let start = (
                        inner.scrolled.hadjustment().value(),
                        inner.scrolled.vadjustment().value(),
                    );
                    inner.state.borrow_mut().pan_start = Some(start);
                    inner.scrolled.set_cursor_from_name(Some("grabbing"));
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                } else {
                    gesture.set_state(gtk::EventSequenceState::Denied);
                }
            }
        ));
        drag.connect_drag_update(glib::clone!(
            #[strong]
            weak,
            move |_, dx, dy| {
                if let Some(inner) = weak.upgrade() {
                    if let Some((sx, sy)) = inner.state.borrow().pan_start {
                        inner.scrolled.hadjustment().set_value(sx - dx);
                        inner.scrolled.vadjustment().set_value(sy - dy);
                    }
                }
            }
        ));
        drag.connect_drag_end(glib::clone!(
            #[strong]
            weak,
            move |_, _, _| {
                if let Some(inner) = weak.upgrade() {
                    let space = {
                        let mut state = inner.state.borrow_mut();
                        state.pan_start = None;
                        state.space
                    };
                    inner.scrolled.set_cursor_from_name(space.then_some("grab"));
                }
            }
        ));
        self.inner.scrolled.add_controller(drag);
    }

    fn view_size(&self) -> (f64, f64) {
        (
            f64::from(self.inner.scrolled.width()),
            f64::from(self.inner.scrolled.height()),
        )
    }

    /// Zoom to fit (Ctrl+0): the whole canvas visible and centred, at most 100 %.
    pub fn zoom_fit(&self) {
        let view_size = self.view_size();
        if view_size.0 < 1.0 || view_size.1 < 1.0 {
            return;
        }
        let canvas = self.inner.state.borrow().doc.canvas().rect;
        self.set_zoom(view::fit_zoom(canvas, view_size), None, true);
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
        let view_size = self.view_size();
        let anchor = anchor.unwrap_or((view_size.0 / 2.0, view_size.1 / 2.0));
        let hadj = self.inner.scrolled.hadjustment();
        let vadj = self.inner.scrolled.vadjustment();
        let (canvas, old_zoom, was_fitted) = {
            let state = self.inner.state.borrow();
            (state.doc.canvas().rect, state.zoom, state.fitted)
        };
        if (zoom - old_zoom).abs() < 1e-9 && fitted == was_fitted {
            return;
        }
        let scroll = view::scroll_for_zoom(
            canvas,
            view_size,
            old_zoom,
            (hadj.value(), vadj.value()),
            zoom,
            anchor,
        );
        {
            let mut state = self.inner.state.borrow_mut();
            state.zoom = zoom;
            state.fitted = fitted;
        }
        let (cw, ch) = view::content_size(canvas, zoom);
        self.inner.area.set_content_width(cw.ceil() as i32);
        self.inner.area.set_content_height(ch.ceil() as i32);
        // The new content size is only allocated later; extend the ranges now so the scroll
        // position is not clamped to the old size.
        hadj.set_upper(cw.max(view_size.0));
        vadj.set_upper(ch.max(view_size.1));
        hadj.set_value(scroll.0);
        vadj.set_value(scroll.1);
        self.inner.area.queue_draw();
        for listener in self.inner.listeners.borrow().iter() {
            listener(zoom, fitted);
        }
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
                self.inner.state.borrow_mut().space = true;
                self.inner.scrolled.set_cursor_from_name(Some("grab"));
            }
            _ => return false,
        }
        true
    }

    pub fn key_released(&self, key: gdk::Key) {
        if key == gdk::Key::space {
            let panning = {
                let mut state = self.inner.state.borrow_mut();
                state.space = false;
                state.pan_start.is_some()
            };
            if !panning {
                self.inner.scrolled.set_cursor(None);
            }
        }
    }
}

fn upgrade(weak: &Weak<Inner>) -> Option<Canvas> {
    weak.upgrade().map(|inner| Canvas { inner })
}

fn draw(inner: &Inner, cr: &gtk::cairo::Context, width: f64, height: f64) {
    let state = inner.state.borrow();
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
        view::origin(canvas, state.zoom, (width, height)),
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
