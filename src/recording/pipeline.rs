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
//! (nothing on a still screen, bursts, a variable rate) into a constant 30 fps.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::time::Duration;

use futures_channel::mpsc;
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
/// How long [`Recording::stop`] waits for the encoder to write the end of the file.
const FINISH_TIMEOUT: Duration = Duration::from_secs(10);
/// Most bytes queued in front of each encoder input before the glue waits (backpressure: a live
/// source then drops frames itself instead of Annota's memory growing). Two 4K BGRx frames.
const QUEUE_BYTES: u64 = 2 * 3840 * 2160 * 4;
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
    /// The last video buffer pushed to the encoder, with its caps and output time.
    last_video: Option<(gst::Buffer, Option<gst::Caps>, gst::ClockTime)>,
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

    fn muted(&self, source: AudioSource, out: gst::ClockTime) -> bool {
        self.mutes[source.index()]
            .iter()
            .rev()
            .find(|(at, _)| *at <= out)
            .is_some_and(|(_, muted)| *muted)
    }
}

/// How the encoder ended.
#[derive(Default)]
struct End {
    eos: bool,
    error: Option<RecordingError>,
}

struct Shared {
    timeline: Mutex<Timeline>,
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

    fn send(&self, event: RecordingEvent) {
        let _ = self.events.unbounded_send(event);
    }

    /// The first error wins; later ones are consequences.
    fn fail(&self, error: RecordingError) {
        let first = {
            let mut end = self.end();
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
        end: Mutex::new(End::default()),
        ended: Condvar::new(),
        events: sender,
    });
    match build(config, &path, &shared) {
        Ok(recording) => {
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

fn encoder_input(name: &str) -> gst_app::AppSrc {
    gst_app::AppSrc::builder()
        .name(name)
        .format(gst::Format::Time)
        .is_live(false)
        .block(true)
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
                        cropper.on_buffer(buffer);
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
                    glue_video(&shared_sample, &input, &sample)
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
        done: false,
    })
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

    fn on_buffer(&mut self, buffer: &gst::BufferRef) {
        let (VideoArea::Window(content), Some(info)) = (&self.area, &self.info) else {
            return;
        };
        let size = Size::new(
            i32::try_from(info.width()).unwrap_or(0),
            i32::try_from(info.height()).unwrap_or(0),
        );
        let Some(rect) = content(buffer, info)
            .and_then(|r| r.intersect(IRect::new(0, 0, size.w, size.h)))
            .or(self.current)
        else {
            return;
        };
        self.apply(rect, size);
    }

    fn apply(&mut self, rect: IRect, buffer: Size) {
        if self.frame.is_none() {
            let first = geometry::even_rect(rect);
            let frame = Size::new(first.w.max(2), first.h.max(2));
            self.frame = Some(frame);
            if let Some(size) = self.size.upgrade() {
                size.set_property("caps", frame_caps(Some(frame)));
            }
        }
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

fn push(
    input: &gst_app::AppSrc,
    buffer: &gst::Buffer,
    caps: Option<&gst::Caps>,
) -> Result<gst::FlowSuccess, gst::FlowError> {
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
    sample: &gst::Sample,
) -> Result<gst::FlowSuccess, gst::FlowError> {
    let Some(shared) = shared.upgrade() else {
        return Err(gst::FlowError::Flushing);
    };
    let Some(rt) = running_time(sample) else {
        return Ok(gst::FlowSuccess::Ok);
    };
    let (buffer, held) = {
        let mut timeline = shared.timeline();
        let Some(out) = timeline.map(rt).filter(|_| !timeline.closed) else {
            return Ok(gst::FlowSuccess::Ok);
        };
        let Some(buffer) = restamp(sample, out) else {
            return Ok(gst::FlowSuccess::Ok);
        };
        let held = timeline
            .last_video
            .replace((buffer.clone(), sample.caps_owned(), out))
            .and_then(|last| held_until(last, out));
        (buffer, held)
    };
    if let Some((again, caps)) = held {
        push(input, &again, caps.as_ref())?;
    }
    push(input, &buffer, sample.caps_owned().as_ref())
}

/// After a gap (a still screen sends no buffers), the previous frame again, stamped one frame
/// before the next one. `videorate` fills a gap with whichever neighbour is closer in time, so
/// without it the second half of a still period would show the frame that ends it.
fn held_until(
    last: (gst::Buffer, Option<gst::Caps>, gst::ClockTime),
    next: gst::ClockTime,
) -> Option<(gst::Buffer, Option<gst::Caps>)> {
    let (buffer, caps, out) = last;
    let frame = gst::ClockTime::SECOND / FRAME_RATE as u64;
    let at = next.checked_sub(frame)?;
    if at <= out + frame / 2 {
        return None;
    }
    let mut again = buffer.copy();
    again.make_mut().set_pts(at);
    Some((again, caps))
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
        Some(buffer) => push(input, &buffer, sample.caps_owned().as_ref()),
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
            shared.fail(RecordingError::Stream { element, message });
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
fn level_event(
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
        if let Some((start, end @ None)) = timeline.pauses.last_mut() {
            *end = Some(rt.max(*start));
        }
    }

    fn set_muted_at(&self, source: AudioSource, rt: gst::ClockTime, muted: bool) {
        let mut timeline = self.shared.timeline();
        let out = timeline.out_time(rt);
        timeline.mutes[source.index()].push((out, muted));
    }

    /// Ends the recording and finishes the working file, without waiting for the source to end
    /// (P5/P6): the sources stop, the last frame is repeated up to now, the encoder gets its end
    /// of stream from here. After an error, what was written is kept as it is.
    pub fn stop(self) -> Finished {
        let now = self.now();
        self.stop_at(now)
    }

    fn stop_at(mut self, rt: gst::ClockTime) -> Finished {
        self.done = true;
        let (stop_out, last) = {
            let mut timeline = self.shared.timeline();
            timeline.closed = true;
            (timeline.out_time(rt), timeline.last_video.clone())
        };
        let failed = self.shared.end().error.is_some();
        if failed {
            // The encoder no longer drains: stop it first, so no glue push waits on it.
            let _ = self.encoder.set_state(gst::State::Null);
        }
        let _ = self.sources.set_state(gst::State::Null);
        if !failed {
            if let Some((buffer, caps, out)) = &last {
                // A still screen at the end: repeat the last frame up to the stop time.
                if *out < stop_out {
                    let mut again = buffer.copy();
                    again.make_mut().set_pts(stop_out);
                    let _ = push(&self.video_in, &again, caps.as_ref());
                }
            }
            let _ = self.video_in.end_of_stream();
            for (_, input) in &self.audio_in {
                let _ = input.end_of_stream();
            }
            let end = self.shared.end();
            let (end, timeout) = self
                .shared
                .ended
                .wait_timeout_while(end, FINISH_TIMEOUT, |end| !end.eos && end.error.is_none())
                .unwrap_or_else(|e| e.into_inner());
            let timed_out = timeout.timed_out() && !end.eos && end.error.is_none();
            drop(end);
            if timed_out {
                eprintln!("annota: recording: the working file was not finished in time");
                self.shared.fail(RecordingError::Timeout);
            }
        }
        let _ = self.encoder.set_state(gst::State::Null);
        let error = self.shared.end().error.clone();
        let duration = match (&error, &last) {
            (Some(_), Some((_, _, out))) => *out,
            (Some(_), None) => gst::ClockTime::ZERO,
            (None, _) => stop_out,
        };
        Finished {
            path: self.path.clone(),
            duration,
            audio: self.audio_sources(),
            error,
        }
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        if !self.done {
            // Abandoned: like a crash, no end of stream; the file keeps what was written.
            let _ = self.encoder.set_state(gst::State::Null);
            let _ = self.sources.set_state(gst::State::Null);
        }
        for pipeline in [&self.encoder, &self.sources] {
            if let Some(bus) = pipeline.bus() {
                bus.unset_sync_handler();
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
