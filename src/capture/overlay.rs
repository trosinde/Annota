//! Region-selection overlay (spec: Region capture; mockups 02–04).
//!
//! One undecorated full-screen window per monitor shows that monitor's part of the frozen
//! screenshot. The user drags a rectangle (live size chip, loupe, snapping), adjusts it with
//! handles, arrow keys or the W × H fields, and confirms with Enter or "Capture".
//!
//! add-screen-recording ("Recording setup", mockups 01–02): the mode pill at the bottom switches
//! between Screenshot and Video. In Video mode the recording setup (`recording::setup`) sits
//! beside the selection instead of the toolbar; `Enter` is Start Recording and `Esc` cancels
//! without any request. Start Recording ends the overlay with [`Outcome::Record`].

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use futures_channel::oneshot;
use futures_util::StreamExt;
use gettextrs::gettext;
use gtk::{cairo, gdk, gio, glib, prelude::*};

use super::frame::{self, Frame, Rgb};
use super::geometry::{
    bounding_box, image_px, image_scale, loupe_position, monitor_crop, pointer_chip_position,
    size_chip_position, toolbar_position, Direction, FRect, Handle, IRect, Selection,
    HANDLE_DIAMETER, LOUPE_DIAMETER, LOUPE_ZOOM,
};
use super::snap::{self, Axis, Edge};
use crate::cli::CaptureMode;
use crate::recording::setup::Setup;

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
/// Recording setup placement (add-screen-recording redline 01): 18 px beside the selection,
/// kept this far inside the monitor, the arrow this far from the setup's top.
const SETUP_GAP: f64 = 18.0;
const SETUP_MARGIN: f64 = 8.0;
const SETUP_ARROW_Y: f64 = 40.0;
/// The mode pill's distance from the bottom edge (redline 01), or from the top edge while the
/// bottom would cover the selection's bottom handles.
const PILL_EDGE: f64 = 24.0;
/// Gap between loupe and its colour chip.
const LOUPE_CHIP_GAP: f64 = 8.0;
/// How long the pointer probe waits for the pointer to enter one of its windows.
const PROBE_TIMEOUT_MS: u64 = 400;
/// How long a freshly mapped probe window may still be animated in by the compositor. GNOME
/// Shell scales a new window up from its bottom centre (150 ms), and the pointer coordinates it
/// reports meanwhile are those of the scaled window: measured 2026-11-23 on GNOME Shell 50.1,
/// the first `enter` came ~120 ms after `present()` at 1107.7,179.9 for a pointer at 1200,500;
/// events from ~270 ms on were exact.
const PROBE_SETTLE_MS: u64 = 300;
/// How long the probe waits for its cover windows to take the pointer (measured: ~210 ms).
const PROBE_COVER_TIMEOUT_MS: u64 = 1000;

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

/// The glyph box the slash spans, in logical px. Deliberately larger than the arrow's own
/// bounding box (12.6 x 19.5), so the slash overshoots it at both ends as the mockups show.
const PREVIEW_GLYPH: (f64, f64) = (15.0, 22.0);

/// What the pointer preview draws this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Preview {
    /// Hot spot in this monitor's logical px.
    pointer: (f64, f64),
    /// The capture would contain the pointer.
    inside: bool,
    /// Outside, but within 1 image px of the selection's border.
    on_edge: bool,
}

/// What the preview shows, from nothing but values — so it is testable without a display and
/// provably reaches the same verdict as `capture::region_pointer`.
///
/// `None` means no arrow and no chip: Include Pointer is off, the pointer is on another monitor,
/// or there is no position at all (over the toolbar or the hint bar, or confirmed from the
/// keyboard). The hint-bar segment does *not* follow this — it stays whenever Include Pointer is
/// on, because that is exactly when the user needs to be told how to get the pointer into the
/// image (redlines, "Not drawn on purpose").
fn preview_for(
    include_pointer: bool,
    pointer: Option<PointerAt>,
    monitor: usize,
    rect: Option<IRect>,
    scale: f64,
) -> Option<Preview> {
    if !include_pointer {
        return None;
    }
    let at = pointer.filter(|at| at.monitor == monitor)?;
    let pointer = (at.x, at.y);
    let inside = rect.is_some_and(|r| r.pointer_inside(pointer, scale).is_some());
    Some(Preview {
        pointer,
        inside,
        on_edge: !inside && rect.is_some_and(|r| on_edge(r, pointer, scale)),
    })
}

/// Within 1 image px of the selection's border — the trap after releasing a drag, where the hot
/// spot sits exactly on the edge the user just dragged to.
fn on_edge(rect: IRect, pointer: (f64, f64), scale: f64) -> bool {
    let (x, y) = (pointer.0 * scale, pointer.1 * scale);
    let near = |v: f64, lo: i32, hi: i32| v >= f64::from(lo) - 1.0 && v <= f64::from(hi) + 1.0;
    near(x, rect.x, rect.right()) && near(y, rect.y, rect.bottom())
}

/// Pointer position: monitor index and logical coordinates on that monitor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerAt {
    pub monitor: usize,
    pub x: f64,
    pub y: f64,
}

/// What the overlay is for: a screenshot (the capture toolbar) or a video (the recording setup).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Screenshot,
    Video,
}

/// How the overlay ended.
#[derive(Debug)]
pub enum Outcome {
    /// Esc, "Cancel" or the window was closed: nothing is captured.
    Cancelled,
    /// Start Recording in Video mode: the setup's source, and for Region the selection in
    /// logical desktop px (the coordinates of a ScreenCast stream's `position`/`size`).
    Record {
        source: CaptureMode,
        selection: Option<IRect>,
    },
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
    /// Where the pointer is now, across all monitors, or `None` while it is not over any
    /// overlay's frozen image. `confirm` hands this to the capture, so the pointer is drawn
    /// where it stands when the user takes the shot (spec: Cursor inclusion).
    pointer: Cell<Option<PointerAt>>,
    mode: Cell<Mode>,
    /// The recording setup, made when Video mode is first shown.
    setup: RefCell<Option<Rc<Setup>>>,
    /// Keeps the capture toolbar's Include Pointer on the saved setting, which the setup's
    /// Include pointer row changes (add-screen-recording: "Same setting as for captures").
    pointer_setting: RefCell<Option<(gio::Settings, glib::SignalHandlerId)>>,
}

impl Session {
    fn new(sender: Option<oneshot::Sender<Outcome>>) -> Self {
        Self {
            sender: RefCell::new(sender),
            overlays: RefCell::new(Vec::new()),
            active: Cell::new(None),
            pointer: Cell::new(None),
            mode: Cell::new(Mode::Screenshot),
            setup: RefCell::new(None),
            pointer_setting: RefCell::new(None),
        }
    }

    /// The capture toolbar's Include Pointer follows the saved setting while the overlay is
    /// open, so a change in the setup's row shows when the user goes back to Screenshot.
    fn follow_pointer_setting(self: &Rc<Self>) {
        let settings = gio::Settings::new(crate::config::APP_ID);
        let weak = Rc::downgrade(self);
        let handler = settings.connect_changed(Some("include-pointer"), move |settings, key| {
            let Some(session) = weak.upgrade() else {
                return;
            };
            let on = settings.boolean(key);
            let overlays = session.overlays.borrow().clone();
            for overlay in overlays {
                overlay.w.pointer_button.set_active(on);
            }
        });
        self.pointer_setting.replace(Some((settings, handler)));
    }

    fn finish(&self, outcome: Outcome) {
        if let Some(sender) = self.sender.borrow_mut().take() {
            let _ = sender.send(outcome);
        }
        if let Some((settings, handler)) = self.pointer_setting.take() {
            settings.disconnect(handler);
        }
        // The level preview goes with the setup.
        let setup = self.setup.borrow_mut().take();
        if let Some(setup) = setup {
            setup.close();
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

    /// Switches every monitor's overlay to `mode` (the mode pill).
    fn set_mode(self: &Rc<Self>, mode: Mode) {
        if self.mode.replace(mode) == mode {
            return;
        }
        let overlays = self.overlays.borrow().clone();
        if mode == Mode::Video && self.setup.borrow().is_none() {
            let builders: Vec<gtk::Builder> = overlays.iter().map(|o| o.builder.clone()).collect();
            let weak = Rc::downgrade(self);
            let setup = Setup::new(&builders, move || {
                if let Some(session) = weak.upgrade() {
                    session.update_all();
                }
            });
            if setup.is_none() {
                eprintln!("annota: the recording setup cannot be shown");
            }
            self.setup.replace(setup);
        }
        if let Some(setup) = self.setup.borrow().as_ref() {
            setup.set_shown(mode == Mode::Video);
        }
        for overlay in &overlays {
            let button = match mode {
                Mode::Screenshot => &overlay.w.mode_screenshot_button,
                Mode::Video => &overlay.w.mode_video_button,
            };
            button.set_active(true);
            // `Enter` in the setup is Start Recording (redline "Shortcuts": the default widget;
            // the countdown spin activates it after taking its typed value).
            let default = (mode == Mode::Video).then_some(&overlay.w.record_start_button);
            overlay.w.window.set_default_widget(default);
            overlay.update_ui();
        }
        if mode == Mode::Video {
            self.focus_start();
        }
    }

    /// Initial focus of the setup: Start Recording (redline 01), on the monitor holding the
    /// selection, else the first.
    fn focus_start(&self) {
        let index = self.active.get().unwrap_or(0);
        if let Some(overlay) = self.overlay(index) {
            overlay.w.record_start_button.grab_focus();
        }
    }

    fn update_all(&self) {
        let overlays = self.overlays.borrow().clone();
        for overlay in overlays {
            overlay.update_ui();
        }
    }

    fn note_pointer(&self, at: PointerAt) {
        self.pointer.set(Some(at));
    }

    /// The pointer left `monitor`'s frozen image — onto the toolbar, the hint bar or another
    /// monitor. Only clears a position that still belongs to that monitor: GTK does not
    /// guarantee that the old surface's leave arrives before the new one's enter, and clearing
    /// unconditionally would throw away the position the next monitor just reported.
    fn clear_pointer(&self, monitor: usize) {
        if self.pointer.get().is_some_and(|at| at.monitor == monitor) {
            self.pointer.set(None);
        }
    }
}

/// The pointer segment prepended to one of the three hint boxes. Only the box belonging to the
/// current phase is on screen, so all three carry the same text and the visible one shows it.
struct PointerHint {
    box_: gtk::Box,
    glyph: gtk::Image,
    label: gtk::Label,
}

impl PointerHint {
    fn load(b: &gtk::Builder, suffix: &str) -> Option<Self> {
        Some(Self {
            box_: object(b, &format!("pointer_hint_{suffix}"))?,
            glyph: object(b, &format!("pointer_glyph_{suffix}"))?,
            label: object(b, &format!("pointer_label_{suffix}"))?,
        })
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
    pointer_chip: gtk::Box,
    pointer_chip_glyph: gtk::Image,
    pointer_chip_label: gtk::Label,
    pointer_chip_suffix: gtk::Label,
    pointer_hints: [PointerHint; 3],
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
    mode_screenshot_button: gtk::ToggleButton,
    mode_video_button: gtk::ToggleButton,
    mode_pill: gtk::Box,
    hint_video: gtk::Box,
    record_setup: gtk::Box,
    record_setup_arrow: gtk::Box,
    record_start_button: gtk::Button,
    record_countdown_spin: gtk::SpinButton,
}

fn set_class(widget: &impl IsA<gtk::Widget>, class: &str, on: bool) {
    if on {
        widget.add_css_class(class);
    } else {
        widget.remove_css_class(class);
    }
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
            pointer_chip: object(&b, "pointer_chip")?,
            pointer_chip_glyph: object(&b, "pointer_chip_glyph")?,
            pointer_chip_label: object(&b, "pointer_chip_label")?,
            pointer_chip_suffix: object(&b, "pointer_chip_suffix")?,
            pointer_hints: [
                PointerHint::load(&b, "drag")?,
                PointerHint::load(&b, "snap")?,
                PointerHint::load(&b, "adjust")?,
            ],
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
            mode_screenshot_button: object(&b, "mode_screenshot_button")?,
            mode_video_button: object(&b, "mode_video_button")?,
            mode_pill: object(&b, "mode_pill")?,
            hint_video: object(&b, "hint_video")?,
            record_setup: object(&b, "record_setup")?,
            record_setup_arrow: object(&b, "record_setup_arrow")?,
            record_start_button: object(&b, "record_start_button")?,
            record_countdown_spin: object(&b, "record_countdown_spin")?,
        };
        Some((widgets, b))
    }
}

struct MonitorOverlay {
    index: usize,
    scale: f64,
    /// The monitor's logical position on the desktop.
    origin: (i32, i32),
    /// For the recording setup's widgets (`recording::setup`).
    builder: gtk::Builder,
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
        (image_px(x, self.scale), image_px(y, self.scale))
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
        // In Video mode the setup's Start Recording keeps the initial focus (redline 01).
        if self.mode() == Mode::Video && self.w.record_start_button.is_sensitive() {
            self.w.record_start_button.grab_focus();
        } else {
            self.w.canvas.grab_focus();
        }
    }

    /// The pointer left this monitor's frozen image. The controller sits on the canvas, so this
    /// also fires when it moves onto the toolbar or the hint bar — Annota's own chrome, which is
    /// not in the frozen image. Either way there is no honest position to capture, so the
    /// session forgets it rather than keeping the last one over the canvas.
    fn pointer_left(&self) {
        if let Some(session) = self.session() {
            session.clear_pointer(self.index);
        }
        // Without this the arrow and the chip stay on screen exactly as they were: the canvas
        // keeps its cached render node and the overlay never re-runs `child_position`. With the
        // toolbar inside the selection (`geometry::toolbar_position`'s third branch, reachable
        // after Ctrl+A) the frozen chip would still read "Pointer in capture" while the capture
        // no longer contains one.
        self.update_ui();
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
            // The position at this moment, not the first one the overlay ever saw. `None` —
            // confirmed from the keyboard without the pointer over any frozen image — stays
            // `None`; there is no stale position to fall back to (spec: Pointer outside the
            // selection).
            let pointer = session.pointer.get();
            session.finish(Outcome::Region {
                monitor: self.index,
                rect,
                include_pointer: self.w.pointer_button.is_active(),
                delay: self.delay.get(),
                pointer,
            });
        }
    }

    /// `rect` (image px of this monitor) in logical desktop px.
    fn logical(&self, rect: IRect) -> IRect {
        let to = |v: i32| (f64::from(v) / self.scale).round() as i32;
        let (x0, y0) = (to(rect.x), to(rect.y));
        let (x1, y1) = (to(rect.right()), to(rect.bottom()));
        IRect::new(self.origin.0 + x0, self.origin.1 + y0, x1 - x0, y1 - y0)
    }

    fn mode(&self) -> Mode {
        self.session().map_or(Mode::Screenshot, |s| s.mode.get())
    }

    /// Start Recording (button or `Enter` in Video mode): ends the overlay with the setup's
    /// source; for Region with this monitor's selection, which stays as the recorded area.
    fn start_recording(&self) {
        let Some(session) = self.session() else {
            return;
        };
        let rect = self.state.borrow().selection.rect();
        let Some((source, can_start)) = session
            .setup
            .borrow()
            .as_ref()
            .map(|setup| (setup.source(), setup.can_start(rect.is_some())))
        else {
            return;
        };
        if !can_start {
            return;
        }
        let selection = match source {
            CaptureMode::Region => rect.map(|rect| self.logical(rect)),
            CaptureMode::Window | CaptureMode::Screen => None,
        };
        session.finish(Outcome::Record { source, selection });
    }

    /// Whether this monitor shows the recording setup: Video mode, not while the selection is
    /// being dragged, on the monitor holding the selection (every monitor before there is one).
    fn shows_setup(&self, phase: Phase) -> bool {
        let Some(session) = self.session() else {
            return false;
        };
        session.mode.get() == Mode::Video
            && matches!(phase, Phase::Idle | Phase::Adjusting)
            && session.active.get().is_none_or(|index| index == self.index)
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

        let video = self.mode() == Mode::Video;
        self.w
            .toolbar
            .set_visible(!video && matches!(phase, Phase::Adjusting) && rect.is_some());
        // The pill moves to the top edge while it would cover a bottom handle of the selection.
        let top = self.pill_top();
        let edge = PILL_EDGE as i32;
        self.w.mode_pill.set_valign(if top {
            gtk::Align::Start
        } else {
            gtk::Align::End
        });
        self.w.mode_pill.set_margin_top(if top { edge } else { 0 });
        self.w
            .mode_pill
            .set_margin_bottom(if top { 0 } else { edge });
        let setup = self.shows_setup(phase);
        self.w.record_setup.set_visible(setup);
        // Without an arrow (no selection, or no room beside, below or above it) the arrow's
        // widget would have no position and GtkOverlay would put it at (0, 0).
        self.w
            .record_setup_arrow
            .set_visible(setup && self.setup_placement().arrow.is_some());
        if let Some(session) = self.session() {
            if let Some(setup) = session.setup.borrow().as_ref() {
                self.w
                    .record_start_button
                    .set_sensitive(setup.can_start(rect.is_some()));
            }
        }

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
        self.w
            .hint_snap
            .set_visible(!video && snapped && !adjusting);
        self.w
            .hint_drag
            .set_visible(!video && !snapped && !adjusting);
        self.w.hint_adjust.set_visible(!video && adjusting);
        self.w.hint_video.set_visible(video);

        self.update_pointer_preview();

        self.w.overlay.queue_allocate();
        self.w.canvas.queue_draw();
    }

    /// Chip and hint segment of the pointer preview (redlines, "Pointer chip" and "Hint bar").
    /// All three hint segments carry the same text; only the box of the current phase is on
    /// screen, so the visible one shows it.
    fn update_pointer_preview(&self) {
        // The capture's pointer preview; a recording embeds the live pointer instead.
        let on = self.w.pointer_button.is_active() && self.mode() == Mode::Screenshot;
        let preview = self.preview_state();
        self.w.pointer_chip.set_visible(preview.is_some());
        // The segment stays while Include Pointer is on, even with the pointer over Annota's own
        // chrome or on another monitor — that is when the gesture needs teaching most. Only the
        // toggle takes it away (mockup 04).
        for hint in &self.w.pointer_hints {
            hint.box_.set_visible(on);
        }
        if !on {
            return;
        }
        // No position — over the toolbar, on another monitor, keyboard only — counts as outside:
        // no arrow, no chip, and the hint keeps its "outside" wording.
        let inside = preview.is_some_and(|p| p.inside);
        let on_edge = preview.is_some_and(|p| p.on_edge);
        let (glyph, chip_text, hint_text) = if inside {
            (
                "annota-pointer-included-symbolic",
                gettext("Pointer in capture"),
                gettext("Pointer in capture"),
            )
        } else {
            (
                "annota-pointer-excluded-symbolic",
                gettext("Pointer not in capture"),
                gettext("Move the pointer into the selection to include it"),
            )
        };
        self.w.pointer_chip_glyph.set_icon_name(Some(glyph));
        self.w.pointer_chip_label.set_text(&chip_text);
        self.w.pointer_chip_suffix.set_visible(on_edge);
        if on_edge {
            self.w.pointer_chip_suffix.set_text(&gettext("on the edge"));
        }
        set_class(&self.w.pointer_chip, "annota-pointer-in", inside);
        for hint in &self.w.pointer_hints {
            hint.glyph.set_icon_name(Some(glyph));
            hint.label.set_text(&hint_text);
            set_class(&hint.box_, "annota-pointer-in", inside);
        }
    }

    /// This monitor's selection in logical px.
    fn selection_rect(&self) -> Option<FRect> {
        self.state
            .borrow()
            .selection
            .rect()
            .map(|r| FRect::from_image(r, self.scale))
    }

    /// Whether the mode pill sits at the top edge ([`pill_at_top`]).
    fn pill_top(&self) -> bool {
        pill_at_top(
            self.selection_rect(),
            natural_size(self.w.mode_pill.upcast_ref()),
            self.area(),
        )
    }

    /// Where the recording setup and its arrow go on this monitor ([`setup_placement`]).
    fn setup_placement(&self) -> SetupPlacement {
        let area = self.area();
        let pill = self.w.mode_pill.is_visible().then(|| {
            pill_rect(
                self.pill_top(),
                natural_size(self.w.mode_pill.upcast_ref()),
                area,
            )
        });
        setup_placement(
            self.selection_rect(),
            natural_size(self.w.record_setup.upcast_ref()),
            area,
            pill,
        )
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
        if child == self.w.pointer_chip.upcast_ref::<gtk::Widget>() {
            let size = natural(child);
            let preview = self.preview_state()?;
            return Some(rect_at(
                pointer_chip_position(preview.pointer, size, area),
                size,
            ));
        }
        if child == self.w.record_setup.upcast_ref::<gtk::Widget>() {
            let size = natural(child);
            drop(state);
            return Some(rect_at(self.setup_placement().position, size));
        }
        if child == self.w.record_setup_arrow.upcast_ref::<gtk::Widget>() {
            let size = natural(child);
            drop(state);
            let (x, y) = self.setup_placement().arrow?;
            return Some(rect_at((x - size.0 / 2.0, y - size.1 / 2.0), size));
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

        // Last, so it sits above the dim and the selection border. The system cursor is drawn by
        // the compositor and stays on top of everything here.
        if let Some(preview) = self.preview_state() {
            self.draw_preview_pointer(cr, preview)?;
        }
        Ok(())
    }

    /// What the pointer preview shows right now, or `None` while nothing is drawn: Include
    /// Pointer off, or no pointer on this monitor's frozen image (spec: Pointer preview in the
    /// region overlay).
    fn preview_state(&self) -> Option<Preview> {
        preview_for(
            self.w.pointer_button.is_active() && self.mode() == Mode::Screenshot,
            self.session().and_then(|s| s.pointer.get()),
            self.index,
            self.state.borrow().selection.rect(),
            self.scale,
        )
    }

    /// The preview arrow at the hot spot: solid white when it will be captured, ghosted and
    /// slashed when it will not. Never carried by colour alone (redlines, "Preview arrow").
    fn draw_preview_pointer(
        &self,
        cr: &cairo::Context,
        preview: Preview,
    ) -> Result<(), cairo::Error> {
        cr.save()?;
        cr.translate(preview.pointer.0, preview.pointer.1);
        frame::pointer_path(cr);
        if preview.inside {
            // Redline: white fill, a 1 px outline on a 2.8 px halo — the same path stroked
            // twice. No drop shadow: cairo has no cheap blur and the halo already carries the
            // contrast the shadow was there for (recorded in design.md).
            cr.set_line_width(2.8);
            cr.set_source_rgba(0.0, 0.0, 0.024, 1.0);
            cr.stroke_preserve()?;
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.fill_preserve()?;
            cr.set_line_width(1.0);
            cr.set_source_rgba(0.0, 0.0, 0.024, 1.0);
            cr.stroke()?;
        } else {
            cr.set_line_width(3.0);
            cr.set_source_rgba(0.0, 0.0, 0.024, 0.6);
            cr.stroke_preserve()?;
            cr.set_source_rgba(0.0, 0.0, 0.024, 0.45);
            cr.fill_preserve()?;
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.48);
            cr.fill_preserve()?;
            cr.set_line_width(1.5);
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.stroke()?;
            // Slash across the glyph box, lower left to upper right. Kept inside the box so it
            // cannot merge with the overlay's own crosshair cursor.
            cr.move_to(0.0, PREVIEW_GLYPH.1);
            cr.line_to(PREVIEW_GLYPH.0, 0.0);
            cr.set_line_width(4.4);
            cr.set_source_rgba(0.0, 0.0, 0.024, 0.75);
            cr.stroke_preserve()?;
            cr.set_line_width(2.1);
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.stroke()?;
        }
        cr.restore()?;
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

/// Where the recording setup and its arrow go (redline 01).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SetupPlacement {
    /// The setup's top-left corner.
    pub(crate) position: (f64, f64),
    /// The centre of the arrow, on the setup's edge facing the selection. `None`: no arrow.
    pub(crate) arrow: Option<(f64, f64)>,
}

/// A widget's natural size in logical px.
fn natural_size(widget: &gtk::Widget) -> (f64, f64) {
    let (_, natural) = widget.preferred_size();
    (f64::from(natural.width()), f64::from(natural.height()))
}

/// `v` within `lo..=hi`, or their middle when the range is empty.
fn between(v: f64, lo: f64, hi: f64) -> f64 {
    if lo > hi {
        (lo + hi) / 2.0
    } else {
        v.clamp(lo, hi)
    }
}

/// Whether `a` and `b` come closer than `gap` to each other.
fn near_each_other(a: FRect, b: FRect, gap: f64) -> bool {
    a.x < b.x + b.w + gap && b.x < a.x + a.w + gap && a.y < b.y + b.h + gap && b.y < a.y + a.h + gap
}

/// The setup 18 px right of the selection, top-aligned with it; where there is no room, left of
/// it, else below it, else above it; where there is no room at all, at the monitor's right edge
/// without an arrow. It never covers the mode pill (`pill`, kept `SETUP_MARGIN` clear): beside
/// the selection it moves up or down to clear it, below or above it moves sideways. Without a
/// selection it is centred.
pub(crate) fn setup_placement(
    selection: Option<FRect>,
    size: (f64, f64),
    area: (f64, f64),
    pill: Option<FRect>,
) -> SetupPlacement {
    let (w, h) = size;
    let fit_x = |x: f64| between(x, SETUP_MARGIN, area.0 - w - SETUP_MARGIN);
    let fit_y = |y: f64| between(y, SETUP_MARGIN, area.1 - h - SETUP_MARGIN);
    let inside = |x: f64, y: f64| {
        x >= SETUP_MARGIN
            && y >= SETUP_MARGIN
            && x + w + SETUP_MARGIN <= area.0
            && y + h + SETUP_MARGIN <= area.1
    };
    let clear = |x: f64, y: f64| {
        pill.is_none_or(|pill| !near_each_other(FRect { x, y, w, h }, pill, SETUP_MARGIN))
    };
    // Where the setup goes along one axis to clear the pill: before it or after it.
    let around_pill = |start: f64, len: f64, vertical: bool| -> [f64; 3] {
        match pill {
            Some(p) if vertical => [start, p.y - SETUP_MARGIN - len, p.y + p.h + SETUP_MARGIN],
            Some(p) => [start, p.x - SETUP_MARGIN - len, p.x + p.w + SETUP_MARGIN],
            None => [start; 3],
        }
    };
    let Some(sel) = selection else {
        let (x, y) = (fit_x((area.0 - w) / 2.0), fit_y((area.1 - h) / 2.0));
        let y = around_pill(y, h, true)
            .into_iter()
            .find(|y| inside(x, *y) && clear(x, *y))
            .unwrap_or(y);
        return SetupPlacement {
            position: (x, y),
            arrow: None,
        };
    };

    // Beside: the arrow near the setup's top and within the selection's height, so the setup
    // must overlap the selection's height by the arrow's room.
    let (sel_top, sel_bottom) = (sel.y + 12.0, sel.y + sel.h - 12.0);
    let beside = |x: f64, edge: f64| {
        around_pill(fit_y(sel.y), h, true)
            .into_iter()
            .filter(|y| inside(x, *y) && clear(x, *y))
            .find(|y| {
                y + 20.0 <= sel_bottom.max(sel.y + sel.h / 2.0)
                    && y + h - 20.0 >= sel_top.min(sel.y + sel.h / 2.0)
            })
            .map(|y| {
                let along = between(y + SETUP_ARROW_Y, sel_top, sel_bottom);
                SetupPlacement {
                    position: (x, y),
                    arrow: Some((edge, between(along, y + 20.0, y + h - 20.0))),
                }
            })
    };
    // Below or above: centred on the selection, the arrow within the selection's width.
    let (sel_left, sel_right) = (sel.x + 12.0, sel.x + sel.w - 12.0);
    let centre = sel.x + sel.w / 2.0;
    let vertical = |y: f64, edge: f64| {
        around_pill(fit_x(centre - w / 2.0), w, false)
            .into_iter()
            .filter(|x| inside(*x, y) && clear(*x, y))
            .find(|x| x + 20.0 <= sel_right.max(centre) && x + w - 20.0 >= sel_left.min(centre))
            .map(|x| SetupPlacement {
                position: (x, y),
                arrow: Some((between(centre, x + 20.0, x + w - 20.0), edge)),
            })
    };
    let right = sel.x + sel.w + SETUP_GAP;
    let left = sel.x - SETUP_GAP - w;
    let below = sel.y + sel.h + SETUP_GAP;
    let above = sel.y - SETUP_GAP - h;
    beside(right, right)
        .or_else(|| beside(left, left + w))
        .or_else(|| vertical(below, below))
        .or_else(|| vertical(above, above + h))
        .unwrap_or_else(|| {
            let x = fit_x(area.0 - w);
            let y = around_pill(fit_y(sel.y), h, true)
                .into_iter()
                .find(|y| inside(x, *y) && clear(x, *y))
                .unwrap_or_else(|| fit_y(sel.y));
            SetupPlacement {
                position: (x, y),
                arrow: None,
            }
        })
}

/// Whether the mode pill goes to the top edge: at the bottom (24 px above the edge, centred) it
/// would cover one of the selection's bottom handles. It stays at the bottom when the top would
/// cover a top handle as well.
pub(crate) fn pill_at_top(selection: Option<FRect>, pill: (f64, f64), area: (f64, f64)) -> bool {
    let Some(sel) = selection else {
        return false;
    };
    let covers = |top: bool, y: f64| {
        let rect = pill_rect(top, pill, area);
        let r = HANDLE_DIAMETER / 2.0;
        [sel.x, sel.x + sel.w / 2.0, sel.x + sel.w].iter().any(|x| {
            near_each_other(
                FRect {
                    x: x - r,
                    y: y - r,
                    w: 2.0 * r,
                    h: 2.0 * r,
                },
                rect,
                0.0,
            )
        })
    };
    covers(false, sel.bottom()) && !covers(true, sel.y)
}

/// The mode pill's rectangle: centred, `PILL_EDGE` px from the bottom (or top) edge.
pub(crate) fn pill_rect(top: bool, pill: (f64, f64), area: (f64, f64)) -> FRect {
    let y = if top {
        PILL_EDGE
    } else {
        area.1 - PILL_EDGE - pill.1
    };
    FRect {
        x: (area.0 - pill.0) / 2.0,
        y,
        w: pill.0,
        h: pill.1,
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
    motion.connect_leave(glib::clone!(
        #[strong]
        weak,
        move |_| {
            if let Some(this) = weak.upgrade() {
                this.pointer_left();
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

    // `Enter` anywhere in the setup starts the recording, also on a source toggle or a switch,
    // whose own `Enter` binding would toggle them (they keep `Space`), and in the countdown spin
    // after taking its typed value.
    let enter = gtk::EventControllerKey::new();
    enter.set_propagation_phase(gtk::PropagationPhase::Capture);
    enter.connect_key_pressed(glib::clone!(
        #[strong]
        weak,
        move |_, key, _, _| {
            let Some(this) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let enter = matches!(
                key,
                gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter
            );
            if !enter || this.mode() != Mode::Video {
                return glib::Propagation::Proceed;
            }
            let inside = |focus: &gtk::Widget, widget: &gtk::Widget| {
                focus == widget || focus.is_ancestor(widget)
            };
            let Some(focus) = GtkWindowExt::focus(&this.w.window) else {
                return glib::Propagation::Proceed;
            };
            if !inside(&focus, this.w.record_setup.upcast_ref()) {
                return glib::Propagation::Proceed;
            }
            let spin = &this.w.record_countdown_spin;
            if inside(&focus, spin.upcast_ref()) {
                // GTK's spin commits typed text on the first `Enter` and activates the default
                // widget only on a second one (measured, GTK 4.22): take the value, then start.
                spin.update();
            }
            this.start_recording();
            glib::Propagation::Stop
        }
    ));
    w.window.add_controller(enter);

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

    // Include Pointer decides whether the preview is drawn at all, so it has to repaint.
    w.pointer_button.connect_toggled(glib::clone!(
        #[strong]
        weak,
        move |_| {
            if let Some(this) = weak.upgrade() {
                this.update_ui();
            }
        }
    ));
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
    w.record_start_button.connect_clicked(glib::clone!(
        #[strong]
        weak,
        move |_| {
            if let Some(this) = weak.upgrade() {
                this.start_recording();
            }
        }
    ));
    for (button, mode) in [
        (&w.mode_screenshot_button, Mode::Screenshot),
        (&w.mode_video_button, Mode::Video),
    ] {
        button.connect_toggled(glib::clone!(
            #[strong]
            weak,
            move |button| {
                if button.is_active() {
                    if let Some(session) = weak.upgrade().and_then(|this| this.session()) {
                        session.set_mode(mode);
                    }
                }
            }
        ));
    }
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

fn is_arrow(key: gdk::Key) -> bool {
    matches!(
        key,
        gdk::Key::Left
            | gdk::Key::KP_Left
            | gdk::Key::Right
            | gdk::Key::KP_Right
            | gdk::Key::Up
            | gdk::Key::KP_Up
            | gdk::Key::Down
            | gdk::Key::KP_Down
    )
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
            match session.mode.get() {
                Mode::Screenshot => target.confirm(),
                Mode::Video => target.start_recording(),
            }
            true
        }
        // In Video mode the arrows nudge only from the selection area; in the setup they move
        // the focus as everywhere else.
        _ if is_arrow(key) && session.mode.get() == Mode::Video && !this.w.canvas.has_focus() => {
            false
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

/// One monitor's overlay: widgets loaded, the frozen image in place, every handler connected —
/// everything but showing it. `None` when the UI or the image cannot be loaded.
fn build_overlay(
    index: usize,
    shot: &MonitorShot,
    session: &Rc<Session>,
    include_pointer: bool,
) -> Option<Rc<MonitorOverlay>> {
    let (w, builder) = Widgets::load()?;
    let surface = match shot.frame.to_surface() {
        Ok(surface) => surface,
        Err(err) => {
            eprintln!("annota: cannot show the frozen screen: {err}");
            return None;
        }
    };
    w.pointer_button.set_active(include_pointer);
    let origin = shot.monitor.as_ref().map_or((0, 0), |monitor| {
        let g = monitor.geometry();
        (g.x(), g.y())
    });
    let overlay = Rc::new(MonitorOverlay {
        index,
        scale: shot.scale,
        origin,
        builder,
        frame: shot.frame.clone(),
        surface,
        session: Rc::downgrade(session),
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
    Some(overlay)
}

/// Shows the overlay on every monitor in `mode` (Video: with the recording setup) and waits for
/// the user's choice.
pub async fn select_region(
    app: &adw::Application,
    desktop: &Desktop,
    include_pointer: bool,
    mode: Mode,
) -> Outcome {
    let (sender, receiver) = oneshot::channel();
    let session = Rc::new(Session::new(Some(sender)));
    #[cfg(test)]
    OPEN.with(|open| open.replace(Rc::downgrade(&session)));

    for (index, shot) in desktop.monitors.iter().enumerate() {
        let Some(overlay) = build_overlay(index, shot, &session, include_pointer) else {
            session.finish(Outcome::Cancelled);
            break;
        };
        overlay.w.window.set_application(Some(app));
        match &shot.monitor {
            Some(monitor) => overlay.w.window.fullscreen_on_monitor(monitor),
            None => overlay.w.window.fullscreen(),
        }
        session.overlays.borrow_mut().push(overlay.clone());
        overlay.w.window.present();
        overlay.w.canvas.grab_focus();
    }
    session.follow_pointer_setting();
    if mode == Mode::Video {
        session.set_mode(Mode::Video);
    }

    receiver.await.unwrap_or(Outcome::Cancelled)
}

#[cfg(test)]
thread_local! {
    /// The overlay session on the screen, for [`test_select`].
    static OPEN: RefCell<Weak<Session>> = const { RefCell::new(Weak::new()) };
}

/// Live tests: selects `rect` (image px) on monitor `monitor` of the open overlay, as a drag
/// that ended there would. `false` without an open overlay.
#[cfg(test)]
pub(crate) fn test_select(monitor: usize, rect: IRect) -> bool {
    let Some(session) = OPEN.with(|open| open.borrow().upgrade()) else {
        return false;
    };
    let Some(overlay) = session.overlay(monitor) else {
        return false;
    };
    session.activate(monitor);
    {
        let mut state = overlay.state.borrow_mut();
        state.selection.set_rect(rect);
        state.phase = Phase::Adjusting;
    }
    overlay.drag_end();
    true
}

/// The selection on `monitor` of the overlay on the screen (image px), for live tests.
#[cfg(test)]
pub(crate) fn test_selection(monitor: usize) -> Option<IRect> {
    let session = OPEN.with(|open| open.borrow().upgrade())?;
    let rect = session.overlay(monitor)?.state.borrow().selection.rect();
    rect
}

/// What a probe window reports.
#[derive(Clone, Copy, Debug, PartialEq)]
enum ProbeEvent {
    /// The pointer entered or moved on the probe window of `PointerAt::monitor`.
    At(PointerAt),
    /// The pointer left a probe window.
    Left,
}

/// What `probe_pointer` found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Probed {
    /// The monitor is always right; `x`, `y` only when `exact`.
    pub at: PointerAt,
    /// Whether the position was reported for a settled window, so the pointer may be drawn
    /// there. Otherwise only the monitor counts.
    pub exact: bool,
}

/// What `probe_pointer` does next.
#[derive(Clone, Copy, Debug, PartialEq)]
enum ProbeStep {
    /// Keep waiting, with the current phase's deadline.
    Stay,
    /// A new phase began; wait with its own deadline.
    Settle,
    /// Map the cover windows over the probe windows.
    Cover,
    /// Remove the cover windows.
    Uncover,
    /// The probe is over.
    Done(Option<Probed>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProbePhase {
    /// The probe windows are being mapped; waiting for the pointer to reach one.
    Mapping,
    /// The pointer is on a probe window, which may still be animated in.
    Settling,
    /// The cover windows are mapped; waiting for them to take the pointer.
    Covering,
    /// The cover windows are gone; waiting for the pointer to come back to a probe window.
    Uncovered,
}

/// The decisions of `probe_pointer`, apart from its windows.
///
/// A position the compositor reports while it is still animating the probe window in is the
/// position on the scaled window, not on the screen (see `PROBE_SETTLE_MS`), and a pointer that
/// does not move gets no second event once the animation is over. So the first event only says
/// that the pointer is on one of the probe windows. After the animation, cover windows showing
/// the same frozen image are mapped over the probe windows and removed again as soon as they have
/// the pointer: the compositor then hands the pointer back to the settled probe window with an
/// exact position, whether the pointer moves or not.
#[derive(Debug)]
struct PointerProbe {
    phase: ProbePhase,
    /// Whether the position counts, or only the monitor (see `probe_pointer`).
    exact: bool,
    /// The first event: its monitor is right, its position may be a scaled one.
    first: Option<PointerAt>,
}

impl PointerProbe {
    fn new(exact: bool) -> Self {
        Self {
            phase: ProbePhase::Mapping,
            exact,
            first: None,
        }
    }

    /// How long the current phase may last.
    fn deadline(&self) -> std::time::Duration {
        std::time::Duration::from_millis(match self.phase {
            ProbePhase::Mapping | ProbePhase::Uncovered => PROBE_TIMEOUT_MS,
            ProbePhase::Settling => PROBE_SETTLE_MS,
            ProbePhase::Covering => PROBE_COVER_TIMEOUT_MS,
        })
    }

    fn event(&mut self, event: ProbeEvent) -> ProbeStep {
        match (self.phase, event) {
            // The monitor is right even while the window is scaled in; the position is not.
            (ProbePhase::Mapping, ProbeEvent::At(at)) if !self.exact => {
                ProbeStep::Done(Some(Probed { at, exact: false }))
            }
            (ProbePhase::Mapping, ProbeEvent::At(at)) => {
                self.first = Some(at);
                self.phase = ProbePhase::Settling;
                ProbeStep::Settle
            }
            (ProbePhase::Covering, ProbeEvent::Left) => {
                self.phase = ProbePhase::Uncovered;
                ProbeStep::Uncover
            }
            // Only the compositor's look at the probe windows after the covers are gone is known
            // to see a settled window; before that, even a moving pointer is not taken.
            (ProbePhase::Uncovered, ProbeEvent::At(at)) => {
                ProbeStep::Done(Some(Probed { at, exact: true }))
            }
            (ProbePhase::Mapping | ProbePhase::Uncovered, ProbeEvent::Left)
            | (ProbePhase::Settling | ProbePhase::Covering, _) => ProbeStep::Stay,
        }
    }

    fn timed_out(&mut self) -> ProbeStep {
        match self.phase {
            // The pointer is on none of the probe windows.
            ProbePhase::Mapping => ProbeStep::Done(None),
            // The cover step did not bring an exact position back. The first event's monitor
            // still holds, so a screen capture keeps the right monitor; nothing is drawn.
            ProbePhase::Uncovered => {
                ProbeStep::Done(self.first.map(|at| Probed { at, exact: false }))
            }
            ProbePhase::Settling => {
                self.phase = ProbePhase::Covering;
                ProbeStep::Cover
            }
            // The covers never took the pointer; removing them still makes the compositor look
            // at the probe windows again.
            ProbePhase::Covering => {
                self.phase = ProbePhase::Uncovered;
                ProbeStep::Uncover
            }
        }
    }
}

/// A fullscreen window on `shot`'s monitor showing the frozen image, so that mapping it changes
/// nothing the user sees.
fn frozen_window(
    app: &adw::Application,
    shot: &MonitorShot,
) -> Option<(gtk::Window, gtk::DrawingArea)> {
    let surface = shot.frame.to_surface().ok()?;
    let scale = shot.scale;
    let canvas = gtk::DrawingArea::new();
    canvas.set_draw_func(move |_, cr, _, _| {
        cr.scale(1.0 / scale, 1.0 / scale);
        if cr.set_source_surface(&surface, 0.0, 0.0).is_ok() {
            let _ = cr.paint();
        }
    });
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
    Some((window, canvas))
}

/// Finds the monitor under the pointer (and where the pointer is) by showing the frozen
/// image full-screen on every monitor for a moment: the compositor reports the pointer to
/// whichever window it enters. Applications cannot query the global pointer position on
/// Wayland otherwise. Visually nothing changes, since the windows show the frozen screen.
/// `PointerProbe` says why it takes a second set of windows to get an exact position; with
/// `exact` false only the monitor counts, and the first event is enough.
pub async fn probe_pointer(
    app: &adw::Application,
    desktop: &Desktop,
    exact: bool,
) -> Option<Probed> {
    let (sender, mut events) = futures_channel::mpsc::unbounded::<ProbeEvent>();
    let mut windows = ProbeWindows::default();
    for (index, shot) in desktop.monitors.iter().enumerate() {
        let Some((window, canvas)) = frozen_window(app, shot) else {
            continue;
        };
        let motion = gtk::EventControllerMotion::new();
        let report = glib::clone!(
            #[strong]
            sender,
            move |_: &gtk::EventControllerMotion, x: f64, y: f64| {
                let _ = sender.unbounded_send(ProbeEvent::At(PointerAt {
                    monitor: index,
                    x,
                    y,
                }));
            }
        );
        motion.connect_enter(report.clone());
        motion.connect_motion(report);
        motion.connect_leave(glib::clone!(
            #[strong]
            sender,
            move |_| {
                let _ = sender.unbounded_send(ProbeEvent::Left);
            }
        ));
        canvas.add_controller(motion);
        window.present();
        windows.probes.push(window);
    }
    drop(sender);

    let mut probe = PointerProbe::new(exact);
    let mut phase_end = std::time::Instant::now() + probe.deadline();
    let found = loop {
        let left = phase_end.saturating_duration_since(std::time::Instant::now());
        let step = match glib::future_with_timeout(left, events.next()).await {
            Ok(Some(event)) => probe.event(event),
            // Every probe window is gone.
            Ok(None) => ProbeStep::Done(None),
            Err(_) => probe.timed_out(),
        };
        match step {
            ProbeStep::Stay => continue,
            ProbeStep::Settle => {}
            ProbeStep::Cover => {
                windows.covers = desktop
                    .monitors
                    .iter()
                    .filter_map(|shot| frozen_window(app, shot).map(|(window, _)| window))
                    .collect();
                for cover in &windows.covers {
                    cover.present();
                }
            }
            ProbeStep::Uncover => {
                for cover in windows.covers.drain(..) {
                    cover.destroy();
                }
            }
            ProbeStep::Done(found) => break found,
        }
        phase_end = std::time::Instant::now() + probe.deadline();
    };
    found
}

/// The probe's windows, destroyed on every way out of `probe_pointer`, a dropped future
/// included.
#[derive(Default)]
struct ProbeWindows {
    probes: Vec<gtk::Window>,
    covers: Vec<gtk::Window>,
}

impl Drop for ProbeWindows {
    fn drop(&mut self) {
        for window in self.covers.drain(..).chain(self.probes.drain(..)) {
            window.destroy();
        }
    }
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

    /// A session with no overlays: enough for what `confirm` reads out of it.
    fn session() -> Session {
        Session::new(None)
    }

    fn at(monitor: usize, x: f64, y: f64) -> PointerAt {
        PointerAt { monitor, x, y }
    }

    // spec: capture / Pointer preview in the region overlay / Indication follows the selection
    #[test]
    fn the_preview_follows_the_selection_before_confirming() {
        // The preview is computed from the live selection (`preview_state` reads
        // `state.selection.rect()`), and every edit of the selection ends in `update_ui`, which
        // calls `update_pointer_preview`. So moving or resizing the selection flips the verdict
        // with no confirmation in between. Pointer 5 px inside the selection's right edge.
        let mut selection = Selection::new(1920, 1080);
        selection.set_rect(IRect::new(100, 100, 400, 300));
        let pointer = Some(at(0, 495.0, 250.0));
        let inside = |s: &Selection| preview_for(true, pointer, 0, s.rect(), 1.0).map(|p| p.inside);
        assert_eq!(inside(&selection), Some(true));

        // Shift+Left moves the selection 10 px out from under the pointer, Shift+Right back.
        selection.nudge(Direction::Left, true);
        assert_eq!(inside(&selection), Some(false), "after moving out");
        selection.nudge(Direction::Right, true);
        assert_eq!(inside(&selection), Some(true), "after moving back");

        // Dragging the right handle in past the pointer, then out again.
        selection.resize(Handle::East, (480, 250));
        assert_eq!(inside(&selection), Some(false), "after resizing smaller");
        selection.resize(Handle::East, (520, 250));
        assert_eq!(inside(&selection), Some(true), "after resizing larger");
    }

    // spec: capture / Pointer preview in the region overlay / No indication when excluded
    #[test]
    fn the_preview_is_off_while_include_pointer_is_off() {
        // Scenario "No indication when excluded": nothing at all, whatever the pointer does.
        let rect = Some(IRect::new(0, 0, 400, 300));
        assert_eq!(
            preview_for(false, Some(at(0, 100.0, 100.0)), 0, rect, 1.0),
            None
        );
    }

    // spec: capture / Pointer preview in the region overlay / Pointer will not be included
    #[test]
    fn the_preview_has_no_state_without_a_position_on_this_monitor() {
        // Over the toolbar or the hint bar, on another monitor, or confirmed from the keyboard.
        // All three give no arrow and no chip; the hint segment is driven by the toggle instead,
        // so it keeps teaching the gesture.
        let rect = Some(IRect::new(0, 0, 400, 300));
        assert_eq!(
            preview_for(true, None, 0, rect, 1.0),
            None,
            "no position at all"
        );
        assert_eq!(
            preview_for(true, Some(at(1, 100.0, 100.0)), 0, rect, 1.0),
            None,
            "pointer on another monitor"
        );
    }

    fn probe_at(x: f64, y: f64) -> ProbeEvent {
        ProbeEvent::At(PointerAt { monitor: 0, x, y })
    }

    // spec: capture / Cursor inclusion / Delayed region capture places the pointer
    #[test]
    fn the_probe_ignores_positions_from_a_window_still_animating_in() {
        // Live check 8 (2026-11-23, GNOME Shell 50.1, 3840×2160 at 125 %), pointer still at
        // 1200,500 after the countdown. The probe window's first `enter` came while GNOME Shell
        // was still scaling the window in and said 1107.7,179.9; one more event during the
        // animation said 1184.5,442.1. Neither may become the result: taken as is, the arrow
        // lands outside the selection and is dropped, which was the defect.
        let mut probe = PointerProbe::new(true);
        assert_eq!(probe.event(probe_at(1107.7, 179.9)), ProbeStep::Settle);
        assert_eq!(probe.event(probe_at(1184.5, 442.1)), ProbeStep::Stay);
        // After the settle time the covers go up, take the pointer and come down again; the
        // compositor then reports the exact position to the settled probe window.
        assert_eq!(probe.timed_out(), ProbeStep::Cover);
        assert_eq!(probe.event(ProbeEvent::Left), ProbeStep::Uncover);
        assert_eq!(
            probe.event(probe_at(1200.0, 500.0)),
            ProbeStep::Done(Some(Probed {
                at: PointerAt {
                    monitor: 0,
                    x: 1200.0,
                    y: 500.0
                },
                exact: true
            }))
        );
    }

    // spec: capture / Cursor inclusion / Delayed region capture places the pointer
    #[test]
    fn a_moving_pointer_also_waits_for_the_covers() {
        // The settle time is a guess at the animation's length; a position before the covers
        // came down might still be a scaled one on a slow machine, so none is taken.
        let mut probe = PointerProbe::new(true);
        assert_eq!(probe.event(probe_at(10.0, 10.0)), ProbeStep::Settle);
        assert_eq!(probe.timed_out(), ProbeStep::Cover);
        assert_eq!(probe.event(probe_at(1201.0, 500.0)), ProbeStep::Stay);
        assert_eq!(probe.event(ProbeEvent::Left), ProbeStep::Uncover);
        assert_eq!(
            probe.event(probe_at(1200.0, 500.0)),
            ProbeStep::Done(Some(Probed {
                at: PointerAt {
                    monitor: 0,
                    x: 1200.0,
                    y: 500.0
                },
                exact: true
            }))
        );
    }

    // spec: capture / Cursor inclusion / Delayed region capture places the pointer
    #[test]
    fn without_a_pointer_on_its_windows_the_probe_finds_none() {
        // The pointer on no monitor of the desktop, or held by another client.
        let mut probe = PointerProbe::new(true);
        assert_eq!(probe.timed_out(), ProbeStep::Done(None));
        // Covers that never take the pointer are removed, and the probe still ends.
        let mut probe = PointerProbe::new(true);
        assert_eq!(probe.event(probe_at(10.0, 10.0)), ProbeStep::Settle);
        assert_eq!(probe.timed_out(), ProbeStep::Cover);
        assert_eq!(probe.timed_out(), ProbeStep::Uncover);
        assert_eq!(probe.event(ProbeEvent::Left), ProbeStep::Stay);
        // The first event's monitor survives, its position is not to be drawn.
        assert_eq!(
            probe.timed_out(),
            ProbeStep::Done(Some(Probed {
                at: PointerAt {
                    monitor: 0,
                    x: 10.0,
                    y: 10.0
                },
                exact: false
            }))
        );
    }

    // spec: capture / Full-screen capture / Multi-monitor
    #[test]
    fn a_failed_cover_step_keeps_the_monitor() {
        // Exact mode (include-pointer on), pointer on monitor 2, covers that never bring an
        // exact position back: the screen capture must still take monitor 2, without an arrow.
        let mut probe = PointerProbe::new(true);
        let first = PointerAt {
            monitor: 1,
            x: 1107.7,
            y: 179.9,
        };
        assert_eq!(probe.event(ProbeEvent::At(first)), ProbeStep::Settle);
        assert_eq!(probe.timed_out(), ProbeStep::Cover);
        assert_eq!(probe.timed_out(), ProbeStep::Uncover);
        assert_eq!(
            probe.timed_out(),
            ProbeStep::Done(Some(Probed {
                at: first,
                exact: false
            }))
        );
    }

    // spec: capture / Full-screen capture / Multi-monitor
    #[test]
    fn a_probe_for_the_monitor_takes_the_first_event() {
        // The monitor of a window being scaled in is right; only its position is not.
        let mut probe = PointerProbe::new(false);
        let at = PointerAt {
            monitor: 1,
            x: 1107.7,
            y: 179.9,
        };
        assert_eq!(
            probe.event(ProbeEvent::At(at)),
            ProbeStep::Done(Some(Probed { at, exact: false }))
        );
    }

    // spec: capture / Pointer preview in the region overlay / Pointer will be included
    // spec: capture / Pointer preview in the region overlay / Pointer will not be included
    #[test]
    fn the_preview_reaches_the_captures_verdict() {
        // Task 4.2: the preview's own decision path (`preview_for`, toggle and monitor filter
        // included) against the image the capture really produces: "in capture" exactly when
        // the composed region differs from the plain crop. Positions straddle every edge of the
        // selection by fractions of a pixel, at 100 % and 125 %, because that is where the two
        // could part ways.
        let rect = IRect::new(140, 212, 400, 300);
        let around = |lo: i32, hi: i32| {
            let mut values = vec![f64::from(lo + hi) / 2.0];
            for edge in [lo, hi] {
                for d in [-1.6, -1.0, -0.6, -0.5, -0.4, 0.0, 0.4, 0.5, 0.6, 1.0] {
                    values.push(f64::from(edge) + d);
                }
            }
            values
        };
        for scale in [1.0, 1.25] {
            let desktop = desktop(800, 600, scale);
            let plain = desktop.full.crop(rect).expect("crop");
            for ix in around(rect.x, rect.right()) {
                for iy in around(rect.y, rect.bottom()) {
                    let pointer = Some(at(0, ix / scale, iy / scale));
                    let preview = preview_for(true, pointer, 0, Some(rect), scale);
                    let image = super::super::compose_region(&desktop, 0, rect, true, pointer)
                        .expect("region");
                    assert_eq!(
                        preview.is_some_and(|p| p.inside),
                        image != plain,
                        "preview and image disagree at image px {ix}, {iy}, scale {scale}"
                    );
                }
            }
        }
    }

    // spec: capture / Pointer preview in the region overlay / Pointer will be included
    // spec: capture / Pointer preview in the region overlay / Pointer will not be included
    #[test]
    fn the_edge_suffix_fires_only_just_outside() {
        // The chip's "on the edge" sub-state, which names the trap after releasing a drag.
        let rect = IRect::new(0, 0, 800, 600);
        let edge = preview_for(true, Some(at(0, 800.0, 600.0)), 0, Some(rect), 1.0).unwrap();
        assert!(!edge.inside && edge.on_edge, "released on the corner");
        let far = preview_for(true, Some(at(0, 900.0, 300.0)), 0, Some(rect), 1.0).unwrap();
        assert!(
            !far.inside && !far.on_edge,
            "well outside is not 'on the edge'"
        );
        let inside = preview_for(true, Some(at(0, 400.0, 300.0)), 0, Some(rect), 1.0).unwrap();
        assert!(inside.inside && !inside.on_edge);
    }

    // spec: capture / Pointer preview in the region overlay / Pointer will not be included
    #[test]
    fn without_a_selection_nothing_is_in_capture() {
        // Phase Idle: the overlay is up but nothing is selected yet.
        let p = preview_for(true, Some(at(0, 100.0, 100.0)), 0, None, 1.0).unwrap();
        assert!(!p.inside && !p.on_edge);
    }

    // spec: capture / Cursor inclusion / Pointer inside the selection
    #[test]
    fn the_pointer_is_the_one_at_confirmation_not_the_first_seen() {
        // The reported defect: the overlay kept only the *first* position it ever saw — where
        // the mouse stood when the overlay opened, in practice over the capture button — so the
        // arrow was drawn outside the region the user then dragged and cairo clipped it away.
        let session = session();
        session.note_pointer(at(0, 12.0, 8.0));
        session.note_pointer(at(0, 310.0, 488.0));
        assert_eq!(session.pointer.get(), Some(at(0, 310.0, 488.0)));
    }

    // spec: capture / Cursor inclusion / Pointer outside the selection
    #[test]
    fn a_keyboard_only_capture_has_no_pointer_position() {
        // Confirmed with Enter without the pointer ever being over a frozen image. Treated as
        // "outside": no pointer drawn, and no stale position to fall back to.
        assert_eq!(session().pointer.get(), None);
    }

    // spec: capture / Cursor inclusion / Pointer outside the selection
    #[test]
    fn leaving_the_frozen_image_forgets_the_position() {
        // Onto the toolbar or the hint bar: that chrome is not in the frozen image, so no
        // honest position is left.
        let session = session();
        session.note_pointer(at(0, 310.0, 488.0));
        session.clear_pointer(0);
        assert_eq!(session.pointer.get(), None);
    }

    // spec: capture / Cursor inclusion / Pointer inside the selection
    #[test]
    fn a_late_leave_does_not_erase_the_next_monitors_position() {
        // GTK does not guarantee that the old surface's leave arrives before the new surface's
        // enter, so the leave must only clear a position that is still its own monitor's.
        let session = session();
        session.note_pointer(at(0, 10.0, 10.0));
        session.note_pointer(at(1, 20.0, 20.0));
        session.clear_pointer(0);
        assert_eq!(session.pointer.get(), Some(at(1, 20.0, 20.0)));
    }

    /// One uniformly grey monitor of `w` x `h` image px at `scale`.
    fn desktop(w: i32, h: i32, scale: f64) -> Desktop {
        let full = Frame::filled(
            w,
            h,
            Rgb {
                r: 0x80,
                g: 0x80,
                b: 0x80,
            },
        )
        .expect("frame");
        Desktop {
            monitors: vec![MonitorShot {
                monitor: None,
                crop: full.bounds(),
                frame: full.clone(),
                scale,
            }],
            full,
        }
    }

    // spec: capture / Cursor inclusion / Pointer on the selection edge
    #[test]
    fn a_fractional_release_on_the_edge_draws_no_pointer() {
        // Wayland reports fractional positions, at 125 % almost always. The overlay rounds the
        // drag's end to an image pixel (`to_image`); the hot spot must be mapped the same way,
        // or a release at x.6 truncates one pixel *inside* the edge and leaves a sliver of arrow
        // in the image while the chip claims "Pointer in capture".
        for scale in [1.0, 1.25] {
            let desktop = desktop(800, 600, scale);
            for frac in [0.0, 0.25, 0.4, 0.5, 0.6, 0.75] {
                let start = (100.3, 80.7);
                let release = (399.0 + frac, 299.0 + frac);
                let mut selection = Selection::new(800, 600);
                selection.begin((image_px(start.0, scale), image_px(start.1, scale)));
                selection.drag_to(
                    (image_px(release.0, scale), image_px(release.1, scale)),
                    false,
                );
                let rect = selection.rect().expect("a selection");
                let pointer = Some(at(0, release.0, release.1));
                let what = format!("release at {release:?}, scale {scale}");

                assert_eq!(rect.pointer_inside(release, scale), None, "{what}");
                let preview = preview_for(true, pointer, 0, Some(rect), scale).expect("preview");
                assert!(!preview.inside && preview.on_edge, "{what}");
                let image =
                    super::super::compose_region(&desktop, 0, rect, true, pointer).expect("region");
                assert!(
                    image == desktop.full.crop(rect).expect("crop"),
                    "part of a pointer in the image: {what}"
                );
            }
        }
    }

    /// The overlay as `select_region` builds it, over one grey monitor, with a session of its
    /// own. Needs a display, the compiled resources and libadwaita; nothing here talks to a
    /// portal or to GSettings.
    fn live_overlay(include_pointer: bool) -> (Rc<Session>, Rc<MonitorOverlay>) {
        let resources = std::env::var_os("ANNOTA_GRESOURCE")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("_build/data/resources.gresource")
            });
        gio::resources_register(
            &gio::Resource::load(&resources).expect("build _build/data/resources.gresource first"),
        );
        crate::ui::test_support::init();
        let session = Rc::new(session());
        let desktop = desktop(800, 600, 1.0);
        let overlay = build_overlay(0, &desktop.monitors[0], &session, include_pointer)
            .expect("the overlay loads");
        session.overlays.borrow_mut().push(overlay.clone());
        overlay.w.window.present();
        crate::ui::test_support::pump(100);
        (session, overlay)
    }

    /// What the overlay's widgets say about the pointer right now.
    #[derive(Debug, PartialEq)]
    struct Shown {
        chip: Option<(String, bool)>,
        hint: Option<String>,
    }

    fn shown(overlay: &MonitorOverlay) -> Shown {
        let w = &overlay.w;
        let hint = w
            .pointer_hints
            .iter()
            .find(|h| h.box_.is_visible() && h.box_.parent().is_some_and(|p| p.is_visible()))
            .map(|h| h.label.text().to_string());
        Shown {
            chip: w.pointer_chip.is_visible().then(|| {
                (
                    w.pointer_chip_label.text().to_string(),
                    w.pointer_chip_suffix.is_visible(),
                )
            }),
            hint,
        }
    }

    // spec: capture / Pointer preview in the region overlay / Pointer will be included
    // spec: capture / Pointer preview in the region overlay / Pointer will not be included
    // spec: capture / Pointer preview in the region overlay / Indication follows the selection
    // spec: capture / Pointer preview in the region overlay / No indication when excluded
    #[test]
    #[ignore = "opens a window; needs xvfb-run and an isolated bus, see src/ui/mod.rs test_support"]
    fn the_overlay_shows_what_will_be_captured() {
        // The widgets themselves, driven through the handlers GTK calls: the decision behind
        // them is unit-tested, this proves the plumbing acts on it (the earlier review found
        // `pointer_left` leaving a stale "Pointer in capture" on screen).
        let (_session, overlay) = live_overlay(true);
        {
            let mut state = overlay.state.borrow_mut();
            state.selection.set_rect(IRect::new(100, 100, 400, 300));
            state.phase = Phase::Adjusting;
        }
        let in_capture = || Some((gettext("Pointer in capture"), false));
        let teach = || Some(gettext("Move the pointer into the selection to include it"));

        // Inside: the arrow will be in the image.
        overlay.motion(495.0, 250.0);
        assert_eq!(
            shown(&overlay),
            Shown {
                chip: in_capture(),
                hint: Some(gettext("Pointer in capture"))
            },
            "pointer inside"
        );

        // Shift+Left moves the selection 10 px out from under the pointer, no confirmation.
        assert!(overlay.nudge(Direction::Left, true));
        assert_eq!(
            shown(&overlay),
            Shown {
                chip: Some((gettext("Pointer not in capture"), false)),
                hint: teach()
            },
            "selection moved away"
        );
        assert!(overlay.nudge(Direction::Right, true));
        assert_eq!(shown(&overlay).chip, in_capture(), "and back");

        // Released on the lower-right corner, at a fractional position: the trap is named.
        overlay.motion(499.6, 399.6);
        assert_eq!(
            shown(&overlay),
            Shown {
                chip: Some((gettext("Pointer not in capture"), true)),
                hint: teach()
            },
            "on the edge"
        );

        // Onto the toolbar: no position, no chip, but the hint keeps teaching.
        overlay.motion(300.0, 250.0);
        overlay.pointer_left();
        assert_eq!(
            shown(&overlay),
            Shown {
                chip: None,
                hint: teach()
            },
            "left the canvas"
        );

        // Include Pointer off, through the toggle's own signal: nothing about the pointer.
        overlay.motion(300.0, 250.0);
        overlay.w.pointer_button.set_active(false);
        assert_eq!(
            shown(&overlay),
            Shown {
                chip: None,
                hint: None
            },
            "toggle off"
        );
        overlay.w.pointer_button.set_active(true);
        assert_eq!(shown(&overlay).chip, in_capture(), "toggle on again");
        overlay.w.window.destroy();
    }
}
