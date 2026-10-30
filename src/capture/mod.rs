//! Capability `capture`: region, window and screen captures through xdg-desktop-portal,
//! delayed captures, global shortcuts and new captures from the clipboard or a file.

pub mod countdown;
pub mod delivery;
pub mod frame;
pub mod geometry;
pub mod import;
pub mod overlay;
pub mod permission;
pub mod portal;
pub mod shortcut_dialog;
pub mod shortcuts;
pub mod snap;
pub mod texture;

use std::cell::Cell;
use std::time::Duration;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};

use crate::application;
use crate::cli::CaptureMode;
use crate::config;
use delivery::Source;
use frame::Frame;
use overlay::{Desktop, Outcome, PointerAt};
use portal::{CaptureError, GrabOptions};

/// Time for the compositor to take Annota's own windows off the screen before a grab.
const HIDE_SETTLE: Duration = Duration::from_millis(250);

/// One capture request (launcher, `--capture`, global shortcut).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request {
    pub mode: CaptureMode,
    pub delay: u32,
    pub include_pointer: bool,
}

impl Request {
    /// A request using the saved defaults (delay, include pointer); `delay` overrides.
    pub fn with_defaults(mode: CaptureMode, delay: Option<u32>) -> Self {
        let settings = gio::Settings::new(config::APP_ID);
        Self {
            mode,
            delay: delay.unwrap_or_else(|| settings.int("capture-delay").clamp(0, 60) as u32),
            include_pointer: settings.boolean("include-pointer"),
        }
    }
}

thread_local! {
    static BUSY: Cell<bool> = const { Cell::new(false) };
}

/// Starts a capture. Annota's own windows are hidden first so they are not captured, and the
/// application is held while no window is open (`annota --capture` from a shortcut).
pub fn start(app: &adw::Application, request: Request) {
    if BUSY.with(|busy| busy.replace(true)) {
        return;
    }
    let app = app.clone();
    glib::spawn_future_local(async move {
        let hold = app.hold();
        let hidden: Vec<gtk::Window> = app
            .windows()
            .into_iter()
            .filter(|w| w.is_visible())
            .collect();
        for window in &hidden {
            window.set_visible(false);
        }
        if !hidden.is_empty() {
            glib::timeout_future(HIDE_SETTLE).await;
        }

        let result = run(&app, request).await;

        for window in &hidden {
            window.set_visible(true);
        }
        BUSY.with(|busy| busy.set(false));

        match result {
            Ok(Some((frame, source))) => {
                delivery::deliver(&app, frame, source, !hidden.is_empty());
            }
            Ok(None) | Err(CaptureError::Cancelled) => {}
            Err(CaptureError::PermissionDenied) => permission::present(&app),
            Err(err) => {
                eprintln!("annota: capture failed: {err}");
                let window = application::present_window(&app);
                let text = match err {
                    CaptureError::PortalUnavailable => {
                        gettext("Capturing needs xdg-desktop-portal, which is not running")
                    }
                    _ => gettext("The capture failed. Nothing was captured."),
                };
                application::show_toast(&window, &adw::Toast::new(&text));
            }
        }
        drop(hold);
    });
}

async fn run(
    app: &adw::Application,
    request: Request,
) -> Result<Option<(Frame, Source)>, CaptureError> {
    let mut mode = request.mode;
    let mut delay = request.delay;
    loop {
        if delay > 0 && countdown::run(app, mode, delay).await == countdown::End::Cancelled {
            return Ok(None);
        }
        match mode {
            CaptureMode::Window => {
                // Wayland: the desktop's own picker (mockup 05); Annota draws nothing.
                let frame = portal::grab(GrabOptions::for_mode(CaptureMode::Window)).await?;
                return Ok(Some((frame, Source::Window)));
            }
            CaptureMode::Screen => {
                let frame = portal::grab(GrabOptions::for_mode(CaptureMode::Screen)).await?;
                let frame = screen(app, frame, request.include_pointer).await?;
                return Ok(Some((frame, Source::Screen)));
            }
            CaptureMode::Region => {
                let desktop = Desktop::new(portal::grab(GrabOptions::FROZEN).await?);
                match overlay::select_region(app, &desktop, request.include_pointer).await {
                    Outcome::Cancelled => return Ok(None),
                    Outcome::Switch(next) => {
                        // The overlay must be off the screen before the next grab.
                        glib::timeout_future(HIDE_SETTLE).await;
                        mode = next;
                        delay = 0;
                    }
                    Outcome::Region {
                        monitor,
                        rect,
                        include_pointer,
                        delay,
                        pointer,
                    } => {
                        let frame =
                            region(app, desktop, monitor, rect, include_pointer, delay, pointer)
                                .await?;
                        return Ok(Some((frame, Source::Region)));
                    }
                }
            }
        }
    }
}

/// Crops the confirmed region; with a toolbar delay, waits and grabs the screen again first.
async fn region(
    app: &adw::Application,
    mut desktop: Desktop,
    monitor: usize,
    rect: geometry::IRect,
    include_pointer: bool,
    delay: u32,
    mut pointer: Option<PointerAt>,
) -> Result<Frame, CaptureError> {
    if delay > 0 {
        if countdown::run(app, CaptureMode::Region, delay).await == countdown::End::Cancelled {
            return Err(CaptureError::Cancelled);
        }
        desktop = Desktop::new(portal::grab(GrabOptions::FROZEN).await?);
        if include_pointer {
            pointer = overlay::probe_pointer(app, &desktop).await;
        }
    }
    let shot = desktop
        .monitors
        .get(monitor)
        .ok_or_else(|| CaptureError::Failed(String::from("monitor disappeared")))?;
    let mut frame = shot
        .frame
        .crop(rect)
        .ok_or_else(|| CaptureError::Failed(String::from("empty selection")))?;
    if include_pointer {
        if let Some(at) = pointer.filter(|p| p.monitor == monitor) {
            draw_pointer(
                &mut frame,
                at.x * shot.scale - f64::from(rect.x),
                at.y * shot.scale - f64::from(rect.y),
                shot.scale,
            );
        }
    }
    Ok(frame)
}

/// Screen mode: the monitor under the pointer, or every monitor with `capture-all-monitors`.
async fn screen(
    app: &adw::Application,
    full: Frame,
    include_pointer: bool,
) -> Result<Frame, CaptureError> {
    let all_monitors = gio::Settings::new(config::APP_ID).boolean("capture-all-monitors");
    let desktop = Desktop::new(full);
    let needs_probe = include_pointer || (!all_monitors && desktop.monitors.len() > 1);
    let pointer = if needs_probe {
        overlay::probe_pointer(app, &desktop).await
    } else {
        None
    };
    let crops: Vec<geometry::IRect> = desktop.monitors.iter().map(|m| m.crop).collect();
    let rect = geometry::screen_capture_rect(
        &crops,
        pointer.map(|p| p.monitor),
        all_monitors,
        desktop.full.bounds(),
    );
    let mut frame = desktop
        .full
        .crop(rect)
        .ok_or_else(|| CaptureError::Failed(String::from("no monitor")))?;
    if let Some(at) = pointer.filter(|_| include_pointer) {
        if let Some(shot) = desktop.monitors.get(at.monitor) {
            // Pointer position in full-desktop pixels, relative to the kept part.
            let x = f64::from(shot.crop.x - rect.x) + at.x * shot.scale;
            let y = f64::from(shot.crop.y - rect.y) + at.y * shot.scale;
            if rect.contains(x as i32 + rect.x, y as i32 + rect.y) {
                draw_pointer(&mut frame, x, y, shot.scale);
            }
        }
    }
    Ok(frame)
}

fn draw_pointer(frame: &mut Frame, x: f64, y: f64, scale: f64) {
    if let Err(err) = frame.draw_pointer(x, y, scale) {
        eprintln!("annota: could not draw the pointer: {err}");
    }
}
