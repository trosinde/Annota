//! Screen recording (add-screen-recording): the display-independent core.
//!
//! API for the later phases (portal session, setup overlay, control window, export window):
//! - [`pipeline::start`]`(`[`RecordingConfig`]`)` opens a crash-safe Matroska working file in the
//!   recordings folder ([`files::recordings_folder`]) and returns a [`Recording`] plus a receiver
//!   of [`RecordingEvent`]s (level meters per source, errors, end of the video source);
//!   [`pipeline::forward_events`] delivers them on the GLib main context.
//! - [`Recording`]: `pause`/`resume`, `set_muted(source, bool)`, `elapsed` (without paused
//!   time), `stop` → a future of [`Finished`] (working file, duration, audio tracks, error if it
//!   ended early), finished on a worker thread within a bounded time. `stop` never waits for the
//!   source to end (P5/P6); on a [`RecordingEvent::Error`] call it to keep what was written.
//! - The video source is any raw-video element (`pipewiresrc` from the ScreenCast session,
//!   Phase B); [`VideoArea`] says what is recorded: the whole buffer, a region mapped by
//!   [`geometry::region_crop`], or a window cropped per buffer to `screencast::window_rect`'s rectangle
//!   ([`window::WindowContent`]) and letterboxed into the first frame, at its size on screen. Audio: [`audio::source_element`] per wanted and present
//!   source ([`audio::sources_to_record`], devices behind [`audio::AudioDevices`]).
//!
//! Phase B (portal session, stop paths, waiting indicator) and Phase C1 (setup, countdown):
//! - [`start`]`(app, `[`RecordRequest`]`)`: `--record`, the Record shortcut, the Record card.
//!   Window and Screen request the source at once ([`source::request`], with the waiting
//!   indicator); Region and Toggle (with nothing running) open the region overlay in Video mode
//!   (`capture::start_recording_setup`, the setup in [`setup`]), whose Start Recording comes back
//!   through [`start_from_setup`] ([`start_region`] for a region). Toggle cancels a countdown and
//!   stops a recording. A second request while one runs starts nothing ([`is_active`]).
//! - The flow: source with waiting indicator → indicator gone (+ `countdown::SETTLE`) →
//!   [`Hooks::countdown`] (the OSD for `record-countdown` seconds, gone + `SETTLE` before it
//!   resolves; `Enter` starts now, `Esc`, the Record shortcut or the desktop closing the session
//!   end it with nothing written) → [`pipeline::start`] → a [`RecordingSession`] whose end
//!   ([`stop`], the desktop ending the session, an error) delivers one [`Ended`] to
//!   [`Hooks::on_ended`] (Phase D opens the Export Recording window from it). How a start ended
//!   goes to [`Hooks::on_outcome`] as a [`StartOutcome`], whose [`StartOutcome::message`] is the
//!   toast text (shown in the main window, `application::show_recording_message`).
//! - Phase D2: the Record shortcut is the GlobalShortcuts ID `record` of
//!   `capture::shortcuts::GlobalShortcut` (one bind with the capture shortcuts; its `Activated`
//!   runs `app.record toggle`, as `annota --record toggle` from the GNOME keybinding does); the
//!   welcome page's Record card runs `app.record region` (the setup, or the control while one
//!   runs).
//! - [`source::offered`] says which source types the portal offers (setup, "Source not offered").
//!   [`Hooks::devices`] which audio devices exist, probed off the main loop and only when audio
//!   is wanted.

// The API above is driven by Phases B-D of add-screen-recording; until then most of it is used
// only by its tests.
#![allow(dead_code)]

pub mod audio;
pub mod control;
pub mod export;
pub mod export_window;
pub mod files;
pub mod geometry;
#[cfg(test)]
mod live_tests;
pub mod pipeline;
pub mod session;
pub mod setup;
pub mod source;
pub mod window;

#[allow(unused_imports)]
pub use pipeline::{
    Finished, Recording, RecordingConfig, RecordingError, RecordingEvent, VideoArea,
};

use std::cell::RefCell;
use std::os::fd::RawFd;
use std::rc::Rc;

use adw::prelude::*;
use futures_channel::oneshot;
use futures_util::future::{self, Either, LocalBoxFuture};
use gettextrs::gettext;
use gstreamer as gst;
use gtk::{gio, glib};
use std::pin::pin;

use crate::capture::geometry::IRect;
use crate::capture::{self, countdown, window_wait};
use crate::cli::{CaptureMode, RecordMode};
use crate::config;
use audio::{AudioSource, SystemDevices};
use geometry::{MonitorStream, RegionError, Size};
#[allow(unused_imports)]
pub use session::{EndReason, Ended, RecordingSession};
use source::{Granted, Requested, SourceKind};
use window::WindowContent;

/// One request to record (`--record`, Record shortcut, Record card).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordRequest {
    pub mode: RecordMode,
}

/// How the countdown ended.
enum Counted {
    Done,
    Cancelled,
    DesktopEnded,
}

/// What is recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// A selection of the overlay, in logical desktop px (the coordinates of the stream's
    /// `position` and `size`).
    Region(IRect),
    Window,
    Screen,
}

impl Target {
    fn kind(self) -> SourceKind {
        match self {
            Self::Window => SourceKind::Window,
            Self::Region(_) | Self::Screen => SourceKind::Monitor,
        }
    }
}

/// How a start ended, for a message (a toast in the main window, mockups 13 and 15).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartOutcome {
    /// The recording runs.
    Recording,
    /// Cancel/`Esc` in the waiting indicator, the chooser cancelled in the desktop, or the
    /// countdown cancelled. Nothing is said.
    Cancelled,
    /// The portal closed the session while Annota waited for Start's answer.
    DesktopEnded,
    /// The region is not inside the chosen monitor.
    RegionNotOnScreen,
    /// Anything else; the cause is logged.
    Failed(String),
}

impl StartOutcome {
    /// The text to show, if any.
    pub fn message(&self) -> Option<String> {
        match self {
            Self::DesktopEnded => Some(gettext(
                "The desktop ended the screen-sharing request. Nothing was recorded.",
            )),
            Self::RegionNotOnScreen => Some(gettext(
                "The selected area is not on the screen you chose. Nothing was recorded.",
            )),
            Self::Recording | Self::Cancelled | Self::Failed(_) => None,
        }
    }
}

/// Builds the video source from the PipeWire remote and node.
pub type VideoSource = Rc<dyn Fn(RawFd, u32) -> Result<gst::Element, RecordingError>>;
/// Builds the audio sources for (record the microphone, record system audio), both already
/// known to exist.
pub type AudioSources = Rc<dyn Fn(bool, bool) -> Vec<(AudioSource, gst::Element)>>;
/// The countdown of `seconds` between the granted source and the first frame; `false`:
/// cancelled. It resolves only once its OSD is off the screen (plus `countdown::SETTLE`).
pub type Countdown = Rc<dyn Fn(&adw::Application, Target, u32) -> LocalBoxFuture<'static, bool>>;
/// Which audio devices exist (probed off the main loop).
pub type Devices = Rc<dyn Fn() -> LocalBoxFuture<'static, SystemDevices>>;
/// The setup's level preview source for a device.
pub type PreviewSource = Rc<dyn Fn(AudioSource) -> Option<gst::Element>>;

/// Told how a start ended.
pub type OnOutcome = Rc<dyn Fn(&adw::Application, &StartOutcome)>;
/// Told that a recording ended.
pub type OnEnded = Rc<dyn Fn(&adw::Application, Ended)>;

/// The slots the later phases (and the tests) fill.
#[derive(Clone)]
pub struct Hooks {
    /// `pipewiresrc` on the granted stream ([`source::pipewire_source`]).
    pub video_source: VideoSource,
    /// `pulsesrc` per wanted and present device.
    pub audio_sources: AudioSources,
    /// The countdown OSD (task 4.3, D5); none for 0 s.
    pub countdown: Countdown,
    /// The audio devices (setup rows, which sources are recorded).
    pub devices: Devices,
    /// The setup's level meters (task 4.1).
    pub preview_source: PreviewSource,
    /// Level meters and errors while recording (the control, Phase D).
    pub on_event: Rc<dyn Fn(&RecordingEvent)>,
    /// How a start ended: the toast for [`StartOutcome::message`] (mockups 13, 15).
    pub on_outcome: OnOutcome,
    /// A recording ended: the Export Recording window (D1).
    pub on_ended: OnEnded,
    /// The Record shortcut's trigger for the control's Stop tooltip, as the portal reports it
    /// (`shortcuts::record_trigger`); `None`: no chip.
    pub record_trigger: control::RecordTrigger,
}

impl Default for Hooks {
    fn default() -> Self {
        Self {
            video_source: Rc::new(source::pipewire_source),
            audio_sources: Rc::new(system_audio_sources),
            countdown: Rc::new(countdown_osd),
            devices: Rc::new(|| Box::pin(audio::probe_async())),
            preview_source: Rc::new(|source| audio::source_element(source).ok()),
            on_event: Rc::new(|_| {}),
            // Mockups 13 and 15 (tasks 4.6, 4.7): a toast in the main window.
            on_outcome: Rc::new(|app, outcome| {
                if let Some(message) = outcome.message() {
                    crate::application::show_recording_message(app, &message);
                }
            }),
            // The Export Recording window (D1, tasks 5.1-5.4), for every end.
            on_ended: Rc::new(export_window::open),
            // Task 4.5: the trigger every surface shows for the Record shortcut.
            record_trigger: Rc::new(crate::capture::shortcuts::record_trigger),
        }
    }
}

/// `pulsesrc` for each wanted source; the caller has checked that its device exists.
fn system_audio_sources(microphone: bool, system_audio: bool) -> Vec<(AudioSource, gst::Element)> {
    AudioSource::ALL
        .into_iter()
        .filter(|source| match source {
            AudioSource::Microphone => microphone,
            AudioSource::SystemAudio => system_audio,
        })
        .filter_map(|source| match audio::source_element(source) {
            Ok(element) => Some((source, element)),
            Err(err) => {
                eprintln!("annota: recording: no audio source element: {err}");
                None
            }
        })
        .collect()
}

/// The recording countdown (D5, mockup 04): the capture's OSD with the recording's label, red
/// ring and, for a region, the dashed outline. 0 s shows nothing.
fn countdown_osd(
    app: &adw::Application,
    target: Target,
    seconds: u32,
) -> LocalBoxFuture<'static, bool> {
    let app = app.clone();
    Box::pin(async move {
        if seconds == 0 {
            return true;
        }
        let region = match target {
            Target::Region(selection) => Some(selection),
            Target::Window | Target::Screen => None,
        };
        let purpose = countdown::Purpose::Recording { region };
        countdown::run_for(&app, purpose, seconds).await != countdown::End::Cancelled
    })
}

/// Where the single recording flow is.
enum Active {
    Idle,
    /// The source request with the waiting indicator.
    Waiting,
    /// Granted, before the first frame (countdown); the sender cancels it.
    Starting(Rc<RefCell<Option<oneshot::Sender<()>>>>),
    Recording(Rc<RecordingSession>),
}

thread_local! {
    static ACTIVE: RefCell<Active> = const { RefCell::new(Active::Idle) };
    static HOOKS: RefCell<Hooks> = RefCell::new(Hooks::default());
    /// Annota's windows the capture flow hid for the setup: off the screen while the recording
    /// starts and runs, back when the start ends without a recording or the recording has
    /// ended ([`show_hidden`]).
    static HIDDEN: RefCell<Vec<gtk::Window>> = const { RefCell::new(Vec::new()) };
}

/// Shows the windows hidden for the setup again (those still open).
fn show_hidden(app: &adw::Application) {
    let hidden = HIDDEN.with(RefCell::take);
    show_windows(app, &hidden);
}

fn show_windows(app: &adw::Application, windows: &[gtk::Window]) {
    let open = app.windows();
    for window in windows.iter().filter(|w| open.contains(w)) {
        window.set_visible(true);
    }
}

/// Changes the hooks (later phases, tests).
pub fn set_hooks(change: impl FnOnce(&mut Hooks)) {
    HOOKS.with(|hooks| change(&mut hooks.borrow_mut()));
}

fn hooks() -> Hooks {
    HOOKS.with(|hooks| hooks.borrow().clone())
}

fn set_active(active: Active) {
    ACTIVE.with(|a| a.replace(active));
}

/// Whether a recording flow runs (waiting for the source, counting down or recording).
pub fn is_active() -> bool {
    ACTIVE.with(|a| !matches!(*a.borrow(), Active::Idle))
}

/// The running recording, if one runs (control: pause, mute, elapsed time).
pub fn running() -> Option<Rc<RecordingSession>> {
    ACTIVE.with(|a| match &*a.borrow() {
        Active::Recording(session) => Some(session.clone()),
        _ => None,
    })
}

/// Stops the running recording (Stop button, Record shortcut). `false`: none runs. The file is
/// finished off the main thread; the flow stays active until its [`Ended`] is delivered.
pub fn stop() -> bool {
    match running() {
        Some(session) => {
            // The control goes at once; the file is finished meanwhile.
            control::close();
            session.stop();
            true
        }
        None => false,
    }
}

/// Cancels a running countdown (the Record shortcut, `--record toggle`). `false`: none runs.
fn cancel_countdown() -> bool {
    let cancel = ACTIVE.with(|a| match &*a.borrow() {
        Active::Starting(cancel) => cancel.borrow_mut().take(),
        _ => None,
    });
    match cancel {
        Some(cancel) => {
            let _ = cancel.send(());
            true
        }
        None => false,
    }
}

/// Starts the recording flow for `request` (`--record`, Record shortcut, Record card). Toggle
/// semantics (D7): idle → the region overlay in Video mode; countdown → cancelled; recording →
/// stopped.
pub fn start(app: &adw::Application, request: RecordRequest) {
    if is_active() {
        match request.mode {
            RecordMode::Toggle if stop() || cancel_countdown() => {}
            // Recording: the control is presented; while waiting, the indicator comes up.
            _ => {
                if !control::present() {
                    window_wait::raise();
                }
            }
        }
        return;
    }
    match request.mode {
        RecordMode::Window => spawn(app, Target::Window),
        RecordMode::Screen => spawn(app, Target::Screen),
        // The overlay's Start Recording comes back through `start_from_setup`.
        RecordMode::Region | RecordMode::Toggle => capture::start_recording_setup(app),
    }
}

/// Start Recording in the overlay's setup: `source` as chosen there; a region records
/// `selection` (logical desktop px). `hidden`: Annota's windows the capture flow hid; they stay
/// hidden until this start ends without a recording or the recording has ended.
pub fn start_from_setup(
    app: &adw::Application,
    source: CaptureMode,
    selection: Option<IRect>,
    hidden: Vec<gtk::Window>,
) {
    if is_active() {
        // Another flow runs (its control or indicator comes up): nothing starts here.
        show_windows(app, &hidden);
        route_setup(app, source, selection);
        return;
    }
    HIDDEN.with(|h| h.replace(hidden));
    route_setup(app, source, selection);
    if !is_active() {
        // Nothing started (no region).
        show_hidden(app);
    }
}

fn route_setup(app: &adw::Application, source: CaptureMode, selection: Option<IRect>) {
    match (source, selection) {
        (CaptureMode::Region, Some(selection)) => start_region(app, selection),
        (CaptureMode::Region, None) => eprintln!("annota: recording: no region selected"),
        (CaptureMode::Window, _) => start(
            app,
            RecordRequest {
                mode: RecordMode::Window,
            },
        ),
        (CaptureMode::Screen, _) => start(
            app,
            RecordRequest {
                mode: RecordMode::Screen,
            },
        ),
    }
}

/// Records `selection` (logical desktop px): the setup's Start Recording with source Region.
pub fn start_region(app: &adw::Application, selection: IRect) {
    if is_active() {
        if !control::present() {
            window_wait::raise();
        }
        return;
    }
    spawn(app, Target::Region(selection));
}

fn spawn(app: &adw::Application, target: Target) {
    set_active(Active::Waiting);
    let app = app.clone();
    // Taken before the first poll: `annota --record screen` has no window holding the app.
    let hold = app.hold();
    glib::spawn_future_local(async move {
        let outcome = record(&app, target).await;
        if outcome != StartOutcome::Recording {
            set_active(Active::Idle);
            // Before the outcome: its message is shown in Annota's main window.
            show_hidden(&app);
        }
        if let StartOutcome::Failed(cause) = &outcome {
            eprintln!("annota: recording did not start: {cause}");
        }
        (hooks().on_outcome)(&app, &outcome);
        drop(hold);
    });
}

async fn record(app: &adw::Application, target: Target) -> StartOutcome {
    let settings = gio::Settings::new(config::APP_ID);
    let granted =
        match source::request(app, target.kind(), settings.boolean("include-pointer")).await {
            Requested::Granted(granted) => *granted,
            Requested::Cancelled => return StartOutcome::Cancelled,
            Requested::DesktopEnded => return StartOutcome::DesktopEnded,
            Requested::Failed(cause) => return StartOutcome::Failed(cause),
        };
    let (cancel, cancelled) = oneshot::channel::<()>();
    set_active(Active::Starting(Rc::new(RefCell::new(Some(cancel)))));
    let mut granted = granted;
    let area = match area(target, &granted) {
        Ok(area) => area,
        Err(outcome) => {
            granted.close().await;
            return outcome;
        }
    };
    let hooks = hooks();
    // Which devices exist, asked only when audio is wanted; before the countdown, so the first
    // frame follows the OSD without a probe in between.
    let (microphone, system_audio) = (
        settings.boolean("record-microphone"),
        settings.boolean("record-system-audio"),
    );
    let wanted = if microphone || system_audio {
        let devices = (hooks.devices)().await;
        audio::sources_to_record(&devices, microphone, system_audio)
    } else {
        Vec::new()
    };
    let seconds = settings.int("record-countdown").clamp(0, 10) as u32;
    // The countdown ends by itself, by `Enter`/`Esc`, by the Record shortcut, or because the
    // desktop closed the session meanwhile (then nothing is sent back, nothing written).
    let counted = {
        let countdown = (hooks.countdown)(app, target, seconds);
        let shortcut = pin!(async {
            let _ = cancelled.await;
        });
        let ended = pin!(granted.closed_by_portal());
        match future::select(countdown, future::select(shortcut, ended)).await {
            Either::Left((true, _)) => Counted::Done,
            Either::Left((false, _)) | Either::Right((Either::Left(_), _)) => Counted::Cancelled,
            Either::Right((Either::Right(_), _)) => Counted::DesktopEnded,
        }
    };
    match counted {
        Counted::Done => {}
        Counted::Cancelled => {
            granted.close().await;
            return StartOutcome::Cancelled;
        }
        Counted::DesktopEnded => {
            granted.closed_by_desktop();
            return StartOutcome::DesktopEnded;
        }
    }
    let video = match granted.fd() {
        Some(fd) => (hooks.video_source)(fd, granted.node),
        None => Err(RecordingError::Build(String::from("no PipeWire remote"))),
    };
    let video = match video {
        Ok(video) => video,
        Err(err) => {
            granted.close().await;
            return StartOutcome::Failed(err.to_string());
        }
    };
    let folder = files::recordings_folder(&settings.string("record-folder"));
    let mut config = RecordingConfig::new(folder, video, area);
    let audio = (hooks.audio_sources)(
        wanted.contains(&AudioSource::Microphone),
        wanted.contains(&AudioSource::SystemAudio),
    );
    for (source, element) in audio {
        config = config.with_audio(source, element);
    }
    let (recording, events) = match pipeline::start(config) {
        Ok(started) => started,
        Err(err) => {
            granted.close().await;
            return match err {
                RecordingError::RegionNotOnScreen => StartOutcome::RegionNotOnScreen,
                other => StartOutcome::Failed(other.to_string()),
            };
        }
    };
    let on_ended = {
        let (app, on_ended) = (app.clone(), hooks.on_ended.clone());
        move |ended: Ended| {
            control::close();
            set_active(Active::Idle);
            // Annota's hidden windows come back first, so the Export Recording window that
            // `on_ended` opens and presents is the newest window, in front of them.
            show_hidden(&app);
            on_ended(&app, ended);
        }
    };
    let on_event = {
        let on_event = hooks.on_event.clone();
        Rc::new(move |event: &RecordingEvent| {
            control::on_event(event);
            on_event(event);
        })
    };
    let session = RecordingSession::begin(app, recording, events, granted, on_event, on_ended);
    // An error may have ended it already.
    if session.is_running() {
        set_active(Active::Recording(session.clone()));
        // Task 4.4: the control, from the first recorded frame on.
        control::show(app, &session, hooks.record_trigger.clone());
    }
    StartOutcome::Recording
}

/// What of the granted stream is recorded (design.md D2).
fn area(target: Target, granted: &Granted) -> Result<VideoArea, StartOutcome> {
    let size = granted.size.map(|(w, h)| Size::new(w, h));
    match target {
        Target::Screen => Ok(VideoArea::Whole),
        // The window is found in each buffer by its pixels; the scales only set the frame size
        // (the window's size on screen in monitor pixels, maintainer decision 2026-11-24).
        Target::Window => Ok(WindowContent::new().into_area(geometry::Scales {
            stream_size: granted.size,
            monitor: source::monitor_scale(granted.position, granted.size),
        })),
        Target::Region(selection) => {
            let size = size.ok_or_else(|| {
                StartOutcome::Failed(String::from("the monitor stream has no size"))
            })?;
            // Measured present on GNOME 50 (P2). Without it the origin is known only with one
            // monitor.
            let position = granted
                .position
                .or_else(|| (source::monitor_count() == 1).then_some((0, 0)))
                .ok_or_else(|| {
                    StartOutcome::Failed(String::from("the monitor stream has no position"))
                })?;
            let stream = MonitorStream { position, size };
            match geometry::region_on_stream(selection, stream) {
                Ok(()) => Ok(VideoArea::Region { selection, stream }),
                Err(RegionError::NotOnScreen) => Err(StartOutcome::RegionNotOnScreen),
                Err(RegionError::Empty) => Err(StartOutcome::Failed(String::from(
                    "the selected area is empty",
                ))),
            }
        }
    }
}
