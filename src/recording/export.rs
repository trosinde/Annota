//! Exporting a recording's working file (tasks 5.1, 5.2; design.md D1, D3; spec "Export after
//! recording").
//!
//! - [`probe`]: size, duration and audio tracks of the working file (GStreamer's Discoverer);
//!   [`poster`]: one decoded frame for the poster. Both block; the window runs them off the main
//!   thread.
//! - [`start`]: the export to MP4 (the H.264 track remuxed unchanged, `h264parse ! mp4mux
//!   faststart`, with `h264timestamper` restoring the decoding timestamps Matroska does
//!   not store; the Opus tracks decoded, mixed by `audiomixer` and encoded to AAC by `voaacenc`)
//!   or WebM (decoded and encoded to VP9 by `vp9enc`, the audio mixed into one Opus track,
//!   `webmmux`). Without Include audio there is no audio pad at all. It runs on a worker thread
//!   and reports [`ExportEvent`]s: progress (the position of the last video buffer that reached
//!   the muxer ÷ the duration) and its end. The target is reserved first with a ` (2)` suffix
//!   where the name is taken ([`files::create_unique`]), so an existing file is never touched;
//!   [`Export::cancel`] (or an error) removes the partly written file. The working file is
//!   never changed here.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_channel::mpsc;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_pbutils as gst_pbutils;
use gtk::glib;

use super::files;

/// How often the worker looks at the bus, the cancel flag and the position.
const POLL: Duration = Duration::from_millis(100);
/// Discoverer's limit for one file.
const PROBE_TIMEOUT: gst::ClockTime = gst::ClockTime::from_seconds(10);
/// The poster frame's width in pixels (192 px at scale 2, redline 07).
pub const POSTER_WIDTH: i32 = 384;
/// Where the poster frame is taken: one second in, or the middle of a shorter recording.
const POSTER_AT: gst::ClockTime = gst::ClockTime::from_seconds(1);
/// VP9 (D3): realtime deadline with a high `cpu-used` for speed, rows in parallel, constant
/// quality capped by a bitrate. Tuned on this host (a 1280 × 800 screen recording encodes at
/// several times real time); revisit with task 6.2.
const VP9_CPU_USED: i32 = 6;
const VP9_CQ_LEVEL: i32 = 32;
const VP9_MAX_BITRATE: i32 = 8_000_000;
/// The software H.264 decoders, in order of preference (see [`h264_decoder`]).
const H264_DECODERS: &[&str] = &["avdec_h264", "openh264dec"];
/// AAC and the mixed track's format.
const AAC_BITRATE: i32 = 160_000;
const MIX_RATE: i32 = 48_000;
const MIX_CHANNELS: i32 = 2;
/// `voaacenc`'s encoder delay: its output starts with 1600 samples (at [`MIX_RATE`]) of priming
/// that its timestamps do not account for, and `mp4mux` writes no edit list for it (measured,
/// GStreamer 1.28: the sound 33.3 ms behind the picture). The MP4's video is delayed by as much
/// instead; `mp4mux` starts it with an empty edit.
const AAC_DELAY: gst::ClockTime = gst::ClockTime::from_nseconds(1600 * 1_000_000_000 / 48_000);

/// The export formats (redline 07, setting `record-export-format`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Mp4,
    Webm,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Webm => "webm",
        }
    }

    /// The setting's nick.
    pub fn nick(self) -> &'static str {
        self.extension()
    }

    pub fn from_nick(nick: &str) -> Self {
        if nick == "webm" {
            Self::Webm
        } else {
            Self::Mp4
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ExportError {
    #[error("the export was cancelled")]
    Cancelled,
    #[error("the GStreamer element {0} is not installed")]
    MissingElement(String),
    #[error("the export file cannot be written: {0}")]
    Target(String),
    #[error("the export pipeline could not be built: {0}")]
    Build(String),
    /// Element name and error text only, never content (CLAUDE.md §7).
    #[error("{element}: {message}")]
    Stream { element: String, message: String },
    #[error("the recording cannot be read: {0}")]
    Probe(String),
}

/// What the export reports, in order: progress, then exactly one `Done`.
#[derive(Clone, Debug, PartialEq)]
pub enum ExportEvent {
    /// 0.0 … 1.0.
    Progress(f64),
    /// The written file, or why there is none (the partly written file is gone by then).
    Done(Result<PathBuf, ExportError>),
}

/// A running export.
pub struct Export {
    target: PathBuf,
    cancel: Arc<AtomicBool>,
}

impl Export {
    /// The file being written (with its ` (n)` suffix).
    pub fn target(&self) -> &Path {
        &self.target
    }

    /// Stops the export; the worker removes the partly written file and reports
    /// [`ExportError::Cancelled`].
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    #[cfg(test)]
    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
}

/// What the working file holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    pub duration: gst::ClockTime,
    pub audio_tracks: usize,
}

/// Reads size, duration and audio tracks with GStreamer's Discoverer. Blocks.
pub fn probe(path: &Path) -> Result<Info, ExportError> {
    gst::init().map_err(|e| ExportError::Build(e.to_string()))?;
    let discoverer = gst_pbutils::Discoverer::new(PROBE_TIMEOUT)
        .map_err(|e| ExportError::Probe(e.to_string()))?;
    let uri = glib::filename_to_uri(path, None).map_err(|e| ExportError::Probe(e.to_string()))?;
    let info = discoverer
        .discover_uri(&uri)
        .map_err(|e| ExportError::Probe(e.to_string()))?;
    let video = info
        .video_streams()
        .into_iter()
        .next()
        .ok_or_else(|| ExportError::Probe(String::from("no video track")))?;
    Ok(Info {
        width: video.width(),
        height: video.height(),
        duration: info.duration().unwrap_or(gst::ClockTime::ZERO),
        audio_tracks: info.audio_streams().len(),
    })
}

/// A decoded poster frame, RGBA.
#[derive(Clone, Debug)]
pub struct Poster {
    pub width: i32,
    pub height: i32,
    pub stride: usize,
    pub data: Vec<u8>,
}

/// One frame of the working file, [`POSTER_WIDTH`] wide, taken one second in (the middle of a
/// shorter recording). Blocks.
pub fn poster(path: &Path, duration: gst::ClockTime) -> Result<Poster, ExportError> {
    poster_with(path, duration, H264_DECODERS)
}

/// [`poster`] with the first installed of `decoders` (tests force one).
fn poster_with(
    path: &Path,
    duration: gst::ClockTime,
    decoders: &[&str],
) -> Result<Poster, ExportError> {
    gst::init().map_err(|e| ExportError::Build(e.to_string()))?;
    let pipeline = gst::Pipeline::new();
    let src = make("filesrc")?;
    src.set_property("location", path.to_string_lossy().as_ref());
    let demux = make("matroskademux")?;
    let decode = [make("queue")?, make("h264parse")?, h264_decoder(decoders)?];
    let convert = make("videoconvert")?;
    let scale = make("videoscale")?;
    let sink = gstreamer_app::AppSink::builder()
        .caps(
            &gst::Caps::builder("video/x-raw")
                .field("format", "RGBA")
                .field("width", POSTER_WIDTH)
                .field("pixel-aspect-ratio", gst::Fraction::new(1, 1))
                .build(),
        )
        .sync(false)
        .build();
    pipeline
        .add_many([&src, &demux, &convert, &scale, sink.upcast_ref()])
        .map_err(build_error)?;
    pipeline.add_many(&decode).map_err(build_error)?;
    src.link(&demux).map_err(build_error)?;
    gst::Element::link_many(&decode).map_err(build_error)?;
    gst::Element::link_many([&decode[2], &convert, &scale, sink.upcast_ref()])
        .map_err(build_error)?;
    let video_in = decode[0].static_pad("sink");
    let weak = pipeline.downgrade();
    demux.connect_pad_added(move |_, pad| match &video_in {
        Some(video_in) if pad.name().starts_with("video") && !video_in.is_linked() => {
            let _ = pad.link(video_in);
        }
        _ => {
            // The audio tracks: dropped.
            if let Some(pipeline) = weak.upgrade() {
                let _ = drop_pad(&pipeline, pad);
            }
        }
    });
    let result = (|| {
        set_and_wait(&pipeline, gst::State::Paused)?;
        let at = if duration > POSTER_AT * 2 {
            POSTER_AT
        } else {
            duration / 2
        };
        if at > gst::ClockTime::ZERO {
            pipeline
                .seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE, at)
                .map_err(|e| ExportError::Probe(e.to_string()))?;
            set_and_wait(&pipeline, gst::State::Paused)?;
        }
        let sample = sink
            .pull_preroll()
            .map_err(|e| ExportError::Probe(e.to_string()))?;
        let caps = sample
            .caps()
            .ok_or_else(|| ExportError::Probe(String::from("no caps")))?;
        let info = gstreamer_video::VideoInfo::from_caps(caps)
            .map_err(|e| ExportError::Probe(e.to_string()))?;
        let buffer = sample
            .buffer()
            .ok_or_else(|| ExportError::Probe(String::from("no frame")))?;
        let map = buffer
            .map_readable()
            .map_err(|e| ExportError::Probe(e.to_string()))?;
        Ok(Poster {
            width: info.width() as i32,
            height: info.height() as i32,
            stride: info.stride()[0] as usize,
            data: map.as_slice().to_vec(),
        })
    })();
    let _ = pipeline.set_state(gst::State::Null);
    result
}

/// Sets `state` and waits for it (or the first error).
fn set_and_wait(pipeline: &gst::Pipeline, state: gst::State) -> Result<(), ExportError> {
    pipeline
        .set_state(state)
        .map_err(|_| ExportError::Probe(String::from("the file cannot be decoded")))?;
    let (result, _, _) = pipeline.state(gst::ClockTime::from_seconds(10));
    result
        .map(|_| ())
        .map_err(|_| ExportError::Probe(String::from("the file cannot be decoded")))
}

/// A software H.264 decoder (the poster frame, WebM): `avdec_h264` (gstreamer1.0-libav) or
/// `openh264dec` (gstreamer1.0-plugins-bad). Named, not left to `decodebin`, which picks a
/// hardware decoder where one ranks higher (seen: NVIDIA's, whose threads kept a test process
/// from exiting).
fn h264_decoder(decoders: &[&str]) -> Result<gst::Element, ExportError> {
    decoders
        .iter()
        .find_map(|factory| make(factory).ok())
        .ok_or_else(|| ExportError::MissingElement(decoders.join(" or ")))
}

fn make(factory: &str) -> Result<gst::Element, ExportError> {
    gst::ElementFactory::make(factory)
        .build()
        .map_err(|_| ExportError::MissingElement(factory.to_string()))
}

fn build_error(err: impl std::fmt::Display) -> ExportError {
    ExportError::Build(err.to_string())
}

/// Exports `source` as `<name>.<format>` into `folder` (created if missing), with the audio
/// tracks mixed into one when `include_audio` (and the file has any; `audio_tracks` from
/// [`probe`] or the recording). `duration` drives the progress. The events are delivered to
/// the receiver from the worker thread.
pub fn start(
    source: &Path,
    folder: &Path,
    name: &str,
    format: Format,
    include_audio: bool,
    audio_tracks: usize,
    duration: gst::ClockTime,
) -> Result<(Export, mpsc::UnboundedReceiver<ExportEvent>), ExportError> {
    start_with(
        source,
        folder,
        name,
        format,
        include_audio,
        audio_tracks,
        duration,
        Options::default(),
    )
}

/// What tests change in an export.
#[derive(Clone, Copy)]
struct Options {
    /// WebM's H.264 decoder: the first of these that is installed.
    decoders: &'static [&'static str],
    /// Tests: the video holds for a second once it has reached this time (a remux is too fast
    /// to be cancelled while it runs otherwise).
    stall_at: Option<gst::ClockTime>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            decoders: H264_DECODERS,
            stall_at: None,
        }
    }
}

/// [`start`] with `options`.
#[allow(clippy::too_many_arguments)]
fn start_with(
    source: &Path,
    folder: &Path,
    name: &str,
    format: Format,
    include_audio: bool,
    audio_tracks: usize,
    duration: gst::ClockTime,
    options: Options,
) -> Result<(Export, mpsc::UnboundedReceiver<ExportEvent>), ExportError> {
    gst::init().map_err(|e| ExportError::Build(e.to_string()))?;
    let (target, file) = files::create_unique(folder, name, format.extension())
        .map_err(|e| ExportError::Target(e.to_string()))?;
    drop(file);
    let cancel = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::unbounded();
    let job = Job {
        source: source.to_path_buf(),
        target: target.clone(),
        format,
        audio: include_audio && audio_tracks > 0,
        duration,
        cancel: cancel.clone(),
        options,
    };
    let spawned = std::thread::Builder::new()
        .name(String::from("annota-export"))
        .spawn(move || {
            let result = job.run(&sender);
            if result.is_err() {
                job.remove_partial();
            }
            let _ = sender.unbounded_send(ExportEvent::Done(result));
        });
    if let Err(err) = spawned {
        let _ = std::fs::remove_file(&target);
        return Err(ExportError::Build(err.to_string()));
    }
    Ok((Export { target, cancel }, receiver))
}

struct Job {
    source: PathBuf,
    target: PathBuf,
    format: Format,
    audio: bool,
    duration: gst::ClockTime,
    cancel: Arc<AtomicBool>,
    options: Options,
}

impl Job {
    /// mp4mux's temporary file for `faststart`, next to the target (not in `/tmp`, which may be
    /// small), hidden.
    fn faststart_file(&self) -> PathBuf {
        let name = self
            .target
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.target.with_file_name(format!(".{name}.faststart"))
    }

    fn remove_partial(&self) {
        let _ = std::fs::remove_file(&self.target);
        let _ = std::fs::remove_file(self.faststart_file());
    }

    fn run(&self, events: &mpsc::UnboundedSender<ExportEvent>) -> Result<PathBuf, ExportError> {
        let position = Arc::new(AtomicU64::new(0));
        let pipeline = self.build(&position)?;
        let bus = pipeline
            .bus()
            .ok_or_else(|| ExportError::Build(String::from("no bus")))?;
        let result = (|| {
            pipeline.set_state(gst::State::Playing).map_err(|_| {
                first_error(&bus).unwrap_or_else(|| {
                    ExportError::Build(String::from("the export pipeline did not start"))
                })
            })?;
            let mut shown = -1.0;
            loop {
                if self.cancel.load(Ordering::SeqCst) {
                    return Err(ExportError::Cancelled);
                }
                let message = bus.timed_pop_filtered(
                    gst::ClockTime::from_nseconds(POLL.as_nanos() as u64),
                    &[gst::MessageType::Eos, gst::MessageType::Error],
                );
                match message.as_ref().map(|m| m.view()) {
                    Some(gst::MessageView::Eos(_)) => break,
                    Some(gst::MessageView::Error(err)) => return Err(stream_error(err)),
                    _ => {}
                }
                let done = fraction(position.load(Ordering::Relaxed), self.duration);
                if (done - shown).abs() >= 0.001 {
                    shown = done;
                    let _ = events.unbounded_send(ExportEvent::Progress(done));
                }
            }
            Ok(())
        })();
        let _ = pipeline.set_state(gst::State::Null);
        result?;
        let _ = events.unbounded_send(ExportEvent::Progress(1.0));
        Ok(self.target.clone())
    }

    /// `filesrc ! matroskademux`, the video branch (remux or VP9) and, with audio, every audio
    /// track decoded into one `audiomixer` and encoded once, into the muxer and `filesink`.
    fn build(&self, position: &Arc<AtomicU64>) -> Result<gst::Pipeline, ExportError> {
        let pipeline = gst::Pipeline::new();
        let src = make("filesrc")?;
        src.set_property("location", self.source.to_string_lossy().as_ref());
        let demux = make("matroskademux")?;
        let (mux, video_chain) = match self.format {
            Format::Mp4 => {
                let mux = make("mp4mux")?;
                mux.set_property("faststart", true);
                mux.set_property(
                    "faststart-file",
                    self.faststart_file().to_string_lossy().as_ref(),
                );
                // Matroska stores no decoding timestamps; with x264's B-frames `mp4mux` needs
                // them, so `h264timestamper` (plugins-bad) derives them from the stream.
                (
                    mux,
                    vec![make("queue")?, make("h264parse")?, make("h264timestamper")?],
                )
            }
            Format::Webm => {
                let mux = make("webmmux")?;
                let encode = make("vp9enc")?;
                encode.set_property("deadline", 1i64);
                encode.set_property("cpu-used", VP9_CPU_USED);
                encode.set_property("row-mt", true);
                encode.set_property_from_str("end-usage", "cq");
                encode.set_property("cq-level", VP9_CQ_LEVEL);
                encode.set_property("target-bitrate", VP9_MAX_BITRATE);
                encode.set_property(
                    "threads",
                    std::thread::available_parallelism().map_or(4, |n| n.get().min(16)) as i32,
                );
                encode.set_property("keyframe-max-dist", 120i32);
                (
                    mux,
                    vec![
                        make("queue")?,
                        make("h264parse")?,
                        h264_decoder(self.options.decoders)?,
                        make("videoconvert")?,
                        encode,
                        make("queue")?,
                    ],
                )
            }
        };
        let sink = make("filesink")?;
        sink.set_property("location", self.target.to_string_lossy().as_ref());
        pipeline
            .add_many([&src, &demux, &mux, &sink])
            .map_err(build_error)?;
        pipeline.add_many(&video_chain).map_err(build_error)?;
        src.link(&demux).map_err(build_error)?;
        mux.link(&sink).map_err(build_error)?;
        gst::Element::link_many(&video_chain).map_err(build_error)?;
        let video_last = video_chain
            .last()
            .ok_or_else(|| ExportError::Build(String::from("no video chain")))?;
        let mux_video = mux
            .request_pad_simple("video_%u")
            .ok_or_else(|| ExportError::Build(String::from("no video pad on the muxer")))?;
        let video_out = video_last
            .static_pad("src")
            .ok_or_else(|| ExportError::Build(String::from("no video output")))?;
        video_out.link(&mux_video).map_err(build_error)?;
        if self.format == Format::Mp4 && self.audio {
            video_out.set_offset(AAC_DELAY.nseconds() as i64);
        }
        // Progress: the newest video time that reached the muxer, in stream time
        // (`h264timestamper` moves its timestamps 1000 h on and its segment with them).
        let position = Arc::clone(position);
        let stall_at = self.options.stall_at;
        let stalled = AtomicBool::new(false);
        mux_video.add_probe(gst::PadProbeType::BUFFER, move |pad, info| {
            let Some((pts, duration)) = info.buffer().and_then(|b| Some((b.pts()?, b.duration())))
            else {
                return gst::PadProbeReturn::Ok;
            };
            let at = pad
                .sticky_event::<gst::event::Segment>(0)
                .and_then(|event| {
                    event
                        .segment()
                        .downcast_ref::<gst::ClockTime>()
                        .and_then(|segment| segment.to_stream_time(pts))
                })
                .unwrap_or(pts);
            let end = at + duration.unwrap_or(gst::ClockTime::ZERO);
            position.fetch_max(end.nseconds(), Ordering::Relaxed);
            if stall_at.is_some_and(|stall| at >= stall) && !stalled.swap(true, Ordering::SeqCst) {
                std::thread::sleep(Duration::from_secs(1));
            }
            gst::PadProbeReturn::Ok
        });
        let video_in = video_chain
            .first()
            .and_then(|e| e.static_pad("sink"))
            .ok_or_else(|| ExportError::Build(String::from("no video input")))?;

        // The mixed audio track, with Include audio only.
        let mixer = if self.audio {
            let mixer = make("audiomixer")?;
            let caps = make("capsfilter")?;
            caps.set_property(
                "caps",
                gst::Caps::builder("audio/x-raw")
                    .field("rate", MIX_RATE)
                    .field("channels", MIX_CHANNELS)
                    .build(),
            );
            let convert = make("audioconvert")?;
            let encode = match self.format {
                Format::Mp4 => {
                    let aac = make("voaacenc")?;
                    aac.set_property("bitrate", AAC_BITRATE);
                    aac
                }
                Format::Webm => make("opusenc")?,
            };
            let queue = make("queue")?;
            let chain = [&mixer, &caps, &convert, &encode, &queue];
            pipeline.add_many(chain).map_err(build_error)?;
            gst::Element::link_many(chain).map_err(build_error)?;
            let mux_audio = mux
                .request_pad_simple("audio_%u")
                .ok_or_else(|| ExportError::Build(String::from("no audio pad on the muxer")))?;
            queue
                .static_pad("src")
                .ok_or_else(|| ExportError::Build(String::from("no audio output")))?
                .link(&mux_audio)
                .map_err(build_error)?;
            Some(mixer)
        } else {
            None
        };

        // The demuxer's pads: video into its chain, each audio track decoded into the mixer
        // (or dropped without Include audio).
        let weak = pipeline.downgrade();
        demux.connect_pad_added(move |_, pad| {
            let Some(pipeline) = weak.upgrade() else {
                return;
            };
            let name = pad.name();
            if name.starts_with("video") {
                if !video_in.is_linked() {
                    let _ = pad.link(&video_in);
                }
                return;
            }
            let linked = match (&mixer, name.starts_with("audio")) {
                (Some(mixer), true) => link_audio_track(&pipeline, pad, mixer),
                _ => drop_pad(&pipeline, pad),
            };
            if let Err(err) = linked {
                gst::element_error!(
                    pipeline,
                    gst::CoreError::Negotiation,
                    ("{}", err.to_string())
                );
            }
        });
        Ok(pipeline)
    }
}

/// `queue ! opusdec ! audioconvert ! audioresample` from a demuxed track into a new mixer pad.
fn link_audio_track(
    pipeline: &gst::Pipeline,
    pad: &gst::Pad,
    mixer: &gst::Element,
) -> Result<(), ExportError> {
    let chain = [
        make("queue")?,
        make("opusdec")?,
        make("audioconvert")?,
        make("audioresample")?,
    ];
    pipeline.add_many(&chain).map_err(build_error)?;
    gst::Element::link_many(&chain).map_err(build_error)?;
    let mixer_in = mixer
        .request_pad_simple("sink_%u")
        .ok_or_else(|| ExportError::Build(String::from("no mixer pad")))?;
    chain[3]
        .static_pad("src")
        .ok_or_else(|| ExportError::Build(String::from("no audio chain output")))?
        .link(&mixer_in)
        .map_err(build_error)?;
    for element in &chain {
        element.sync_state_with_parent().map_err(build_error)?;
    }
    let input = chain[0]
        .static_pad("sink")
        .ok_or_else(|| ExportError::Build(String::from("no audio chain input")))?;
    pad.link(&input).map_err(build_error)?;
    Ok(())
}

/// A track that is not used goes into `queue ! fakesink`. The queue matters: a sink blocks its
/// streaming thread while the pipeline is paused (the poster frame), which would be the
/// demuxer's own thread and starve the video track.
fn drop_pad(pipeline: &gst::Pipeline, pad: &gst::Pad) -> Result<(), ExportError> {
    let queue = make("queue")?;
    let fake = make("fakesink")?;
    fake.set_property("sync", false);
    fake.set_property("async", false);
    pipeline.add_many([&queue, &fake]).map_err(build_error)?;
    queue.link(&fake).map_err(build_error)?;
    fake.sync_state_with_parent().map_err(build_error)?;
    queue.sync_state_with_parent().map_err(build_error)?;
    let input = queue
        .static_pad("sink")
        .ok_or_else(|| ExportError::Build(String::from("no queue input")))?;
    pad.link(&input).map_err(build_error)?;
    Ok(())
}

fn fraction(position_ns: u64, duration: gst::ClockTime) -> f64 {
    if duration.is_zero() {
        return 0.0;
    }
    (position_ns as f64 / duration.nseconds() as f64).clamp(0.0, 1.0)
}

fn stream_error(err: &gst::message::Error) -> ExportError {
    let element = err
        .src()
        .map(|s| s.name().to_string())
        .unwrap_or_else(|| String::from("pipeline"));
    ExportError::Stream {
        element,
        message: err.error().to_string(),
    }
}

/// The first error on `bus`, if one is waiting.
fn first_error(bus: &gst::Bus) -> Option<ExportError> {
    bus.pop_filtered(&[gst::MessageType::Error])
        .and_then(|message| match message.view() {
            gst::MessageView::Error(err) => Some(stream_error(err)),
            _ => None,
        })
}

#[cfg(test)]
mod tests;
