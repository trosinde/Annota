//! The waiting OSD of a window capture (fix-window-capture-screencast, design.md D8, sheet 02).
//!
//! "Waiting for a window…" with Cancel is on the screen from the ScreenCast portal's `Start` until
//! the desktop's window chooser answers. It is the chooser's parent window: a request without a
//! visible parent was measured to leave GNOME's chooser invisible and unanswered. Cancel or `Esc`
//! closes the session, which also ends GNOME's pending request, so Annota never depends on GNOME
//! reporting a closed chooser.
//!
//! Like the countdown OSD it sits centred in a transparent, maximised window. Only the OSD itself
//! takes pointer input, so nothing underneath is blocked and nothing is dimmed.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use futures_channel::oneshot;
use gtk::{cairo, gdk, glib, graphene, prelude::*};

use super::portal::{CaptureError, ParentWindow};
use super::screencast::{self, Chosen, WindowSource};

const UI_WINDOW_WAIT: &str = "/io/github/trosinde/Annota/ui/window-wait.ui";
/// One turn of the activity ring (redline: 1.2 s).
const TURN: Duration = Duration::from_millis(1200);
/// Ring geometry (redline: 72 × 72, stroke 6, a quarter turn of arc, round caps).
pub(super) const RING_STROKE: f64 = 6.0;
pub(super) const RING_RADIUS: f64 = 32.0;
const ARC: f64 = std::f64::consts::FRAC_PI_2;
const CORAL: (f64, f64, f64) = (1.0, 0x5a as f64 / 255.0, 0x4e as f64 / 255.0);

thread_local! {
    /// The OSD while it is shown, for [`raise`].
    static SHOWN: RefCell<Option<gtk::Window>> = const { RefCell::new(None) };
}

/// Brings the OSD to the front if a window capture is waiting for its chooser. A second
/// window-capture request does this and starts nothing new (spec "Window capture").
pub fn raise() -> bool {
    SHOWN.with(|shown| match shown.borrow().as_ref() {
        Some(window) => {
            window.present();
            true
        }
        None => false,
    })
}

/// Whether the OSD is on the screen, for tests.
#[cfg(test)]
pub(super) fn shown() -> Option<gtk::Window> {
    SHOWN.with(|shown| shown.borrow().clone())
}

/// Shows the OSD, opens the desktop's window chooser with it as parent, and takes the OSD away
/// again once the chooser has answered or the user cancelled.
pub async fn choose(
    app: &adw::Application,
    source: &WindowSource,
    include_pointer: bool,
) -> Result<Chosen, CaptureError> {
    let cursor = screencast::cursor_mode(include_pointer, source.offers_embedded_pointer());
    let Some((window, cancelled)) = show(app) else {
        // A broken resource bundle: without a parent GNOME's chooser was measured not to show.
        return Err(CaptureError::Failed(String::from(
            "window-wait.ui is incomplete",
        )));
    };
    let parent = ParentWindow::new(&window).await;
    let identifier = parent.for_request();
    if identifier.is_none() {
        eprintln!("annota: no parent window handle for the window chooser");
    }
    let cancel = async move {
        // A dropped sender means the OSD is gone, which ends the wait just the same.
        let _ = cancelled.await;
    };
    let result = screencast::choose_window(source, cursor, identifier, cancel).await;
    drop(parent);
    SHOWN.with(|shown| shown.borrow_mut().take());
    window.destroy();
    result
}

/// Builds and presents the OSD. The receiver resolves on Cancel, `Esc` or a close request.
fn show(app: &adw::Application) -> Option<(gtk::Window, oneshot::Receiver<()>)> {
    let builder = gtk::Builder::from_resource(UI_WINDOW_WAIT);
    let window = builder.object::<gtk::Window>("wait_window")?;
    let osd = builder.object::<gtk::Box>("wait_box")?;
    let ring = builder.object::<gtk::DrawingArea>("ring")?;
    let cancel = builder.object::<gtk::Button>("cancel_button")?;

    let (sender, receiver) = oneshot::channel::<()>();
    let sender = Rc::new(RefCell::new(Some(sender)));
    let finish = move || {
        if let Some(sender) = sender.borrow_mut().take() {
            let _ = sender.send(());
        }
    };

    cancel.connect_clicked({
        let finish = finish.clone();
        move |_| finish()
    });
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed({
        let finish = finish.clone();
        move |_, key, _, _| {
            if key == gdk::Key::Escape {
                finish();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        }
    });
    window.add_controller(keys);
    window.connect_close_request(move |_| {
        finish();
        glib::Propagation::Stop // destroyed by `choose` once the session is closed
    });

    animate(&ring);
    // Clicks outside the OSD go to the windows underneath (redline: non-modal).
    // Set after each paint, once the OSD has its allocation, and only when it moved.
    window.connect_map(move |window| {
        let (Some(surface), Some(clock)) = (window.surface(), window.frame_clock()) else {
            return;
        };
        let (window, osd) = (window.clone(), osd.clone());
        let last = Cell::new(None);
        clock.connect_after_paint(move |_| {
            let rect = input_rect(&window, &osd);
            if rect.is_some() && rect != last.get() {
                last.set(rect);
                if let Some(rect) = rect {
                    surface.set_input_region(&cairo::Region::create_rectangle(&rect));
                }
            }
        });
    });

    window.set_application(Some(app));
    window.maximize();
    window.present();
    // Redline: Cancel has the default focus; Enter and Space activate it.
    cancel.grab_focus();
    SHOWN.with(|shown| shown.replace(Some(window.clone())));
    Some((window, receiver))
}

/// The OSD's rectangle in surface coordinates.
fn input_rect(window: &gtk::Window, osd: &gtk::Box) -> Option<cairo::RectangleInt> {
    let bounds = osd.compute_bounds(window)?;
    let (dx, dy) = window.surface_transform();
    let rect = graphene::Rect::new(
        bounds.x() + dx as f32,
        bounds.y() + dy as f32,
        bounds.width(),
        bounds.height(),
    );
    Some(cairo::RectangleInt::new(
        rect.x().floor() as i32,
        rect.y().floor() as i32,
        rect.width().ceil() as i32,
        rect.height().ceil() as i32,
    ))
}

/// Turns the ring once per [`TURN`]; with animations off (GNOME's Reduce Animation) it stands
/// still and the label carries the meaning.
fn animate(ring: &gtk::DrawingArea) {
    let angle = Rc::new(Cell::new(0.0));
    ring.set_draw_func({
        let angle = angle.clone();
        move |_, cr, width, height| {
            let _ = draw_ring(cr, f64::from(width), f64::from(height), angle.get());
        }
    });
    let animations = gtk::Settings::default().is_some_and(|s| s.is_gtk_enable_animations());
    if animations {
        ring.add_tick_callback(move |ring, clock| {
            angle.set(ring_angle(clock.frame_time()));
            ring.queue_draw();
            glib::ControlFlow::Continue
        });
    }
}

/// Where the arc starts at `micros` of frame-clock time, in radians from twelve o'clock.
pub fn ring_angle(micros: i64) -> f64 {
    let turn = TURN.as_micros() as i64;
    let phase = micros.rem_euclid(turn) as f64 / turn as f64;
    phase * std::f64::consts::TAU
}

fn draw_ring(cr: &cairo::Context, width: f64, height: f64, angle: f64) -> Result<(), cairo::Error> {
    let (cx, cy) = (width / 2.0, height / 2.0);
    cr.set_line_width(RING_STROKE);
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.18);
    cr.arc(cx, cy, RING_RADIUS, 0.0, std::f64::consts::TAU);
    cr.stroke()?;
    let start = -std::f64::consts::FRAC_PI_2 + angle;
    cr.set_line_cap(cairo::LineCap::Round);
    cr.set_source_rgb(CORAL.0, CORAL.1, CORAL.2);
    cr.arc(cx, cy, RING_RADIUS, start, start + ARC);
    cr.stroke()
}
