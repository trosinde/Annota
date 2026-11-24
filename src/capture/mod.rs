//! Capability `capture`: region, window and screen captures through xdg-desktop-portal,
//! delayed captures, global shortcuts and new captures from the clipboard or a file.

pub mod countdown;
pub mod delivery;
pub mod frame;
pub mod geometry;
pub mod import;
#[cfg(test)]
pub(crate) mod live_tests;
pub mod overlay;
pub mod permission;
pub mod portal;
pub mod screencast;
pub mod shortcut_dialog;
pub mod shortcuts;
pub mod snap;
pub mod texture;
pub mod window_wait;

use std::cell::{Cell, RefCell};
use std::time::Duration;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gio, glib};

use crate::application;
use crate::cli::CaptureMode;
use crate::config;
use delivery::Source;
use frame::Frame;
use overlay::{Desktop, Mode, Outcome, PointerAt};
use permission::{FirstStep, Refusal};
use portal::{CaptureError, GrabOptions};

/// Time for the compositor to take Annota's own windows off the screen before a grab.
const HIDE_SETTLE: Duration = Duration::from_millis(250);
/// How long a window on screen for the permission question may take to get the focus. The focus
/// only matters if the window has no handle to pass to the portal; then GNOME Shell asks instead.
const FOCUS_WAIT: Duration = Duration::from_secs(2);
const FOCUS_POLL: Duration = Duration::from_millis(50);

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
    /// The mode the region overlay of the running request opens in.
    static OVERLAY_MODE: Cell<Mode> = const { Cell::new(Mode::Screenshot) };
    /// Start Recording in the overlay's Video mode: what to record, handed to the recording
    /// flow once the capture flow has ended (add-screen-recording).
    static RECORD: Cell<Option<(CaptureMode, Option<geometry::IRect>)>> = const { Cell::new(None) };
    /// The windows `run_hidden` hid, when Start Recording ended the flow: they stay hidden and
    /// go to the recording flow, which shows them again once it ends (design.md, "Decided
    /// during apply", Phase C1 review).
    static HIDDEN_FOR_RECORDING: RefCell<Vec<gtk::Window>> = const { RefCell::new(Vec::new()) };
}

/// Starts a capture. Annota's own windows are hidden first so they are not captured, except for
/// the permission question (see [`with_permission`]), and the application is held while no window
/// is open (`annota --capture` from a shortcut).
pub fn start(app: &adw::Application, request: Request) {
    start_in(app, request, Mode::Screenshot);
}

/// Whether a capture flow runs (live tests of other modules).
#[cfg(test)]
pub(crate) fn busy() -> bool {
    BUSY.with(Cell::get)
}

/// Opens the region overlay in Video mode with the recording setup (add-screen-recording
/// "Recording setup": `--record region`, `--record toggle`, the Record shortcut and card). Its
/// Start Recording continues in `recording::start_from_setup`; `Esc` ends it with nothing
/// requested and nothing written.
pub fn start_recording_setup(app: &adw::Application) {
    start_in(
        app,
        Request::with_defaults(CaptureMode::Region, Some(0)),
        Mode::Video,
    );
}

fn start_in(app: &adw::Application, request: Request, overlay_mode: Mode) {
    if BUSY.with(|busy| busy.replace(true)) {
        // A window capture waiting for its chooser: raise its OSD, start nothing new.
        if request.mode == CaptureMode::Window {
            window_wait::raise();
        }
        return;
    }
    let app = app.clone();
    // Must be taken synchronously, before the first poll of the future below: a cold
    // `annota --capture <mode>` has no window and nothing else holding the application, so
    // glib can decide to quit before an async-block `app.hold()` ever gets a chance to run.
    let hold = app.hold();
    glib::spawn_future_local(async move {
        OVERLAY_MODE.with(|mode| mode.set(overlay_mode));
        RECORD.with(|record| record.set(None));
        let result = with_permission(&app, request).await;
        BUSY.with(|busy| busy.set(false));
        if let Some((source, selection)) = RECORD.with(Cell::take) {
            let hidden = HIDDEN_FOR_RECORDING.with(RefCell::take);
            crate::recording::start_from_setup(&app, source, selection, hidden);
        }

        match result {
            Ok(Some((frame, source))) => {
                // What is on the screen now is what the user had open: `run_hidden` brought those
                // windows back, and a window opened only for the question is gone.
                delivery::deliver(&app, frame, source, !visible_windows(&app).is_empty());
            }
            Ok(None) | Err(CaptureError::Cancelled) => {}
            Err(CaptureError::PermissionDenied) => permission::present(&app),
            Err(err) => {
                eprintln!("annota: capture failed: {err}");
                let window = application::present_for_message(&app);
                let text = match err {
                    CaptureError::PortalUnavailable => {
                        gettext("Capturing needs xdg-desktop-portal, which is not running")
                    }
                    CaptureError::CouldNotAsk => gettext(
                        "GNOME could not ask for screen access. Click this window and try again.",
                    ),
                    CaptureError::NothingCaptured => gettext("Nothing was captured"),
                    _ => gettext("The capture failed. Nothing was captured."),
                };
                application::show_toast(&window, &adw::Toast::new(&text));
            }
        }
        drop(hold);
    });
}

fn visible_windows(app: &adw::Application) -> Vec<gtk::Window> {
    app.windows()
        .into_iter()
        .filter(|w| w.is_visible())
        .collect()
}

/// Runs the capture and sorts out the screenshot permission around it.
///
/// With no stored answer the portal asks on the first frozen grab, so a window stays on the screen
/// for that request ([`window_for_question`]). "Allow" stores the answer and the capture itself
/// then starts as always, with the windows hidden, so no capture ever shows Annota's own window.
/// A stored "no" ends here: the portal would refuse, so no window hides and no countdown runs.
async fn with_permission(
    app: &adw::Application,
    request: Request,
) -> Result<Option<(Frame, Source)>, CaptureError> {
    let store = permission::read_store().await;
    let mut ask = match permission::first_step(request.mode, store) {
        FirstStep::Denied => return Err(CaptureError::PermissionDenied),
        FirstStep::Ask => true,
        FirstStep::Grab => false,
    };
    let mut asked = false;
    // The window opened only for the question, if one was.
    let mut question_window = None;
    loop {
        let answer = if ask {
            asked = true;
            let (window, created) = window_for_question(app).await;
            if created {
                question_window = Some(window.clone());
            }
            portal::ask_permission(&window).await
        } else {
            Ok(())
        };
        let result = match answer {
            Ok(()) => run_hidden(app, request, question_window.take()).await,
            Err(CaptureError::Cancelled) => {
                // Nothing will use it: no capture follows and nothing is said.
                if let Some(window) = question_window.take() {
                    window.destroy();
                }
                Err(CaptureError::Cancelled)
            }
            Err(err) => Err(err),
        };
        if !matches!(result, Err(CaptureError::Refused)) {
            return result;
        }
        let store = permission::read_store().await;
        match permission::after_refusal(request.mode, store, asked) {
            Refusal::Denied => return Err(CaptureError::PermissionDenied),
            Refusal::CouldNotAsk => return Err(CaptureError::CouldNotAsk),
            // GNOME's screenshot UI ended without an image. Measured against the live portal
            // on 2026-11-14: a UI the user closes answers after seconds, a UI that never opens
            // (its modal grab failed) answers in milliseconds — and both carry response code 2
            // with nothing to tell them apart. So Annota says the one thing that is true either
            // way, and never calls it a failure or passes it off as a cancel.
            Refusal::PickerClosed => {
                eprintln!("annota: the window picker ended without an image");
                return Err(CaptureError::NothingCaptured);
            }
            Refusal::Failed => {
                return Err(CaptureError::Failed(String::from(
                    "the portal refused the request",
                )))
            }
            Refusal::Ask => ask = true,
        }
    }
}

/// The window the permission question is asked over: the one in use, or a new one if a shortcut
/// or `annota --capture` started the capture with none open. `true`: it was opened for this.
async fn window_for_question(app: &adw::Application) -> (gtk::Window, bool) {
    let existing = app.windows();
    let window = app
        .active_window()
        .filter(|w| w.is_visible())
        .or_else(|| visible_windows(app).into_iter().next())
        .unwrap_or_else(|| application::present_window(app).upcast());
    let created = !existing.contains(&window);
    window.present();
    let start = std::time::Instant::now();
    while !window.is_active() && start.elapsed() < FOCUS_WAIT {
        glib::timeout_future(FOCUS_POLL).await;
    }
    (window, created)
}

/// One attempt at the capture with Annota's windows off the screen. Afterwards the windows that
/// were hidden are back, except `question_window`, which is gone, as a capture from a shortcut
/// opens no window of its own. The question can take long, so a window opened meanwhile is kept
/// and a window closed meanwhile stays closed.
async fn run_hidden(
    app: &adw::Application,
    request: Request,
    question_window: Option<gtk::Window>,
) -> Result<Option<(Frame, Source)>, CaptureError> {
    let visible = visible_windows(app);
    for window in &visible {
        window.set_visible(false);
    }
    if !visible.is_empty() {
        glib::timeout_future(HIDE_SETTLE).await;
    }
    if let Some(window) = &question_window {
        window.destroy();
    }
    let result = run(app, request).await;
    // Anything can have closed one of them by now, and showing a destroyed window is not safe.
    let open = app.windows();
    let hidden = visible.into_iter().filter(|w| open.contains(w));
    if RECORD.with(Cell::get).is_some() {
        // Start Recording: the area was chosen on a frozen desktop without Annota, so Annota
        // stays off the screen until the recording flow ends.
        HIDDEN_FOR_RECORDING.with(|keep| keep.replace(hidden.collect()));
    } else {
        for window in hidden {
            window.set_visible(true);
        }
    }
    result
}

async fn run(
    app: &adw::Application,
    request: Request,
) -> Result<Option<(Frame, Source)>, CaptureError> {
    let mut mode = request.mode;
    let mut delay = request.delay;
    loop {
        // Window mode orders its own countdown (`window_steps`).
        if mode != CaptureMode::Window
            && delay > 0
            && countdown::run(app, mode, delay).await == countdown::End::Cancelled
        {
            return Ok(None);
        }
        match mode {
            CaptureMode::Window => {
                let frame = window(app, delay, request.include_pointer).await?;
                return Ok(frame.map(|frame| (frame, Source::Window)));
            }
            CaptureMode::Screen => {
                let frame = portal::grab(GrabOptions::for_mode(CaptureMode::Screen)).await?;
                let frame = screen(app, frame, request.include_pointer).await?;
                return Ok(Some((frame, Source::Screen)));
            }
            CaptureMode::Region => {
                let desktop = Desktop::new(portal::grab(GrabOptions::FROZEN).await?);
                let overlay_mode = OVERLAY_MODE.with(Cell::get);
                match overlay::select_region(app, &desktop, request.include_pointer, overlay_mode)
                    .await
                {
                    Outcome::Cancelled => return Ok(None),
                    Outcome::Record { source, selection } => {
                        RECORD.with(|record| record.set(Some((source, selection))));
                        return Ok(None);
                    }
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

/// One step of a window capture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WindowStep {
    /// The waiting OSD and the desktop's window chooser (ScreenCast portal).
    Chooser,
    /// The capture delay's countdown.
    Countdown,
    /// One frame of the chosen window, then the session closes.
    Frame,
    /// The Screenshot portal's interactive UI, where ScreenCast offers no window source.
    ScreenshotUi,
}

/// The order of a window capture (design.md D4, D6). Through ScreenCast the chooser comes first
/// and the countdown after it, so the user picks the window and then has the delay to bring it
/// into shape. The Screenshot-portal fallback keeps today's order: countdown, then its UI.
fn window_steps(delay: u32, screencast: bool) -> &'static [WindowStep] {
    use WindowStep::*;
    match (screencast, delay > 0) {
        (true, false) => &[Chooser, Frame],
        (true, true) => &[Chooser, Countdown, Frame],
        (false, false) => &[ScreenshotUi],
        (false, true) => &[Countdown, ScreenshotUi],
    }
}

/// Window mode. `None`: the countdown was cancelled.
async fn window(
    app: &adw::Application,
    delay: u32,
    include_pointer: bool,
) -> Result<Option<Frame>, CaptureError> {
    let source = screencast::window_source_available().await;
    // Dropping a chosen window closes its session, on every way out of this function.
    let mut chosen = None;
    for step in window_steps(delay, source.is_some()) {
        match step {
            WindowStep::Chooser => {
                let source = source
                    .as_ref()
                    .ok_or_else(|| CaptureError::Failed(String::from("no window source")))?;
                chosen = Some(window_wait::choose(app, source, include_pointer).await?);
            }
            WindowStep::Countdown => {
                if countdown::run(app, CaptureMode::Window, delay).await
                    == countdown::End::Cancelled
                {
                    return Ok(None);
                }
            }
            WindowStep::Frame => {
                let chosen = chosen
                    .take()
                    .ok_or_else(|| CaptureError::Failed(String::from("no window chosen")))?;
                return chosen.grab_frame().await.map(Some);
            }
            WindowStep::ScreenshotUi => {
                // The desktop's screenshot UI (add-screen-capture mockup 05); Annota draws
                // nothing, and `with_permission` sorts an ending without an image.
                return portal::grab(GrabOptions::for_mode(CaptureMode::Window))
                    .await
                    .map(Some);
            }
        }
    }
    Err(CaptureError::Failed(String::from("no window step")))
}

/// Crops the confirmed region; with a toolbar delay, waits and grabs the screen again first.
async fn region(
    app: &adw::Application,
    mut desktop: Desktop,
    monitor: usize,
    rect: geometry::IRect,
    include_pointer: bool,
    delay: u32,
    pointer: Option<PointerAt>,
) -> Result<Frame, CaptureError> {
    let mut after_countdown = None;
    if delay > 0 {
        if countdown::run(app, CaptureMode::Region, delay).await == countdown::End::Cancelled {
            return Err(CaptureError::Cancelled);
        }
        desktop = Desktop::new(portal::grab(GrabOptions::FROZEN).await?);
        if include_pointer {
            // Only an exact position may be drawn.
            after_countdown = overlay::probe_pointer(app, &desktop, true)
                .await
                .filter(|probed| probed.exact)
                .map(|probed| probed.at);
        }
    }
    let pointer = capture_pointer(delay, pointer, after_countdown);
    compose_region(&desktop, monitor, rect, include_pointer, pointer)
}

/// The pointer position a region capture uses. Without a delay it is the one at confirmation.
/// With a delay it is the one probed when the countdown ended, because the user may have moved
/// the pointer into the region during the countdown (spec: Delayed region capture places the
/// pointer); the position at confirmation is stale by then and never falls back in.
fn capture_pointer(
    delay: u32,
    at_confirmation: Option<PointerAt>,
    after_countdown: Option<PointerAt>,
) -> Option<PointerAt> {
    if delay > 0 {
        after_countdown
    } else {
        at_confirmation
    }
}

/// The region image: the selection cut out of `monitor`'s frozen frame, with the pointer drawn
/// in only when it is included and its hot spot lies inside the selection.
fn compose_region(
    desktop: &Desktop,
    monitor: usize,
    rect: geometry::IRect,
    include_pointer: bool,
    pointer: Option<PointerAt>,
) -> Result<Frame, CaptureError> {
    let shot = desktop
        .monitors
        .get(monitor)
        .ok_or_else(|| CaptureError::Failed(String::from("monitor disappeared")))?;
    let mut frame = shot
        .frame
        .crop(rect)
        .ok_or_else(|| CaptureError::Failed(String::from("empty selection")))?;
    if include_pointer {
        if let Some((x, y)) = region_pointer(pointer, monitor, rect, shot.scale) {
            draw_pointer(&mut frame, x, y, shot.scale);
        }
    }
    Ok(frame)
}

/// Where the pointer is drawn in a region capture, relative to the selection's origin — or
/// `None` when it is on another monitor, or its hot spot lies outside the selection.
///
/// The same decision the overlay's preview shows the user before confirming, so what was
/// promised and what the image contains cannot disagree. Without it the arrow was handed to
/// cairo outside the crop and clipped away with nothing to show for it.
fn region_pointer(
    pointer: Option<PointerAt>,
    monitor: usize,
    rect: geometry::IRect,
    scale: f64,
) -> Option<(f64, f64)> {
    let at = pointer.filter(|p| p.monitor == monitor)?;
    rect.pointer_inside((at.x, at.y), scale)
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
    let probed = if needs_probe {
        // Without the pointer drawn, only the monitor under it counts.
        overlay::probe_pointer(app, &desktop, include_pointer).await
    } else {
        None
    };
    // A position that is not exact still picks the monitor, but is never drawn.
    let draw = include_pointer && probed.is_some_and(|probed| probed.exact);
    compose_screen(&desktop, all_monitors, draw, probed.map(|probed| probed.at))
}

/// The screen image: the monitor under the pointer (or every monitor), with the pointer drawn
/// in only when it is included.
fn compose_screen(
    desktop: &Desktop,
    all_monitors: bool,
    include_pointer: bool,
    pointer: Option<PointerAt>,
) -> Result<Frame, CaptureError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use geometry::IRect;

    fn at(monitor: usize, x: f64, y: f64) -> PointerAt {
        PointerAt { monitor, x, y }
    }

    const GREY: frame::Rgb = frame::Rgb {
        r: 0x80,
        g: 0x80,
        b: 0x80,
    };
    const WHITE: frame::Rgb = frame::Rgb {
        r: 0xff,
        g: 0xff,
        b: 0xff,
    };

    /// One 800 x 600 monitor at scale 1, uniformly grey, so anything drawn on it shows.
    fn desktop() -> Desktop {
        let full = Frame::filled(800, 600, GREY).expect("frame");
        Desktop {
            monitors: vec![overlay::MonitorShot {
                monitor: None,
                crop: full.bounds(),
                frame: full.clone(),
                scale: 1.0,
            }],
            full,
        }
    }

    // spec: capture / Full-screen capture / Multi-monitor
    #[test]
    fn a_screen_capture_keeps_only_the_monitor_under_the_pointer() {
        // Two 400 x 300 monitors side by side, grey and white. `probe_pointer` with `exact`
        // false reports the monitor of its first event (overlay tests); that monitor alone is
        // the image.
        let mut full = Frame::filled(800, 300, GREY).expect("frame");
        let left = IRect::new(0, 0, 400, 300);
        let right = IRect::new(400, 0, 400, 300);
        full.draw_with(|cr| {
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.rectangle(400.0, 0.0, 400.0, 300.0);
            cr.fill()
        })
        .expect("draw");
        assert_eq!(full.pixel(600, 150), Some(WHITE));
        let shot = |crop: IRect| overlay::MonitorShot {
            monitor: None,
            crop,
            frame: full.crop(crop).expect("crop"),
            scale: 1.0,
        };
        let desktop = Desktop {
            monitors: vec![shot(left), shot(right)],
            full: full.clone(),
        };
        let screen =
            compose_screen(&desktop, false, false, Some(at(1, 120.0, 80.0))).expect("screen");
        assert!(
            screen == full.crop(right).expect("crop"),
            "not monitor 2 alone"
        );
        let screen =
            compose_screen(&desktop, false, false, Some(at(0, 120.0, 80.0))).expect("screen");
        assert!(
            screen == full.crop(left).expect("crop"),
            "not monitor 1 alone"
        );
    }

    // spec: capture / Cursor inclusion / Cursor excluded
    #[test]
    fn an_excluded_pointer_leaves_the_portal_frame_unmodified() {
        // The pointer stands well inside the selection, where it would be drawn if included.
        // Both modes in which Annota draws the pointer itself are compared pixel for pixel with
        // the plain portal frame of the captured area. Window stills take the ScreenCast
        // portal's own cursor mode instead (`screencast::pointer_mode_for_all_four_combinations`).
        let desktop = desktop();
        let rect = IRect::new(100, 100, 400, 300);
        let pointer = Some(at(0, 300.0, 250.0));
        let plain_region = desktop.full.crop(rect).expect("crop");

        let region = compose_region(&desktop, 0, rect, false, pointer).expect("region");
        assert!(region == plain_region, "region capture was modified");
        let screen = compose_screen(&desktop, false, false, pointer).expect("screen");
        assert!(screen == desktop.full, "screen capture was modified");

        // Control: with the pointer included the same captures differ, so the comparison above
        // is able to fail.
        let region = compose_region(&desktop, 0, rect, true, pointer).expect("region");
        assert!(region != plain_region);
        let screen = compose_screen(&desktop, false, true, pointer).expect("screen");
        assert!(screen != desktop.full);
    }

    // spec: capture / Cursor inclusion / Delayed region capture places the pointer
    #[test]
    fn a_delayed_region_capture_draws_the_pointer_where_the_countdown_ended() {
        // Confirmed with the pointer over the toolbar (no position), then moved into the region
        // during the countdown: the position probed after the countdown is the one drawn.
        let desktop = desktop();
        let rect = IRect::new(100, 100, 400, 300);
        let after = Some(at(0, 300.0, 250.0));
        let pointer = capture_pointer(3, None, after);
        assert_eq!(pointer, after);
        let image = compose_region(&desktop, 0, rect, true, pointer).expect("region");
        // Hot spot at 200, 150 in the crop; inside the arrow body the fill is white
        // (same offset as `frame::pointer_is_visible_after_drawing`).
        assert_eq!(image.pixel(203, 160), Some(WHITE));

        // The position at confirmation is stale after a countdown and never falls back in:
        // inside at confirmation, outside when the countdown ended, no pointer.
        let pointer = capture_pointer(3, Some(at(0, 300.0, 250.0)), Some(at(0, 50.0, 50.0)));
        let image = compose_region(&desktop, 0, rect, true, pointer).expect("region");
        assert!(image == desktop.full.crop(rect).expect("crop"));

        // Without a delay the position at confirmation counts.
        let confirmed = Some(at(0, 1.0, 2.0));
        assert_eq!(capture_pointer(0, confirmed, after), confirmed);
    }

    // spec: capture / Window capture / Delayed window capture
    #[test]
    fn the_chooser_comes_before_the_countdown() {
        // Task 4.3, scenario "Delayed window capture": chooser, countdown, frame.
        use WindowStep::*;
        assert_eq!(window_steps(3, true), [Chooser, Countdown, Frame]);
        assert_eq!(
            window_steps(0, true),
            [Chooser, Frame],
            "no delay: no countdown"
        );
    }

    // spec: capture / Window capture / No window source
    #[test]
    fn without_a_window_source_the_screenshot_ui_stays_as_before() {
        // Scenario "No window source": today's order, countdown first, no waiting OSD.
        use WindowStep::*;
        assert_eq!(window_steps(0, false), [ScreenshotUi]);
        assert_eq!(window_steps(3, false), [Countdown, ScreenshotUi]);
    }

    // spec: capture / Cursor inclusion / Pointer inside the selection
    // spec: capture / Cursor inclusion / Pointer outside the selection
    #[test]
    fn a_region_capture_draws_the_pointer_only_inside_the_selection() {
        // No delay: the position at confirmation is used as it is. Inside, the arrow is in the
        // image at that position; outside, the image is byte for byte the selected region.
        let desktop = desktop();
        let rect = IRect::new(100, 100, 400, 300);
        let plain = desktop.full.crop(rect).expect("crop");

        let inside = capture_pointer(0, Some(at(0, 300.0, 250.0)), None);
        let image = compose_region(&desktop, 0, rect, true, inside).expect("region");
        // Hot spot at 200, 150 in the crop; inside the arrow body the fill is white
        // (same offset as `frame::pointer_is_visible_after_drawing`).
        assert_eq!(image.pixel(203, 160), Some(WHITE));
        assert_eq!(image.pixel(150, 100), Some(GREY), "nothing else changed");

        for outside in [(99.0, 250.0), (300.0, 99.0), (500.0, 250.0), (300.0, 400.0)] {
            let pointer = capture_pointer(0, Some(at(0, outside.0, outside.1)), None);
            let image = compose_region(&desktop, 0, rect, true, pointer).expect("region");
            assert!(image == plain, "pointer at {outside:?} changed the image");
        }
    }

    // spec: capture / Cursor inclusion / Pointer outside the selection
    #[test]
    fn a_pointer_on_another_monitor_is_not_drawn() {
        // Scenario "Pointer outside the selection", multi-monitor case: the selection lives on
        // one monitor and the pointer may be on another. The monitor filter runs before the
        // hot-spot test, so a position that would land inside this selection by coordinate is
        // still rejected.
        let rect = IRect::new(0, 0, 800, 600);
        assert_eq!(
            region_pointer(Some(at(0, 100.0, 100.0)), 0, rect, 1.0),
            Some((100.0, 100.0))
        );
        assert_eq!(
            region_pointer(Some(at(1, 100.0, 100.0)), 0, rect, 1.0),
            None,
            "other monitor"
        );
    }

    // spec: capture / Cursor inclusion / Pointer outside the selection
    #[test]
    fn no_pointer_position_draws_nothing() {
        // Scenario "Pointer outside the selection": confirmed from the keyboard, or the pointer
        // was over the toolbar. There is no stale position to fall back to.
        let rect = IRect::new(0, 0, 800, 600);
        assert_eq!(region_pointer(None, 0, rect, 1.0), None);
    }

    // spec: capture / Cursor inclusion / Pointer inside the selection
    #[test]
    fn the_selections_origin_is_subtracted() {
        // The capture is the crop, so the drawn position is relative to its top-left corner.
        let rect = IRect::new(140, 212, 800, 560);
        assert_eq!(
            region_pointer(Some(at(0, 340.0, 412.0)), 0, rect, 1.0),
            Some((200.0, 200.0))
        );
        // Inside the monitor but left of and above the selection.
        assert_eq!(
            region_pointer(Some(at(0, 139.0, 412.0)), 0, rect, 1.0),
            None
        );
        assert_eq!(
            region_pointer(Some(at(0, 340.0, 211.0)), 0, rect, 1.0),
            None
        );
    }
}
