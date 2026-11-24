//! Export pipeline tests: working files made by the real recording pipeline from test sources
//! (synthetic time, fast), exported and measured again. No display.

use std::time::Instant;

use futures_util::StreamExt;
use gstreamer_pbutils::prelude::*;

use super::*;
use crate::recording::audio::AudioSource;
use crate::recording::pipeline::tests::{
    analyse, near, remove_working_file, rms_range, working_file, working_file_marked, RED,
};

/// Runs an export to its end on a main context of its own; all events in order.
fn run_export(
    source: &Path,
    folder: &Path,
    name: &str,
    format: Format,
    include_audio: bool,
    audio_tracks: usize,
    duration: gst::ClockTime,
) -> Vec<ExportEvent> {
    let (_export, receiver) = start(
        source,
        folder,
        name,
        format,
        include_audio,
        audio_tracks,
        duration,
    )
    .unwrap();
    collect(receiver)
}

fn collect(receiver: mpsc::UnboundedReceiver<ExportEvent>) -> Vec<ExportEvent> {
    let context = glib::MainContext::new();
    context
        .with_thread_default(|| context.block_on(receiver.collect::<Vec<_>>()))
        .unwrap()
}

fn done(events: &[ExportEvent]) -> Result<PathBuf, ExportError> {
    match events.last() {
        Some(ExportEvent::Done(result)) => result.clone(),
        other => panic!("no Done at the end: {other:?}"),
    }
}

fn progress(events: &[ExportEvent]) -> Vec<f64> {
    events
        .iter()
        .filter_map(|e| match e {
            ExportEvent::Progress(p) => Some(*p),
            ExportEvent::Done(_) => None,
        })
        .collect()
}

/// The caps names of the file's streams: (video, audio…).
fn streams(path: &Path) -> (String, Vec<String>) {
    let discoverer = gst_pbutils::Discoverer::new(PROBE_TIMEOUT).unwrap();
    let info = discoverer
        .discover_uri(&glib::filename_to_uri(path, None).unwrap())
        .unwrap();
    let name = |caps: Option<gst::Caps>| {
        caps.and_then(|c| c.structure(0).map(|s| s.name().to_string()))
            .unwrap_or_default()
    };
    let video = info
        .video_streams()
        .into_iter()
        .next()
        .map(|v| name(v.caps()))
        .unwrap_or_default();
    let audio = info
        .audio_streams()
        .into_iter()
        .map(|a| name(a.caps()))
        .collect();
    (video, audio)
}

fn export_folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("annota-export-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

// spec: screen-recording / Export after recording / Export as MP4
#[test]
fn an_mp4_export_remuxes_h264_and_mixes_the_audio_into_one_aac_track() {
    let recording = working_file(
        "export-mp4",
        20.0,
        (64, 48),
        &[
            (AudioSource::Microphone, 0.5),
            (AudioSource::SystemAudio, 0.1),
        ],
    );
    let info = probe(&recording.path).unwrap();
    assert_eq!((info.width, info.height), (64, 48));
    assert_eq!(info.audio_tracks, 2);
    let length = info.duration.nseconds() as f64 / 1e9;
    assert!((length - 20.0).abs() <= 0.2, "{length} s");
    // The window's poster frame: one second in (the second second's colour, blue, not the
    // first's green), 384 px wide, the recording's aspect ratio.
    let poster = poster(&recording.path, info.duration).unwrap();
    assert_eq!((poster.width, poster.height), (POSTER_WIDTH, 288));
    assert!(poster.data.len() >= poster.stride * poster.height as usize);
    let px = &poster.data[(144 * poster.stride + 4 * 192)..][..3];
    assert!(px[2] > 200 && px[1] < 60, "blue: {px:?}");

    let folder = export_folder("mp4");
    let events = run_export(
        &recording.path,
        &folder,
        "Recording 2026-11-22 10-42-07",
        Format::Mp4,
        true,
        info.audio_tracks,
        info.duration,
    );
    let written = done(&events).unwrap();
    assert_eq!(written, folder.join("Recording 2026-11-22 10-42-07.mp4"));
    let steps = progress(&events);
    assert!(steps.windows(2).all(|p| p[1] >= p[0]), "{steps:?}");
    assert_eq!(steps.last(), Some(&1.0));

    let (video, audio) = streams(&written);
    assert_eq!(video, "video/x-h264");
    assert_eq!(audio, ["audio/mpeg"], "one AAC track");
    let analysis = analyse(&written);
    let duration = analysis.duration();
    assert!((duration - 20.0).abs() <= 0.2, "lasts {duration} s");
    // Remuxed, not re-encoded: every recorded frame is there.
    let recorded = analyse(&recording.path).frames.len();
    assert_eq!(analysis.frames.len(), recorded);
    assert_eq!(analysis.audio.len(), 1);
    // Both sources mixed: louder than the microphone alone (0.5 → RMS ≈ 0.35).
    let (quietest, _) = rms_range(&analysis.audio[0], 0.5, 19.5);
    assert!(quietest > 0.2, "audio present throughout: {quietest}");
    // faststart: `moov` before `mdat`; the temporary file is gone.
    let bytes = std::fs::read(&written).unwrap();
    let at = |tag: &[u8]| bytes.windows(4).position(|w| w == tag).unwrap();
    assert!(at(b"moov") < at(b"mdat"), "faststart");
    assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 1);
    // The working file is not touched by the export.
    assert!(recording.path.exists());
    let _ = std::fs::remove_dir_all(&folder);
    remove_working_file(&recording.path);
}

// spec: screen-recording / Export after recording / Export as WebM
#[test]
fn a_webm_export_has_vp9_and_opus_and_reports_progress() {
    let recording = working_file(
        "export-webm",
        6.0,
        (1280, 720),
        &[(AudioSource::Microphone, 0.5)],
    );
    let folder = export_folder("webm");
    let events = run_export(
        &recording.path,
        &folder,
        "Demo",
        Format::Webm,
        true,
        1,
        recording.duration,
    );
    let written = done(&events).unwrap();
    assert_eq!(written, folder.join("Demo.webm"));
    let steps = progress(&events);
    assert!(
        steps.iter().any(|p| *p > 0.0 && *p < 1.0),
        "progress while it runs: {steps:?}"
    );
    assert!(steps.windows(2).all(|p| p[1] >= p[0]), "{steps:?}");
    let (video, audio) = streams(&written);
    assert_eq!(video, "video/x-vp9");
    assert_eq!(audio, ["audio/x-opus"]);
    let analysis = analyse(&written);
    assert!((analysis.duration() - 6.0).abs() <= 0.2);
    assert!(rms_range(&analysis.audio[0], 0.5, 5.5).0 > 0.1);
    let _ = std::fs::remove_dir_all(&folder);
    remove_working_file(&recording.path);
}

// spec: screen-recording / Export after recording / Export without audio
#[test]
fn without_include_audio_the_export_has_no_audio_track() {
    let recording = working_file(
        "export-no-audio",
        3.0,
        (64, 48),
        &[
            (AudioSource::Microphone, 0.5),
            (AudioSource::SystemAudio, 0.1),
        ],
    );
    let folder = export_folder("no-audio");
    for format in [Format::Mp4, Format::Webm] {
        let events = run_export(
            &recording.path,
            &folder,
            "Silent",
            format,
            false,
            2,
            recording.duration,
        );
        let written = done(&events).unwrap();
        let (video, audio) = streams(&written);
        assert!(!video.is_empty());
        assert!(
            audio.is_empty(),
            "{format:?}: no audio track, got {audio:?}"
        );
        assert!(analyse(&written).audio.is_empty());
    }
    let _ = std::fs::remove_dir_all(&folder);
    remove_working_file(&recording.path);
}

// spec: screen-recording / Export after recording / Cancel the export
#[test]
fn a_cancelled_export_removes_the_partly_written_file() {
    let recording = working_file("export-cancel", 10.0, (1920, 1080), &[]);
    let folder = export_folder("cancel");
    for format in [Format::Webm, Format::Mp4] {
        let (export, mut receiver) = start(
            &recording.path,
            &folder,
            "Cancelled",
            format,
            true,
            0,
            recording.duration,
        )
        .unwrap();
        let target = export.target().to_path_buf();
        assert!(target.exists(), "reserved at once");
        export.cancel();
        let context = glib::MainContext::new();
        let events = context
            .with_thread_default(|| {
                context.block_on(async {
                    let mut all = Vec::new();
                    while let Some(event) = receiver.next().await {
                        all.push(event);
                    }
                    all
                })
            })
            .unwrap();
        assert_eq!(done(&events), Err(ExportError::Cancelled), "{format:?}");
        assert!(!target.exists(), "the partly written file is removed");
        assert_eq!(
            std::fs::read_dir(&folder).unwrap().count(),
            0,
            "nothing left behind"
        );
    }
    // Cancelled while it runs: after the first progress.
    let started = Instant::now();
    let (export, mut receiver) = start(
        &recording.path,
        &folder,
        "Cancelled",
        Format::Webm,
        true,
        0,
        recording.duration,
    )
    .unwrap();
    let context = glib::MainContext::new();
    let events = context
        .with_thread_default(|| {
            context.block_on(async {
                let mut all = Vec::new();
                while let Some(event) = receiver.next().await {
                    if matches!(event, ExportEvent::Progress(p) if p > 0.0) {
                        export.cancel();
                    }
                    all.push(event);
                }
                all
            })
        })
        .unwrap();
    assert_eq!(
        done(&events),
        Err(ExportError::Cancelled),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 0);
    assert!(recording.path.exists(), "the working file is kept");
    let _ = std::fs::remove_dir_all(&folder);
    remove_working_file(&recording.path);
}

// spec: screen-recording / Export after recording / Target name taken
#[test]
fn a_taken_target_name_gets_a_suffix_and_the_other_file_stays() {
    let recording = working_file("export-taken", 2.0, (64, 48), &[]);
    let folder = export_folder("taken");
    std::fs::create_dir_all(&folder).unwrap();
    let existing = folder.join("Demo.mp4");
    std::fs::write(&existing, b"an earlier export").unwrap();
    let events = run_export(
        &recording.path,
        &folder,
        "Demo",
        Format::Mp4,
        true,
        0,
        recording.duration,
    );
    assert_eq!(done(&events).unwrap(), folder.join("Demo (2).mp4"));
    assert_eq!(std::fs::read(&existing).unwrap(), b"an earlier export");
    let events = run_export(
        &recording.path,
        &folder,
        "Demo",
        Format::Mp4,
        true,
        0,
        recording.duration,
    );
    assert_eq!(done(&events).unwrap(), folder.join("Demo (3).mp4"));
    let _ = std::fs::remove_dir_all(&folder);
    remove_working_file(&recording.path);
}

/// The time of the first red frame and of the first audible sample (|v| > 0.05): the marker of
/// [`working_file_marked`] as a player shows it, in running time (edit lists applied).
fn marker_onsets(path: &Path) -> (f64, f64) {
    /// The earliest time seen.
    type Earliest = Arc<std::sync::Mutex<Option<f64>>>;
    fn note(earliest: &Earliest, at: f64) {
        let mut earliest = earliest.lock().unwrap();
        if earliest.is_none_or(|e| at < e) {
            *earliest = Some(at);
        }
    }
    /// The buffer's start in running time (s).
    fn running(sample: &gst::Sample) -> f64 {
        let pts = sample.buffer().and_then(|b| b.pts()).unwrap();
        let segment = sample.segment().unwrap();
        let segment = segment.downcast_ref::<gst::format::Time>().unwrap();
        segment.to_running_time(pts).unwrap().nseconds() as f64 / 1e9
    }
    let flash: Earliest = Arc::default();
    let beep: Earliest = Arc::default();
    let pipeline = gst::Pipeline::new();
    let src = make("filesrc").unwrap();
    src.set_property("location", path.to_str().unwrap());
    let decode = make("decodebin").unwrap();
    pipeline.add_many([&src, &decode]).unwrap();
    src.link(&decode).unwrap();
    let weak = pipeline.downgrade();
    let (flash_in, beep_in) = (Arc::clone(&flash), Arc::clone(&beep));
    decode.connect_pad_added(move |_, pad| {
        let pipeline = weak.upgrade().unwrap();
        let caps = pad.current_caps().unwrap();
        let video = caps.structure(0).unwrap().name().starts_with("video/");
        let (convert, caps) = if video {
            (
                make("videoconvert").unwrap(),
                gst::Caps::builder("video/x-raw")
                    .field("format", "RGB")
                    .build(),
            )
        } else {
            (
                make("audioconvert").unwrap(),
                gst::Caps::builder("audio/x-raw")
                    .field("format", "F32LE")
                    .field("channels", 1i32)
                    .field("layout", "interleaved")
                    .build(),
            )
        };
        let sink = gstreamer_app::AppSink::builder()
            .caps(&caps)
            .sync(false)
            .build();
        let (flash, beep) = (Arc::clone(&flash_in), Arc::clone(&beep_in));
        sink.set_callbacks(
            gstreamer_app::AppSinkCallbacks::builder()
                .new_sample(move |sink| {
                    let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                    let at = running(&sample);
                    let caps = sample.caps().unwrap();
                    let buffer = sample.buffer().unwrap();
                    let map = buffer.map_readable().unwrap();
                    if video {
                        let info = gstreamer_video::VideoInfo::from_caps(caps).unwrap();
                        let (x, y) = (info.width() as usize / 2, info.height() as usize / 2);
                        let i = y * info.stride()[0] as usize + x * 3;
                        if near([map[i], map[i + 1], map[i + 2]], RED) {
                            note(&flash, at);
                        }
                    } else {
                        let rate = caps.structure(0).unwrap().get::<i32>("rate").unwrap();
                        let first = map.chunks_exact(4).position(|b| {
                            f32::from_le_bytes([b[0], b[1], b[2], b[3]]).abs() > 0.05
                        });
                        if let Some(i) = first {
                            note(&beep, at + i as f64 / f64::from(rate));
                        }
                    }
                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );
        pipeline.add_many([&convert, sink.upcast_ref()]).unwrap();
        convert.link(&sink).unwrap();
        convert.sync_state_with_parent().unwrap();
        sink.sync_state_with_parent().unwrap();
        pad.link(&convert.static_pad("sink").unwrap()).unwrap();
    });
    pipeline.set_state(gst::State::Playing).unwrap();
    let message = pipeline
        .bus()
        .unwrap()
        .timed_pop_filtered(
            gst::ClockTime::from_seconds(60),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        )
        .expect("decoding finishes");
    assert!(matches!(message.view(), gst::MessageView::Eos(_)));
    pipeline.set_state(gst::State::Null).unwrap();
    let flash = flash.lock().unwrap().expect("a red frame");
    let beep = beep.lock().unwrap().expect("an audible sample");
    (flash, beep)
}

/// Picture and sound stay together: a flash and a beep at 2 s, recorded through the real
/// recording pipeline, are at most [`AV_SYNC_LIMIT`] apart in the working file, the MP4 and the
/// WebM export (the AAC encoder's priming delay is taken out of the MP4, design.md Phase D1).
// spec: screen-recording / Export after recording / Export as MP4
// spec: screen-recording / Export after recording / Export as WebM
#[test]
fn picture_and_sound_stay_in_sync_in_mp4_and_webm() {
    let recording = working_file_marked(
        "export-sync",
        4.0,
        (64, 48),
        &[(AudioSource::Microphone, 0.5)],
        Some(2.0),
    );
    // The reference is FFmpeg's AAC decoder (`avdec_aac`), as in most players. `faad`, the
    // only other one (gstreamer1.0-plugins-bad, picked where libav is missing), puts the decoded
    // sound one AAC frame (1024 samples) earlier: measured 21.3 ms early on Ubuntu 24.04 and
    // with `faad` forced on 1.28.
    assert!(
        gst::ElementFactory::find("avdec_aac").is_some(),
        "measuring needs avdec_aac (gstreamer1.0-libav)"
    );
    let folder = export_folder("sync");
    let mut offsets = vec![("working file", recording.path.clone())];
    for format in [Format::Mp4, Format::Webm] {
        let events = run_export(
            &recording.path,
            &folder,
            "Sync",
            format,
            true,
            1,
            recording.duration,
        );
        offsets.push((format.extension(), done(&events).unwrap()));
    }
    for (what, path) in offsets {
        let (video, audio) = marker_onsets(&path);
        let offset = audio - video;
        eprintln!(
            "{what}: flash at {video:.3} s, beep at {audio:.3} s, audio {:+.1} ms",
            offset * 1e3
        );
        // The flash where it was recorded (the MP4's picture starts AAC_DELAY late).
        assert!((video - 2.0).abs() < 0.04, "{what}: flash at {video} s");
        assert!(
            offset.abs() <= AV_SYNC_LIMIT,
            "{what}: audio {:+.1} ms off the picture",
            offset * 1e3
        );
    }
    let _ = std::fs::remove_dir_all(&folder);
    remove_working_file(&recording.path);
}

/// How far apart picture and sound may be (s).
const AV_SYNC_LIMIT: f64 = 0.005;

/// Whether the file's H.264 track has B-frames: its frames are stored out of display order.
fn has_b_frames(path: &Path) -> bool {
    let pipeline = gst::Pipeline::new();
    let src = make("filesrc").unwrap();
    src.set_property("location", path.to_str().unwrap());
    let demux = make("matroskademux").unwrap();
    let sink = gstreamer_app::AppSink::builder().sync(false).build();
    pipeline
        .add_many([&src, &demux, sink.upcast_ref()])
        .unwrap();
    src.link(&demux).unwrap();
    let weak = pipeline.downgrade();
    let video_in = sink.static_pad("sink").unwrap();
    demux.connect_pad_added(move |_, pad| {
        let pipeline = weak.upgrade().unwrap();
        if pad.name().starts_with("video") {
            pad.link(&video_in).unwrap();
        } else {
            drop_pad(&pipeline, pad).unwrap();
        }
    });
    let order = Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = Arc::clone(&order);
    sink.set_callbacks(
        gstreamer_app::AppSinkCallbacks::builder()
            .new_sample(move |sink| {
                let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                if let Some(pts) = sample.buffer().and_then(|b| b.pts()) {
                    seen.lock().unwrap().push(pts);
                }
                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );
    pipeline.set_state(gst::State::Playing).unwrap();
    pipeline
        .bus()
        .unwrap()
        .timed_pop_filtered(
            gst::ClockTime::from_seconds(60),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        )
        .expect("demuxing finishes");
    pipeline.set_state(gst::State::Null).unwrap();
    let order = order.lock().unwrap();
    order.windows(2).any(|p| p[1] < p[0])
}

/// `openh264dec` (plugins-bad, the decoder a `.deb` install has; this host also has
/// `avdec_h264`, which is preferred): the poster frame and a WebM export of a working file with
/// B-frames decode with it alone.
// spec: screen-recording / Export after recording / Export as WebM
#[test]
fn openh264dec_decodes_the_poster_and_a_webm_export() {
    let recording = working_file(
        "export-openh264",
        3.0,
        (320, 240),
        &[(AudioSource::Microphone, 0.5)],
    );
    assert!(has_b_frames(&recording.path), "x264 wrote B-frames");
    let only: &'static [&'static str] = &["openh264dec"];
    let poster = poster_with(&recording.path, recording.duration, only).unwrap();
    assert_eq!((poster.width, poster.height), (POSTER_WIDTH, 288));
    // One second in: the second second's colour, blue.
    let px = &poster.data[(144 * poster.stride + 4 * 192)..][..3];
    assert!(px[2] > 200 && px[1] < 60, "blue: {px:?}");
    assert!(poster_with(&recording.path, recording.duration, &["no-such-decoder"]).is_err());

    let folder = export_folder("openh264");
    let (_export, receiver) = start_with(
        &recording.path,
        &folder,
        "Demo",
        Format::Webm,
        true,
        1,
        recording.duration,
        Options {
            decoders: only,
            ..Options::default()
        },
    )
    .unwrap();
    let written = done(&collect(receiver)).unwrap();
    let (video, audio) = streams(&written);
    assert_eq!(video, "video/x-vp9");
    assert_eq!(audio, ["audio/x-opus"]);
    let analysis = analyse(&written);
    assert_eq!(analysis.frames.len(), analyse(&recording.path).frames.len());
    assert!((analysis.duration() - 3.0).abs() <= 0.1);
    // The frames in display order: green, blue, red seconds.
    for (at, colour) in [(0.5, [0, 255, 0]), (1.5, [0, 0, 255]), (2.5, RED)] {
        let frame = analysis
            .frames
            .iter()
            .find(|f| (f.pts - at).abs() < 0.02)
            .unwrap();
        assert!(near(frame.at(160, 120), colour), "{at} s");
    }
    let _ = std::fs::remove_dir_all(&folder);
    remove_working_file(&recording.path);
}

/// Cancel while an MP4 export runs (its video held for a second at 2 s, since a remux is too
/// fast to catch otherwise): the partly written file and the hidden faststart file go, the
/// working file stays.
// spec: screen-recording / Export after recording / Cancel the export
#[test]
fn a_running_mp4_export_cancelled_leaves_nothing_behind() {
    let recording = working_file(
        "export-cancel-mp4",
        10.0,
        (640, 360),
        &[
            (AudioSource::Microphone, 0.5),
            (AudioSource::SystemAudio, 0.1),
        ],
    );
    let folder = export_folder("cancel-mp4");
    let (export, mut receiver) = start_with(
        &recording.path,
        &folder,
        "Cancelled",
        Format::Mp4,
        true,
        2,
        recording.duration,
        Options {
            stall_at: Some(gst::ClockTime::from_seconds(2)),
            ..Options::default()
        },
    )
    .unwrap();
    let target = export.target().to_path_buf();
    let faststart = folder.join(".Cancelled.mp4.faststart");
    let context = glib::MainContext::new();
    let events = context
        .with_thread_default(|| {
            context.block_on(async {
                let mut all = Vec::new();
                while let Some(event) = receiver.next().await {
                    if matches!(event, ExportEvent::Progress(p) if p > 0.0 && p < 1.0)
                        && !export.is_cancelled()
                    {
                        assert!(target.exists(), "the target is being written");
                        assert!(
                            std::fs::metadata(&faststart).is_ok_and(|m| m.len() > 0),
                            "the faststart file is being written"
                        );
                        export.cancel();
                    }
                    all.push(event);
                }
                all
            })
        })
        .unwrap();
    assert!(export.is_cancelled(), "cancelled while it ran: {events:?}");
    assert_eq!(done(&events), Err(ExportError::Cancelled));
    assert!(!target.exists(), "the partly written file is removed");
    assert!(!faststart.exists(), "the faststart file is removed");
    assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 0);
    assert!(recording.path.exists(), "the working file is kept");
    let _ = std::fs::remove_dir_all(&folder);
    remove_working_file(&recording.path);
}
