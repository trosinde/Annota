//! Export pipeline tests: working files made by the real recording pipeline from test sources
//! (synthetic time, fast), exported and measured again. No display.

use std::time::Instant;

use futures_util::StreamExt;
use gstreamer_pbutils::prelude::*;

use super::*;
use crate::recording::audio::AudioSource;
use crate::recording::pipeline::tests::{analyse, remove_working_file, rms_range, working_file};

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
