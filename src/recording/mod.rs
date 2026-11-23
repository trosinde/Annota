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
//!   [`geometry::region_crop`], or a window cropped per buffer ([`geometry::window_content`]) and
//!   letterboxed into the first frame. Audio: [`audio::source_element`] per wanted and present
//!   source ([`audio::sources_to_record`], devices behind [`audio::AudioDevices`]).
//!
//! Phase B (portal session, stop paths, waiting indicator):
//! - [`start`]`(app, `[`RecordRequest`]`)`: `--record`, the Record shortcut, the Record card.
//!   Window and Screen request the source at once ([`source::request`], with the waiting
//!   indicator); Region needs the overlay's selection and enters through [`start_region`], which
//!   the setup (Phase C) calls. A second request while one runs starts nothing ([`is_active`]).
//! - The flow: source with waiting indicator → indicator gone → [`Hooks::countdown`] (Phase C; 0 s
//!   until then) → [`pipeline::start`] → a [`RecordingSession`] whose end ([`stop`], the desktop
//!   ending the session, an error) delivers one [`Ended`] to [`Hooks::on_ended`] (Phase D opens
//!   the Export Recording window from it). How a start ended goes to [`Hooks::on_outcome`] as a
//!   [`StartOutcome`], whose [`StartOutcome::message`] is the toast text (Phase D shows it).
//! - [`source::offered`] says which source types the portal offers (setup, "Source not offered").

// The API above is driven by Phases B-D of add-screen-recording; until then most of it is used
// only by its tests.
#![allow(dead_code)]

pub mod audio;
pub mod files;
pub mod geometry;
#[cfg(test)]
mod live_tests;
pub mod pipeline;
pub mod session;
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
use futures_util::future::LocalBoxFuture;
use gettextrs::gettext;
use gstreamer as gst;
use gtk::{gio, glib};

use crate::capture::geometry::IRect;
use crate::capture::window_wait;
use crate::cli::RecordMode;
use crate::config;
use audio::AudioSource;
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

/// How a start ended, for a message (Phase D shows it as a toast, mockup 13 / task 4.7).
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
/// Builds the audio sources for (microphone wanted, system audio wanted).
pub type AudioSources = Rc<dyn Fn(bool, bool) -> Vec<(AudioSource, gst::Element)>>;
/// The countdown between the granted source and the first frame; `false`: cancelled.
pub type Countdown = Rc<dyn Fn(&adw::Application, Target) -> LocalBoxFuture<'static, bool>>;

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
    /// Phase C (task 4.3): the countdown OSD. Until then none (0 s).
    pub countdown: Countdown,
    /// Level meters and errors while recording (the control, Phase C).
    pub on_event: Rc<dyn Fn(&RecordingEvent)>,
    /// How a start ended (Phase D: the toast for [`StartOutcome::message`]).
    pub on_outcome: OnOutcome,
    /// A recording ended (Phase D: the Export Recording window, D1).
    pub on_ended: OnEnded,
}

impl Default for Hooks {
    fn default() -> Self {
        Self {
            video_source: Rc::new(source::pipewire_source),
            audio_sources: Rc::new(system_audio_sources),
            countdown: Rc::new(|_, _| Box::pin(async { true })),
            on_event: Rc::new(|_| {}),
            on_outcome: Rc::new(|_, outcome| {
                // Phase D (task 4.7, mockup 13): shown as a toast.
                if let Some(message) = outcome.message() {
                    eprintln!("annota: recording: {message}");
                }
            }),
            // Phase D: the Export Recording window opens here (D1). Stub until then.
            on_ended: Rc::new(|_, ended| {
                eprintln!(
                    "annota: recording finished ({:?}): {}",
                    ended.reason,
                    ended.finished.path.display()
                );
            }),
        }
    }
}

fn system_audio_sources(microphone: bool, system_audio: bool) -> Vec<(AudioSource, gst::Element)> {
    let devices = audio::SystemDevices::probe();
    audio::sources_to_record(&devices, microphone, system_audio)
        .into_iter()
        .filter_map(|source| match audio::source_element(source) {
            Ok(element) => Some((source, element)),
            Err(err) => {
                eprintln!("annota: recording: no audio source element: {err}");
                None
            }
        })
        .collect()
}

/// Where the single recording flow is.
enum Active {
    Idle,
    /// The source request with the waiting indicator.
    Waiting,
    /// Granted, before the first frame (countdown).
    Starting,
    Recording(Rc<RecordingSession>),
}

thread_local! {
    static ACTIVE: RefCell<Active> = const { RefCell::new(Active::Idle) };
    static HOOKS: RefCell<Hooks> = RefCell::new(Hooks::default());
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
            session.stop();
            true
        }
        None => false,
    }
}

/// Starts the recording flow for `request` (`--record`, Record shortcut, Record card).
pub fn start(app: &adw::Application, request: RecordRequest) {
    if is_active() {
        match request.mode {
            RecordMode::Toggle if stop() => {}
            // Phase C (task 4.4): raise the control. While waiting, the indicator comes up.
            _ => {
                window_wait::raise();
            }
        }
        return;
    }
    match request.mode {
        RecordMode::Window => spawn(app, Target::Window),
        RecordMode::Screen => spawn(app, Target::Screen),
        RecordMode::Region | RecordMode::Toggle => {
            // STUB (Phase C, task 4.1): opens the region overlay in Video mode, whose Start
            // Recording calls `start_region` with the selection.
            eprintln!("annota: --record region: the recording setup is not available yet");
        }
    }
}

/// Start Recording in the overlay's setup (Phase C): records `selection` (logical desktop px).
pub fn start_region(app: &adw::Application, selection: IRect) {
    if is_active() {
        window_wait::raise();
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
    set_active(Active::Starting);
    let area = match area(target, &granted) {
        Ok(area) => area,
        Err(outcome) => {
            granted.close().await;
            return outcome;
        }
    };
    let hooks = hooks();
    if !(hooks.countdown)(app, target).await {
        granted.close().await;
        return StartOutcome::Cancelled;
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
        settings.boolean("record-microphone"),
        settings.boolean("record-system-audio"),
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
            set_active(Active::Idle);
            on_ended(&app, ended);
        }
    };
    let session =
        RecordingSession::begin(app, recording, events, granted, hooks.on_event, on_ended);
    // An error may have ended it already.
    if session.is_running() {
        set_active(Active::Recording(session));
    }
    StartOutcome::Recording
}

/// What of the granted stream is recorded (design.md D2).
fn area(target: Target, granted: &Granted) -> Result<VideoArea, StartOutcome> {
    let size = granted.size.map(|(w, h)| Size::new(w, h));
    match target {
        Target::Screen => Ok(VideoArea::Whole),
        Target::Window => match size {
            Some(size) => {
                let scale = source::monitor_scale(granted.position, granted.size);
                Ok(WindowContent::new(size, scale).into_area())
            }
            // No size to convert the crop meta with: the alpha rule alone.
            None => Ok(WindowContent::new(Size::new(0, 0), None).into_area()),
        },
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
