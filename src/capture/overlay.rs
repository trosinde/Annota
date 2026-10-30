//! Region-selection overlay (spec: Region capture; mockups 02–04).
//!
//! One undecorated full-screen window per monitor shows that monitor's part of the frozen
//! screenshot. The user drags a rectangle (live size chip, loupe, snapping), adjusts it with
//! handles, arrow keys or the W × H fields, and confirms with Enter or "Capture".

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use futures_channel::oneshot;
use gettextrs::gettext;
use gtk::{cairo, gdk, gio, glib, prelude::*};

use super::frame::{Frame, Rgb};
use super::geometry::{
    bounding_box, image_scale, loupe_position, monitor_crop, size_chip_position, toolbar_position,
    Direction, FRect, Handle, IRect, Selection, HANDLE_DIAMETER, LOUPE_DIAMETER, LOUPE_ZOOM,
};
use super::snap::{self, Axis, Edge};
use crate::cli::CaptureMode;

const UI_OVERLAY: &str = "/io/github/trosinde/Annota/ui/region-overlay.ui";

/// Kit colours (`docs/design/kit/tokens.css`).
const ACCENT: (f64, f64, f64) = (
    0x35 as f64 / 255.0,
    0x84 as f64 / 255.0,
    0xe4 as f64 / 255.0,
);
const VIOLET: (f64, f64, f64) = (
    0x8b as f64 / 255.0,
    0x5c as f64 / 255.0,
    0xf6 as f64 / 255.0,
);
const CORAL: (f64, f64, f64) = (1.0, 0x5a as f64 / 255.0, 0x4e as f64 / 255.0);
/// Outside dim `rgb(0 0 6 / 48%)`, 32 % while dragging (redline 02).
const DIM: f64 = 0.48;
const DIM_DRAGGING: f64 = 0.32;
const DIM_RGB: (f64, f64, f64) = (0.0, 0.0, 6.0 / 255.0);
/// Gap between loupe and its colour chip.
const LOUPE_CHIP_GAP: f64 = 8.0;
/// How long the pointer probe waits for the pointer to enter one of its windows.
const PROBE_TIMEOUT_MS: u64 = 400;

/// One monitor's part of the frozen desktop.
pub struct MonitorShot {
    pub monitor: Option<gdk::Monitor>,
    /// Where this monitor sits in the full screenshot (image px).
    pub crop: IRect,
    pub frame: Frame,
    /// Image px per logical px.
    pub scale: f64,
}

/// The frozen full-desktop screenshot split per monitor.
pub struct Desktop {
    pub full: Frame,
    pub monitors: Vec<MonitorShot>,
}

impl Desktop {
    pub fn new(full: Frame) -> Desktop {
        let mut gdk_monitors: Vec<(gdk::Monitor, IRect)> = Vec::new();
        if let Some(display) = gdk::Display::default() {
            let model = display.monitors();
            for i in 0..model.n_items() {
                if let Some(monitor) = model.item(i).and_downcast::<gdk::Monitor>() {
                    let g = monitor.geometry();
                    gdk_monitors.push((monitor, IRect::new(g.x(), g.y(), g.width(), g.height())));
                }
            }
        }
        let rects: Vec<IRect> = gdk_monitors.iter().map(|(_, r)| *r).collect();
        let monitors = match bounding_box(&rects) {
            Some(layout) => {
                let scale = image_scale(full.width(), layout);
                gdk_monitors
                    .into_iter()
                    .filter_map(|(monitor, rect)| {
                        let crop = monitor_crop(rect, layout, scale).intersect(full.bounds())?;
                        Some(MonitorShot {
                            monitor: Some(monitor),
                            crop,
                            frame: full.crop(crop)?,
                            scale,
                        })
                    })
                    .collect()
            }
            None => Vec::new(),
        };
        let monitors = if monitors.is_empty() {
            vec![MonitorShot {
                monitor: None,
                crop: full.bounds(),
                frame: full.clone(),
                scale: 1.0,
            }]
        } else {
            monitors
        };
        Desktop { full, monitors }
    }
}

/// Pointer position: monitor index and logical coordinates on that monitor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerAt {
    pub monitor: usize,
    pub x: f64,
    pub y: f64,
}

/// How the overlay ended.
#[derive(Debug)]
pub enum Outcome {
    /// Esc, "Cancel" or the window was closed: nothing is captured.
    Cancelled,
    /// The user picked another mode in the toolbar.
    Switch(CaptureMode),
    /// A confirmed region (image px of `monitor`).
    Region {
        monitor: usize,
        rect: IRect,
        include_pointer: bool,
        delay: u32,
        pointer: Option<PointerAt>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    Dragging,
    Adjusting,
    Resizing(Handle),
    Moving,
}

struct State {
    selection: Selection,
    phase: Phase,
    pointer: Option<(f64, f64)>,
    edges: Vec<Edge>,
    space: bool,
    drag_start: (f64, f64),
    last: (i32, i32),
    syncing: bool,
    cursor: &'static str,
}

struct Session {
    sender: RefCell<Option<oneshot::Sender<Outcome>>>,
    overlays: RefCell<Vec<Rc<MonitorOverlay>>>,
    active: Cell<Option<usize>>,
    first_pointer: Cell<Option<PointerAt>>,
}

impl Session {
    fn finish(&self, outcome: Outcome) {
        if let Some(sender) = self.sender.borrow_mut().take() {
            let _ = sender.send(outcome);
        }
        let overlays = self.overlays.take();
        for overlay in overlays {
            overlay.w.window.destroy();
        }
    }

    fn overlay(&self, index: usize) -> Option<Rc<MonitorOverlay>> {
        self.overlays.borrow().get(index).cloned()
    }

    fn activate(&self, index: usize) {
        if self.active.replace(Some(index)) == Some(index) {
            return;
        }
        let others: Vec<Rc<MonitorOverlay>> = self
            .overlays
            .borrow()
            .iter()
            .filter(|o| o.index != index)
            .cloned()
            .collect();
        for other in others {
            other.reset();
        }
    }

    fn note_pointer(&self, at: PointerAt) {
        if self.first_pointer.get().is_none() {
            self.first_pointer.set(Some(at));
        }
    }
}

struct Widgets {
    window: gtk::Window,
    overlay: gtk::Overlay,
    canvas: gtk::DrawingArea,
    size_chip: gtk::Box,
    size_label: gtk::Label,
    size_position_label: gtk::Label,
    loupe_chip: gtk::Box,
    loupe_swatch: gtk::DrawingArea,
    loupe_hex: gtk::Label,
    loupe_coordinates: gtk::Label,
    snap_chip: gtk::Box,
    toolbar: gtk::Box,
    width_text: gtk::Text,
    height_text: gtk::Text,
    pointer_button: gtk::ToggleButton,
    delay_label: gtk::Label,
    custom_delay_spin: gtk::SpinButton,
    hint_drag: gtk::Box,
    hint_snap: gtk::Box,
    hint_adjust: gtk::Box,
    mode_region_button: gtk::ToggleButton,
    mode_window_button: gtk::ToggleButton,
    mode_screen_button: gtk::ToggleButton,
    cancel_button: gtk::Button,
    capture_button: gtk::Button,
}

fn object<T: IsA<glib::Object>>(builder: &gtk::Builder, name: &str) -> Option<T> {
    let found = builder.object::<T>(name);
    if found.is_none() {
        eprintln!("annota: region-overlay.ui is missing object '{name}'");
    }
    found
}

impl Widgets {
    fn load() -> Option<(Widgets, gtk::Builder)> {
        let b = gtk::Builder::from_resource(UI_OVERLAY);
        let widgets = Widgets {
            window: object(&b, "overlay_window")?,
            overlay: object(&b, "overlay")?,
            canvas: object(&b, "canvas")?,
            size_chip: object(&b, "size_chip")?,
            size_label: object(&b, "size_label")?,
            size_position_label: object(&b, "size_position_label")?,
            loupe_chip: object(&b, "loupe_chip")?,
            loupe_swatch: object(&b, "loupe_swatch")?,
            loupe_hex: object(&b, "loupe_hex")?,
            loupe_coordinates: object(&b, "loupe_coordinates")?,
            snap_chip: object(&b, "snap_chip")?,
            toolbar: object(&b, "toolbar")?,
            width_text: object(&b, "width_text")?,
            height_text: object(&b, "height_text")?,
            pointer_button: object(&b, "pointer_button")?,
            delay_label: object(&b, "delay_label")?,
            custom_delay_spin: object(&b, "custom_delay_spin")?,
            hint_drag: object(&b, "hint_drag")?,
            hint_snap: object(&b, "hint_snap")?,
            hint_adjust: object(&b, "hint_adjust")?,
            mode_region_button: object(&b, "mode_region_button")?,
            mode_window_button: object(&b, "mode_window_button")?,
            mode_screen_button: object(&b, "mode_screen_button")?,
            cancel_button: object(&b, "cancel_button")?,
            capture_button: object(&b, "capture_button")?,
        };
        Some((widgets, b))
    }
}

struct MonitorOverlay {
    index: usize,
    scale: f64,
    frame: Frame,
    surface: cairo::ImageSurface,
    session: Weak<Session>,
    state: RefCell<State>,
    swatch: Cell<Rgb>,
    delay: Cell<u32>,
    w: Widgets,
}

/// Delay label in the toolbar: "Off" or "5 s".
pub fn delay_text(seconds: u32) -> String {
    if seconds == 0 {
        gettext("Off")
    } else {
        gettext("{seconds} s").replace("{seconds}", &seconds.to_string())
    }
}

/// Size chip text, e.g. "800 × 600".
pub fn size_text(rect: IRect) -> String {
    format!("{} × {}", rect.w, rect.h)
}

impl MonitorOverlay {
    fn to_image(&self, x: f64, y: f64) -> (i32, i32) {
        (
            (x * self.scale).round() as i32,
            (y * self.scale).round() as i32,
        )
    }

    fn area(&self) -> (f64, f64) {
        (
            f64::from(self.w.canvas.width()),
            f64::from(self.w.canvas.height()),
        )
    }

    fn session(&self) -> Option<Rc<Session>> {
        self.session.upgrade()
    }

    /// Clears this overlay's selection (another monitor became active).
    fn reset(&self) {
        {
            let mut state = self.state.borrow_mut();
            state.selection.clear();
            state.phase = Phase::Idle;
            state.edges.clear();
        }
        self.update_ui();
    }

    fn snap(&self, p: (i32, i32), free: bool) -> ((i32, i32), Vec<Edge>) {
        if free {
            return (p, Vec::new());
        }
        let snapped = snap::snap_point(&self.frame, p);
        (snapped.point, snapped.edges)
    }

    fn drag_begin(&self, x: f64, y: f64, free: bool) {
        if let Some(session) = self.session() {
            session.activate(self.index);
        }
        let p = self.to_image(x, y);
        {
            let mut state = self.state.borrow_mut();
            state.drag_start = (x, y);
            state.last = p;
            let adjusting = matches!(state.phase, Phase::Adjusting);
            let radius = (HANDLE_DIAMETER * self.scale).round() as i32;
            if adjusting {
                if let Some(handle) = state.selection.handle_at(p, radius) {
                    state.phase = Phase::Resizing(handle);
                } else if state.selection.contains(p) {
                    state.phase = Phase::Moving;
                }
            }
            if !matches!(state.phase, Phase::Resizing(_) | Phase::Moving) {
                let (start, edges) = self.snap(p, free);
                state.selection.begin(start);
                state.edges = edges;
                state.phase = Phase::Dragging;
            }
        }
        self.w.mode_region_button.set_active(true);
        self.update_ui();
    }

    fn drag_update(&self, dx: f64, dy: f64, square: bool, free: bool) {
        let (sx, sy) = self.state.borrow().drag_start;
        let p = self.to_image(sx + dx, sy + dy);
        {
            let mut state = self.state.borrow_mut();
            let last = state.last;
            state.last = p;
            match state.phase {
                Phase::Dragging if state.space => {
                    state.selection.move_by(p.0 - last.0, p.1 - last.1);
                    state.edges.clear();
                }
                Phase::Dragging => {
                    let (end, edges) = self.snap(p, free);
                    state.selection.drag_to(end, square);
                    state.edges = edges;
                }
                Phase::Resizing(handle) => {
                    let (end, edges) = self.snap(p, free);
                    state.selection.resize(handle, end);
                    state.edges = edges;
                }
                Phase::Moving => state.selection.move_by(p.0 - last.0, p.1 - last.1),
                Phase::Idle | Phase::Adjusting => {}
            }
            state.pointer = Some((sx + dx, sy + dy));
        }
        self.update_ui();
    }

    fn drag_end(&self) {
        {
            let mut state = self.state.borrow_mut();
            state.edges.clear();
            state.phase = if state.selection.rect().is_some() {
                Phase::Adjusting
            } else {
                Phase::Idle
            };
        }
        self.sync_entries();
        self.update_ui();
        self.w.canvas.grab_focus();
    }

    fn motion(&self, x: f64, y: f64) {
        if let Some(session) = self.session() {
            session.note_pointer(PointerAt {
                monitor: self.index,
                x,
                y,
            });
        }
        let p = self.to_image(x, y);
        let cursor = {
            let mut state = self.state.borrow_mut();
            let previous = state.pointer.replace((x, y));
            // Space held outside a drag moves the selection with the pointer.
            if state.space && matches!(state.phase, Phase::Adjusting) {
                if let Some((px, py)) = previous {
                    let last = self.to_image(px, py);
                    state.selection.move_by(p.0 - last.0, p.1 - last.1);
                }
            }
            let radius = (HANDLE_DIAMETER * self.scale).round() as i32;
            let name = match state.phase {
                Phase::Adjusting => match state.selection.handle_at(p, radius) {
                    Some(handle) => handle.cursor_name(),
                    None if state.selection.contains(p) => "move",
                    None => "crosshair",
                },
                Phase::Moving => "move",
                Phase::Resizing(handle) => handle.cursor_name(),
                Phase::Idle | Phase::Dragging => "crosshair",
            };
            (state.cursor != name).then(|| {
                state.cursor = name;
                name
            })
        };
        if let Some(name) = cursor {
            self.w.canvas.set_cursor_from_name(Some(name));
        }
        if let Some(color) = self.frame.pixel(p.0, p.1) {
            self.swatch.set(color);
        }
        self.update_ui();
    }

    fn nudge(&self, direction: Direction, shift: bool) -> bool {
        let handled = {
            let mut state = self.state.borrow_mut();
            if state.selection.rect().is_some() {
                state.selection.nudge(direction, shift);
                true
            } else {
                false
            }
        };
        if handled {
            self.sync_entries();
            self.update_ui();
        }
        handled
    }

    fn select_all(&self) {
        if let Some(session) = self.session() {
            session.activate(self.index);
        }
        {
            let mut state = self.state.borrow_mut();
            state.selection.select_all();
            state.phase = Phase::Adjusting;
        }
        self.sync_entries();
        self.update_ui();
    }

    /// W × H typed into the toolbar (spec: Fixed size).
    fn apply_typed_size(&self) {
        if self.state.borrow().syncing {
            return;
        }
        let parse = |text: &gtk::Text| text.text().trim().parse::<i32>().ok().filter(|v| *v > 0);
        let (Some(width), Some(height)) = (parse(&self.w.width_text), parse(&self.w.height_text))
        else {
            return;
        };
        {
            let mut state = self.state.borrow_mut();
            state.selection.set_size(width, height);
            state.phase = Phase::Adjusting;
        }
        self.update_ui();
    }

    fn sync_entries(&self) {
        let Some(rect) = self.state.borrow().selection.rect() else {
            return;
        };
        self.state.borrow_mut().syncing = true;
        self.w.width_text.set_text(&rect.w.to_string());
        self.w.height_text.set_text(&rect.h.to_string());
        self.state.borrow_mut().syncing = false;
    }

    fn confirm(&self) {
        let Some(rect) = self.state.borrow().selection.rect() else {
            return;
        };
        if let Some(session) = self.session() {
            let pointer = session.first_pointer.get();
            session.finish(Outcome::Region {
                monitor: self.index,
                rect,
                include_pointer: self.w.pointer_button.is_active(),
                delay: self.delay.get(),
                pointer,
            });
        }
    }

    fn update_ui(&self) {
        let (rect, phase, pointer, snapped) = {
            let state = self.state.borrow();
            (
                state.selection.rect(),
                state.phase,
                state.pointer,
                !state.edges.is_empty(),
            )
        };
        let adjusting = matches!(phase, Phase::Adjusting | Phase::Resizing(_) | Phase::Moving);

        self.w.size_chip.set_visible(rect.is_some());
        if let Some(rect) = rect {
            self.w.size_label.set_text(&size_text(rect));
            self.w.size_position_label.set_text(
                &gettext("at {x}, {y}")
                    .replace("{x}", &rect.x.to_string())
                    .replace("{y}", &rect.y.to_string()),
            );
        }
        self.w.size_position_label.set_visible(adjusting);
        if adjusting {
            self.w.size_chip.add_css_class("annota-chip-accent");
        } else {
            self.w.size_chip.remove_css_class("annota-chip-accent");
        }

        self.w
            .toolbar
            .set_visible(matches!(phase, Phase::Adjusting) && rect.is_some());

        let loupe = pointer.is_some()
            && matches!(phase, Phase::Idle | Phase::Dragging | Phase::Resizing(_));
        self.w.loupe_chip.set_visible(loupe);
        if let Some((x, y)) = pointer.filter(|_| loupe) {
            let (ix, iy) = self.to_image(x, y);
            self.w.loupe_hex.set_text(&self.swatch.get().hex());
            self.w.loupe_coordinates.set_text(&format!("{ix}, {iy}"));
            self.w.loupe_swatch.queue_draw();
        }

        self.w.snap_chip.set_visible(snapped);
        self.w.hint_snap.set_visible(snapped && !adjusting);
        self.w.hint_drag.set_visible(!snapped && !adjusting);
        self.w.hint_adjust.set_visible(adjusting);

        self.w.overlay.queue_allocate();
        self.w.canvas.queue_draw();
    }

    /// Positions of the floating chips and the toolbar (redlines 02–04).
    fn child_position(&self, child: &gtk::Widget) -> Option<gdk::Rectangle> {
        let area = self.area();
        let natural = |w: &gtk::Widget| {
            let (_, natural) = w.preferred_size();
            (f64::from(natural.width()), f64::from(natural.height()))
        };
        let rect_at = |pos: (f64, f64), size: (f64, f64)| {
            gdk::Rectangle::new(
                pos.0.round() as i32,
                pos.1.round() as i32,
                size.0.round() as i32,
                size.1.round() as i32,
            )
        };
        let state = self.state.borrow();
        let selection = state
            .selection
            .rect()
            .map(|r| FRect::from_image(r, self.scale));

        if child == self.w.size_chip.upcast_ref::<gtk::Widget>() {
            let size = natural(child);
            return Some(rect_at(size_chip_position(selection?, size, area), size));
        }
        if child == self.w.toolbar.upcast_ref::<gtk::Widget>() {
            let size = natural(child);
            return Some(rect_at(toolbar_position(selection?, size, area), size));
        }
        if child == self.w.loupe_chip.upcast_ref::<gtk::Widget>() {
            let size = natural(child);
            let (lx, ly) = loupe_position(state.pointer?, area, size.1 + LOUPE_CHIP_GAP);
            let pos = (
                (lx + LOUPE_DIAMETER / 2.0 - size.0 / 2.0).clamp(0.0, (area.0 - size.0).max(0.0)),
                ly + LOUPE_DIAMETER + LOUPE_CHIP_GAP,
            );
            return Some(rect_at(pos, size));
        }
        if child == self.w.snap_chip.upcast_ref::<gtk::Widget>() {
            let size = natural(child);
            let edge = state.edges.first()?;
            let (x, y) = match edge.axis {
                Axis::Vertical => (f64::from(edge.pos), f64::from(edge.start)),
                Axis::Horizontal => (f64::from(edge.start), f64::from(edge.pos)),
            };
            let pos = (
                (x / self.scale + 12.0).clamp(0.0, (area.0 - size.0).max(0.0)),
                (y / self.scale + 12.0).clamp(0.0, (area.1 - size.1).max(0.0)),
            );
            return Some(rect_at(pos, size));
        }
        None
    }

    fn draw(&self, cr: &cairo::Context, width: f64, height: f64) -> Result<(), cairo::Error> {
        let state = self.state.borrow();

        // Frozen screenshot.
        cr.save()?;
        cr.scale(1.0 / self.scale, 1.0 / self.scale);
        cr.set_source_surface(&self.surface, 0.0, 0.0)?;
        cr.paint()?;
        cr.restore()?;

        // Dim everything outside the selection.
        let selection = state
            .selection
            .rect()
            .map(|r| FRect::from_image(r, self.scale));
        let dim = if matches!(state.phase, Phase::Dragging) {
            DIM_DRAGGING
        } else {
            DIM
        };
        cr.set_source_rgba(DIM_RGB.0, DIM_RGB.1, DIM_RGB.2, dim);
        cr.set_fill_rule(cairo::FillRule::EvenOdd);
        cr.rectangle(0.0, 0.0, width, height);
        if let Some(sel) = selection {
            cr.rectangle(sel.x, sel.y, sel.w, sel.h);
        }
        cr.fill()?;

        if let Some(sel) = selection {
            // 1 px outer dark line, 1 px white border.
            cr.set_line_width(1.0);
            cr.set_source_rgba(DIM_RGB.0, DIM_RGB.1, DIM_RGB.2, 0.4);
            cr.rectangle(sel.x - 1.5, sel.y - 1.5, sel.w + 3.0, sel.h + 3.0);
            cr.stroke()?;
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.rectangle(sel.x - 0.5, sel.y - 0.5, sel.w + 1.0, sel.h + 1.0);
            cr.stroke()?;

            if matches!(
                state.phase,
                Phase::Adjusting | Phase::Resizing(_) | Phase::Moving
            ) {
                if let Some(rect) = state.selection.rect() {
                    for handle in Handle::ALL {
                        let (hx, hy) = handle.position(rect);
                        let (hx, hy) = (f64::from(hx) / self.scale, f64::from(hy) / self.scale);
                        cr.new_sub_path();
                        cr.arc(
                            hx,
                            hy,
                            HANDLE_DIAMETER / 2.0 - 1.0,
                            0.0,
                            std::f64::consts::TAU,
                        );
                        cr.set_source_rgb(1.0, 1.0, 1.0);
                        cr.fill_preserve()?;
                        cr.set_line_width(2.0);
                        cr.set_source_rgb(ACCENT.0, ACCENT.1, ACCENT.2);
                        cr.stroke()?;
                    }
                }
            }
        }

        // Violet snap guide along the detected window edge.
        cr.set_line_width(2.0);
        cr.set_source_rgb(VIOLET.0, VIOLET.1, VIOLET.2);
        for edge in &state.edges {
            let pos = f64::from(edge.pos) / self.scale;
            let start = f64::from(edge.start) / self.scale;
            let end = f64::from(edge.end) / self.scale;
            match edge.axis {
                Axis::Vertical => {
                    cr.move_to(pos, start);
                    cr.line_to(pos, end);
                }
                Axis::Horizontal => {
                    cr.move_to(start, pos);
                    cr.line_to(end, pos);
                }
            }
            cr.stroke()?;
        }

        let loupe_visible = matches!(
            state.phase,
            Phase::Idle | Phase::Dragging | Phase::Resizing(_)
        );
        if let Some(pointer) = state.pointer.filter(|_| loupe_visible) {
            let chip_height = {
                let (_, natural) = self.w.loupe_chip.preferred_size();
                f64::from(natural.height())
            };
            let origin = loupe_position(pointer, (width, height), chip_height + LOUPE_CHIP_GAP);
            self.draw_loupe(cr, origin, pointer)?;
        }
        Ok(())
    }

    /// Ø 136 px lens, 8× zoom, pixel grid, coral crosshair with an 8 px open centre box.
    fn draw_loupe(
        &self,
        cr: &cairo::Context,
        origin: (f64, f64),
        pointer: (f64, f64),
    ) -> Result<(), cairo::Error> {
        let radius = LOUPE_DIAMETER / 2.0;
        let (cx, cy) = (origin.0 + radius, origin.1 + radius);
        let ix = (pointer.0 * self.scale).floor();
        let iy = (pointer.1 * self.scale).floor();

        cr.save()?;
        cr.new_path();
        cr.arc(cx, cy, radius, 0.0, std::f64::consts::TAU);
        cr.clip();
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.paint()?;

        cr.save()?;
        cr.translate(cx, cy);
        cr.scale(LOUPE_ZOOM, LOUPE_ZOOM);
        cr.translate(-(ix + 0.5), -(iy + 0.5));
        cr.set_source_surface(&self.surface, 0.0, 0.0)?;
        cr.source().set_filter(cairo::Filter::Nearest);
        cr.paint()?;
        cr.restore()?;

        // Pixel grid.
        cr.set_line_width(1.0);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.12);
        let half = LOUPE_ZOOM / 2.0;
        for k in -9..=9 {
            let offset = f64::from(k) * LOUPE_ZOOM + half;
            cr.move_to(cx + offset, cy - radius);
            cr.line_to(cx + offset, cy + radius);
            cr.move_to(cx - radius, cy + offset);
            cr.line_to(cx + radius, cy + offset);
        }
        cr.stroke()?;

        // Crosshair with open centre.
        cr.set_line_width(2.0);
        cr.set_source_rgb(CORAL.0, CORAL.1, CORAL.2);
        cr.move_to(cx - radius, cy);
        cr.line_to(cx - half, cy);
        cr.move_to(cx + half, cy);
        cr.line_to(cx + radius, cy);
        cr.move_to(cx, cy - radius);
        cr.line_to(cx, cy - half);
        cr.move_to(cx, cy + half);
        cr.line_to(cx, cy + radius);
        cr.stroke()?;
        cr.set_line_width(1.0);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.85);
        cr.rectangle(
            cx - half - 0.5,
            cy - half - 0.5,
            LOUPE_ZOOM + 1.0,
            LOUPE_ZOOM + 1.0,
        );
        cr.stroke()?;
        cr.restore()?;

        // Ring.
        cr.new_path();
        cr.set_line_width(1.0);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.25);
        cr.arc(cx, cy, radius + 2.0, 0.0, std::f64::consts::TAU);
        cr.stroke()?;
        cr.set_line_width(3.0);
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.arc(cx, cy, radius, 0.0, std::f64::consts::TAU);
        cr.stroke()
    }
}

fn connect(overlay: &Rc<MonitorOverlay>) {
    let w = &overlay.w;
    let weak = Rc::downgrade(overlay);

    w.canvas.set_draw_func(glib::clone!(
        #[strong]
        weak,
        move |_, cr, width, height| {
            if let Some(this) = weak.upgrade() {
                if let Err(err) = this.draw(cr, f64::from(width), f64::from(height)) {
                    eprintln!("annota: overlay drawing failed: {err}");
                }
            }
        }
    ));
    w.loupe_swatch.set_draw_func(glib::clone!(
        #[strong]
        weak,
        move |_, cr, width, height| {
            if let Some(this) = weak.upgrade() {
                let c = this.swatch.get();
                cr.set_source_rgb(
                    f64::from(c.r) / 255.0,
                    f64::from(c.g) / 255.0,
                    f64::from(c.b) / 255.0,
                );
                cr.rectangle(0.0, 0.0, f64::from(width), f64::from(height));
                let _ = cr.fill_preserve();
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.8);
                cr.set_line_width(1.0);
                let _ = cr.stroke();
            }
        }
    ));
    w.overlay.connect_get_child_position(glib::clone!(
        #[strong]
        weak,
        move |_, child| weak.upgrade()?.child_position(child)
    ));

    let drag = gtk::GestureDrag::new();
    drag.set_button(gdk::BUTTON_PRIMARY);
    drag.connect_drag_begin(glib::clone!(
        #[strong]
        weak,
        move |gesture, x, y| {
            if let Some(this) = weak.upgrade() {
                let free = gesture
                    .current_event_state()
                    .contains(gdk::ModifierType::ALT_MASK);
                this.drag_begin(x, y, free);
            }
        }
    ));
    drag.connect_drag_update(glib::clone!(
        #[strong]
        weak,
        move |gesture, dx, dy| {
            if let Some(this) = weak.upgrade() {
                let state = gesture.current_event_state();
                this.drag_update(
                    dx,
                    dy,
                    state.contains(gdk::ModifierType::SHIFT_MASK),
                    state.contains(gdk::ModifierType::ALT_MASK),
                );
            }
        }
    ));
    drag.connect_drag_end(glib::clone!(
        #[strong]
        weak,
        move |_, _, _| {
            if let Some(this) = weak.upgrade() {
                this.drag_end();
            }
        }
    ));
    w.canvas.add_controller(drag);

    let motion = gtk::EventControllerMotion::new();
    motion.connect_enter(glib::clone!(
        #[strong]
        weak,
        move |_, x, y| {
            if let Some(this) = weak.upgrade() {
                this.motion(x, y);
            }
        }
    ));
    motion.connect_motion(glib::clone!(
        #[strong]
        weak,
        move |_, x, y| {
            if let Some(this) = weak.upgrade() {
                this.motion(x, y);
            }
        }
    ));
    w.canvas.add_controller(motion);

    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(glib::clone!(
        #[strong]
        weak,
        move |_, key, _, modifiers| match weak.upgrade() {
            Some(this) => key_pressed(&this, key, modifiers),
            None => glib::Propagation::Proceed,
        }
    ));
    keys.connect_key_released(glib::clone!(
        #[strong]
        weak,
        move |_, key, _, _| {
            if let Some(this) = weak.upgrade() {
                if key == gdk::Key::space {
                    this.state.borrow_mut().space = false;
                }
            }
        }
    ));
    w.window.add_controller(keys);

    for text in [&w.width_text, &w.height_text] {
        text.connect_changed(glib::clone!(
            #[strong]
            weak,
            move |_| {
                if let Some(this) = weak.upgrade() {
                    this.apply_typed_size();
                }
            }
        ));
        text.connect_activate(glib::clone!(
            #[strong]
            weak,
            move |_| {
                if let Some(this) = weak.upgrade() {
                    this.apply_typed_size();
                    this.w.canvas.grab_focus();
                }
            }
        ));
    }

    w.cancel_button.connect_clicked(glib::clone!(
        #[strong]
        weak,
        move |_| {
            if let Some(session) = weak.upgrade().and_then(|this| this.session()) {
                session.finish(Outcome::Cancelled);
            }
        }
    ));
    w.capture_button.connect_clicked(glib::clone!(
        #[strong]
        weak,
        move |_| {
            if let Some(this) = weak.upgrade() {
                this.confirm();
            }
        }
    ));
    w.mode_window_button.connect_toggled(glib::clone!(
        #[strong]
        weak,
        move |button| {
            if button.is_active() {
                if let Some(session) = weak.upgrade().and_then(|this| this.session()) {
                    session.finish(Outcome::Switch(CaptureMode::Window));
                }
            }
        }
    ));
    w.mode_screen_button.connect_toggled(glib::clone!(
        #[strong]
        weak,
        move |button| {
            if button.is_active() {
                if let Some(this) = weak.upgrade() {
                    this.select_all();
                }
            }
        }
    ));
    w.window.connect_close_request(glib::clone!(
        #[strong]
        weak,
        move |_| {
            match weak.upgrade().and_then(|this| this.session()) {
                // `finish` destroys every overlay window, this one included.
                Some(session) => {
                    session.finish(Outcome::Cancelled);
                    glib::Propagation::Stop
                }
                None => glib::Propagation::Proceed,
            }
        }
    ));

    // Delay dropdown: Off / 1 / 2 / 3 / 5 / 10 s / Custom (redline 03).
    let delay_action =
        gio::SimpleAction::new_stateful("delay", Some(glib::VariantTy::STRING), &"0".to_variant());
    delay_action.connect_activate(glib::clone!(
        #[strong]
        weak,
        move |action, parameter| {
            let (Some(this), Some(value)) =
                (weak.upgrade(), parameter.and_then(|p| p.get::<String>()))
            else {
                return;
            };
            action.set_state(&value.to_variant());
            let custom = value == "custom";
            this.w.custom_delay_spin.set_visible(custom);
            let seconds = if custom {
                this.w.custom_delay_spin.value_as_int().max(0) as u32
            } else {
                value.parse::<u32>().unwrap_or(0)
            };
            this.delay.set(seconds);
            this.w.delay_label.set_text(&delay_text(seconds));
        }
    ));
    w.custom_delay_spin.connect_value_changed(glib::clone!(
        #[strong]
        weak,
        move |spin| {
            if let Some(this) = weak.upgrade() {
                let seconds = spin.value_as_int().max(0) as u32;
                this.delay.set(seconds);
                this.w.delay_label.set_text(&delay_text(seconds));
            }
        }
    ));
    let actions = gio::SimpleActionGroup::new();
    actions.add_action(&delay_action);
    w.window.insert_action_group("overlay", Some(&actions));
}

fn key_pressed(
    this: &Rc<MonitorOverlay>,
    key: gdk::Key,
    modifiers: gdk::ModifierType,
) -> glib::Propagation {
    let Some(session) = this.session() else {
        return glib::Propagation::Proceed;
    };
    // Keys act on the monitor holding the selection, whichever overlay has keyboard focus.
    let target = session
        .active
        .get()
        .and_then(|index| session.overlay(index))
        .unwrap_or_else(|| this.clone());
    let shift = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
    let handled = match key {
        gdk::Key::Escape => {
            session.finish(Outcome::Cancelled);
            true
        }
        gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter => {
            target.confirm();
            true
        }
        gdk::Key::Left | gdk::Key::KP_Left => target.nudge(Direction::Left, shift),
        gdk::Key::Right | gdk::Key::KP_Right => target.nudge(Direction::Right, shift),
        gdk::Key::Up | gdk::Key::KP_Up => target.nudge(Direction::Up, shift),
        gdk::Key::Down | gdk::Key::KP_Down => target.nudge(Direction::Down, shift),
        gdk::Key::space => {
            target.state.borrow_mut().space = true;
            true
        }
        _ => false,
    };
    if handled {
        glib::Propagation::Stop
    } else {
        glib::Propagation::Proceed
    }
}

/// Shows the overlay on every monitor and waits for the user's choice.
pub async fn select_region(
    app: &adw::Application,
    desktop: &Desktop,
    include_pointer: bool,
) -> Outcome {
    let (sender, receiver) = oneshot::channel();
    let session = Rc::new(Session {
        sender: RefCell::new(Some(sender)),
        overlays: RefCell::new(Vec::new()),
        active: Cell::new(None),
        first_pointer: Cell::new(None),
    });

    for (index, shot) in desktop.monitors.iter().enumerate() {
        let Some((w, _builder)) = Widgets::load() else {
            session.finish(Outcome::Cancelled);
            break;
        };
        let surface = match shot.frame.to_surface() {
            Ok(surface) => surface,
            Err(err) => {
                eprintln!("annota: cannot show the frozen screen: {err}");
                session.finish(Outcome::Cancelled);
                break;
            }
        };
        w.pointer_button.set_active(include_pointer);
        w.window.set_application(Some(app));
        let overlay = Rc::new(MonitorOverlay {
            index,
            scale: shot.scale,
            frame: shot.frame.clone(),
            surface,
            session: Rc::downgrade(&session),
            state: RefCell::new(State {
                selection: Selection::new(shot.frame.width(), shot.frame.height()),
                phase: Phase::Idle,
                pointer: None,
                edges: Vec::new(),
                space: false,
                drag_start: (0.0, 0.0),
                last: (0, 0),
                syncing: false,
                cursor: "",
            }),
            swatch: Cell::new(Rgb { r: 0, g: 0, b: 0 }),
            delay: Cell::new(0),
            w,
        });
        connect(&overlay);
        overlay.w.canvas.set_cursor_from_name(Some("crosshair"));
        match &shot.monitor {
            Some(monitor) => overlay.w.window.fullscreen_on_monitor(monitor),
            None => overlay.w.window.fullscreen(),
        }
        session.overlays.borrow_mut().push(overlay.clone());
        overlay.w.window.present();
        overlay.w.canvas.grab_focus();
    }

    receiver.await.unwrap_or(Outcome::Cancelled)
}

/// Finds the monitor under the pointer (and where the pointer is) by showing the frozen
/// image full-screen on every monitor for a moment: the compositor reports the pointer to
/// whichever window it enters. Applications cannot query the global pointer position on
/// Wayland otherwise. Visually nothing changes, since the windows show the frozen screen.
pub async fn probe_pointer(app: &adw::Application, desktop: &Desktop) -> Option<PointerAt> {
    let (sender, receiver) = oneshot::channel::<PointerAt>();
    let sender = Rc::new(RefCell::new(Some(sender)));
    let mut windows = Vec::new();
    for (index, shot) in desktop.monitors.iter().enumerate() {
        let Ok(surface) = shot.frame.to_surface() else {
            continue;
        };
        let scale = shot.scale;
        let canvas = gtk::DrawingArea::new();
        canvas.set_draw_func(move |_, cr, _, _| {
            cr.scale(1.0 / scale, 1.0 / scale);
            if cr.set_source_surface(&surface, 0.0, 0.0).is_ok() {
                let _ = cr.paint();
            }
        });
        let motion = gtk::EventControllerMotion::new();
        let report = glib::clone!(
            #[strong]
            sender,
            move |_: &gtk::EventControllerMotion, x: f64, y: f64| {
                if let Some(sender) = sender.borrow_mut().take() {
                    let _ = sender.send(PointerAt {
                        monitor: index,
                        x,
                        y,
                    });
                }
            }
        );
        motion.connect_enter(report.clone());
        motion.connect_motion(report);
        canvas.add_controller(motion);
        let window = gtk::Window::builder()
            .application(app)
            .decorated(false)
            .child(&canvas)
            .build();
        window.add_css_class("annota-overlay");
        match &shot.monitor {
            Some(monitor) => window.fullscreen_on_monitor(monitor),
            None => window.fullscreen(),
        }
        window.present();
        windows.push(window);
    }
    let found =
        glib::future_with_timeout(std::time::Duration::from_millis(PROBE_TIMEOUT_MS), receiver)
            .await
            .ok()
            .and_then(Result::ok);
    for window in windows {
        window.destroy();
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_chip_text() {
        assert_eq!(size_text(IRect::new(0, 0, 400, 300)), "400 × 300");
    }

    #[test]
    fn delay_texts() {
        assert_eq!(delay_text(0), "Off");
        assert_eq!(delay_text(5), "5 s");
    }
}
