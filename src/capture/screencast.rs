//! Window stills through the ScreenCast portal (fix-window-capture-screencast, ADR 0002).
//!
//! The desktop's own window chooser for screen sharing picks the window. Annota takes exactly one
//! frame of the stream through GStreamer, cuts it to the window and closes the session at once.
//! No restore token is requested or kept, so the chooser appears for every window capture.
//! Frames stay in memory (CLAUDE.md §7): nothing here writes a file.
//!
//! The portal facts this module relies on were measured on GNOME 50.1 on 2026-11-14 with
//! `tools/portal-probe.py --screencast-window` (design.md D1, D3, D5, D6).

use std::future::Future;
use std::os::fd::{AsRawFd, OwnedFd};
use std::time::Duration;

use ashpd::desktop::screencast::{CursorMode, Screencast, SourceType};
use ashpd::desktop::{PersistMode, Session};
use ashpd::enumflags2::BitFlags;
use ashpd::WindowIdentifier;
use futures_channel::oneshot;
use futures_util::future::{self, Either, FutureExt, LocalBoxFuture};
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;
use gstreamer_video::prelude::*;
use gtk::glib;

use super::frame::Frame;
use super::geometry::IRect;
use super::portal::{self, CaptureError};

/// The spec's limit from the stream's start to the frame ("No frame arrives").
pub const FRAME_TIMEOUT: Duration = Duration::from_secs(5);
/// Extra time on top of [`FRAME_TIMEOUT`] before Annota stops waiting for the worker thread
/// itself. Only a pipeline that hangs while starting gets here.
const WORKER_GRACE: Duration = Duration::from_secs(2);

/// The lowest alpha that counts as the window itself (design.md D3). Measured on GNOME 50.1:
/// the shadow around a window has alpha 28–41, the 1 px outline ~200, the body 255. 128 keeps
/// the outline and drops the shadow.
pub const OPAQUE_ALPHA: u8 = 128;

/// Bytes per pixel of the frame format.
const BPP: usize = 4;

/// The GStreamer format that has cairo's `ARGB32` byte order on this host, so the pixels go into
/// a [`Frame`] as they are: B, G, R, A in memory on little-endian machines.
#[cfg(target_endian = "little")]
const FRAME_FORMAT: &str = "BGRA";
#[cfg(target_endian = "big")]
const FRAME_FORMAT: &str = "ARGB";
/// Where the alpha byte sits within a pixel of [`FRAME_FORMAT`].
#[cfg(target_endian = "little")]
const ALPHA: usize = 3;
#[cfg(target_endian = "big")]
const ALPHA: usize = 0;

/// The ScreenCast portal, when it offers window sources.
pub struct WindowSource {
    proxy: Screencast<'static>,
    cursor_modes: BitFlags<CursorMode>,
}

impl std::fmt::Debug for WindowSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowSource")
            .field("cursor_modes", &self.cursor_modes)
            .finish_non_exhaustive()
    }
}

impl WindowSource {
    /// Whether the stream can carry the pointer drawn into the frame.
    pub fn offers_embedded_pointer(&self) -> bool {
        self.cursor_modes.contains(CursorMode::Embedded)
    }
}

/// The ScreenCast portal, if it offers a WINDOW source (design.md D6). `None` — no portal, no
/// ScreenCast interface, or no window source (on X11 this is unmeasured) — means window mode
/// takes the Screenshot portal's interactive UI instead. Read once per capture.
pub async fn window_source_available() -> Option<WindowSource> {
    portal::ensure_registered().await;
    let proxy = Screencast::new().await.ok()?;
    let types = proxy.available_source_types().await.ok()?;
    if !types.contains(SourceType::Window) {
        return None;
    }
    // Version 1 of the interface has no cursor modes; then only HIDDEN is asked for.
    let cursor_modes = proxy.available_cursor_modes().await.unwrap_or_default();
    Some(WindowSource {
        proxy,
        cursor_modes,
    })
}

/// The pointer setting for the stream (design.md D1/D5): drawn in when the user wants it and the
/// portal offers it, otherwise hidden. Annota never draws a pointer into a window frame itself.
pub fn cursor_mode(include_pointer: bool, embedded_offered: bool) -> CursorMode {
    if include_pointer && embedded_offered {
        CursorMode::Embedded
    } else {
        CursorMode::Hidden
    }
}

/// What a ScreenCast error means for the user (design.md D6). Response code 1 — the chooser was
/// closed — is a silent cancel. Code 2 and everything else are a failure; unlike the Screenshot
/// portal no permission store can turn it into a denial, so it never becomes `Refused`.
pub fn map_error(err: &ashpd::Error) -> CaptureError {
    match portal::map_portal_error(err) {
        CaptureError::Refused => CaptureError::Failed(format!("the window chooser failed: {err}")),
        other => other,
    }
}

/// A chosen window: the open session, the PipeWire remote and the stream's node. Dropping it
/// closes the fd and then the session (a cancelled countdown before the frame).
pub struct Chosen {
    held: Held<OwnedFd, PortalSession>,
    node: u32,
}

/// Opens the desktop's window chooser with `parent` as its parent window and waits for it.
///
/// `cancel` ends the wait from Annota's side (the waiting indicator's Cancel or `Esc`, design.md
/// D8): the session is closed, which also ends GNOME's pending request, and the result is
/// [`CaptureError::Cancelled`]. Whatever the outcome, no session is left open behind it.
pub async fn choose_window(
    source: &WindowSource,
    cursor: CursorMode,
    parent: Option<WindowIdentifier>,
    cancel: impl Future<Output = ()>,
) -> Result<Chosen, CaptureError> {
    // GStreamer's first `init` in a user's session scans the plugin registry. It runs alongside
    // the chooser, off the main loop, so it is done by the time the frame is taken.
    std::thread::spawn(|| {
        let _ = gst::init();
    });
    let proxy = &source.proxy;
    let session = proxy.create_session().await.map_err(|e| map_error(&e))?;
    // Closes the session on every way out below, until a `Chosen` holds it.
    let mut guard = SessionGuard(Some(PortalSession(session)));

    let answer = {
        let session = &guard.get()?.0;
        proxy
            .select_sources(
                session,
                cursor,
                SourceType::Window.into(),
                false,
                None,
                PersistMode::DoNot,
            )
            .await
            .and_then(|request| request.response())
            .map_err(|e| map_error(&e))?;
        let start = std::pin::pin!(proxy.start(session, parent.as_ref()));
        match future::select(start, std::pin::pin!(cancel)).await {
            Either::Left((answer, _)) => Some(answer),
            Either::Right(((), _)) => None,
        }
    };
    let Some(answer) = answer else {
        if let Some(session) = guard.0.take() {
            session.close().await;
        }
        return Err(CaptureError::Cancelled);
    };
    let streams = answer
        .and_then(|request| request.response())
        .map_err(|e| map_error(&e))?;
    let node = streams
        .streams()
        .first()
        .map(|stream| stream.pipe_wire_node_id())
        .ok_or_else(|| CaptureError::Failed(String::from("the portal started no stream")))?;
    let fd = proxy
        .open_pipe_wire_remote(&guard.get()?.0)
        .await
        .map_err(|e| map_error(&e))?;
    Ok(Chosen {
        held: Held {
            fd: Some(fd),
            session: guard,
        },
        node,
    })
}

impl Chosen {
    /// Takes one frame of the window and closes the session. Fails if none arrives within
    /// [`FRAME_TIMEOUT`] or the frame holds no visible pixel.
    pub async fn grab_frame(mut self) -> Result<Frame, CaptureError> {
        let fd = self.held.fd.take();
        let session = self.held.session.0.take();
        let (Some(fd), Some(session)) = (fd, session) else {
            return Err(CaptureError::Failed(String::from("no PipeWire remote")));
        };
        let node = self.node;
        let pulled = pull_then_close(fd, session, FRAME_TIMEOUT + WORKER_GRACE, move |stream| {
            // Already done alongside the chooser; this only reports a failure.
            gst::init().map_err(|err| format!("GStreamer: {err}"))?;
            let fd = stream.fd.as_ref().ok_or("no PipeWire remote")?.as_raw_fd();
            let (pipeline, sink) = one_frame_pipeline(fd, node)?;
            stream.pipeline = Some(pipeline.clone());
            play_and_pull(&pipeline, &sink)
        })
        .await;
        match pulled {
            Some(result) => result.map_err(CaptureError::Failed),
            None => Err(CaptureError::Failed(String::from(
                "no frame within 5 s (the stream did not start)",
            ))),
        }
    }
}

/// Runs `work` on a worker thread, because starting GStreamer, starting a live source and waiting
/// for its frame all block. `work` builds the pipeline into the [`Stream`] it is given; the worker
/// then releases the stream — pipeline `NULL`, then the fd — and only after that is the session
/// closed (design.md D2).
///
/// `None`: the worker did not finish within `deadline`. This is the one exception to D2's order
/// (decided 2026-11-14): the session closes at once, so GNOME's screen-sharing indicator goes
/// away with the error toast. The worker keeps the pipeline and the fd and releases them, in
/// D2's order, whenever it unblocks: `pipewiresrc` waits 30 s of its own (measured: sometimes
/// twice) for a stream that never connects, and stopping it from another thread blocks until
/// then, so the main loop never does.
async fn pull_then_close<P, F, S, T>(
    fd: F,
    session: S,
    deadline: Duration,
    work: impl FnOnce(&mut Stream<P, F>) -> T + Send + 'static,
) -> Option<T>
where
    P: Stop + Send + 'static,
    F: Send + 'static,
    S: Close + 'static,
    T: Send + 'static,
{
    let (sender, receiver) = oneshot::channel();
    std::thread::spawn(move || {
        let mut stream = Stream {
            pipeline: None,
            fd: Some(fd),
        };
        let result = work(&mut stream);
        drop(stream);
        let _ = sender.send(result);
    });
    match future::select(receiver, glib::timeout_future(deadline)).await {
        Either::Left((result, _)) => {
            session.close().await;
            result.ok()
        }
        Either::Right(((), _receiver)) => {
            // Dropping the receiver only makes the worker's late `send` a no-op.
            session.close_detached();
            None
        }
    }
}

/// `pipewiresrc fd=… path=… num-buffers=1 always-copy=true ! videoconvert !
/// video/x-raw,format=BGRA ! appsink` (design.md D2), built element by element so no string is
/// assembled around the fd.
fn one_frame_pipeline(fd: i32, node: u32) -> Result<(gst::Pipeline, gst_app::AppSink), String> {
    let source = gst::ElementFactory::make("pipewiresrc")
        .property("fd", fd)
        .property("path", node.to_string())
        .property("num-buffers", 1i32)
        .build()
        .map_err(|err| format!("no pipewiresrc (gstreamer1.0-pipewire): {err}"))?;
    // Older PipeWire plugins lack it; the frame is then mapped from PipeWire's own buffer.
    if source.find_property("always-copy").is_some() {
        source.set_property("always-copy", true);
    }
    let convert = gst::ElementFactory::make("videoconvert")
        .build()
        .map_err(|err| format!("no videoconvert (gstreamer1.0-plugins-base): {err}"))?;
    let caps = gst::Caps::builder("video/x-raw")
        .field("format", FRAME_FORMAT)
        .build();
    let sink = gst_app::AppSink::builder()
        .caps(&caps)
        .sync(false)
        .max_buffers(1)
        .build();
    let pipeline = gst::Pipeline::new();
    pipeline
        .add_many([&source, &convert, sink.upcast_ref()])
        .map_err(|err| err.to_string())?;
    gst::Element::link_many([&source, &convert, sink.upcast_ref()])
        .map_err(|err| err.to_string())?;
    Ok((pipeline, sink))
}

/// Worker thread: starts the stream, waits up to [`FRAME_TIMEOUT`] for the one frame and cuts it
/// to the window.
fn play_and_pull(pipeline: &gst::Pipeline, sink: &gst_app::AppSink) -> Result<Frame, String> {
    pipeline
        .set_state(gst::State::Playing)
        .map_err(|_| String::from("the stream did not start"))?;
    let sample = sink
        .try_pull_sample(gst::ClockTime::from_seconds(FRAME_TIMEOUT.as_secs()))
        .ok_or_else(|| String::from("no frame within 5 s"))?;
    window_frame(&sample)
}

/// The window's pixels out of one stream sample (design.md D3).
fn window_frame(sample: &gst::Sample) -> Result<Frame, String> {
    let caps = sample.caps().ok_or("the frame has no format")?;
    let info = gst_video::VideoInfo::from_caps(caps).map_err(|err| err.to_string())?;
    let buffer = sample.buffer().ok_or("no frame")?;
    let crop = buffer
        .meta::<gst_video::VideoCropMeta>()
        .map(|meta| meta.rect());
    // Honours a `GstVideoMeta`'s own stride, which a passed-through PipeWire buffer may carry.
    let video = gst_video::VideoFrameRef::from_buffer_ref_readable(buffer, &info)
        .map_err(|err| err.to_string())?;
    let stride = usize::try_from(video.plane_stride()[0]).map_err(|err| err.to_string())?;
    let pixels = Pixels {
        width: video.width(),
        height: video.height(),
        stride,
        data: video.plane_data(0).map_err(|err| err.to_string())?,
    };
    let rect = window_rect(&pixels, crop).ok_or("no frame (no visible pixel)")?;
    pixels.copy(rect).ok_or_else(|| String::from("no frame"))
}

/// One frame's pixels in [`FRAME_FORMAT`], borrowed.
pub struct Pixels<'a> {
    pub width: u32,
    pub height: u32,
    pub stride: usize,
    pub data: &'a [u8],
}

impl Pixels<'_> {
    fn alpha(&self, x: u32, y: u32) -> u8 {
        let offset = y as usize * self.stride + x as usize * BPP + ALPHA;
        self.data.get(offset).copied().unwrap_or(0)
    }

    /// The pixels inside `rect` as a frame of their own.
    fn copy(&self, rect: IRect) -> Option<Frame> {
        let row = rect.w as usize * BPP;
        let mut data = Vec::with_capacity(row * rect.h as usize);
        for y in rect.y..rect.bottom() {
            let start = y as usize * self.stride + rect.x as usize * BPP;
            data.extend_from_slice(self.data.get(start..start + row)?);
        }
        Frame::from_argb32(rect.w, rect.h, row, data)
    }

    /// The bounding box of the pixels within `area` whose alpha passes `keep`.
    fn bounds(&self, area: IRect, keep: impl Fn(u8) -> bool) -> Option<IRect> {
        let mut found: Option<(i32, i32, i32, i32)> = None;
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                if keep(self.alpha(x as u32, y as u32)) {
                    found = Some(match found {
                        None => (x, y, x, y),
                        Some((l, t, r, b)) => (l.min(x), t.min(y), r.max(x), b.max(y)),
                    });
                }
            }
        }
        found.map(|(l, t, r, b)| IRect::new(l, t, r - l + 1, b - t + 1))
    }
}

/// Where the window is in a stream frame (design.md D3):
/// 1. inside the buffer's crop meta when there is one (the window plus its shadow margin),
///    otherwise the whole buffer;
/// 2. the bounding box of pixels with alpha ≥ [`OPAQUE_ALPHA`] — outline and body, no shadow;
///    transparent rounded corners stay inside, as only the outer bound counts;
/// 3. a translucent window has no such pixel: then the box of alpha > 0, faint shadow included.
///
/// `None`: not one visible pixel, which is a failure and not an empty image.
pub fn window_rect(pixels: &Pixels, crop: Option<(u32, u32, u32, u32)>) -> Option<IRect> {
    let width = i32::try_from(pixels.width).ok()?;
    let height = i32::try_from(pixels.height).ok()?;
    let whole = IRect::new(0, 0, width, height);
    let area = crop
        .and_then(|(x, y, w, h)| {
            let rect = IRect::new(
                i32::try_from(x).ok()?,
                i32::try_from(y).ok()?,
                i32::try_from(w).ok()?,
                i32::try_from(h).ok()?,
            );
            rect.intersect(whole)
        })
        .unwrap_or(whole);
    pixels
        .bounds(area, |alpha| alpha >= OPAQUE_ALPHA)
        .or_else(|| pixels.bounds(area, |alpha| alpha > 0))
}

/// The pipeline is set to `NULL` before anything else goes.
pub(super) trait Stop {
    fn stop(&self);
}

/// The portal session, closed last.
pub(super) trait Close {
    /// Closes the session and waits for the portal's answer.
    fn close(self) -> LocalBoxFuture<'static, ()>;
    /// Closes the session from a `Drop`, where nothing can be awaited.
    fn close_detached(self);
}

impl Stop for gst::Pipeline {
    fn stop(&self) {
        let _ = self.set_state(gst::State::Null);
    }
}

pub(super) struct PortalSession(Session<'static, Screencast<'static>>);

impl Close for PortalSession {
    fn close(self) -> LocalBoxFuture<'static, ()> {
        async move {
            // After code 1 or 2 the portal may have closed the session itself; then there is
            // nothing left to close and the error says only that.
            let _ = self.0.close().await;
        }
        .boxed_local()
    }

    fn close_detached(self) {
        glib::MainContext::ref_thread_default().spawn_local(self.close());
    }
}

/// Closes the session when dropped, unless it was taken.
pub(super) struct SessionGuard<S: Close>(Option<S>);

impl<S: Close> SessionGuard<S> {
    fn get(&self) -> Result<&S, CaptureError> {
        self.0
            .as_ref()
            .ok_or_else(|| CaptureError::Failed(String::from("the session is already closed")))
    }
}

impl<S: Close> Drop for SessionGuard<S> {
    fn drop(&mut self) {
        if let Some(session) = self.0.take() {
            session.close_detached();
        }
    }
}

/// A chosen window before its frame: the fd closes before the session.
pub(super) struct Held<F, S: Close> {
    fd: Option<F>,
    session: SessionGuard<S>,
}

impl<F, S: Close> Drop for Held<F, S> {
    fn drop(&mut self) {
        drop(self.fd.take());
        // `session` drops after this, closing the session.
    }
}

/// The pipeline and the PipeWire remote, released in that order (design.md D2).
pub(super) struct Stream<P: Stop, F> {
    pipeline: Option<P>,
    fd: Option<F>,
}

impl<P: Stop, F> Drop for Stream<P, F> {
    fn drop(&mut self) {
        if let Some(pipeline) = self.pipeline.take() {
            pipeline.stop();
        }
        drop(self.fd.take());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    type Log = Arc<Mutex<Vec<&'static str>>>;

    fn note(log: &Log, what: &'static str) {
        log.lock().unwrap().push(what);
    }

    struct FakePipeline(Log);
    impl Stop for FakePipeline {
        fn stop(&self) {
            note(&self.0, "pipeline NULL");
        }
    }

    struct FakeFd(Log);
    impl Drop for FakeFd {
        fn drop(&mut self) {
            note(&self.0, "fd closed");
        }
    }

    struct FakeSession(Log);
    impl Close for FakeSession {
        fn close(self) -> LocalBoxFuture<'static, ()> {
            note(&self.0, "Session.Close");
            async {}.boxed_local()
        }
        fn close_detached(self) {
            note(&self.0, "Session.Close");
        }
    }

    /// What the real worker does first: the pipeline goes into the stream.
    fn build(log: &Log) -> impl FnOnce(&mut Stream<FakePipeline, FakeFd>) + Send + 'static {
        let log = log.clone();
        move |stream| stream.pipeline = Some(FakePipeline(log))
    }

    /// Runs `future` on a main context of its own, as the app's main loop would.
    fn run<T>(future: impl Future<Output = T>) -> T {
        let context = glib::MainContext::new();
        context
            .with_thread_default(|| context.block_on(future))
            .unwrap()
    }

    fn log_of(log: &Log) -> Vec<&'static str> {
        log.lock().unwrap().clone()
    }

    // spec: capture / Window capture / Window on Wayland
    #[test]
    fn a_frame_releases_pipeline_then_fd_then_session() {
        // Task 4.1: the order of design.md D2 after a frame.
        let log = Log::default();
        let session = FakeSession(log.clone());
        let build = build(&log);
        let pulled = run(pull_then_close(
            FakeFd(log.clone()),
            session,
            Duration::from_secs(5),
            move |stream| {
                build(stream);
                "frame"
            },
        ));
        assert_eq!(pulled, Some("frame"));
        assert_eq!(
            log_of(&log),
            ["pipeline NULL", "fd closed", "Session.Close"]
        );
    }

    // spec: capture / Window capture / No frame arrives
    #[test]
    fn a_hanging_stream_closes_the_session_at_the_deadline() {
        // Scenario "No frame arrives" when the stream never starts: at the deadline Annota closes
        // the session at once, without blocking and without waiting for the worker (the one
        // exception to design.md D2). The worker then releases pipeline and fd, in that order,
        // once it unblocks.
        let log = Log::default();
        let session = FakeSession(log.clone());
        let (release, released) = std::sync::mpsc::channel::<()>();
        let context = glib::MainContext::new();
        context
            .with_thread_default(|| {
                let build = build(&log);
                let pulled = context.block_on(pull_then_close(
                    FakeFd(log.clone()),
                    session,
                    Duration::from_millis(20),
                    move |stream| {
                        build(stream);
                        // Blocks like `pipewiresrc` on a stream that never connects.
                        let _ = released.recv();
                    },
                ));
                assert_eq!(pulled, None);
                for _ in 0..10 {
                    context.iteration(false);
                }
                assert_eq!(
                    log_of(&log),
                    ["Session.Close"],
                    "the session closes while the worker still holds pipeline and fd"
                );
            })
            .unwrap();
        release.send(()).unwrap();
        let end = std::time::Instant::now() + Duration::from_secs(5);
        while log_of(&log).len() < 3 && std::time::Instant::now() < end {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            log_of(&log),
            ["Session.Close", "pipeline NULL", "fd closed"]
        );
    }

    // spec: capture / Window capture / Countdown cancelled
    #[test]
    fn a_cancelled_countdown_closes_fd_then_session() {
        // Scenario "Countdown cancelled": a chosen window is dropped before its frame.
        let log = Log::default();
        drop(Held {
            fd: Some(FakeFd(log.clone())),
            session: SessionGuard(Some(FakeSession(log.clone()))),
        });
        assert_eq!(log_of(&log), ["fd closed", "Session.Close"]);
    }

    // spec: capture / Window capture / Chooser closed
    #[test]
    fn an_error_before_the_chooser_answers_closes_the_session() {
        let log = Log::default();
        drop(SessionGuard(Some(FakeSession(log.clone()))));
        assert_eq!(log_of(&log), ["Session.Close"]);
    }

    // spec: capture / Cursor inclusion / Pointer in a window capture
    // spec: capture / Cursor inclusion / Cursor excluded
    #[test]
    fn pointer_mode_for_all_four_combinations() {
        // Task 4.5, scenarios "Pointer in a window capture" and "Cursor excluded".
        assert_eq!(cursor_mode(true, true), CursorMode::Embedded);
        assert_eq!(cursor_mode(true, false), CursorMode::Hidden);
        assert_eq!(cursor_mode(false, true), CursorMode::Hidden);
        assert_eq!(cursor_mode(false, false), CursorMode::Hidden);
    }

    // spec: capture / Window capture / Chooser closed
    #[test]
    fn chooser_closed_is_a_silent_cancel_and_code_2_a_failure() {
        // Task 4.4 / design.md D6: code 1 → Cancelled, code 2 → Failed, never Refused (which
        // would send window mode into the Screenshot permission sorting).
        use ashpd::desktop::ResponseError;
        assert_eq!(
            map_error(&ashpd::Error::Response(ResponseError::Cancelled)),
            CaptureError::Cancelled
        );
        assert!(matches!(
            map_error(&ashpd::Error::Response(ResponseError::Other)),
            CaptureError::Failed(_)
        ));
        assert!(matches!(
            map_error(&ashpd::Error::NoResponse),
            CaptureError::Failed(_)
        ));
    }

    /// A BGRA frame (alpha in byte 3 on little-endian hosts) built from an alpha function.
    struct Image {
        width: u32,
        height: u32,
        stride: usize,
        data: Vec<u8>,
    }

    impl Image {
        fn new(width: u32, height: u32, alpha: impl Fn(u32, u32) -> u8) -> Self {
            // A stride wider than the row, as a real buffer may have.
            let stride = width as usize * BPP + 16;
            let mut data = vec![0u8; stride * height as usize];
            for y in 0..height {
                for x in 0..width {
                    let a = alpha(x, y);
                    let at = y as usize * stride + x as usize * BPP;
                    // Premultiplied grey, so the copy is a valid cairo surface.
                    data[at..at + BPP].fill(a / 2);
                    data[at + ALPHA] = a;
                }
            }
            Self {
                width,
                height,
                stride,
                data,
            }
        }

        fn pixels(&self) -> Pixels<'_> {
            Pixels {
                width: self.width,
                height: self.height,
                stride: self.stride,
                data: &self.data,
            }
        }
    }

    /// The measured profile (design.md D3) at 1/8 scale: a window at (16, 18) of 280 × 220 with a
    /// 1 px outline (alpha 200), 12 px of shadow (alpha 28–41) around it, inside a crop meta of
    /// window + shadow, inside a larger, transparent buffer. `corner` px of rounded corners at the
    /// top stay transparent.
    fn measured_window(corner: u32) -> Image {
        let (wx, wy, ww, wh, shadow) = (16u32, 18u32, 280u32, 220u32, 12u32);
        Image::new(768, 432, move |x, y| {
            let inside = x >= wx && x < wx + ww && y >= wy && y < wy + wh;
            if inside {
                let (dx, dy) = (x - wx, y - wy);
                let (rx, ry) = (ww - 1 - dx, dy);
                let in_corner = dy < corner && (dx < corner || rx < corner);
                let round = in_corner && {
                    let cx = if dx < corner {
                        corner - dx
                    } else {
                        corner - rx
                    };
                    let cy = corner - ry;
                    cx * cx + cy * cy > corner * corner
                };
                if round {
                    0
                } else if dx == 0 || dy == 0 || rx == 0 || y == wy + wh - 1 {
                    200
                } else {
                    255
                }
            } else {
                // Shadow: 28 at the outer edge, rising to 41 next to the window.
                let gx = (wx.saturating_sub(x)).max(x.saturating_sub(wx + ww - 1));
                let gy = (wy.saturating_sub(y)).max(y.saturating_sub(wy + wh - 1));
                let gap = gx.max(gy);
                if x >= wx - shadow
                    && x < wx + ww + shadow
                    && y >= wy - shadow
                    && y < wy + wh + shadow
                {
                    (41 - gap) as u8
                } else {
                    0
                }
            }
        })
    }

    // spec: capture / Window capture / Window on Wayland
    #[test]
    fn the_window_is_cut_without_shadow_or_margin() {
        // Scenario "Window on Wayland", task 4.2: a window in the top-left of a larger buffer.
        let image = measured_window(0);
        let rect = window_rect(&image.pixels(), None).unwrap();
        assert_eq!(rect, IRect::new(16, 18, 280, 220));
        let frame = image.pixels().copy(rect).unwrap();
        assert_eq!((frame.width(), frame.height()), (280, 220));
    }

    // spec: capture / Window capture / Window on Wayland
    #[test]
    fn transparent_rounded_corners_stay_inside() {
        // Only the outer bound is cut: the corners keep their transparent pixels (redline e).
        let image = measured_window(12);
        let rect = window_rect(&image.pixels(), None).unwrap();
        assert_eq!(rect, IRect::new(16, 18, 280, 220));
        let frame = image.pixels().copy(rect).unwrap();
        assert_eq!(
            frame.data()[ALPHA],
            0,
            "top-left corner pixel stays transparent"
        );
    }

    // spec: capture / Window capture / Window on Wayland
    #[test]
    fn the_crop_meta_limits_the_search() {
        // A crop meta of window + shadow (as measured); an opaque stray pixel outside it, e.g.
        // stale content in the buffer's margin, must not widen the cut.
        let mut image = measured_window(0);
        let stray = 400 * image.stride + 700 * BPP + ALPHA;
        image.data[stray] = 255;
        let crop = Some((4, 6, 304, 244));
        assert_eq!(
            window_rect(&image.pixels(), crop),
            Some(IRect::new(16, 18, 280, 220))
        );
        assert_eq!(
            window_rect(&image.pixels(), None),
            Some(IRect::new(16, 18, 685, 383)),
            "without the meta the stray pixel counts"
        );
    }

    // spec: capture / Window capture / Window on Wayland
    #[test]
    fn a_crop_meta_beyond_the_buffer_is_clipped() {
        let image = measured_window(0);
        let rect = window_rect(&image.pixels(), Some((0, 0, 10_000, 10_000)));
        assert_eq!(rect, Some(IRect::new(16, 18, 280, 220)));
    }

    // spec: capture / Window capture / Window on Wayland
    #[test]
    fn a_translucent_window_keeps_a_faint_shadow() {
        // No pixel reaches OPAQUE_ALPHA: fall back to alpha > 0 (design.md D3, step 3).
        let image = Image::new(100, 80, |x, y| {
            if (10..90).contains(&x) && (5..60).contains(&y) {
                if (14..86).contains(&x) && (9..56).contains(&y) {
                    100
                } else {
                    30
                }
            } else {
                0
            }
        });
        assert_eq!(
            window_rect(&image.pixels(), None),
            Some(IRect::new(10, 5, 80, 55))
        );
    }

    // spec: capture / Window capture / No frame arrives
    #[test]
    fn a_frame_without_a_visible_pixel_is_no_frame() {
        // Scenario "No frame arrives", task 4.2: an error, not an empty image.
        let image = Image::new(64, 48, |_, _| 0);
        assert_eq!(window_rect(&image.pixels(), None), None);
        assert_eq!(window_rect(&image.pixels(), Some((0, 0, 32, 32))), None);
    }

    // spec: capture / Window capture / Window on Wayland
    #[test]
    fn the_threshold_keeps_the_outline_and_drops_the_shadow() {
        // The measured values around the threshold: shadow 28–41 below, outline ~200 above.
        const { assert!(41 < OPAQUE_ALPHA && OPAQUE_ALPHA <= 200) };
    }
}
