//! The recording pipeline (design.md D3, D5, D6): sources → Matroska working file.
//!
//! Two GStreamer pipelines, joined by Annota's own code ("the glue"):
//!
//! ```text
//! sources:  <video source> ─ appsink        <audio source> ─ appsink   (one per audio source)
//!                               │ glue: pause drop + running-time offset, stop
//!                               ▼
//! encoder:  appsrc ─ videocrop ─ videoconvert ─ videoscale(add-borders) ─ I420 W×H
//!                  ─ videorate ─ 30 fps ─ x264enc ──────────────────────────────┐
//!           appsrc ─ volume(mute) ─ level ─ audioconvert ─ audioresample ─ opusenc ─┼─ matroskamux ─ filesink
//! ```
//!
//! Why two: the screen-sharing stream never sends an end of stream when GNOME ends the session
//! (P5) or the recorded window closes (P6). The encoder side therefore gets its end of stream from
//! Annota ([`Recording::stop`]), whatever the source does, and a still screen at the end is
//! filled by repeating the last frame up to the stop time. Pausing drops buffers in the glue and
//! shifts later timestamps by the paused time, so the file has no gap (D5).
//!
//! Timestamps are the sources' running times; `videorate` turns whatever the stream delivers
//! (nothing on a still screen, bursts, a variable rate) into a constant 30 fps. `matroskamux`
//! writes only while every track has data, so on a still screen (a damage-driven window or screen
//! stream sends nothing, P6) a heartbeat thread pushes the last frame again at the current time:
//! the file and the audio keep advancing while nothing moves.
//!
//! [`Recording::stop`] returns at once; the end of stream and the encoder's flush run on a worker
//! thread and the [`Finished`] arrives as a future, bounded by `FINISH_TIMEOUT`.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use futures_channel::{mpsc, oneshot};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;
use gtk::glib;

use super::audio::AudioSource;
use super::files;
use super::geometry::{self, MonitorStream, RegionError, Size};
use crate::capture::geometry::IRect;

/// The recording's constant frame rate (requirement "Recording frame rate").
pub const FRAME_RATE: i32 = 30;
/// How long finishing a recording ([`Recording::stop`]) may take before its [`Finished`] is
/// delivered anyway, with [`RecordingError::Timeout`].
const FINISH_TIMEOUT: Duration = Duration::from_secs(10);
/// Most bytes queued in front of each encoder input before the glue waits (backpressure: a live
/// source then drops frames itself instead of Annota's memory growing). Two 4K BGRx frames.
const QUEUE_BYTES: u64 = 2 * 3840 * 2160 * 4;
/// While the queue is full, how often a waiting push looks again (and whether it should give up).
const QUEUE_POLL: Duration = Duration::from_millis(5);
/// How often the heartbeat looks at the video track.
const HEARTBEAT_TICK: Duration = Duration::from_millis(50);
/// After this long without a video buffer, the heartbeat repeats the last one at the current time.
const HEARTBEAT_AFTER: gst::ClockTime = gst::ClockTime::from_mseconds(200);
/// One output frame.
const FRAME: gst::ClockTime = gst::ClockTime::from_nseconds(1_000_000_000 / FRAME_RATE as u64);
/// x264 constant quantizer (design.md D3, `pass=quant`, quantizer ≈ 20).
const QUANTIZER: u32 = 20;
/// A key frame at least every 2 s: a file cut short by a crash loses at most one group.
const KEY_INT_MAX: u32 = 2 * FRAME_RATE as u32;

/// The window's content rectangle in one buffer of a window stream (Phase B: from the crop meta
/// via `geometry::window_content`). `None` keeps the previous rectangle.
pub type ContentRect =
    Arc<dyn Fn(&gst::BufferRef, &gst_video::VideoInfo) -> Option<IRect> + Send + Sync>;

/// Which part of the video stream's buffers is recorded (design.md D2).
#[derive(Clone)]
pub enum VideoArea {
    /// Screen: every buffer whole (trimmed to an even size for x264).
    Whole,
    /// Region: the selection in logical px of the monitor stream, mapped to buffer pixels by the
    /// first buffer's size.
    Region {
        selection: IRect,
        stream: MonitorStream,
    },
    /// Window: each buffer cropped to its own content rectangle. The first one (even-sized) sets
    /// the frame size; later ones are scaled to fit and letterboxed (D2).
    Window(ContentRect),
}

impl std::fmt::Debug for VideoArea {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Whole => write!(f, "Whole"),
            Self::Region { selection, stream } => write!(f, "Region({selection:?}, {stream:?})"),
            Self::Window(_) => write!(f, "Window"),
        }
    }
}

/// What to record and where.
pub struct RecordingConfig {
    /// The recordings folder (`files::recordings_folder`), created if missing.
    pub folder: PathBuf,
    /// The working file's name without extension (`files::base_name`).
    pub base_name: String,
    /// A source element (or bin) with a `src` pad delivering raw video: `pipewiresrc` in
    /// production.
    pub video: gst::Element,
    pub area: VideoArea,
    /// Up to one element per audio source, raw audio on a `src` pad: `pulsesrc`
    /// (`audio::source_element`) in production.
    pub audio: Vec<(AudioSource, gst::Element)>,
    /// Tests only: an `identity` that fails after this many video buffers.
    fault_after: Option<i32>,
    /// What "now" is for the heartbeat.
    clock: Clock,
}

/// The current time of the sources, for the heartbeat.
#[derive(Clone, Default)]
enum Clock {
    /// The sources pipeline's running time (live sources: `pipewiresrc`, `pulsesrc`).
    #[default]
    Pipeline,
    /// Tests with synthetic timestamps: the running time the test sets (`None`: no heartbeat).
    #[cfg(test)]
    Manual(Arc<Mutex<Option<gst::ClockTime>>>),
}

impl Clock {
    fn now(&self, sources: &gst::Pipeline) -> Option<gst::ClockTime> {
        match self {
            Self::Pipeline => sources.current_running_time(),
            #[cfg(test)]
            Self::Manual(time) => *time.lock().unwrap_or_else(|e| e.into_inner()),
        }
    }
}

impl RecordingConfig {
    /// A recording into `folder`, named after the current local time.
    pub fn new(folder: PathBuf, video: gst::Element, area: VideoArea) -> Self {
        let base_name = glib::DateTime::now_local()
            .map(|now| files::base_name(&now))
            .unwrap_or_else(|_| String::from("Recording"));
        Self {
            folder,
            base_name,
            video,
            area,
            audio: Vec::new(),
            fault_after: None,
            clock: Clock::default(),
        }
    }

    pub fn with_audio(mut self, source: AudioSource, element: gst::Element) -> Self {
        self.audio.retain(|(s, _)| *s != source);
        self.audio.push((source, element));
        self
    }
}

/// Why a recording could not start or stopped early.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RecordingError {
    #[error("the recordings folder cannot be written: {0}")]
    Folder(String),
    #[error("the selected area is not on the chosen screen")]
    RegionNotOnScreen,
    #[error("the selected area is empty")]
    EmptyRegion,
    #[error("the GStreamer element {0} is not installed")]
    MissingElement(String),
    #[error("the recording pipeline could not be built: {0}")]
    Build(String),
    /// An element failed while recording (write error, device gone, …). Element name and error
    /// text only, never content (CLAUDE.md §7).
    #[error("{element}: {message}")]
    Stream { element: String, message: String },
    #[error("the working file was not finished in time")]
    Timeout,
}

/// Something the caller shows: level meters, an error, the end of the video source.
#[derive(Clone, Debug, PartialEq)]
pub enum RecordingEvent {
    /// Loudest channel's peak and RMS in dB (0 = full scale), every 100 ms per source.
    Level {
        source: AudioSource,
        peak_db: f64,
        rms_db: f64,
    },
    /// The recording stopped writing; call [`Recording::stop`] to finish what was written.
    Error(RecordingError),
    /// The video source sent an end of stream (a window stream that ends with one).
    SourceEnded,
}

/// A finished working file.
#[derive(Debug)]
pub struct Finished {
    pub path: PathBuf,
    /// Recorded time without paused time.
    pub duration: gst::ClockTime,
    /// The audio tracks in the file, in track order.
    pub audio: Vec<AudioSource>,
    /// Why the recording ended early, if it did (the file keeps what was written).
    pub error: Option<RecordingError>,
}

/// Pause and mute state over the sources' running time.
#[derive(Default)]
struct Timeline {
    /// Paused intervals [start, end) in source running time; the last may be open.
    pauses: Vec<(gst::ClockTime, Option<gst::ClockTime>)>,
    /// Mute switches per source, in output time (the file's timestamps).
    mutes: [Vec<(gst::ClockTime, bool)>; 2],
    /// The last video frame for the encoder (pushed, or due at resume).
    last_video: Option<LastVideo>,
    /// The newest frame that arrived while paused: the screen as it is at resume.
    paused_video: Option<(gst::Buffer, Option<gst::Caps>)>,
    video_ended: bool,
    audio_ended: [bool; 2],
    /// Stopping or failed: the glue passes nothing on.
    closed: bool,
}

impl Timeline {
    /// Output time of a buffer at running time `rt`, or `None` while paused.
    fn map(&self, rt: gst::ClockTime) -> Option<gst::ClockTime> {
        let mut paused = gst::ClockTime::ZERO;
        for (start, end) in &self.pauses {
            match end {
                _ if rt < *start => {}
                Some(end) if rt >= *end => paused += *end - *start,
                _ => return None,
            }
        }
        Some(rt.saturating_sub(paused))
    }

    /// Output time of `rt`, a moment inside a pause counting as its start.
    fn out_time(&self, rt: gst::ClockTime) -> gst::ClockTime {
        let inside = self
            .pauses
            .iter()
            .find(|(start, end)| rt >= *start && end.is_none_or(|end| rt < end))
            .map(|(start, _)| *start);
        let at = inside.unwrap_or(rt);
        self.map(at).unwrap_or(gst::ClockTime::ZERO)
    }

    fn is_paused(&self) -> bool {
        self.pauses.last().is_some_and(|(_, end)| end.is_none())
    }

    /// What the heartbeat pushes at running time `now`: nothing while frames arrive or while
    /// paused; after [`HEARTBEAT_AFTER`] without one, the frame due at resume (if any) and the
    /// last frame again at the current output time.
    fn heartbeat(&mut self, now: gst::ClockTime) -> Vec<(gst::Buffer, Option<gst::Caps>)> {
        let mut due = Vec::new();
        let Some(now_out) = self.map(now) else {
            return due;
        };
        let Some(last) = self.last_video.as_mut() else {
            return due;
        };
        if now_out < last.out + HEARTBEAT_AFTER {
            return due;
        }
        if !last.pushed {
            last.pushed = true;
            due.push(last.at(last.out));
        }
        // Repeated from here on: Annota's own copy, so the source's buffer pool gets its buffer
        // back while the screen stays still (design.md, risks: P8).
        if !last.owned {
            if let Ok(copy) = last.buffer.copy_deep() {
                last.buffer = copy;
                last.owned = true;
            }
        }
        last.out = now_out;
        due.push(last.at(now_out));
        due
    }

    fn muted(&self, source: AudioSource, out: gst::ClockTime) -> bool {
        self.mutes[source.index()]
            .iter()
            .rev()
            .find(|(at, _)| *at <= out)
            .is_some_and(|(_, muted)| *muted)
    }
}

/// The last video frame and where it is in the file.
struct LastVideo {
    buffer: gst::Buffer,
    caps: Option<gst::Caps>,
    /// Output time it was pushed at (or is due at).
    out: gst::ClockTime,
    /// `false`: the newest paused frame, due at the resume time and not pushed yet.
    pushed: bool,
    /// The memory is Annota's own copy, not a buffer of the source's pool.
    owned: bool,
}

impl LastVideo {
    /// The frame again, stamped `at` (memory shared, not copied).
    fn at(&self, at: gst::ClockTime) -> (gst::Buffer, Option<gst::Caps>) {
        let mut again = self.buffer.copy();
        {
            let again = again.make_mut();
            again.set_pts(at);
            again.set_dts(gst::ClockTime::NONE);
            again.set_duration(gst::ClockTime::NONE);
        }
        (again, self.caps.clone())
    }
}

/// How the encoder ended.
#[derive(Default)]
struct End {
    eos: bool,
    error: Option<RecordingError>,
    /// The error came from the encoder pipeline: it no longer drains, so no end of stream.
    encoder_failed: bool,
}

struct Shared {
    timeline: Mutex<Timeline>,
    /// Held from deciding a video push until it is done: the glue and the heartbeat hand frames
    /// to the encoder in timestamp order.
    video_push: Mutex<()>,
    end: Mutex<End>,
    ended: Condvar,
    events: mpsc::UnboundedSender<RecordingEvent>,
}

impl Shared {
    fn timeline(&self) -> MutexGuard<'_, Timeline> {
        self.timeline.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn end(&self) -> MutexGuard<'_, End> {
        self.end.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn video_push(&self) -> MutexGuard<'_, ()> {
        self.video_push.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn closed(&self) -> bool {
        self.timeline().closed
    }

    fn send(&self, event: RecordingEvent) {
        let _ = self.events.unbounded_send(event);
    }

    /// The first error wins; later ones are consequences. `in_encoder`: the encoder pipeline
    /// failed (it then gets no end of stream).
    fn fail(&self, error: RecordingError, in_encoder: bool) {
        let first = {
            let mut end = self.end();
            end.encoder_failed |= in_encoder;
            let first = end.error.is_none();
            if first {
                end.error = Some(error.clone());
            }
            first
        };
        if first {
            self.timeline().closed = true;
            self.send(RecordingEvent::Error(error));
        }
        self.ended.notify_all();
    }
}

/// A running recording. Main thread; drop without [`stop`](Self::stop) abandons it like a crash
/// (pipelines to `NULL`, no end of stream, the file keeps what was written).
pub struct Recording {
    path: PathBuf,
    sources: gst::Pipeline,
    encoder: gst::Pipeline,
    video_in: gst_app::AppSrc,
    audio_in: Vec<(AudioSource, gst_app::AppSrc)>,
    shared: Arc<Shared>,
    heartbeat: Option<JoinHandle<()>>,
    done: bool,
}

/// Starts recording into a new working file. Events (levels, errors) arrive on the receiver;
/// [`forward_events`] delivers them on the GLib main context.
pub fn start(
    config: RecordingConfig,
) -> Result<(Recording, mpsc::UnboundedReceiver<RecordingEvent>), RecordingError> {
    gst::init().map_err(|err| RecordingError::Build(err.to_string()))?;
    // Known before the stream starts: refuse before anything is written.
    if let VideoArea::Region { selection, stream } = &config.area {
        geometry::region_on_stream(*selection, *stream).map_err(|err| match err {
            RegionError::NotOnScreen => RecordingError::RegionNotOnScreen,
            RegionError::Empty => RecordingError::EmptyRegion,
        })?;
    }
    let (path, file) = files::create_working_file(&config.folder, &config.base_name)
        .map_err(|err| RecordingError::Folder(err.to_string()))?;
    drop(file);
    let (sender, receiver) = mpsc::unbounded();
    let shared = Arc::new(Shared {
        timeline: Mutex::new(Timeline::default()),
        video_push: Mutex::new(()),
        end: Mutex::new(End::default()),
        ended: Condvar::new(),
        events: sender,
    });
    let clock = config.clock.clone();
    match build(config, &path, &shared) {
        Ok(mut recording) => {
            let started = recording
                .encoder
                .set_state(gst::State::Playing)
                .and_then(|_| recording.sources.set_state(gst::State::Playing));
            if started.is_err() {
                let error = shared.end().error.clone().unwrap_or_else(|| {
                    RecordingError::Build(String::from("the pipeline did not start"))
                });
                drop(recording);
                let _ = std::fs::remove_file(&path);
                return Err(error);
            }
            recording.heartbeat = spawn_heartbeat(&recording, clock);
            Ok((recording, receiver))
        }
        Err(err) => {
            let _ = std::fs::remove_file(&path);
            Err(err)
        }
    }
}

/// Calls `handler` on the thread-default GLib main context for every event of a recording.
pub fn forward_events(
    mut events: mpsc::UnboundedReceiver<RecordingEvent>,
    mut handler: impl FnMut(RecordingEvent) + 'static,
) {
    use futures_util::StreamExt;
    glib::spawn_future_local(async move {
        while let Some(event) = events.next().await {
            handler(event);
        }
    });
}

fn make(factory: &str, name: &str) -> Result<gst::Element, RecordingError> {
    gst::ElementFactory::make(factory)
        .name(name)
        .build()
        .map_err(|_| RecordingError::MissingElement(factory.to_string()))
}

fn add_and_link(pipeline: &gst::Pipeline, chain: &[&gst::Element]) -> Result<(), RecordingError> {
    pipeline
        .add_many(chain.iter().copied())
        .map_err(|err| RecordingError::Build(err.to_string()))?;
    gst::Element::link_many(chain.iter().copied())
        .map_err(|err| RecordingError::Build(err.to_string()))
}

/// An encoder input. Never blocks: the glue waits for room itself ([`push`]), so a stop can
/// always end the wait.
fn encoder_input(name: &str) -> gst_app::AppSrc {
    gst_app::AppSrc::builder()
        .name(name)
        .format(gst::Format::Time)
        .is_live(false)
        .block(false)
        .max_bytes(QUEUE_BYTES)
        .build()
}

fn build(
    config: RecordingConfig,
    path: &Path,
    shared: &Arc<Shared>,
) -> Result<Recording, RecordingError> {
    let encoder = gst::Pipeline::with_name("recording-encoder");
    let sources = gst::Pipeline::with_name("recording-sources");

    // Encoder: muxer and file.
    let mux = make("matroskamux", "mux")?;
    let sink = gst::ElementFactory::make("filesink")
        .name("file")
        .property("location", path.to_string_lossy().as_ref())
        .property("sync", false)
        .build()
        .map_err(|_| RecordingError::MissingElement(String::from("filesink")))?;
    // Every write reaches the file at once: nothing is held in a userspace buffer that a crash
    // would lose ("Recording survives a crash").
    sink.set_property_from_str("buffer-mode", "unbuffered");
    add_and_link(&encoder, &[&mux, &sink])?;

    // Encoder: video.
    let video_in = encoder_input("video-in");
    let crop = make("videocrop", "crop")?;
    let convert = make("videoconvert", "convert")?;
    let scale = make("videoscale", "scale")?;
    scale.set_property("add-borders", true);
    let size = make("capsfilter", "size")?;
    size.set_property("caps", frame_caps(None));
    let rate = make("videorate", "rate")?;
    let fps = make("capsfilter", "fps")?;
    fps.set_property(
        "caps",
        gst::Caps::builder("video/x-raw")
            .field("framerate", gst::Fraction::new(FRAME_RATE, 1))
            .build(),
    );
    let x264 = make("x264enc", "x264")?;
    x264.set_property_from_str("speed-preset", "superfast");
    x264.set_property_from_str("pass", "quant");
    x264.set_property("quantizer", QUANTIZER);
    x264.set_property("key-int-max", KEY_INT_MAX);
    let mut chain: Vec<&gst::Element> = vec![video_in.upcast_ref()];
    let fault = match config.fault_after {
        Some(n) => {
            let identity = make("identity", "fault")?;
            identity.set_property("error-after", n);
            Some(identity)
        }
        None => None,
    };
    if let Some(identity) = &fault {
        chain.push(identity);
    }
    chain.extend([&crop, &convert, &scale, &size, &rate, &fps, &x264]);
    add_and_link(&encoder, &chain)?;
    x264.link(&mux)
        .map_err(|err| RecordingError::Build(err.to_string()))?;
    let cropper = Arc::new(Mutex::new(Cropper {
        area: config.area,
        crop: crop.downgrade(),
        size: size.downgrade(),
        info: None,
        current: None,
        frame: None,
    }));
    if let Some(pad) = crop.static_pad("sink") {
        pad.add_probe(
            gst::PadProbeType::BUFFER | gst::PadProbeType::EVENT_DOWNSTREAM,
            move |_, info| {
                let mut cropper = cropper.lock().unwrap_or_else(|e| e.into_inner());
                match &mut info.data {
                    Some(gst::PadProbeData::Event(event)) => {
                        if let gst::EventView::Caps(caps) = event.view() {
                            cropper.on_caps(caps.caps());
                        }
                    }
                    Some(gst::PadProbeData::Buffer(buffer)) => {
                        if !cropper.on_buffer(buffer) {
                            // A window whose content rectangle is not known yet.
                            return gst::PadProbeReturn::Drop;
                        }
                        // The rectangle above already used the stream's crop meta; left on the
                        // buffer, `videocrop` would apply it again on top of its own crop
                        // (Phase B, a window stream's meta at a non-zero origin).
                        if buffer.meta::<gst_video::VideoCropMeta>().is_some() {
                            if let Some(meta) =
                                buffer.make_mut().meta_mut::<gst_video::VideoCropMeta>()
                            {
                                let _ = meta.remove();
                            }
                        }
                    }
                    _ => {}
                }
                gst::PadProbeReturn::Ok
            },
        );
    }

    // Sources: video.
    let video_out = gst_app::AppSink::builder()
        .name("video-out")
        // System memory only: pipewiresrc then negotiates shared memory, not DMA-BUF.
        .caps(&gst::Caps::builder("video/x-raw").build())
        .sync(false)
        // No preroll: one source must never wait for the others' first buffer.
        .async_(false)
        .build();
    sources
        .add_many([&config.video, video_out.upcast_ref()])
        .map_err(|err| RecordingError::Build(err.to_string()))?;
    config
        .video
        .link(&video_out)
        .map_err(|err| RecordingError::Build(err.to_string()))?;
    {
        let shared_sample = Arc::downgrade(shared);
        let shared_eos = Arc::downgrade(shared);
        let input = video_in.clone();
        video_out.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |sink| {
                    let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                    glue_video(&shared_sample, &input, sample)
                })
                .eos(move |_| {
                    if let Some(shared) = shared_eos.upgrade() {
                        shared.timeline().video_ended = true;
                        shared.send(RecordingEvent::SourceEnded);
                    }
                })
                .build(),
        );
    }

    // Audio: one track per source.
    let mut audio_in = Vec::new();
    for (source, element) in config.audio {
        let tag = source.tag();
        let input = encoder_input(&format!("audio-in-{tag}"));
        let volume = make("volume", &format!("volume-{tag}"))?;
        let level = make("level", &format!("level-{tag}"))?;
        let aconvert = make("audioconvert", &format!("aconvert-{tag}"))?;
        let resample = make("audioresample", &format!("resample-{tag}"))?;
        let opus = make("opusenc", &format!("opus-{tag}"))?;
        add_and_link(
            &encoder,
            &[
                input.upcast_ref(),
                &volume,
                &level,
                &aconvert,
                &resample,
                &opus,
            ],
        )?;
        opus.link(&mux)
            .map_err(|err| RecordingError::Build(err.to_string()))?;
        if let Some(pad) = volume.static_pad("sink") {
            let shared = Arc::downgrade(shared);
            let volume = volume.downgrade();
            pad.add_probe(gst::PadProbeType::BUFFER, move |_, info| {
                if let (Some(gst::PadProbeData::Buffer(buffer)), Some(shared), Some(volume)) =
                    (&info.data, shared.upgrade(), volume.upgrade())
                {
                    if let Some(pts) = buffer.pts() {
                        let muted = shared.timeline().muted(source, pts);
                        if volume.property::<bool>("mute") != muted {
                            volume.set_property("mute", muted);
                        }
                    }
                }
                gst::PadProbeReturn::Ok
            });
        }

        let output = gst_app::AppSink::builder()
            .name(format!("audio-out-{tag}"))
            .caps(&gst::Caps::builder("audio/x-raw").build())
            .sync(false)
            .async_(false)
            .build();
        sources
            .add_many([&element, output.upcast_ref()])
            .map_err(|err| RecordingError::Build(err.to_string()))?;
        element
            .link(&output)
            .map_err(|err| RecordingError::Build(err.to_string()))?;
        let shared_sample = Arc::downgrade(shared);
        let shared_eos = Arc::downgrade(shared);
        let push_to = input.clone();
        output.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample(move |sink| {
                    let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                    glue_audio(&shared_sample, &push_to, &sample)
                })
                .eos(move |_| {
                    if let Some(shared) = shared_eos.upgrade() {
                        shared.timeline().audio_ended[source.index()] = true;
                    }
                })
                .build(),
        );
        audio_in.push((source, input));
    }

    for (pipeline, is_encoder) in [(&encoder, true), (&sources, false)] {
        if let Some(bus) = pipeline.bus() {
            let shared = Arc::downgrade(shared);
            bus.set_sync_handler(move |_, message| {
                if let Some(shared) = shared.upgrade() {
                    on_message(&shared, message, is_encoder);
                }
                gst::BusSyncReply::Drop
            });
        }
    }

    Ok(Recording {
        path: path.to_path_buf(),
        sources,
        encoder,
        video_in,
        audio_in,
        shared: Arc::clone(shared),
        heartbeat: None,
        done: false,
    })
}

/// The heartbeat (design.md "Decided during apply", still screen): while no video buffer
/// arrives, the last frame again at the current time, so `matroskamux` (which writes only while
/// every track has data) and with it the audio keep advancing. Ends when the recording closes.
fn spawn_heartbeat(recording: &Recording, clock: Clock) -> Option<JoinHandle<()>> {
    let shared = Arc::downgrade(&recording.shared);
    let input = recording.video_in.clone();
    let sources = recording.sources.clone();
    let spawned = std::thread::Builder::new()
        .name(String::from("recording-heartbeat"))
        .spawn(move || loop {
            std::thread::sleep(HEARTBEAT_TICK);
            let Some(shared) = shared.upgrade() else {
                return;
            };
            if shared.closed() {
                return;
            }
            let Some(now) = clock.now(&sources) else {
                continue;
            };
            let _order = shared.video_push();
            let due = {
                let mut timeline = shared.timeline();
                // Stopped while waiting for the glue: the stop pushes the last frame itself.
                if timeline.closed {
                    return;
                }
                timeline.heartbeat(now)
            };
            for (buffer, caps) in due {
                if push(&input, &buffer, caps.as_ref(), || shared.closed()).is_err() {
                    break;
                }
            }
        });
    match spawned {
        Ok(handle) => Some(handle),
        Err(err) => {
            eprintln!("annota: recording: no heartbeat thread: {err}");
            None
        }
    }
}

/// 20 ms of silence for a track whose source never delivered anything.
fn silent_track() -> gst::Sample {
    const RATE: usize = 48_000;
    let caps = gst::Caps::builder("audio/x-raw")
        .field("format", "S16LE")
        .field("rate", RATE as i32)
        .field("channels", 1i32)
        .field("layout", "interleaved")
        .build();
    let mut buffer = gst::Buffer::from_mut_slice(vec![0u8; RATE / 50 * 2]);
    if let Some(buffer) = buffer.get_mut() {
        buffer.set_pts(gst::ClockTime::ZERO);
        buffer.set_duration(gst::ClockTime::from_mseconds(20));
    }
    gst::Sample::builder().buffer(&buffer).caps(&caps).build()
}

/// `video/x-raw,format=I420,pixel-aspect-ratio=1/1` and, once known, the fixed frame size.
fn frame_caps(size: Option<Size>) -> gst::Caps {
    let mut caps = gst::Caps::builder("video/x-raw")
        .field("format", "I420")
        .field("pixel-aspect-ratio", gst::Fraction::new(1, 1));
    if let Some(size) = size {
        caps = caps.field("width", size.w).field("height", size.h);
    }
    caps.build()
}

/// Sets `videocrop` and the frame size from the buffers (design.md D2).
struct Cropper {
    area: VideoArea,
    crop: glib::WeakRef<gst::Element>,
    size: glib::WeakRef<gst::Element>,
    info: Option<gst_video::VideoInfo>,
    current: Option<IRect>,
    frame: Option<Size>,
}

impl Cropper {
    fn on_caps(&mut self, caps: &gst::CapsRef) {
        let Ok(info) = gst_video::VideoInfo::from_caps(caps) else {
            return;
        };
        let buffer = Size::new(
            i32::try_from(info.width()).unwrap_or(0),
            i32::try_from(info.height()).unwrap_or(0),
        );
        self.info = Some(info);
        let whole = geometry::even_rect(IRect::new(0, 0, buffer.w, buffer.h));
        let rect = match &self.area {
            VideoArea::Whole => whole,
            VideoArea::Region { selection, stream } => {
                // Checked in `start`; a stream of another size than announced records whole.
                geometry::region_crop(*selection, *stream, buffer).unwrap_or(whole)
            }
            VideoArea::Window(_) => return,
        };
        self.apply(rect, buffer);
    }

    /// `false`: drop the buffer (a window whose first content rectangle is not known yet, so
    /// the frame size is not either).
    fn on_buffer(&mut self, buffer: &gst::BufferRef) -> bool {
        let VideoArea::Window(content) = &self.area else {
            return true;
        };
        let Some(info) = &self.info else {
            return false;
        };
        let size = Size::new(
            i32::try_from(info.width()).unwrap_or(0),
            i32::try_from(info.height()).unwrap_or(0),
        );
        let Some(rect) = content(buffer, info)
            .and_then(|r| r.intersect(IRect::new(0, 0, size.w, size.h)))
            .or(self.current)
        else {
            return false;
        };
        self.apply(rect, size);
        true
    }

    fn apply(&mut self, rect: IRect, buffer: Size) {
        let even = geometry::even_rect(rect);
        let frame = match self.frame {
            Some(frame) => frame,
            None => {
                let frame = Size::new(even.w.max(2), even.h.max(2));
                self.frame = Some(frame);
                if let Some(size) = self.size.upgrade() {
                    size.set_property("caps", frame_caps(Some(frame)));
                }
                frame
            }
        };
        // A rectangle of the frame's size once trimmed to even is cropped to exactly that: no
        // resampling by a pixel (`videoscale` would stretch an odd width by 0.999).
        let rect = if Size::new(even.w, even.h) == frame {
            even
        } else {
            rect
        };
        if self.current == Some(rect) {
            return;
        }
        self.current = Some(rect);
        if let Some(crop) = self.crop.upgrade() {
            crop.set_property("left", rect.x);
            crop.set_property("top", rect.y);
            crop.set_property("right", (buffer.w - rect.right()).max(0));
            crop.set_property("bottom", (buffer.h - rect.bottom()).max(0));
        }
    }
}

/// The running time of a sample's buffer in its segment.
fn running_time(sample: &gst::Sample) -> Option<gst::ClockTime> {
    let pts = sample.buffer()?.pts()?;
    match sample.segment() {
        Some(segment) => segment
            .downcast_ref::<gst::ClockTime>()
            .and_then(|segment| segment.to_running_time(pts)),
        None => Some(pts),
    }
}

/// `sample`'s buffer restamped to `out` (memory shared, not copied).
fn restamp(sample: &gst::Sample, out: gst::ClockTime) -> Option<gst::Buffer> {
    let mut buffer = sample.buffer()?.copy();
    {
        let buffer = buffer.make_mut();
        buffer.set_pts(out);
        buffer.set_dts(gst::ClockTime::NONE);
        buffer.set_duration(gst::ClockTime::NONE);
    }
    Some(buffer)
}

/// Hands `buffer` to an encoder input, waiting while its queue is full (backpressure) until
/// there is room or `give_up` says so (stopping); then the buffer is dropped.
fn push(
    input: &gst_app::AppSrc,
    buffer: &gst::Buffer,
    caps: Option<&gst::Caps>,
    give_up: impl Fn() -> bool,
) -> Result<gst::FlowSuccess, gst::FlowError> {
    while input.current_level_bytes() >= QUEUE_BYTES {
        if give_up() {
            return Ok(gst::FlowSuccess::Ok);
        }
        std::thread::sleep(QUEUE_POLL);
    }
    let mut sample = gst::Sample::builder().buffer(buffer);
    if let Some(caps) = caps {
        sample = sample.caps(caps);
    }
    match input.push_sample(&sample.build()) {
        // The encoder is stopping: the source keeps running until it is stopped too.
        Err(gst::FlowError::Flushing) | Err(gst::FlowError::Eos) => Ok(gst::FlowSuccess::Ok),
        other => other,
    }
}

fn glue_video(
    shared: &Weak<Shared>,
    input: &gst_app::AppSrc,
    sample: gst::Sample,
) -> Result<gst::FlowSuccess, gst::FlowError> {
    let Some(shared) = shared.upgrade() else {
        return Err(gst::FlowError::Flushing);
    };
    let Some(rt) = running_time(&sample) else {
        return Ok(gst::FlowSuccess::Ok);
    };
    let _order = shared.video_push();
    let due = {
        let mut timeline = shared.timeline();
        if timeline.closed {
            return Ok(gst::FlowSuccess::Ok);
        }
        let Some(out) = timeline.map(rt) else {
            // Paused: nothing is recorded, but the newest frame is the screen at resume.
            if let Some(buffer) = sample.buffer_owned() {
                timeline.paused_video = Some((buffer, sample.caps_owned()));
            }
            return Ok(gst::FlowSuccess::Ok);
        };
        let mut due = Vec::new();
        let mut out = out;
        let previous = match timeline.last_video.take() {
            // The frame from the pause, shown from the resume until this one, unless this one
            // follows at once.
            Some(mut paused) if !paused.pushed => (out > paused.out + FRAME / 2).then(|| {
                due.push(paused.at(paused.out));
                paused.pushed = true;
                paused
            }),
            other => other,
        };
        if let Some(previous) = &previous {
            // The heartbeat repeated the last frame at a time this buffer did not reach yet.
            if out <= previous.out {
                out = previous.out + gst::ClockTime::from_nseconds(1);
            }
            if let Some(again) = held_until(previous, out) {
                due.push(again);
            }
        }
        let Some(buffer) = restamp(&sample, out) else {
            timeline.last_video = previous;
            return Ok(gst::FlowSuccess::Ok);
        };
        let caps = sample.caps_owned();
        due.push((buffer.clone(), caps.clone()));
        timeline.last_video = Some(LastVideo {
            buffer,
            caps,
            out,
            pushed: true,
            owned: false,
        });
        due
    };
    for (buffer, caps) in due {
        push(input, &buffer, caps.as_ref(), || shared.closed())?;
    }
    Ok(gst::FlowSuccess::Ok)
}

/// After a gap (a still screen sends no buffers), the previous frame again, stamped one frame
/// before the next one. `videorate` fills a gap with whichever neighbour is closer in time, so
/// without it the second half of a gap shorter than the heartbeat's (or every gap, where time is
/// synthetic) would show the frame that ends it.
fn held_until(last: &LastVideo, next: gst::ClockTime) -> Option<(gst::Buffer, Option<gst::Caps>)> {
    let at = next.checked_sub(FRAME)?;
    if at <= last.out + FRAME / 2 {
        return None;
    }
    Some(last.at(at))
}

fn glue_audio(
    shared: &Weak<Shared>,
    input: &gst_app::AppSrc,
    sample: &gst::Sample,
) -> Result<gst::FlowSuccess, gst::FlowError> {
    let Some(shared) = shared.upgrade() else {
        return Err(gst::FlowError::Flushing);
    };
    let Some(rt) = running_time(sample) else {
        return Ok(gst::FlowSuccess::Ok);
    };
    let buffer = {
        let timeline = shared.timeline();
        let Some(out) = timeline.map(rt).filter(|_| !timeline.closed) else {
            return Ok(gst::FlowSuccess::Ok);
        };
        restamp(sample, out)
    };
    match buffer {
        Some(buffer) => push(input, &buffer, sample.caps_owned().as_ref(), || {
            shared.closed()
        }),
        None => Ok(gst::FlowSuccess::Ok),
    }
}

/// Bus messages of both pipelines, on the thread that posts them.
///
/// Only the encoder's end of stream means the file is finished; the sources' one is reported
/// by the video appsink ([`RecordingEvent::SourceEnded`]).
fn on_message(shared: &Shared, message: &gst::Message, from_encoder: bool) {
    match message.view() {
        gst::MessageView::Eos(_) if from_encoder => {
            shared.end().eos = true;
            shared.ended.notify_all();
        }
        gst::MessageView::Error(err) => {
            let element = message
                .src()
                .map(|src| src.name().to_string())
                .unwrap_or_default();
            let message = err.error().to_string();
            eprintln!("annota: recording error in {element}: {message}");
            shared.fail(RecordingError::Stream { element, message }, from_encoder);
        }
        gst::MessageView::Element(element) => {
            if let Some(event) = level_event(message, element.structure()) {
                shared.send(event);
            }
        }
        _ => {}
    }
}

/// A `level` element message as a [`RecordingEvent::Level`] of the loudest channel.
pub(super) fn level_event(
    message: &gst::Message,
    structure: Option<&gst::StructureRef>,
) -> Option<RecordingEvent> {
    let structure = structure.filter(|s| s.name() == "level")?;
    let name = message.src()?.name();
    let source = AudioSource::ALL
        .into_iter()
        .find(|source| name == format!("level-{}", source.tag()))?;
    let loudest = |field: &str| {
        structure
            .get::<glib::ValueArray>(field)
            .ok()
            .and_then(|values| {
                values
                    .iter()
                    .filter_map(|v| v.get::<f64>().ok())
                    .reduce(f64::max)
            })
    };
    Some(RecordingEvent::Level {
        source,
        peak_db: loudest("peak")?,
        rms_db: loudest("rms")?,
    })
}

impl Recording {
    /// The working file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The audio sources being recorded, in track order.
    pub fn audio_sources(&self) -> Vec<AudioSource> {
        self.audio_in.iter().map(|(s, _)| *s).collect()
    }

    /// The sources' running time now.
    fn now(&self) -> gst::ClockTime {
        self.sources
            .current_running_time()
            .unwrap_or(gst::ClockTime::ZERO)
    }

    /// Recorded time so far, without paused time; frozen while paused.
    pub fn elapsed(&self) -> gst::ClockTime {
        let now = self.now();
        self.shared.timeline().out_time(now)
    }

    pub fn is_paused(&self) -> bool {
        self.shared.timeline().is_paused()
    }

    /// Stops writing video and audio until [`resume`](Self::resume) (D5).
    pub fn pause(&self) {
        self.pause_at(self.now());
    }

    pub fn resume(&self) {
        self.resume_at(self.now());
    }

    /// Writes silence for `source` from now on (`volume mute`), or sound again.
    pub fn set_muted(&self, source: AudioSource, muted: bool) {
        self.set_muted_at(source, self.now(), muted);
    }

    pub fn is_muted(&self, source: AudioSource) -> bool {
        self.shared
            .timeline()
            .mutes
            .get(source.index())
            .and_then(|m| m.last())
            .is_some_and(|(_, muted)| *muted)
    }

    fn pause_at(&self, rt: gst::ClockTime) {
        let mut timeline = self.shared.timeline();
        let after_last = timeline
            .pauses
            .last()
            .is_none_or(|(_, end)| end.is_some_and(|end| rt >= end));
        if after_last {
            timeline.pauses.push((rt, None));
        }
    }

    fn resume_at(&self, rt: gst::ClockTime) {
        let mut timeline = self.shared.timeline();
        let Some((start, end @ None)) = timeline.pauses.last_mut() else {
            return;
        };
        let resumed = rt.max(*start);
        *end = Some(resumed);
        // The screen changed while paused and may stand still from now on: its newest frame is
        // due at the resume time (the glue, the heartbeat or the stop pushes it).
        if let Some((buffer, caps)) = timeline.paused_video.take() {
            let out = timeline.out_time(resumed);
            timeline.last_video = Some(LastVideo {
                buffer,
                caps,
                out,
                pushed: false,
                owned: false,
            });
        }
    }

    fn set_muted_at(&self, source: AudioSource, rt: gst::ClockTime, muted: bool) {
        let mut timeline = self.shared.timeline();
        let out = timeline.out_time(rt);
        timeline.mutes[source.index()].push((out, muted));
    }

    /// Ends the recording and finishes the working file, without waiting for the source to end
    /// (P5/P6): writing stops now; the sources stop, the last frame is repeated up to now and the
    /// encoder gets its end of stream from here, on a worker thread. The [`Finished`] arrives
    /// through the returned future (poll it on the GLib main context) within `FINISH_TIMEOUT`,
    /// with [`RecordingError::Timeout`] if the file could not be finished in time. After an
    /// error, what was written is kept as it is.
    pub fn stop(self) -> impl Future<Output = Finished> + 'static {
        let now = self.now();
        self.stop_at(now)
    }

    fn stop_at(self, rt: gst::ClockTime) -> impl Future<Output = Finished> + 'static {
        let finisher = self.finisher(rt);
        let late = finisher.late();
        let (sender, receiver) = oneshot::channel();
        let spawned = std::thread::Builder::new()
            .name(String::from("recording-finish"))
            .spawn(move || {
                let _ = sender.send(finisher.run());
            });
        if let Err(err) = &spawned {
            // The finisher went down with the closure and stopped the pipelines.
            eprintln!("annota: recording: no thread to finish the working file: {err}");
        }
        async move {
            use futures_util::future::{select, Either};
            // The worker keeps to FINISH_TIMEOUT itself; this bounds a hang in a state change.
            let backstop = glib::timeout_future(FINISH_TIMEOUT + Duration::from_secs(1));
            match select(receiver, backstop).await {
                Either::Left((Ok(finished), _)) => finished,
                _ => {
                    eprintln!("annota: recording: the working file was not finished in time");
                    late
                }
            }
        }
    }

    /// Stopping, on the calling thread: nothing more is written from now on.
    fn finisher(mut self, rt: gst::ClockTime) -> Finisher {
        self.done = true;
        let stop_out = {
            let mut timeline = self.shared.timeline();
            timeline.closed = true;
            timeline.out_time(rt)
        };
        Finisher {
            path: self.path.clone(),
            sources: self.sources.clone(),
            encoder: self.encoder.clone(),
            video_in: self.video_in.clone(),
            audio_in: self.audio_in.clone(),
            shared: Arc::clone(&self.shared),
            heartbeat: self.heartbeat.take(),
            stop_out,
            deadline: Instant::now() + FINISH_TIMEOUT,
            ran: false,
        }
    }

    /// Tests: stops and finishes on this thread.
    #[cfg(test)]
    fn stop_now(self, rt: gst::ClockTime) -> Finished {
        self.finisher(rt).run()
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        if !self.done {
            // Abandoned: like a crash, no end of stream; the file keeps what was written.
            self.shared.timeline().closed = true;
            let _ = self.encoder.set_state(gst::State::Null);
            let _ = self.sources.set_state(gst::State::Null);
            unset_bus_handlers(&[&self.encoder, &self.sources]);
        }
    }
}

fn unset_bus_handlers(pipelines: &[&gst::Pipeline]) {
    for pipeline in pipelines {
        if let Some(bus) = pipeline.bus() {
            bus.unset_sync_handler();
        }
    }
}

/// Finishes a stopped recording's working file (worker thread).
struct Finisher {
    path: PathBuf,
    sources: gst::Pipeline,
    encoder: gst::Pipeline,
    video_in: gst_app::AppSrc,
    audio_in: Vec<(AudioSource, gst_app::AppSrc)>,
    shared: Arc<Shared>,
    heartbeat: Option<JoinHandle<()>>,
    /// The file's end: the output time of the stop.
    stop_out: gst::ClockTime,
    deadline: Instant,
    ran: bool,
}

impl Finisher {
    fn audio_sources(&self) -> Vec<AudioSource> {
        self.audio_in.iter().map(|(s, _)| *s).collect()
    }

    /// What is delivered if finishing takes longer than `FINISH_TIMEOUT`.
    fn late(&self) -> Finished {
        let duration = self
            .shared
            .timeline()
            .last_video
            .as_ref()
            .map_or(gst::ClockTime::ZERO, |last| last.out.min(self.stop_out));
        Finished {
            path: self.path.clone(),
            duration,
            audio: self.audio_sources(),
            error: Some(RecordingError::Timeout),
        }
    }

    fn run(mut self) -> Finished {
        self.ran = true;
        let encoder_failed = self.shared.end().encoder_failed;
        if encoder_failed {
            // The encoder no longer drains: stop it first, so no push waits on it.
            let _ = self.encoder.set_state(gst::State::Null);
        }
        // Closed: the heartbeat and every waiting glue push give up at once.
        if let Some(heartbeat) = self.heartbeat.take() {
            let _ = heartbeat.join();
        }
        let _ = self.sources.set_state(gst::State::Null);
        // Read only now: no glue push is in flight any more.
        let last = self.shared.timeline().last_video.take();
        if !encoder_failed {
            // Audio first: the muxer may be waiting for an audio track that delivers nothing,
            // with the video queue full behind it.
            for (_, input) in &self.audio_in {
                if input.caps().is_none() {
                    // A source that never delivered: `matroskamux` writes nothing until every
                    // track has its format, which the encoder learns only from data. 20 ms of
                    // silence at the start make it an empty track.
                    let _ = input.push_sample(&silent_track());
                }
                let _ = input.end_of_stream();
            }
            let deadline = self.deadline;
            let late = move || Instant::now() >= deadline;
            if let Some(last) = &last {
                let mut due = Vec::new();
                if !last.pushed && last.out < self.stop_out {
                    due.push(last.at(last.out));
                }
                // A still screen at the end: the last frame again at the stop time.
                if last.out < self.stop_out {
                    due.push(last.at(self.stop_out));
                }
                for (buffer, caps) in due {
                    let _ = push(&self.video_in, &buffer, caps.as_ref(), late);
                }
            }
            let _ = self.video_in.end_of_stream();
            let end = self.shared.end();
            let wait = self.deadline.saturating_duration_since(Instant::now());
            let (end, _) = self
                .shared
                .ended
                .wait_timeout_while(end, wait, |end| !end.eos && !end.encoder_failed)
                .unwrap_or_else(|e| e.into_inner());
            let timed_out = !end.eos && !end.encoder_failed;
            drop(end);
            if timed_out {
                eprintln!("annota: recording: the working file was not finished in time");
                self.shared.fail(RecordingError::Timeout, true);
            }
        }
        let _ = self.encoder.set_state(gst::State::Null);
        unset_bus_handlers(&[&self.encoder, &self.sources]);
        let (eos, error) = {
            let end = self.shared.end();
            (end.eos, end.error.clone())
        };
        // Finished with its end of stream: the file reaches the stop time (an error in a source
        // ended the writing early, but the end was still written). Otherwise up to the last frame.
        let duration = if eos {
            self.stop_out
        } else {
            last.as_ref()
                .map_or(gst::ClockTime::ZERO, |last| last.out.min(self.stop_out))
        };
        Finished {
            path: self.path.clone(),
            duration,
            audio: self.audio_sources(),
            error,
        }
    }
}

impl Drop for Finisher {
    fn drop(&mut self) {
        if !self.ran {
            let _ = self.encoder.set_state(gst::State::Null);
            let _ = self.sources.set_state(gst::State::Null);
            unset_bus_handlers(&[&self.encoder, &self.sources]);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
