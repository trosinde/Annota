//! Pipeline tests with test sources (`appsrc` with chosen timestamps), fast: synthetic time, no
//! display, no portal, no audio device. Every working file is decoded again and measured.

use std::sync::atomic::{AtomicUsize, Ordering};

use gstreamer_video::prelude::*;

use super::*;
use crate::recording::audio::{sources_to_record, AudioDevices};
use crate::recording::geometry::Scales;

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
        Self::with_format(w, h, "BGRx")
    }

    fn with_format(w: u32, h: u32, format: &str) -> Self {
        gst::init().unwrap();
        let caps = gst::Caps::builder("video/x-raw")
            .field("format", format)
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
        self.tone_of(from, to, self.amplitude);
    }

    /// The tone at `amplitude` (0: silence) from `from` to `to`.
    fn tone_of(&self, from: f64, to: f64, amplitude: f64) {
        let chunk = Self::RATE / 50;
        let mut start = (from * Self::RATE as f64).round() as usize;
        let end = (to * Self::RATE as f64).round() as usize;
        while start < end {
            let n = chunk.min(end - start);
            let data: Vec<u8> = (start..start + n)
                .flat_map(|i| {
                    let t = i as f64 / Self::RATE as f64;
                    let v = (t * 440.0 * std::f64::consts::TAU).sin() * amplitude;
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
pub(crate) struct AChunk {
    pub(crate) pts: f64,
    pub(crate) rms: f64,
}

pub(crate) struct Analysis {
    pub(crate) frames: Vec<VFrame>,
    pub(crate) audio: Vec<Vec<AChunk>>,
}

impl Analysis {
    /// Video length: last frame's timestamp plus one frame.
    pub(crate) fn duration(&self) -> f64 {
        self.frames.last().map_or(0.0, |f| f.pts + FRAME)
    }
}

/// Decodes a working file completely (also one that was never finished).
pub(crate) fn analyse(path: &Path) -> Analysis {
    // The parent of a crash test creates no pipeline of its own before this.
    gst::init().unwrap();
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

/// A recording of synthetic time without a heartbeat (the clock is never set): every gap is
/// filled only by the glue and the stop.
fn record(name: &str, video: &TestVideo, area: VideoArea) -> Recording {
    let config = RecordingConfig::new(folder(name), video.element(), area)
        .manual_clock(&Arc::new(Mutex::new(None)));
    start(config).unwrap().0
}

fn events(receiver: &mut mpsc::UnboundedReceiver<RecordingEvent>) -> Vec<RecordingEvent> {
    let mut all = Vec::new();
    while let Ok(event) = receiver.try_recv() {
        all.push(event);
    }
    all
}

/// Runs a stop's future on a main context of its own, as the app's main loop would.
fn finish(stopping: impl Future<Output = Finished>) -> Finished {
    let context = glib::MainContext::new();
    context
        .with_thread_default(|| context.block_on(stopping))
        .expect("a main context of its own")
}

impl RecordingConfig {
    /// Time is synthetic: the heartbeat follows `clock` (none until it is set) instead of the
    /// sources' running time.
    fn manual_clock(mut self, clock: &Arc<Mutex<Option<gst::ClockTime>>>) -> Self {
        self.clock = Clock::Manual(Arc::clone(clock));
        self
    }
}

/// Waits (up to 60 s) until `done` holds.
fn wait_until(done: impl Fn() -> bool, what: &str) {
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    while !done() {
        assert!(std::time::Instant::now() < deadline, "timed out: {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
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
    let done = recording.stop_now(secs(6.0 * FRAME));
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
    let done = recording.stop_now(secs(3.0 * FRAME));
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
    let done = recording.stop_now(secs(3.0 * FRAME));
    let analysis = analyse(&done.path);
    assert!(analysis.frames.len() >= 3);
    for frame in &analysis.frames {
        assert_eq!((frame.width, frame.height), (1280, 720));
        assert!(near(frame.at(2, 2), BLUE) && near(frame.at(1277, 717), BLUE));
        assert!(near(frame.at(640, 360), GREEN));
    }
    cleanup(&done.path);
}

/// Scales unknown and buffers at the monitor's scale: the frame keeps the buffer's pixels.
const KEEP_PIXELS: Scales = Scales {
    stream_size: None,
    monitor: None,
};

/// A window [`VideoArea`] whose content rectangle is the crop meta as it is.
fn meta_window(scales: Scales) -> VideoArea {
    let content: ContentRect = Arc::new(|buffer, _| {
        let (x, y, w, h) = buffer.meta::<gst_video::VideoCropMeta>()?.rect();
        Some(IRect::new(x as i32, y as i32, w as i32, h as i32))
    });
    VideoArea::Window { content, scales }
}

// spec: screen-recording / Recording source / Recorded window resized
#[test]
fn a_resized_window_is_cropped_per_buffer_and_letterboxed() {
    // Buffers at the monitor's scale (1): the window's on-screen size is its buffer size.
    let video = TestVideo::new(1920, 1080);
    let recording = record("window", &video, meta_window(KEEP_PIXELS));
    // Content green (normal) or blue (maximised); everything outside it red (undefined pixels).
    let normal = video.painted(IRect::new(0, 0, 400, 250), GREEN, RED);
    let maximised = video.painted(IRect::new(0, 0, 1920, 1050), BLUE, RED);
    video.push(0.0, normal.clone(), Some((0, 0, 400, 250)));
    video.push(FRAME, maximised, Some((0, 0, 1920, 1050)));
    video.push(2.0 * FRAME, normal, Some((0, 0, 400, 250)));
    drain(&recording, &video, &[]);
    let done = recording.stop_now(secs(3.0 * FRAME));
    assert_eq!(done.error, None);
    let analysis = analyse(&done.path);
    assert!(analysis.frames.len() >= 3);
    for (i, frame) in analysis.frames.iter().enumerate() {
        assert_eq!((frame.width, frame.height), (400, 250), "frame size kept");
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
    assert!(near(first.at(200, 10), GREEN) && near(back.at(200, 10), GREEN));
    // 1920×1050 scaled by 400/1920 = 400×219, black bars above and below.
    let fit = geometry::letterbox(Size::new(400, 250), Size::new(1920, 1050));
    assert_eq!(fit, IRect::new(0, 15, 400, 219));
    assert!(near(larger.at(200, 5), BLACK) && near(larger.at(200, 245), BLACK));
    assert!(near(larger.at(200, 22), BLUE) && near(larger.at(200, 228), BLUE));
    assert!(near(larger.at(3, 125), BLUE) && near(larger.at(396, 125), BLUE));
    cleanup(&done.path);
}

/// The maintainer's decision of 2026-11-24 through the real encoder: a 640×400 logical window
/// on a monitor at 1.25 whose stream delivers 6144×3456 buffers at scale 2 (stream `size`
/// 3072×1728), found by `window_rect` past its monitor-scale crop meta (0, 0, 800, 500), is
/// recorded at 800×500, its size on screen; with its outline (1284×804 buffer px) at 802×502.
/// The whole window is in the frame: its four coloured corners where they belong.
// spec: screen-recording / Recording source / Window recorded at its on-screen size
// spec: screen-recording / Recording source / Record a window
#[test]
fn a_window_is_recorded_at_its_size_on_screen_in_monitor_pixels() {
    use crate::recording::window::tests::layered;
    use crate::recording::window::WindowContent;
    let bgr = |c: [u8; 3]| [c[2], c[1], c[0]];
    const YELLOW: [u8; 3] = [255, 255, 0];
    const WHITE: [u8; 3] = [255, 255, 255];
    let scales = Scales {
        stream_size: Some((3072, 1728)),
        monitor: Some(1.25),
    };
    for (window, meta, frame) in [
        (IRect::new(0, 0, 1280, 800), (0, 0, 800, 500), (800, 500)),
        (IRect::new(0, 0, 1284, 804), (0, 0, 803, 503), (802, 502)),
    ] {
        let sq = 64;
        let (r, b) = (window.right() - sq, window.bottom() - sq);
        let data = layered(
            6144,
            3456,
            &[
                (IRect::new(3000, 0, 3144, 3456), bgr(WHITE), 255),
                (window, bgr(GREEN), 255),
                (IRect::new(0, 0, sq, sq), bgr(RED), 255),
                (IRect::new(r, 0, sq, sq), bgr(BLUE), 255),
                (IRect::new(0, b, sq, sq), bgr(YELLOW), 255),
                (IRect::new(r, b, sq, sq), bgr(WHITE), 255),
            ],
        );
        let video = TestVideo::with_format(6144, 3456, "BGRA");
        let recording = record(
            "window-size",
            &video,
            WindowContent::new().into_area(scales),
        );
        for i in 0..3 {
            video.push(f64::from(i) * FRAME, data.clone(), Some(meta));
        }
        drain(&recording, &video, &[]);
        let done = recording.stop_now(secs(3.0 * FRAME));
        assert_eq!(done.error, None);
        let analysis = analyse(&done.path);
        assert!(analysis.frames.len() >= 3);
        let (fw, fh) = frame;
        for f in &analysis.frames {
            assert_eq!((f.width, f.height), frame, "the window's size on screen");
            // The corner squares, 40 px at the frame's scale: whole window, no border.
            assert!(near(f.at(5, 5), RED), "top-left");
            assert!(near(f.at(fw - 6, 5), BLUE), "top-right");
            assert!(near(f.at(5, fh - 6), YELLOW), "bottom-left");
            assert!(near(f.at(fw - 6, fh - 6), WHITE), "bottom-right");
            assert!(near(f.at(fw / 2, fh / 2), GREEN), "centre");
            assert!(
                near(f.at(fw / 2, fh - 1), GREEN),
                "no bar at the bottom edge"
            );
            assert!(
                near(f.at(fw - 1, fh / 2), GREEN),
                "no bar at the right edge"
            );
        }
        cleanup(&done.path);
    }
}

/// The live sequence of 2026-11-24 through the real encoder, with the production
/// [`WindowContent`](crate::recording::window::WindowContent) (alpha trim per buffer): P6's
/// 6144×3456 BGRA buffers at scale 1.25, the window at rest with its CSD shadow, GNOME's
/// maximize animation frame and then the settled maximized window under the same crop meta,
/// the unmaximize animation frames and the window at rest again. The frame is the window's size
/// on screen (1284×804 buffer px × 1.25 ÷ 2 = 802×502); the settled maximized window is
/// letterboxed whole (header and title buttons), and the window at rest is framed exactly as at
/// the start; the shadow (red here) and the undefined pixels never show.
// spec: screen-recording / Recording source / Recorded window resized
#[test]
fn a_maximized_and_restored_window_is_letterboxed_whole_and_framed_as_at_start() {
    use crate::recording::window::tests::{layered, MAX_META, REST_META};
    use crate::recording::window::WindowContent;
    // B, G, R as in a BGRA buffer.
    let bgr = |c: [u8; 3]| [c[2], c[1], c[0]];
    const WHITE: [u8; 3] = [255, 255, 255];
    const STALE: [u8; 3] = [200, 0, 200];
    let rest = |outline: u8| {
        layered(
            6144,
            3456,
            &[
                (IRect::new(2000, 0, 4144, 3456), bgr(STALE), 255),
                (IRect::new(0, 0, 1484, 1004), bgr(RED), 30),
                (IRect::new(100, 100, 1283, 804), bgr(GREEN), 255),
                (IRect::new(1383, 100, 1, 804), bgr(GREEN), outline),
                (IRect::new(100, 100, 1284, 80), bgr(WHITE), 255),
                (IRect::new(1290, 120, 60, 40), bgr(BLUE), 255),
            ],
        )
    };
    let maximized = layered(
        6144,
        3456,
        &[
            (IRect::new(0, 0, 6144, 3360), bgr(GREEN), 255),
            (IRect::new(0, 0, 6144, 96), bgr(WHITE), 255),
            (IRect::new(5900, 20, 200, 56), bgr(BLUE), 255),
        ],
    );
    let mid_maximize = layered(
        6144,
        3456,
        &[(IRect::new(4762, 2458, 1280, 800), bgr(GREEN), 255)],
    );
    let mid_unmaximize = layered(
        6144,
        3456,
        &[(IRect::new(0, 0, 1484, 1004), bgr(STALE), 255)],
    );
    let meta = |r: IRect| Some((r.x as u32, r.y as u32, r.w as u32, r.h as u32));

    let video = TestVideo::with_format(6144, 3456, "BGRA");
    let area = WindowContent::new().into_area(Scales {
        stream_size: Some((3072, 1728)),
        monitor: Some(1.25),
    });
    let recording = record("window-maximize", &video, area);
    video.push(0.0, rest(255), meta(REST_META));
    video.push(FRAME, mid_maximize, meta(MAX_META));
    video.push(2.0 * FRAME, maximized.clone(), meta(MAX_META));
    video.push(3.0 * FRAME, maximized, meta(MAX_META));
    video.push(
        4.0 * FRAME,
        mid_unmaximize.clone(),
        meta(IRect::new(0, 0, 830, 530)),
    );
    video.push(5.0 * FRAME, mid_unmaximize, meta(REST_META));
    video.push(6.0 * FRAME, rest(100), meta(REST_META));
    video.push(7.0 * FRAME, rest(255), meta(REST_META));
    drain(&recording, &video, &[]);
    let done = recording.stop_now(secs(8.0 * FRAME));
    assert_eq!(done.error, None);
    let analysis = analyse(&done.path);
    assert!(analysis.frames.len() >= 8);
    for frame in &analysis.frames {
        assert_eq!((frame.width, frame.height), (802, 502), "frame size kept");
        assert!(
            frame.grid().iter().all(|c| !near(*c, RED)),
            "frame at {} s shows the shadow",
            frame.pts
        );
    }
    let at = |i: f64| {
        analysis
            .frames
            .iter()
            .find(|f| (f.pts - i * FRAME).abs() < 0.005)
            .expect("a frame at each buffer's time")
    };
    // At rest, at the start and after the unmaximize alike: the window whole, framed exactly.
    for i in [0.0, 6.0, 7.0] {
        let f = at(i);
        assert!(f.grid().iter().all(|c| !near(*c, STALE)), "frame {i}");
        assert!(near(f.at(400, 25), WHITE), "frame {i}: header");
        assert!(near(f.at(762, 25), BLUE), "frame {i}: title buttons");
        assert!(
            near(f.at(400, 250), GREEN) && near(f.at(2, 250), GREEN),
            "frame {i}"
        );
        assert!(near(f.at(799, 250), GREEN), "frame {i}: right edge");
    }
    // Maximized and settled: 6144×3360 scaled by 802/6144 to 802×439, black bars of 31 px
    // above and below; the header and the title buttons inside the picture.
    let fit = geometry::letterbox(Size::new(802, 502), Size::new(6144, 3360));
    assert_eq!(fit, IRect::new(0, 31, 802, 439));
    for i in [2.0, 3.0] {
        let f = at(i);
        assert!(f.grid().iter().all(|c| !near(*c, STALE)), "frame {i}");
        assert!(
            near(f.at(400, 12), BLACK) && near(f.at(400, 492), BLACK),
            "frame {i}"
        );
        assert!(near(f.at(400, 37), WHITE), "frame {i}: header");
        assert!(near(f.at(783, 37), BLUE), "frame {i}: title buttons");
        assert!(near(f.at(400, 250), GREEN), "frame {i}");
        assert!(
            near(f.at(2, 250), GREEN) && near(f.at(799, 250), GREEN),
            "frame {i}"
        );
    }
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
    let done = recording.stop_now(secs(20.0));
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
    let done = recording.stop_now(secs(10.0));
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
    let done = recording.stop_now(secs(25.0));
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

/// Set in the child processes of [`a_killed_recording_stays_playable`]: `moving|still:<folder>`.
const CRASH_CHILD: &str = "ANNOTA_TEST_RECORDING_CRASH_CHILD";
/// The line a crash child prints once its recording runs, followed by the working file's path.
const CRASH_STARTED: &str = "annota-test-recording-started ";

/// The crash child: records live test sources through [`start`] until it is killed. `still`:
/// the video stream delivers nothing after its first second (a still screen), audio goes on.
fn crash_child(spec: &str) -> ! {
    let (variant, dir) = spec.split_once(':').expect("variant:folder");
    gst::init().unwrap();
    let bin = gst::Bin::new();
    let src = gst::ElementFactory::make("videotestsrc")
        .property("is-live", true)
        .property_from_str("pattern", "ball")
        .build()
        .unwrap();
    let caps = gst::ElementFactory::make("capsfilter")
        .property(
            "caps",
            gst::Caps::builder("video/x-raw")
                .field("format", "BGRx")
                .field("width", 320)
                .field("height", 240)
                .field("framerate", gst::Fraction::new(30, 1))
                .build(),
        )
        .build()
        .unwrap();
    bin.add_many([&src, &caps]).unwrap();
    src.link(&caps).unwrap();
    let pad = caps.static_pad("src").unwrap();
    if variant == "still" {
        pad.add_probe(gst::PadProbeType::BUFFER, |_, info| match info.buffer() {
            Some(buffer)
                if buffer
                    .pts()
                    .is_some_and(|pts| pts >= gst::ClockTime::SECOND) =>
            {
                gst::PadProbeReturn::Drop
            }
            _ => gst::PadProbeReturn::Ok,
        });
    }
    bin.add_pad(&gst::GhostPad::with_target(&pad).unwrap())
        .unwrap();
    let mic = gst::ElementFactory::make("audiotestsrc")
        .property("is-live", true)
        .property("volume", 0.5)
        .build()
        .unwrap();
    let config = RecordingConfig::new(PathBuf::from(dir), bin.upcast(), VideoArea::Whole)
        .with_audio(AudioSource::Microphone, mic);
    let (recording, _events) = start(config).unwrap();
    // On a line of its own: the harness has already printed "test <name> ... ".
    println!("\n{CRASH_STARTED}{}", recording.path().display());
    use std::io::Write;
    std::io::stdout().flush().unwrap();
    // Killed by the parent; never longer than a minute if it is not.
    std::thread::sleep(Duration::from_secs(60));
    drop(recording);
    std::process::exit(3)
}

/// Kills the crash children if the test fails before it does.
struct Children(Vec<std::process::Child>);

impl Drop for Children {
    fn drop(&mut self) {
        for child in &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// The spec's WHEN taken literally: a process records with live sources through [`start`] and is
/// killed (SIGKILL) 10 s in, once with a moving picture and once with a screen that stands still
/// after 1 s while the microphone goes on (the heartbeat keeps the file advancing). Each working
/// file then plays for at least 9 s, video and audio.
// spec: screen-recording / Working file / Recording survives a crash
#[test]
fn a_killed_recording_stays_playable() {
    if let Ok(spec) = std::env::var(CRASH_CHILD) {
        crash_child(&spec);
    }
    use std::io::BufRead;
    let exe = std::env::current_exe().unwrap();
    // The test's name as the harness knows it (the module path without the crate).
    let name = format!(
        "{}::a_killed_recording_stays_playable",
        module_path!().split_once("::").unwrap().1
    );
    let mut children = Children(Vec::new());
    let mut runs = Vec::new();
    for variant in ["moving", "still"] {
        let dir = folder(&format!("crash-{variant}"));
        let child = std::process::Command::new(&exe)
            .args([name.as_str(), "--exact", "--nocapture", "--test-threads=1"])
            .env(CRASH_CHILD, format!("{variant}:{}", dir.display()))
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        children.0.push(child);
        runs.push((variant, dir));
    }
    // When each recording started.
    let mut started = Vec::new();
    for child in &mut children.0 {
        let stdout = std::io::BufReader::new(child.stdout.take().unwrap());
        let path = stdout
            .lines()
            .map(Result::unwrap)
            .find_map(|line| {
                line.split_once(CRASH_STARTED)
                    .map(|(_, path)| PathBuf::from(path))
            })
            .expect("the child started recording");
        started.push((std::time::Instant::now(), path));
    }
    for (child, (at, _)) in children.0.iter_mut().zip(&started) {
        let kill_at = *at + Duration::from_secs(10);
        std::thread::sleep(kill_at.saturating_duration_since(std::time::Instant::now()));
        // SIGKILL: no handler, no end of stream, no flush.
        child.kill().unwrap();
        child.wait().unwrap();
    }
    for ((variant, dir), (_, path)) in runs.iter().zip(&started) {
        assert!(path.exists(), "{variant}: the working file exists");
        let analysis = analyse(path);
        let video = analysis.duration();
        assert!(video >= 9.0, "{variant}: video plays for {video} s");
        assert_eq!(analysis.audio.len(), 1, "{variant}: one audio track");
        let audio = analysis.audio[0].last().map_or(0.0, |c| c.pts);
        assert!(audio >= 9.0, "{variant}: audio until {audio} s");
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// B1: a still screen (no video buffer for 19 s) with the microphone on. `matroskamux` writes
/// only while every track has data; the heartbeat repeats the last frame at the current time, so
/// the running file keeps advancing, video and audio, up to near the present.
// spec: screen-recording / Recording frame rate / Still screen
#[test]
fn a_still_screen_keeps_the_file_and_the_audio_advancing() {
    let video = TestVideo::new(SMALL.0, SMALL.1);
    let mic = TestAudio::new(0.5);
    let clock = Arc::new(Mutex::new(None));
    let config = RecordingConfig::new(folder("still-audio"), video.element(), VideoArea::Whole)
        .manual_clock(&clock)
        .with_audio(AudioSource::Microphone, mic.element());
    let (recording, _events) = start(config).unwrap();
    let path = recording.path().to_path_buf();
    video.frames(0.0, 1.0, 30.0, GREEN);
    wait_until(
        || {
            recording
                .shared
                .timeline()
                .last_video
                .as_ref()
                .is_some_and(|last| last.out >= secs(29.0 * FRAME))
        },
        "the first second of video",
    );
    // From 0:01 nothing moves; time and the microphone go on, half a second at a time.
    for step in 1..=40 {
        let t = f64::from(step) * 0.5;
        mic.tone(t - 0.5, t);
        *clock.lock().unwrap() = Some(secs(t));
        std::thread::sleep(HEARTBEAT_TICK * 2);
    }
    let mut written = (0.0, 0.0);
    let advanced = (0..600).any(|_| {
        let analysis = analyse(&path);
        let audio = analysis
            .audio
            .first()
            .and_then(|track| track.last())
            .map_or(0.0, |c| c.pts);
        written = (analysis.duration(), audio);
        if written.0 >= 18.0 && written.1 >= 18.0 {
            assert!(analysis.frames.iter().all(|f| near(f.at(32, 24), GREEN)));
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
        false
    });
    assert!(
        advanced,
        "while recording, the file holds {} s of video and {} s of audio",
        written.0, written.1
    );
    let done = recording.stop_now(secs(20.0));
    assert_eq!(done.error, None);
    let analysis = analyse(&done.path);
    assert!((analysis.duration() - 20.0).abs() <= 0.2);
    cleanup(&done.path);
}

/// B2: stopping while the muxer waits for a microphone that delivers nothing and the video queue
/// in front of the encoder is full: the audio ends first, so the file is finished, and the stop
/// never blocks the caller: its future resolves within `FINISH_TIMEOUT`.
// spec: screen-recording / Stop recording / Stop by button
#[test]
fn stopping_with_a_silent_microphone_and_a_full_video_queue_finishes_in_time() {
    let video = TestVideo::new(1920, 1080);
    let mic = TestAudio::new(0.5);
    let config = RecordingConfig::new(folder("stop-full"), video.element(), VideoArea::Whole)
        .manual_clock(&Arc::new(Mutex::new(None)))
        .with_audio(AudioSource::Microphone, mic.element());
    let (recording, _events) = start(config).unwrap();
    let frame = video.solid(GREEN);
    let src = video.src.clone();
    let pusher = std::thread::spawn(move || {
        for i in 0..60 {
            let mut buffer = gst::Buffer::from_slice(frame.clone());
            buffer
                .get_mut()
                .unwrap()
                .set_pts(secs(f64::from(i) * FRAME));
            // Refused once the sources stop.
            if src.push_buffer(buffer).is_err() {
                return;
            }
        }
    });
    let frame_bytes = 1920 * 1080 * 4;
    wait_until(
        || recording.video_in.current_level_bytes() + frame_bytes > QUEUE_BYTES,
        "a full video queue",
    );
    let stopping = recording.stop_at(secs(2.0));
    let asked = std::time::Instant::now();
    let done = finish(stopping);
    let took = asked.elapsed();
    assert!(took < FINISH_TIMEOUT, "stopping took {took:?}");
    assert_eq!(done.error, None);
    assert_eq!(done.audio, vec![AudioSource::Microphone]);
    let analysis = analyse(&done.path);
    assert!(!analysis.frames.is_empty(), "the video was written");
    pusher.join().unwrap();
    cleanup(&done.path);
}

/// N2: the screen changed while paused and then stood still: from the resume on, the recording
/// shows the newest paused frame (the screen as it is), not the one from before the pause; the
/// paused time itself is left out.
// spec: screen-recording / Pause and resume / Paused time is not recorded
#[test]
fn after_resume_a_still_screen_shows_the_newest_frame() {
    let video = TestVideo::new(SMALL.0, SMALL.1);
    let recording = record("pause-still", &video, VideoArea::Whole);
    video.frames(0.0, 10.0, 30.0, BLUE);
    recording.pause_at(secs(10.0));
    video.frames(10.0, 15.0, 30.0, RED);
    wait_until(
        || {
            recording
                .shared
                .timeline()
                .paused_video
                .as_ref()
                .and_then(|(buffer, _)| buffer.pts())
                .is_some_and(|pts| pts >= secs(14.9))
        },
        "the glue to see the paused frames",
    );
    recording.resume_at(secs(15.0));
    // Nothing moves after the resume.
    drain(&recording, &video, &[]);
    let done = recording.stop_now(secs(25.0));
    assert_eq!(done.duration, secs(20.0));
    let analysis = analyse(&done.path);
    assert!((analysis.duration() - 20.0).abs() <= 0.2);
    for frame in &analysis.frames {
        let want = if frame.pts < 9.99 { BLUE } else { RED };
        assert!(near(frame.at(32, 24), want), "frame at {} s", frame.pts);
    }
    cleanup(&done.path);
}

/// N3/N4: a window stream whose first buffers have no content rectangle yet (nothing recorded
/// from them) and whose first rectangle is odd-sized: the frame is the rectangle trimmed to even
/// and `videocrop` takes exactly that, so nothing is resampled.
// spec: screen-recording / Recording source / Recorded window resized
#[test]
fn a_window_is_recorded_from_its_first_known_rectangle_without_resampling() {
    let video = TestVideo::new(640, 480);
    let recording = record("window-odd", &video, meta_window(KEEP_PIXELS));
    // No rectangle known: a frame of undefined pixels.
    video.push(0.0, video.solid(RED), None);
    video.push(FRAME, video.solid(RED), None);
    let window = IRect::new(10, 20, 301, 201);
    let painted = video.painted(window, GREEN, RED);
    for i in 2..6 {
        video.push(
            f64::from(i) * FRAME,
            painted.clone(),
            Some((10, 20, 301, 201)),
        );
    }
    drain(&recording, &video, &[]);
    let crop = recording.encoder.by_name("crop").unwrap();
    // Read once the encoder has seen every buffer (the end of the file is written).
    let done = recording.stop_now(secs(6.0 * FRAME));
    let edges: Vec<i32> = ["left", "top", "right", "bottom"]
        .iter()
        .map(|p| crop.property::<i32>(p))
        .collect();
    assert_eq!(edges, [10, 20, 640 - 310, 480 - 220], "cropped to 300×200");
    assert_eq!(done.error, None);
    let analysis = analyse(&done.path);
    assert!(!analysis.frames.is_empty());
    for frame in &analysis.frames {
        assert_eq!((frame.width, frame.height), (300, 200));
        assert!(
            frame.grid().iter().all(|c| near(*c, GREEN)),
            "frame at {} s shows more than the window",
            frame.pts
        );
    }
    cleanup(&done.path);
}

/// N5: an error in the source pipeline still finishes the file: the encoder gets its end of
/// stream, so x264's delayed frames and the end of the file are written.
// spec: screen-recording / Working file / Recording error
#[test]
fn an_error_in_the_source_still_finishes_the_file() {
    let video = TestVideo::new(640, 480);
    let recording = record("source-error", &video, VideoArea::Whole);
    video.frames(0.0, 5.0, 30.0, GREEN);
    drain(&recording, &video, &[]);
    gst::element_error!(
        video.src,
        gst::StreamError::Failed,
        ("the stand-in stream failed")
    );
    wait_until(|| recording.shared.end().error.is_some(), "the error");
    let done = recording.stop_now(secs(5.0));
    assert!(matches!(done.error, Some(RecordingError::Stream { .. })));
    assert_eq!(done.duration, secs(5.0));
    let analysis = analyse(&done.path);
    // Without the end of stream x264's delayed frames were lost (3.1 s of 5 s at 640×480).
    let duration = analysis.duration();
    assert!((4.95..=5.1).contains(&duration), "plays for {duration} s");
    cleanup(&done.path);
}

// spec: screen-recording / Working file / Recording error
#[test]
fn an_error_mid_recording_is_reported_and_keeps_what_was_written() {
    let video = TestVideo::new(SMALL.0, SMALL.1);
    let mut config = RecordingConfig::new(folder("error"), video.element(), VideoArea::Whole)
        .manual_clock(&Arc::new(Mutex::new(None)));
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
    let done = finish(recording.stop());
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
pub(crate) fn rms_range(track: &[AChunk], from: f64, to: f64) -> (f64, f64) {
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

// spec: screen-recording / Pause and resume / Paused state
#[test]
fn the_recorded_time_stands_at_the_pause_while_paused() {
    let mut timeline = Timeline::default();
    timeline.pauses.push((secs(10.0), Some(secs(15.0))));
    timeline.pauses.push((secs(20.0), None));
    assert_eq!(timeline.out_time(secs(5.0)), secs(5.0));
    // Inside a pause: the time from the moment of pausing, also at that very moment.
    assert_eq!(timeline.out_time(secs(10.0)), secs(10.0));
    assert_eq!(timeline.out_time(secs(12.0)), secs(10.0));
    assert_eq!(timeline.out_time(secs(17.0)), secs(12.0));
    assert_eq!(timeline.out_time(secs(20.0)), secs(15.0));
    assert_eq!(timeline.out_time(secs(30.0)), secs(15.0));
}

// spec: screen-recording / Audio / Mute the microphone
#[test]
fn a_muted_microphone_is_silent_only_while_muted() {
    let video = TestVideo::new(SMALL.0, SMALL.1);
    let mic = TestAudio::new(0.5);
    let config = RecordingConfig::new(folder("mute-mic"), video.element(), VideoArea::Whole)
        .manual_clock(&Arc::new(Mutex::new(None)))
        .with_audio(AudioSource::Microphone, mic.element());
    let (recording, mut receiver) = start(config).unwrap();
    recording.set_muted_at(AudioSource::Microphone, secs(5.0), true);
    recording.set_muted_at(AudioSource::Microphone, secs(8.0), false);
    assert!(!recording.is_muted(AudioSource::Microphone));
    video.frames(0.0, 10.0, 30.0, GREEN);
    mic.tone(0.0, 10.0);
    drain(&recording, &video, &[&mic]);
    let done = recording.stop_now(secs(10.0));
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
        .manual_clock(&Arc::new(Mutex::new(None)))
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
    let done = recording.stop_now(secs(8.0));
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
    let done = recording.stop_now(secs(5.0));
    assert!(done.audio.is_empty());
    let analysis = analyse(&done.path);
    assert!((analysis.duration() - 5.0).abs() <= 0.2);
    assert!(analysis.audio.is_empty(), "no audio track");
    cleanup(&done.path);
}

/// A finished working file for the export tests: `seconds` of synthetic time (frames of
/// `size`, a colour per second, 30 fps) through the real pipeline, with a 440 Hz tone of the
/// given amplitude per audio source. Encodes much faster than real time. The caller removes
/// the file's folder ([`remove_working_file`]).
pub(crate) fn working_file(
    name: &str,
    seconds: f64,
    size: (u32, u32),
    audio: &[(AudioSource, f64)],
) -> Finished {
    working_file_marked(name, seconds, size, audio, None)
}

/// [`working_file`], or with `marker`: black frames and silence until `marker` seconds, from
/// there red frames and the tone (an A/V sync marker: flash and beep at the same time).
pub(crate) fn working_file_marked(
    name: &str,
    seconds: f64,
    size: (u32, u32),
    audio: &[(AudioSource, f64)],
    marker: Option<f64>,
) -> Finished {
    let video = TestVideo::new(size.0, size.1);
    let tones: Vec<TestAudio> = audio.iter().map(|(_, a)| TestAudio::new(*a)).collect();
    let mut config = RecordingConfig::new(folder(name), video.element(), VideoArea::Whole)
        .manual_clock(&Arc::new(Mutex::new(None)));
    for ((source, _), tone) in audio.iter().zip(&tones) {
        config = config.with_audio(*source, tone.element());
    }
    let (recording, _events) = start(config).unwrap();
    let colours = [GREEN, BLUE, RED, BLACK];
    let whole = seconds.floor() as usize;
    let feeder = {
        let src = video.src.clone();
        let (w, h) = (video.w, video.h);
        std::thread::spawn(move || {
            let video = TestVideo { src, w, h };
            if let Some(at) = marker {
                video.frames(0.0, at, 30.0, BLACK);
                video.frames(at, seconds, 30.0, RED);
                return;
            }
            for second in 0..=whole {
                let from = second as f64;
                let to = (from + 1.0).min(seconds);
                if to > from {
                    video.frames(from, to, 30.0, colours[second % colours.len()]);
                }
            }
        })
    };
    // Audio on this thread while the video goes in on its own (the muxer interleaves both).
    for tone in &tones {
        match marker {
            Some(at) => {
                tone.tone_of(0.0, at, 0.0);
                tone.tone(at, seconds);
            }
            None => tone.tone(0.0, seconds),
        }
    }
    feeder.join().unwrap();
    let refs: Vec<&TestAudio> = tones.iter().collect();
    drain(&recording, &video, &refs);
    recording.stop_now(secs(seconds))
}

/// Removes a file made by [`working_file`] and its folder.
pub(crate) fn remove_working_file(path: &Path) {
    cleanup(path);
}
