//! Delayed capture with a visible countdown (spec: Delayed capture; mockup 07).
//!
//! The OSD sits in a transparent, maximised window whose input region is empty, so the user
//! can keep working (open a menu, hover a tooltip) during the countdown. The window is
//! destroyed and the compositor given time to repaint before the screen is grabbed, so the
//! countdown is never part of the capture.
//!
//! A recording's start uses the same OSD ([`Purpose::Recording`], add-screen-recording D5): red
//! ring, its own label and hints, and the region's dashed outline.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use futures_channel::oneshot;
use gettextrs::gettext;
use gtk::{cairo, gdk, glib, prelude::*};

use super::geometry::IRect;
use crate::cli::CaptureMode;

const UI_COUNTDOWN: &str = "/io/github/trosinde/Annota/ui/countdown.ui";
/// Refresh rate of the ring.
const TICK: Duration = Duration::from_millis(50);
/// Time for the compositor to remove the OSD from the screen before the grab.
pub const SETTLE: Duration = Duration::from_millis(200);
/// Ring geometry (redline 07): Ø 148, 8 px track.
const RING_WIDTH: f64 = 8.0;
const CORAL: (f64, f64, f64) = (1.0, 0x5a as f64 / 255.0, 0x4e as f64 / 255.0);
/// `--destructive-bg` (#e01b24): the ring and outline of a recording countdown (Q6).
const RED: (f64, f64, f64) = (
    0xe0 as f64 / 255.0,
    0x1b as f64 / 255.0,
    0x24 as f64 / 255.0,
);
/// The region outline of a recording countdown (redline 04): 2 px, dashed, 2 px outside.
const OUTLINE_WIDTH: f64 = 2.0;
const OUTLINE_OFFSET: f64 = 2.0;
const OUTLINE_DASH: [f64; 2] = [6.0, 4.0];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// The delay ran out.
    Elapsed,
    /// Enter: capture right away.
    Now,
    /// Esc: no capture.
    Cancelled,
}

/// The number shown: whole seconds left, rounded up (5, 4, … 1).
pub fn seconds_left(remaining: Duration) -> u32 {
    let millis = remaining.as_millis();
    millis.div_ceil(1000) as u32
}

/// Fraction of the ring still drawn (1.0 at the start, 0.0 at the end).
pub fn ring_fraction(remaining: Duration, total: Duration) -> f64 {
    if total.is_zero() {
        0.0
    } else {
        (remaining.as_secs_f64() / total.as_secs_f64()).clamp(0.0, 1.0)
    }
}

/// "Capturing region in 3 s".
pub fn message(mode: CaptureMode, seconds: u32) -> String {
    let template = match mode {
        CaptureMode::Region => gettext("Capturing region in {seconds} s"),
        CaptureMode::Window => gettext("Capturing window in {seconds} s"),
        CaptureMode::Screen => gettext("Capturing screen in {seconds} s"),
    };
    template.replace("{seconds}", &seconds.to_string())
}

/// What a countdown is for: its label, ring colour, hints and outline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    /// A delayed capture (mockup 07 of add-screen-capture): Coral ring.
    Capture(CaptureMode),
    /// The start of a recording (add-screen-recording mockup 04, D5): red ring, "Recording
    /// starts in N s", hints Start now / Cancel. A region recording's selection (logical
    /// desktop px) gets a 2 px dashed red outline 2 px outside it, drawn by the same window.
    Recording { region: Option<IRect> },
}

impl Purpose {
    fn message(self, seconds: u32) -> String {
        match self {
            Self::Capture(mode) => message(mode, seconds),
            Self::Recording { .. } => recording_message(seconds),
        }
    }

    fn arc(self) -> (f64, f64, f64) {
        match self {
            Self::Capture(_) => CORAL,
            Self::Recording { .. } => RED,
        }
    }
}

/// "Recording starts in 3 s" (add-screen-recording mockup 04).
pub fn recording_message(seconds: u32) -> String {
    gettext("Recording starts in {seconds} s").replace("{seconds}", &seconds.to_string())
}

/// The outline around a region in its monitor's coordinates (`origin`: the monitor's logical
/// position): the stroke's centre line 3 px outside the selection, so a 2 px stroke leaves a
/// 2 px gap (redline 04).
pub fn outline_rect(region: IRect, origin: (i32, i32)) -> (f64, f64, f64, f64) {
    let gap = OUTLINE_OFFSET + OUTLINE_WIDTH / 2.0;
    (
        f64::from(region.x - origin.0) - gap,
        f64::from(region.y - origin.1) - gap,
        f64::from(region.w) + 2.0 * gap,
        f64::from(region.h) + 2.0 * gap,
    )
}

/// Shows the countdown for `seconds` and reports how it ended. Returns only after the OSD has
/// left the screen.
pub async fn run(app: &adw::Application, mode: CaptureMode, seconds: u32) -> End {
    run_for(app, Purpose::Capture(mode), seconds).await
}

/// The OSD window and its timer, taken away when dropped: also when the countdown's future is
/// dropped (a recording countdown cancelled by the Record shortcut or by the desktop).
struct Osd {
    window: gtk::Window,
    timer: Rc<RefCell<Option<glib::SourceId>>>,
}

impl Drop for Osd {
    fn drop(&mut self) {
        if let Some(timer) = self.timer.borrow_mut().take() {
            timer.remove();
        }
        self.window.set_visible(false);
        self.window.destroy();
    }
}

/// The monitor holding most of `region` (logical desktop px).
fn monitor_of(region: IRect) -> Option<gdk::Monitor> {
    let monitors = gdk::Display::default()?.monitors();
    (0..monitors.n_items())
        .filter_map(|i| monitors.item(i)?.downcast::<gdk::Monitor>().ok())
        .filter_map(|monitor| {
            let g = monitor.geometry();
            let area = IRect::new(g.x(), g.y(), g.width(), g.height()).intersect(region)?;
            Some((i64::from(area.w) * i64::from(area.h), monitor))
        })
        .max_by_key(|(area, _)| *area)
        .map(|(_, monitor)| monitor)
}

/// [`run`] for any [`Purpose`].
pub async fn run_for(app: &adw::Application, purpose: Purpose, seconds: u32) -> End {
    let total = Duration::from_secs(u64::from(seconds));
    let builder = gtk::Builder::from_resource(UI_COUNTDOWN);
    let (Some(window), Some(ring), Some(number), Some(label)) = (
        builder.object::<gtk::Window>("countdown_window"),
        builder.object::<gtk::DrawingArea>("ring"),
        builder.object::<gtk::Label>("number"),
        builder.object::<gtk::Label>("message"),
    ) else {
        eprintln!("annota: countdown.ui is incomplete; waiting without an OSD");
        glib::timeout_future(total).await;
        return End::Elapsed;
    };

    let (sender, receiver) = oneshot::channel::<End>();
    let sender = Rc::new(RefCell::new(Some(sender)));
    let finish = Rc::new(glib::clone!(
        #[strong]
        sender,
        move |end: End| {
            if let Some(sender) = sender.borrow_mut().take() {
                let _ = sender.send(end);
            }
        }
    ));

    let deadline = Instant::now() + total;
    let remaining = move || deadline.saturating_duration_since(Instant::now());

    let arc = purpose.arc();
    ring.set_draw_func(move |_, cr, width, height| {
        let _ = draw_ring(
            cr,
            f64::from(width),
            f64::from(height),
            ring_fraction(remaining(), total),
            arc,
        );
    });
    let update = glib::clone!(
        #[weak]
        ring,
        #[weak]
        number,
        #[weak]
        label,
        move || {
            let left = seconds_left(remaining());
            number.set_text(&left.to_string());
            label.set_text(&purpose.message(left));
            ring.queue_draw();
        }
    );
    update();

    // The recording variant: its title, hints and the region's outline (redline 04).
    let mut monitor = None;
    if let Purpose::Recording { region } = purpose {
        window.set_title(Some(&gettext("Recording Countdown")));
        if let (Some(capture), Some(recording)) = (
            builder.object::<gtk::Widget>("capture_hints"),
            builder.object::<gtk::Widget>("recording_hints"),
        ) {
            capture.set_visible(false);
            recording.set_visible(true);
        }
        if let (Some(region), Some(outline)) =
            (region, builder.object::<gtk::DrawingArea>("outline"))
        {
            monitor = monitor_of(region);
            let origin = monitor.as_ref().map_or((0, 0), |m| {
                let g = m.geometry();
                (g.x(), g.y())
            });
            let (x, y, w, h) = outline_rect(region, origin);
            outline.set_draw_func(move |_, cr, _, _| {
                cr.set_line_width(OUTLINE_WIDTH);
                cr.set_dash(&OUTLINE_DASH, 0.0);
                cr.set_source_rgb(RED.0, RED.1, RED.2);
                cr.rectangle(x, y, w, h);
                let _ = cr.stroke();
            });
        }
    }

    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(glib::clone!(
        #[strong]
        finish,
        move |_, key, _, _| match key {
            gdk::Key::Escape => {
                finish(End::Cancelled);
                glib::Propagation::Stop
            }
            gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter => {
                finish(End::Now);
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    ));
    window.add_controller(keys);
    window.connect_close_request(glib::clone!(
        #[strong]
        finish,
        move |_| {
            finish(End::Cancelled);
            glib::Propagation::Stop // destroyed below once `run` resumes
        }
    ));
    // Let every click through to the windows underneath.
    window.connect_map(|window| {
        if let Some(surface) = window.surface() {
            surface.set_input_region(&cairo::Region::create());
        }
    });

    let timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    let source = glib::timeout_add_local(
        TICK,
        glib::clone!(
            #[strong]
            finish,
            #[strong]
            timer,
            move || {
                update();
                if remaining().is_zero() {
                    finish(End::Elapsed);
                    // Ends by itself: nothing left to remove.
                    timer.borrow_mut().take();
                    glib::ControlFlow::Break
                } else {
                    glib::ControlFlow::Continue
                }
            }
        ),
    );
    timer.replace(Some(source));

    window.set_application(Some(app));
    // A region's outline needs the monitor's own coordinates: only a full-screen window has
    // a known origin on Wayland. Otherwise maximised, as for a capture.
    match &monitor {
        Some(monitor) => window.fullscreen_on_monitor(monitor),
        None => window.maximize(),
    }
    window.present();
    let osd = Osd { window, timer };

    let end = receiver.await.unwrap_or(End::Cancelled);
    drop(osd);
    glib::timeout_future(SETTLE).await;
    end
}

fn draw_ring(
    cr: &cairo::Context,
    width: f64,
    height: f64,
    fraction: f64,
    arc: (f64, f64, f64),
) -> Result<(), cairo::Error> {
    let (cx, cy) = (width / 2.0, height / 2.0);
    let radius = (width.min(height) - RING_WIDTH) / 2.0;
    cr.set_line_width(RING_WIDTH);
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.18);
    cr.arc(cx, cy, radius, 0.0, std::f64::consts::TAU);
    cr.stroke()?;
    if fraction > 0.0 {
        let start = -std::f64::consts::FRAC_PI_2;
        cr.set_line_cap(cairo::LineCap::Round);
        cr.set_source_rgb(arc.0, arc.1, arc.2);
        cr.arc(
            cx,
            cy,
            radius,
            start,
            start + std::f64::consts::TAU * fraction,
        );
        cr.stroke()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_down_whole_seconds() {
        // Scenario "Countdown": a 5 s delay shows 5, 4, 3, 2, 1.
        assert_eq!(seconds_left(Duration::from_millis(5000)), 5);
        assert_eq!(seconds_left(Duration::from_millis(4001)), 5);
        assert_eq!(seconds_left(Duration::from_millis(4000)), 4);
        assert_eq!(seconds_left(Duration::from_millis(1)), 1);
        assert_eq!(seconds_left(Duration::ZERO), 0);
    }

    #[test]
    fn ring_shrinks_to_zero() {
        let total = Duration::from_secs(5);
        assert_eq!(ring_fraction(total, total), 1.0);
        assert_eq!(ring_fraction(Duration::from_millis(2500), total), 0.5);
        assert_eq!(ring_fraction(Duration::ZERO, total), 0.0);
        assert_eq!(ring_fraction(Duration::ZERO, Duration::ZERO), 0.0);
    }

    #[test]
    fn messages_name_the_mode() {
        assert_eq!(message(CaptureMode::Region, 3), "Capturing region in 3 s");
        assert_eq!(message(CaptureMode::Screen, 1), "Capturing screen in 1 s");
    }

    #[test]
    fn osd_is_gone_before_the_grab() {
        // "The captured image does not contain the countdown": the grab waits at least a
        // few frames (60 Hz → 16.7 ms each) after the OSD window is destroyed.
        assert!(SETTLE >= Duration::from_millis(100));
    }
}
