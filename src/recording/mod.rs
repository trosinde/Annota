//! Screen recording (add-screen-recording): the display-independent core.
//!
//! API for the later phases (portal session, setup overlay, control window, export window):
//! - [`pipeline::start`]`(`[`RecordingConfig`]`)` opens a crash-safe Matroska working file in the
//!   recordings folder ([`files::recordings_folder`]) and returns a [`Recording`] plus a receiver
//!   of [`RecordingEvent`]s (level meters per source, errors, end of the video source);
//!   [`pipeline::forward_events`] delivers them on the GLib main context.
//! - [`Recording`]: `pause`/`resume`, `set_muted(source, bool)`, `elapsed` (without paused
//!   time), `stop` → [`Finished`] (working file, duration, audio tracks, error if it ended early).
//!   `stop` never waits for the source to end (P5/P6); on a [`RecordingEvent::Error`] call it to
//!   keep what was written.
//! - The video source is any raw-video element (`pipewiresrc` from the ScreenCast session,
//!   Phase B); [`VideoArea`] says what is recorded: the whole buffer, a region mapped by
//!   [`geometry::region_crop`], or a window cropped per buffer ([`geometry::window_content`]) and
//!   letterboxed into the first frame. Audio: [`audio::source_element`] per wanted and present
//!   source ([`audio::sources_to_record`], devices behind [`audio::AudioDevices`]).
//!
//! [`start`] is the entry point of `--record`, the Record shortcut and the Record card; until the
//! portal session and the UI exist (Phases B-D) it only reports that recording is unavailable.

// The API above is driven by Phases B-D of add-screen-recording; until then most of it is used
// only by its tests.
#![allow(dead_code)]

pub mod audio;
pub mod files;
pub mod geometry;
pub mod pipeline;

#[allow(unused_imports)]
pub use pipeline::{
    Finished, Recording, RecordingConfig, RecordingError, RecordingEvent, VideoArea,
};

use crate::cli::RecordMode;

/// One request to record (`--record`, Record shortcut, Record card).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordRequest {
    pub mode: RecordMode,
}

/// Starts the recording flow for `request`. Phase A: nothing to open yet, so it says so on
/// stderr and returns; the application keeps running.
pub fn start(_app: &adw::Application, request: RecordRequest) {
    eprintln!(
        "annota: --record {}: screen recording is not available in this build yet",
        request.mode.as_str()
    );
}
