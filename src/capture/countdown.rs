//! Delayed capture with a visible countdown (spec: Delayed capture; mockup 07).
//!
//! The OSD sits in a transparent, maximised window whose input region is empty, so the user
//! can keep working (open a menu, hover a tooltip) during the countdown. The window is
//! destroyed and the compositor given time to repaint before the screen is grabbed, so the
//! countdown is never part of the capture.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use futures_channel::oneshot;
use gettextrs::gettext;
use gtk::{cairo, gdk, glib, prelude::*};

use crate::cli::CaptureMode;

const UI_COUNTDOWN: &str = "/io/github/trosinde/Annota/ui/countdown.ui";
/// Refresh rate of the ring.
const TICK: Duration = Duration::from_millis(50);
/// Time for the compositor to remove the OSD from the screen before the grab.
pub const SETTLE: Duration = Duration::from_millis(200);
/// Ring geometry (redline 07): Ø 148, 8 px track.
const RING_WIDTH: f64 = 8.0;
const CORAL: (f64, f64, f64) = (1.0, 0x5a as f64 / 255.0, 0x4e as f64 / 255.0);

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

/// Shows the countdown for `seconds` and reports how it ended. Returns only after the OSD has
/// left the screen.
pub async fn run(app: &adw::Application, mode: CaptureMode, seconds: u32) -> End {
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

    ring.set_draw_func(move |_, cr, width, height| {
        let _ = draw_ring(
            cr,
            f64::from(width),
            f64::from(height),
            ring_fraction(remaining(), total),
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
            label.set_text(&message(mode, left));
            ring.queue_draw();
        }
    );
    update();

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

    let timer = glib::timeout_add_local(
        TICK,
        glib::clone!(
            #[strong]
            finish,
            move || {
                update();
                if remaining().is_zero() {
                    finish(End::Elapsed);
                    glib::ControlFlow::Break
                } else {
                    glib::ControlFlow::Continue
                }
            }
        ),
    );
    let timer = Rc::new(RefCell::new(Some(timer)));

    window.set_application(Some(app));
    window.maximize();
    window.present();

    let end = receiver.await.unwrap_or(End::Cancelled);
    if end != End::Elapsed {
        if let Some(source) = timer.borrow_mut().take() {
            source.remove();
        }
    }
    window.destroy();
    glib::timeout_future(SETTLE).await;
    end
}

fn draw_ring(
    cr: &cairo::Context,
    width: f64,
    height: f64,
    fraction: f64,
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
        cr.set_source_rgb(CORAL.0, CORAL.1, CORAL.2);
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
