//! Pipeline tests with test sources (`appsrc` with chosen timestamps), fast: synthetic time, no
//! display, no portal, no audio device. Every working file is decoded again and measured.

use std::sync::atomic::{AtomicUsize, Ordering};

use gstreamer_video::prelude::*;

use super::*;
use crate::recording::audio::{sources_to_record, AudioDevices};
use crate::recording::geometry::window_crop_in_buffer;

pub(crate) const RED: [u8; 3] = [255, 0, 0];
pub(crate) const GREEN: [u8; 3] = [0, 255, 0];
const BLUE: [u8; 3] = [0, 0, 255];
const BLACK: [u8; 3] = [0, 0, 0];
const SMALL: (u32, u32) = (64, 48);
const FRAME: f64 = 1.0 / FRAME_RATE as f64;

fn secs(s: f64) -> gst::ClockTime {
    gst::ClockTime::from_nseconds((s * 1e9).round() as u64)
}

fn folder(name: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "annota-rec-{name}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

pub(crate) fn near(c: [u8; 3], target: [u8; 3]) -> bool {
    c.iter()
        .zip(target)
        .all(|(a, b)| (i32::from(*a) - i32::from(b)).abs() <= 48)
}

/// A video source pushed by the test: BGRx frames with chosen timestamps.
struct TestVideo {
    src: gst_app::AppSrc,
    w: u32,
    h: u32,
}

impl TestVideo {
    fn new(w: u32, h: u32) -> Self {
        gst::init().unwrap();
        let caps = gst::Caps::builder("video/x-raw")
            .field("format", "BGRx")
            .field("width", w as i32)
            .field("height", h as i32)
            .field("framerate", gst::Fraction::new(0, 1))
            .build();
        let src = gst_app::AppSrc::builder()
            .caps(&caps)
            .format(gst::Format::Time)
            .is_live(false)
            .block(true)
            .max_bytes(u64::from(w * h * 4) * 2)
            .build();
        Self { src, w, h }
    }

    fn element(&self) -> gst::Element {
        self.src.clone().upcast()
    }

    /// A frame of `outside` with `inside` over `rect`.
    fn painted(&self, rect: IRect, inside: [u8; 3], outside: [u8; 3]) -> Vec<u8> {
        let px = |c: [u8; 3]| [c[2], c[1], c[0], 255];
        let (w, h) = (self.w as usize, self.h as usize);
        let plain: Vec<u8> = (0..w).flat_map(|_| px(outside)).collect();
        let mixed: Vec<u8> = (0..w)
            .flat_map(|x| {
                let x = x as i32;
                px(if x >= rect.x && x < rect.right() {
                    inside
                } else {
                    outside
                })
            })
            .collect();
        let mut data = Vec::with_capacity(w * h * 4);
        for y in 0..h as i32 {
            data.extend_from_slice(if y >= rect.y && y < rect.bottom() {
                &mixed
            } else {
                &plain
            });
        }
        data
    }

    fn solid(&self, color: [u8; 3]) -> Vec<u8> {
        self.painted(IRect::new(0, 0, 0, 0), color, color)
    }

    fn push(&self, pts: f64, data: Vec<u8>, crop_meta: Option<(u32, u32, u32, u32)>) {
        let mut buffer = gst::Buffer::from_slice(data);
        {
            let buffer = buffer.get_mut().unwrap();
            buffer.set_pts(secs(pts));
            if let Some(rect) = crop_meta {
                gst_video::VideoCropMeta::add(buffer, rect);
            }
        }
        self.src.push_buffer(buffer).unwrap();
    }

    /// Frames of one colour at `fps` from `from` (inclusive) to `to` (exclusive).
    fn frames(&self, from: f64, to: f64, fps: f64, color: [u8; 3]) {
        let data = self.solid(color);
        let mut i = 0;
        loop {
            let pts = from + f64::from(i) / fps;
            if pts >= to - 1e-9 {
                break;
            }
            self.push(pts, data.clone(), None);
            i += 1;
        }
    }
}

/// An audio source pushed by the test: a 440 Hz tone, S16LE mono 48 kHz, 20 ms buffers.
struct TestAudio {
    src: gst_app::AppSrc,
    amplitude: f64,
}

impl TestAudio {
    const RATE: usize = 48_000;

    fn new(amplitude: f64) -> Self {
        gst::init().unwrap();
        let caps = gst::Caps::builder("audio/x-raw")
            .field("format", "S16LE")
            .field("rate", Self::RATE as i32)
            .field("channels", 1i32)
            .field("layout", "interleaved")
            .build();
        let src = gst_app::AppSrc::builder()
            .caps(&caps)
            .format(gst::Format::Time)
            .is_live(false)
            .build();
        Self { src, amplitude }
    }

    fn element(&self) -> gst::Element {
        self.src.clone().upcast()
    }

    fn tone(&self, from: f64, to: f64) {
        let chunk = Self::RATE / 50;
        let mut start = (from * Self::RATE as f64).round() as usize;
        let end = (to * Self::RATE as f64).round() as usize;
        while start < end {
            let n = chunk.min(end - start);
            let data: Vec<u8> = (start..start + n)
                .flat_map(|i| {
                    let t = i as f64 / Self::RATE as f64;
                    let v = (t * 440.0 * std::f64::consts::TAU).sin() * self.amplitude;
                    ((v * f64::from(i16::MAX)) as i16).to_le_bytes()
                })
                .collect();
            let mut buffer = gst::Buffer::from_slice(data);
            {
                let buffer = buffer.get_mut().unwrap();
                buffer.set_pts(secs(start as f64 / Self::RATE as f64));
                buffer.set_duration(secs(n as f64 / Self::RATE as f64));
            }
            self.src.push_buffer(buffer).unwrap();
            start += n;
        }
    }
}

/// Ends the test sources and waits until the glue has passed on everything they sent.
fn drain(recording: &Recording, video: &TestVideo, audio: &[&TestAudio]) {
    video.src.end_of_stream().unwrap();
    for source in audio {
        source.src.end_of_stream().unwrap();
    }
    let wanted = recording.audio_sources();
    let deadline = std::time::Instant::now() + Duration::from_secs(120);
    loop {
        {
            let timeline = recording.shared.timeline();
            if timeline.video_ended && wanted.iter().all(|s| timeline.audio_ended[s.index()]) {
                return;
            }
        }
        assert!(
            std::time::Instant::now() < deadline,
            "sources did not drain"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// One decoded video frame, tightly packed RGB.
pub(crate) struct VFrame {
    pub(crate) pts: f64,
    pub(crate) width: usize,
    pub(crate) height: usize,
    rgb: Vec<u8>,
}

impl VFrame {
    pub(crate) fn at(&self, x: usize, y: usize) -> [u8; 3] {
        let i = (y * self.width + x) * 3;
        [self.rgb[i], self.rgb[i + 1], self.rgb[i + 2]]
    }

    /// Points on a grid over the whole frame, edges included (inset 3 px against chroma bleed).
    pub(crate) fn grid(&self) -> Vec<[u8; 3]> {
        let mut points = Vec::new();
        for i in 0..=16 {
            for j in 0..=16 {
                let x = 3 + i * (self.width - 7) / 16;
                let y = 3 + j * (self.height - 7) / 16;
                points.push(self.at(x, y));
            }
        }
        points
    }
}

/// One decoded audio buffer of a track: timestamp and RMS (full scale 1.0).
struct AChunk {
    pts: f64,
    rms: f64,
}

pub(crate) struct Analysis {
    pub(crate) frames: Vec<VFrame>,
    audio: Vec<Vec<AChunk>>,
}

impl Analysis {
    /// Video length: last frame's timestamp plus one frame.
    pub(crate) fn duration(&self) -> f64 {
        self.frames.last().map_or(0.0, |f| f.pts + FRAME)
    }
}

/// Decodes a working file completely (also one that was never finished).
pub(crate) fn analyse(path: &Path) -> Analysis {
    let frames = Arc::new(Mutex::new(Vec::<VFrame>::new()));
    let audio = Arc::new(Mutex::new(Vec::<Vec<AChunk>>::new()));
    let pipeline = gst::Pipeline::new();
    let src = gst::ElementFactory::make("filesrc")
        .property("location", path.to_str().unwrap())
        .build()
        .unwrap();
    let decode = gst::ElementFactory::make("decodebin").build().unwrap();
    pipeline.add_many([&src, &decode]).unwrap();
    src.link(&decode).unwrap();
    let weak = pipeline.downgrade();
    let (frames_in, audio_in) = (Arc::clone(&frames), Arc::clone(&audio));
    decode.connect_pad_added(move |_, pad| {
        let Some(pipeline) = weak.upgrade() else {
            return;
        };
        let caps = pad.current_caps().unwrap();
        let name = caps.structure(0).unwrap().name().to_string();
        let convert = if name.starts_with("video/") {
            gst::ElementFactory::make("videoconvert").build().unwrap()
        } else {
            gst::ElementFactory::make("audioconvert").build().unwrap()
        };
        let sink = if name.starts_with("video/") {
            let sink = gst_app::AppSink::builder()
                .caps(
                    &gst::Caps::builder("video/x-raw")
                        .field("format", "RGB")
                        .build(),
                )
                .sync(false)
                .build();
            let frames = Arc::clone(&frames_in);
            sink.set_callbacks(
                gst_app::AppSinkCallbacks::builder()
                    .new_sample(move |sink| {
                        let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                        let info = gst_video::VideoInfo::from_caps(sample.caps().unwrap()).unwrap();
                        let buffer = sample.buffer().unwrap();
                        let frame =
                            gst_video::VideoFrameRef::from_buffer_ref_readable(buffer, &info)
                                .unwrap();
                        let (w, h) = (info.width() as usize, info.height() as usize);
                        let stride = frame.plane_stride()[0] as usize;
                        let data = frame.plane_data(0).unwrap();
                        let mut rgb = Vec::with_capacity(w * h * 3);
                        for y in 0..h {
                            rgb.extend_from_slice(&data[y * stride..y * stride + w * 3]);
                        }
                        frames.lock().unwrap().push(VFrame {
                            pts: buffer.pts().unwrap().nseconds() as f64 / 1e9,
                            width: w,
                            height: h,
                            rgb,
                        });
                        Ok(gst::FlowSuccess::Ok)
                    })
                    .build(),
            );
            sink
        } else {
            let sink = gst_app::AppSink::builder()
                .caps(
                    &gst::Caps::builder("audio/x-raw")
                        .field("format", "F32LE")
                        .field("channels", 1i32)
                        .field("layout", "interleaved")
                        .build(),
                )
                .sync(false)
                .build();
            let track = {
                let mut audio = audio_in.lock().unwrap();
                audio.push(Vec::new());
                audio.len() - 1
            };
            let audio = Arc::clone(&audio_in);
            sink.set_callbacks(
                gst_app::AppSinkCallbacks::builder()
                    .new_sample(move |sink| {
                        let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                        let buffer = sample.buffer().unwrap();
                        let map = buffer.map_readable().unwrap();
                        let samples: Vec<f32> = map
                            .chunks_exact(4)
                            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                            .collect();
                        let rms = (samples.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>()
                            / samples.len().max(1) as f64)
                            .sqrt();
                        audio.lock().unwrap()[track].push(AChunk {
                            pts: buffer.pts().map_or(0.0, |p| p.nseconds() as f64 / 1e9),
                            rms,
                        });
                        Ok(gst::FlowSuccess::Ok)
                    })
                    .build(),
            );
            sink
        };
        pipeline.add_many([&convert, sink.upcast_ref()]).unwrap();
        convert.link(&sink).unwrap();
        convert.sync_state_with_parent().unwrap();
        sink.sync_state_with_parent().unwrap();
        pad.link(&convert.static_pad("sink").unwrap()).unwrap();
    });
    pipeline.set_state(gst::State::Playing).unwrap();
    let bus = pipeline.bus().unwrap();
    let message = bus
        .timed_pop_filtered(
            gst::ClockTime::from_seconds(120),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        )
        .expect("decoding finishes");
    if let gst::MessageView::Error(err) = message.view() {
        panic!("decoding {} failed: {}", path.display(), err.error());
    }
    pipeline.set_state(gst::State::Null).unwrap();
    let mut frames = std::mem::take(&mut *frames.lock().unwrap());
    frames.sort_by(|a, b| a.pts.total_cmp(&b.pts));
    let audio = std::mem::take(&mut *audio.lock().unwrap());
    Analysis { frames, audio }
}

fn record(name: &str, video: &TestVideo, area: VideoArea) -> Recording {
    let config = RecordingConfig::new(folder(name), video.element(), area);
    start(config).unwrap().0
}

fn events(receiver: &mut mpsc::UnboundedReceiver<RecordingEvent>) -> Vec<RecordingEvent> {
    let mut all = Vec::new();
    while let Ok(event) = receiver.try_recv() {
        all.push(event);
    }
    all
}

fn cleanup(path: &Path) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

// spec: screen-recording / Recording source / Record a region
#[test]
fn a_region_recording_has_the_selection_size_and_only_its_pixels() {
    let video = TestVideo::new(1280, 720);
    let selection = IRect::new(240, 60, 800, 600);
    let stream = MonitorStream {
        position: (0, 0),
        size: Size::new(1280, 720),
    };
    let recording = record("region", &video, VideoArea::Region { selection, stream });
    let frame = video.painted(selection, BLUE, RED);
    for i in 0..6 {
        video.push(f64::from(i) * FRAME, frame.clone(), None);
    }
    drain(&recording, &video, &[]);
    let done = recording.stop_at(secs(6.0 * FRAME));
    assert_eq!(done.error, None);
    let analysis = analyse(&done.path);
    assert!(analysis.frames.len() >= 6);
    for frame in &analysis.frames {
        assert_eq!((frame.width, frame.height), (800, 600));
        assert!(
            frame.grid().iter().all(|c| near(*c, BLUE)),
            "only the selection"
        );
    }
    cleanup(&done.path);
}

// spec: screen-recording / Recording source / Record a region on a scaled monitor
#[test]
fn a_region_on_a_scaled_monitor_records_the_buffer_area() {
    let video = TestVideo::new(3840, 2160);
    let stream = MonitorStream {
        position: (0, 0),
        size: Size::new(3072, 1728),
    };
    let area = VideoArea::Region {
        selection: IRect::new(100, 100, 800, 600),
        stream,
    };
    let recording = record("scaled", &video, area);
    let frame = video.painted(IRect::new(125, 125, 1000, 750), BLUE, RED);
    for i in 0..3 {
        video.push(f64::from(i) * FRAME, frame.clone(), None);
    }
    drain(&recording, &video, &[]);
    let done = recording.stop_at(secs(3.0 * FRAME));
    assert_eq!(done.error, None);
    let analysis = analyse(&done.path);
    assert!(analysis.frames.len() >= 3);
    for frame in &analysis.frames {
        assert_eq!((frame.width, frame.height), (1000, 750));
        assert!(frame.grid().iter().all(|c| near(*c, BLUE)));
    }
    cleanup(&done.path);
}

// spec: screen-recording / Recording source / Region not on the chosen screen
#[test]
fn a_region_off_the_stream_writes_nothing() {
    let video = TestVideo::new(1920, 1080);
    let dir = folder("off-screen");
    let area = VideoArea::Region {
        selection: IRect::new(100, 100, 800, 600),
        stream: MonitorStream {
            position: (1920, 0),
            size: Size::new(1920, 1080),
        },
    };
    let result = start(RecordingConfig::new(dir.clone(), video.element(), area));
    assert_eq!(result.err(), Some(RecordingError::RegionNotOnScreen));
    let written = std::fs::read_dir(&dir).map_or(0, |entries| entries.count());
    assert_eq!(written, 0, "nothing is written");
    let _ = std::fs::remove_dir_all(&dir);
}

// spec: screen-recording / Recording source / Record the screen
#[test]
fn a_screen_recording_keeps_the_full_frame() {
    let video = TestVideo::new(1280, 720);
    let recording = record("screen", &video, VideoArea::Whole);
    // A blue border 8 px wide around green: a crop would cut the border away.
    let frame = video.painted(IRect::new(8, 8, 1264, 704), GREEN, BLUE);
    for i in 0..3 {
        video.push(f64::from(i) * FRAME, frame.clone(), None);
    }
    drain(&recording, &video, &[]);
    let done = recording.stop_at(secs(3.0 * FRAME));
    let analysis = analyse(&done.path);
    assert!(analysis.frames.len() >= 3);
    for frame in &analysis.frames {
        assert_eq!((frame.width, frame.height), (1280, 720));
        assert!(near(frame.at(2, 2), BLUE) && near(frame.at(1277, 717), BLUE));
        assert!(near(frame.at(640, 360), GREEN));
    }
    cleanup(&done.path);
}

// spec: screen-recording / Recording source / Recorded window resized
#[test]
fn a_resized_window_is_cropped_per_buffer_and_letterboxed() {
    // P6's buffers: 6144×3456 for a stream of size 3072×1728 on a monitor at scale 1.25.
    let video = TestVideo::new(6144, 3456);
    let content: ContentRect = Arc::new(|buffer, info| {
        let (x, y, w, h) = buffer.meta::<gst_video::VideoCropMeta>()?.rect();
        window_crop_in_buffer(
            IRect::new(x as i32, y as i32, w as i32, h as i32),
            Size::new(3072, 1728),
            Size::new(info.width() as i32, info.height() as i32),
            Some(1.25),
        )
    });
    let recording = record("window", &video, VideoArea::Window(content));
    // Content green (normal) or blue (maximised); everything outside it red (undefined pixels).
    let normal = video.painted(IRect::new(0, 0, 1280, 800), GREEN, RED);
    let maximised = video.painted(IRect::new(0, 0, 6144, 3360), BLUE, RED);
    video.push(0.0, normal.clone(), Some((0, 0, 800, 500)));
    video.push(FRAME, maximised, Some((0, 0, 3840, 2100)));
    video.push(2.0 * FRAME, normal, Some((0, 0, 800, 500)));
    drain(&recording, &video, &[]);
    let done = recording.stop_at(secs(3.0 * FRAME));
    assert_eq!(done.error, None);
    let analysis = analyse(&done.path);
    assert!(analysis.frames.len() >= 3);
    for (i, frame) in analysis.frames.iter().enumerate() {
        assert_eq!((frame.width, frame.height), (1280, 800), "frame size kept");
        assert!(
            frame.grid().iter().all(|c| !near(*c, RED)),
            "frame {i} shows pixels outside the content rectangle"
        );
    }
    let at = |pts: f64| {
        analysis
            .frames
            .iter()
            .find(|f| (f.pts - pts).abs() < 0.005)
            .expect("a frame at each buffer's time")
    };
    let (first, larger, back) = (at(0.0), at(FRAME), at(2.0 * FRAME));
    assert!(near(first.at(640, 20), GREEN) && near(back.at(640, 20), GREEN));
    // 6144×3360 scaled by 1280/6144 = 1280×700, 50 px black bars above and below.
    let fit = geometry::letterbox(Size::new(1280, 800), Size::new(6144, 3360));
    assert_eq!(fit, IRect::new(0, 50, 1280, 700));
    assert!(near(larger.at(640, 20), BLACK) && near(larger.at(640, 780), BLACK));
    assert!(near(larger.at(640, 60), BLUE) && near(larger.at(640, 740), BLUE));
    assert!(near(larger.at(3, 400), BLUE) && near(larger.at(1276, 400), BLUE));
    cleanup(&done.path);
}

// spec: screen-recording / Recording frame rate / Still screen
#[test]
fn a_still_screen_repeats_the_last_frame() {
    let video = TestVideo::new(SMALL.0, SMALL.1);
    let recording = record("still", &video, VideoArea::Whole);
    video.frames(0.0, 5.0, 30.0, GREEN);
    // No buffer from 0:05 to 0:15.
    video.frames(15.0, 20.0, 30.0, RED);
    drain(&recording, &video, &[]);
    let done = recording.stop_at(secs(20.0));
    let analysis = analyse(&done.path);
    let duration = analysis.duration();
    assert!((duration - 20.0).abs() <= 0.2, "lasts {duration} s");
    for pair in analysis.frames.windows(2) {
        assert!((pair[1].pts - pair[0].pts - FRAME).abs() < 0.002, "30 fps");
    }
    let gap: Vec<&VFrame> = analysis
        .frames
        .iter()
        .filter(|f| f.pts >= 5.0 && f.pts < 14.99)
        .collect();
    assert!(gap.len() >= 299, "{} frames in the gap", gap.len());
    assert!(gap.iter().all(|f| near(f.at(32, 24), GREEN)));
    cleanup(&done.path);
}

// spec: screen-recording / Recording frame rate / Burst of buffers
#[test]
fn a_burst_of_buffers_keeps_its_length() {
    let video = TestVideo::new(SMALL.0, SMALL.1);
    let recording = record("burst", &video, VideoArea::Whole);
    video.frames(0.0, 5.0, 60.0, GREEN);
    video.frames(5.0, 10.0, 30.0, BLUE);
    drain(&recording, &video, &[]);
    let done = recording.stop_at(secs(10.0));
    let analysis = analyse(&done.path);
    let burst = analysis
        .frames
        .iter()
        .filter(|f| near(f.at(32, 24), GREEN))
        .count();
    let length = burst as f64 * FRAME;
    assert!((length - 5.0).abs() <= 0.1, "the burst lasts {length} s");
    assert!((analysis.duration() - 10.0).abs() <= 0.1);
    for pair in analysis.frames.windows(2) {
        assert!((pair[1].pts - pair[0].pts - FRAME).abs() < 0.002, "30 fps");
    }
    cleanup(&done.path);
}

// spec: screen-recording / Pause and resume / Paused time is not recorded
#[test]
fn paused_time_is_left_out_without_a_gap() {
    let video = TestVideo::new(SMALL.0, SMALL.1);
    let recording = record("pause", &video, VideoArea::Whole);
    video.frames(0.0, 10.0, 30.0, BLUE);
    recording.pause_at(secs(10.0));
    assert!(recording.is_paused());
    video.frames(10.0, 15.0, 30.0, RED);
    recording.resume_at(secs(15.0));
    assert!(!recording.is_paused());
    video.frames(15.0, 25.0, 30.0, GREEN);
    drain(&recording, &video, &[]);
    let done = recording.stop_at(secs(25.0));
    assert_eq!(done.duration, secs(20.0));
    let analysis = analyse(&done.path);
    let duration = analysis.duration();
    assert!((duration - 20.0).abs() <= 0.2, "lasts {duration} s");
    assert!(analysis.frames.iter().all(|f| !near(f.at(32, 24), RED)));
    assert!(analysis
        .frames
        .iter()
        .all(|f| near(f.at(32, 24), if f.pts < 9.99 { BLUE } else { GREEN })));
    for pair in analysis.frames.windows(2) {
        assert!((pair[1].pts - pair[0].pts - FRAME).abs() < 0.002, "no gap");
    }
    cleanup(&done.path);
}

// spec: screen-recording / Working file / Recording survives a crash
#[test]
fn an_abandoned_recording_stays_playable() {
    let video = TestVideo::new(SMALL.0, SMALL.1);
    let mic = TestAudio::new(0.5);
    let config = RecordingConfig::new(folder("crash"), video.element(), VideoArea::Whole)
        .with_audio(AudioSource::Microphone, mic.element());
    let (recording, _events) = start(config).unwrap();
    let path = recording.path().to_path_buf();
    video.frames(0.0, 10.0, 30.0, GREEN);
    mic.tone(0.0, 10.0);
    drain(&recording, &video, &[&mic]);
    // A live pipeline keeps up with the stream; give the encoder the time it would have had.
    std::thread::sleep(Duration::from_secs(1));
    // Killed 10 s in: no end of stream, the muxer never finishes the file.
    drop(recording);
    assert!(path.exists());
    let analysis = analyse(&path);
    let duration = analysis.duration();
    assert!(duration >= 9.0, "plays for {duration} s");
    cleanup(&path);
}

// spec: screen-recording / Working file / Recording error
#[test]
fn an_error_mid_recording_is_reported_and_keeps_what_was_written() {
    let video = TestVideo::new(SMALL.0, SMALL.1);
    let mut config = RecordingConfig::new(folder("error"), video.element(), VideoArea::Whole);
    // The pipeline fails 10 s in (300 frames at 30 fps).
    config.fault_after = Some(300);
    let (recording, mut receiver) = start(config).unwrap();
    video.frames(0.0, 15.0, 30.0, GREEN);
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    let error = loop {
        if let Some(RecordingEvent::Error(error)) = events(&mut receiver)
            .into_iter()
            .find(|e| matches!(e, RecordingEvent::Error(_)))
        {
            break error;
        }
        assert!(std::time::Instant::now() < deadline, "no error reported");
        std::thread::sleep(Duration::from_millis(10));
    };
    let RecordingError::Stream { element, message } = &error else {
        panic!("{error:?}")
    };
    assert_eq!(element, "fault");
    assert!(!message.is_empty());
    let done = recording.stop();
    assert_eq!(done.error, Some(error));
    let analysis = analyse(&done.path);
    let duration = analysis.duration();
    assert!(
        (9.0..=10.1).contains(&duration),
        "what was written until the error is kept: {duration} s"
    );
    cleanup(&done.path);
}

/// Every device present, or none.
struct Devices(bool);

impl AudioDevices for Devices {
    fn has_microphone(&self) -> bool {
        self.0
    }
    fn has_output(&self) -> bool {
        self.0
    }
}

/// Loudest and quietest RMS of a track between two times.
fn rms_range(track: &[AChunk], from: f64, to: f64) -> (f64, f64) {
    let values: Vec<f64> = track
        .iter()
        .filter(|c| c.pts >= from && c.pts < to)
        .map(|c| c.rms)
        .collect();
    assert!(!values.is_empty(), "audio between {from} and {to}");
    (
        values.iter().copied().fold(f64::MAX, f64::min),
        values.iter().copied().fold(0.0, f64::max),
    )
}

// spec: screen-recording / Audio / Mute the microphone
#[test]
fn a_muted_microphone_is_silent_only_while_muted() {
    let video = TestVideo::new(SMALL.0, SMALL.1);
    let mic = TestAudio::new(0.5);
    let config = RecordingConfig::new(folder("mute-mic"), video.element(), VideoArea::Whole)
        .with_audio(AudioSource::Microphone, mic.element());
    let (recording, mut receiver) = start(config).unwrap();
    recording.set_muted_at(AudioSource::Microphone, secs(5.0), true);
    recording.set_muted_at(AudioSource::Microphone, secs(8.0), false);
    assert!(!recording.is_muted(AudioSource::Microphone));
    video.frames(0.0, 10.0, 30.0, GREEN);
    mic.tone(0.0, 10.0);
    drain(&recording, &video, &[&mic]);
    let done = recording.stop_at(secs(10.0));
    assert_eq!(done.audio, vec![AudioSource::Microphone]);
    let analysis = analyse(&done.path);
    assert_eq!(analysis.audio.len(), 1);
    let track = &analysis.audio[0];
    assert!(rms_range(track, 0.5, 4.9).0 > 0.1, "present before");
    assert!(rms_range(track, 5.1, 7.9).1 < 0.001, "silent while muted");
    assert!(rms_range(track, 8.1, 9.8).0 > 0.1, "present after");
    // The track keeps its length.
    let end = track.last().map_or(0.0, |c| c.pts);
    assert!(end > 9.8, "audio until {end} s");
    // Level meters were fed.
    assert!(events(&mut receiver).iter().any(|e| matches!(
        e,
        RecordingEvent::Level {
            source: AudioSource::Microphone,
            ..
        }
    )));
    cleanup(&done.path);
}

// spec: screen-recording / Audio / Mute system audio
#[test]
fn muted_system_audio_is_silent_until_unmuted_and_the_mic_goes_on() {
    let video = TestVideo::new(SMALL.0, SMALL.1);
    // Different loudness tells the tracks apart.
    let mic = TestAudio::new(0.5);
    let system = TestAudio::new(0.1);
    let config = RecordingConfig::new(folder("mute-system"), video.element(), VideoArea::Whole)
        .with_audio(AudioSource::Microphone, mic.element())
        .with_audio(AudioSource::SystemAudio, system.element());
    let (recording, _events) = start(config).unwrap();
    recording.set_muted_at(AudioSource::SystemAudio, secs(3.0), true);
    assert!(recording.is_muted(AudioSource::SystemAudio));
    assert!(!recording.is_muted(AudioSource::Microphone));
    recording.set_muted_at(AudioSource::SystemAudio, secs(6.0), false);
    video.frames(0.0, 8.0, 30.0, GREEN);
    mic.tone(0.0, 8.0);
    system.tone(0.0, 8.0);
    drain(&recording, &video, &[&mic, &system]);
    let done = recording.stop_at(secs(8.0));
    let analysis = analyse(&done.path);
    assert_eq!(analysis.audio.len(), 2);
    let loudest = |track: &[AChunk]| rms_range(track, 0.5, 2.9).1;
    let (mic_track, system_track) = if loudest(&analysis.audio[0]) > loudest(&analysis.audio[1]) {
        (&analysis.audio[0], &analysis.audio[1])
    } else {
        (&analysis.audio[1], &analysis.audio[0])
    };
    assert!(rms_range(system_track, 0.5, 2.9).0 > 0.02, "present before");
    assert!(
        rms_range(system_track, 3.1, 5.9).1 < 0.001,
        "silent from then on"
    );
    assert!(
        rms_range(system_track, 6.1, 7.8).0 > 0.02,
        "present after unmuting"
    );
    assert!(
        rms_range(mic_track, 0.5, 7.8).0 > 0.1,
        "the microphone is not muted"
    );
    cleanup(&done.path);
}

// spec: screen-recording / Audio / No audio selected
#[test]
fn without_audio_sources_the_file_has_no_audio_track() {
    assert!(sources_to_record(&Devices(true), false, false).is_empty());
    assert_eq!(
        sources_to_record(&Devices(true), true, false),
        vec![AudioSource::Microphone]
    );
    // A wanted source without a device is not recorded either.
    assert!(sources_to_record(&Devices(false), true, true).is_empty());

    let video = TestVideo::new(SMALL.0, SMALL.1);
    let recording = record("no-audio", &video, VideoArea::Whole);
    video.frames(0.0, 5.0, 30.0, GREEN);
    drain(&recording, &video, &[]);
    let done = recording.stop_at(secs(5.0));
    assert!(done.audio.is_empty());
    let analysis = analyse(&done.path);
    assert!((analysis.duration() - 5.0).abs() <= 0.2);
    assert!(analysis.audio.is_empty(), "no audio track");
    cleanup(&done.path);
}
