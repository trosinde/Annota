//! The ScreenCast session of a recording (add-screen-recording task 3.1, design.md D2).
//!
//! One source per recording, never a restore token: a monitor for Region and Screen, a window
//! for Window, chosen in the desktop's own chooser with Annota's waiting indicator as its parent
//! (`capture::window_wait`, "Waiting for a screen…" or "Waiting for a window…"). The pointer is
//! embedded when the user wants it and the portal offers it (`screencast::cursor_mode`).
//!
//! How each answer is handled (spec "Recording source"):
//! - code 0: the stream is kept ([`Granted`]: node, `position`, `size`, the PipeWire remote and
//!   the session's `Closed` signal, which the recording keeps listening to);
//! - code 1: the desktop's chooser was cancelled. Measured (P1): the portal has already removed
//!   the session, so Annota sends no `Session.Close`;
//! - Cancel or `Esc` in the waiting indicator: Annota closes the session;
//! - `Session.Closed` before Start answers: the indicator goes at once, a later answer is ignored
//!   ([`Requested::DesktopEnded`]).
//!
//! Every way out leaves the indicator closed; a granted source returns only once the indicator
//! has been unmapped and [`countdown::SETTLE`] has passed, so it is in no recorded frame
//! (P10, "Waiting indicator is not recorded").

use std::os::fd::{AsRawFd, OwnedFd, RawFd};
use std::pin::pin;

use ashpd::desktop::screencast::{CursorMode, Screencast, SourceType};
use ashpd::desktop::{PersistMode, Session};
use ashpd::zbus;
use ashpd::WindowIdentifier;
use futures_channel::oneshot;
use futures_util::future::{self, Either};
use futures_util::StreamExt;
use gettextrs::gettext;
use gstreamer as gst;
use gstreamer::prelude::*;
use gtk::{gdk, glib, prelude::*};

use super::RecordingError;
use crate::capture::countdown;
use crate::capture::portal::{self, CaptureError, ParentWindow};
use crate::capture::screencast::{self, Close, PortalSession};
use crate::capture::window_wait::{self, WaitFor};

/// The kind of ScreenCast source a recording asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    /// Region and Screen.
    Monitor,
    Window,
}

impl SourceKind {
    fn portal_type(self) -> SourceType {
        match self {
            Self::Monitor => SourceType::Monitor,
            Self::Window => SourceType::Window,
        }
    }

    /// What the waiting indicator says (mockup 03; the window capture's sheet 02).
    pub fn wait_for(self) -> WaitFor {
        match self {
            Self::Monitor => WaitFor::Screen,
            Self::Window => WaitFor::Window,
        }
    }
}

/// Which source types the ScreenCast portal offers (scenario "Source not offered"). Both
/// `false` without a portal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Offered {
    pub monitor: bool,
    pub window: bool,
}

impl Offered {
    pub fn offers(self, kind: SourceKind) -> bool {
        match kind {
            SourceKind::Monitor => self.monitor,
            SourceKind::Window => self.window,
        }
    }
}

/// Asks the portal for its `AvailableSourceTypes`, for the recording setup (Phase C).
pub async fn offered() -> Offered {
    portal::ensure_registered().await;
    let Ok(proxy) = Screencast::new().await else {
        return Offered::default();
    };
    let types = proxy.available_source_types().await.unwrap_or_default();
    Offered {
        monitor: types.contains(SourceType::Monitor),
        window: types.contains(SourceType::Window),
    }
}

/// The setup's line for a source the portal does not offer.
pub fn not_offered_message() -> String {
    gettext("Screen sharing for this source is not available on this desktop")
}

/// The session's `Closed` signal (raw messages: ashpd 0.11's own `receive_closed` expects an
/// empty body and drops the real `a{sv}` one).
pub type ClosedSignal = zbus::proxy::SignalStream<'static>;

/// A granted source: the open session and its stream. Dropping it releases the PipeWire remote
/// and then closes the session (unless the desktop already closed it).
pub struct Granted {
    pub kind: SourceKind,
    pub node: u32,
    /// The stream's `position` and `size` (logical px, P2).
    pub position: Option<(i32, i32)>,
    pub size: Option<(i32, i32)>,
    fd: Option<OwnedFd>,
    session: Option<PortalSession>,
    closed: Option<ClosedSignal>,
}

impl std::fmt::Debug for Granted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Granted")
            .field("kind", &self.kind)
            .field("node", &self.node)
            .field("position", &self.position)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

impl Granted {
    /// The PipeWire remote for `pipewiresrc`; valid as long as `self` holds it.
    pub fn fd(&self) -> Option<RawFd> {
        self.fd.as_ref().map(AsRawFd::as_raw_fd)
    }

    /// The session's `Closed` signal, taken once by whoever watches it while recording.
    pub fn take_closed(&mut self) -> Option<ClosedSignal> {
        self.closed.take()
    }

    /// Releases the PipeWire remote, then closes the session and waits for the portal.
    pub async fn close(mut self) {
        drop(self.fd.take());
        if let Some(session) = self.session.take() {
            session.close().await;
        }
    }

    /// The desktop closed the session (`Session.Closed`): release the remote, send nothing.
    pub fn closed_by_desktop(mut self) {
        drop(self.fd.take());
        self.session.take();
    }
}

impl Drop for Granted {
    fn drop(&mut self) {
        drop(self.fd.take());
        if let Some(session) = self.session.take() {
            session.close_detached();
        }
    }
}

/// How a source request ended.
#[derive(Debug)]
pub enum Requested {
    Granted(Box<Granted>),
    /// Cancel/`Esc` in the indicator, or the desktop's chooser cancelled (code 1). No message.
    Cancelled,
    /// The portal closed the session before Start answered.
    DesktopEnded,
    /// Anything else (code 2, no portal, no stream); logged, the session closed.
    Failed(String),
}

/// `pipewiresrc fd=… path=<node>` delivering system memory (production video source).
pub fn pipewire_source(fd: RawFd, node: u32) -> Result<gst::Element, RecordingError> {
    let source = gst::ElementFactory::make("pipewiresrc")
        .property("fd", fd)
        .property("path", node.to_string())
        .build()
        .map_err(|_| RecordingError::MissingElement(String::from("pipewiresrc")))?;
    // Older PipeWire plugins lack it; then frames are mapped from PipeWire's own buffers.
    if source.find_property("always-copy").is_some() {
        source.set_property("always-copy", true);
    }
    Ok(source)
}

/// The scale of the GDK monitor a stream of this `position`/`size` (logical px) is on, for the
/// window crop meta conversion (design.md D2). `None` when no monitor or several monitors of
/// different scales match: the window is then found by alpha.
pub fn monitor_scale(position: Option<(i32, i32)>, size: Option<(i32, i32)>) -> Option<f64> {
    let size = size?;
    let monitors = gdk::Display::default()?.monitors();
    let mut scales = (0..monitors.n_items())
        .filter_map(|i| monitors.item(i)?.downcast::<gdk::Monitor>().ok())
        .filter(|monitor| {
            let g = monitor.geometry();
            (g.width(), g.height()) == size && position.is_none_or(|p| (g.x(), g.y()) == p)
        })
        .map(|monitor| monitor.scale());
    let first = scales.next()?;
    scales.all(|s| s == first).then_some(first)
}

/// How many monitors GDK knows.
pub fn monitor_count() -> u32 {
    gdk::Display::default().map_or(0, |display| display.monitors().n_items())
}

/// Requests one source with the waiting indicator as the chooser's parent.
pub async fn request(app: &adw::Application, kind: SourceKind, include_pointer: bool) -> Requested {
    portal::ensure_registered().await;
    let proxy = match Screencast::new().await {
        Ok(proxy) => proxy,
        Err(err) => return failed(&err),
    };
    let types = proxy.available_source_types().await.unwrap_or_default();
    if !types.contains(kind.portal_type()) {
        return Requested::Failed(String::from("the portal does not offer this source type"));
    }
    // Version 1 of the interface has no cursor modes; then only HIDDEN is asked for.
    let embedded = proxy
        .available_cursor_modes()
        .await
        .unwrap_or_default()
        .contains(CursorMode::Embedded);
    let cursor = screencast::cursor_mode(include_pointer, embedded);
    // GStreamer's first `init` scans the plugin registry: done alongside the chooser.
    std::thread::spawn(|| {
        let _ = gst::init();
    });
    let Some((indicator, cancelled)) = window_wait::show(app, kind.wait_for()) else {
        return Requested::Failed(String::from("window-wait.ui is incomplete"));
    };
    let parent = ParentWindow::new(&indicator).await;
    let identifier = parent.for_request();
    if identifier.is_none() {
        eprintln!("annota: no parent window handle for the screen-sharing chooser");
    }
    let requested = negotiate(&proxy, kind, cursor, identifier, cancelled).await;
    drop(parent);
    window_wait::hide(&indicator);
    if matches!(requested, Requested::Granted(_)) {
        // Unmapped now; give the compositor the same time as the countdown OSD gets.
        glib::timeout_future(countdown::SETTLE).await;
    }
    requested
}

fn failed(err: &ashpd::Error) -> Requested {
    match screencast::map_error(err) {
        CaptureError::Cancelled => Requested::Cancelled,
        other => Requested::Failed(other.to_string()),
    }
}

async fn negotiate(
    proxy: &Screencast<'static>,
    kind: SourceKind,
    cursor: CursorMode,
    parent: Option<WindowIdentifier>,
    cancelled: oneshot::Receiver<()>,
) -> Requested {
    let session = match proxy.create_session().await {
        Ok(session) => PortalSession(session),
        Err(err) => return failed(&err),
    };
    // Listening before Start, so an early `Closed` is never missed.
    let mut closed = closed_signal(proxy, &session.0).await;
    if closed.is_none() {
        eprintln!("annota: cannot watch the screen-sharing session for its end");
    }
    let selected = proxy
        .select_sources(
            &session.0,
            cursor,
            kind.portal_type().into(),
            false,
            None,
            PersistMode::DoNot,
        )
        .await
        .and_then(|request| request.response());
    if let Err(err) = selected {
        session.close().await;
        return failed(&err);
    }

    let answer = {
        let start = pin!(proxy.start(&session.0, parent.as_ref()));
        let cancel = pin!(async {
            // A dropped sender means the indicator is gone, which ends the wait just the same.
            let _ = cancelled.await;
        });
        let ended = pin!(async {
            match closed.as_mut() {
                Some(signal) => {
                    signal.next().await;
                }
                None => future::pending::<()>().await,
            }
        });
        match future::select(start, future::select(cancel, ended)).await {
            Either::Left((answer, _)) => Ok(answer),
            Either::Right((Either::Left(_), _)) => Err(false),
            Either::Right((Either::Right(_), _)) => Err(true),
        }
    };
    let answer = match answer {
        Ok(answer) => answer,
        Err(false) => {
            session.close().await;
            return Requested::Cancelled;
        }
        // The portal has closed the session itself; whatever Start answers later is dropped
        // with its request.
        Err(true) => return Requested::DesktopEnded,
    };
    let streams = match answer.and_then(|request| request.response()) {
        Ok(streams) => streams,
        Err(err) => {
            let requested = failed(&err);
            if !matches!(requested, Requested::Cancelled) {
                session.close().await;
            }
            // Code 1: the session counts as closed (P1), so nothing is sent.
            return requested;
        }
    };
    let Some(stream) = streams.streams().first() else {
        session.close().await;
        return Requested::Failed(String::from("the portal started no stream"));
    };
    let (node, position, size) = (stream.pipe_wire_node_id(), stream.position(), stream.size());
    let fd = match proxy.open_pipe_wire_remote(&session.0).await {
        Ok(fd) => fd,
        Err(err) => {
            session.close().await;
            return Requested::Failed(screencast::map_error(&err).to_string());
        }
    };
    Requested::Granted(Box::new(Granted {
        kind,
        node,
        position,
        size,
        fd: Some(fd),
        session: Some(session),
        closed,
    }))
}

/// Subscribes to the session's `Closed` on ashpd's own connection (the portal sends it to the
/// session's owner only).
async fn closed_signal(
    proxy: &Screencast<'static>,
    session: &Session<'static, Screencast<'static>>,
) -> Option<ClosedSignal> {
    let path = session_path(session)?;
    let session_proxy: zbus::Proxy<'static> = zbus::proxy::Builder::new(proxy.connection())
        .destination(proxy.destination().to_owned())
        .ok()?
        .path(path)
        .ok()?
        .interface("org.freedesktop.portal.Session")
        .ok()?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
        .ok()?;
    session_proxy.receive_signal("Closed").await.ok()
}

/// The session's object path: ashpd keeps it private but serialises the session as it.
fn session_path(
    session: &Session<'static, Screencast<'static>>,
) -> Option<zbus::zvariant::OwnedObjectPath> {
    use zbus::zvariant::{serialized::Context, to_bytes, ObjectPath, LE};
    let data = to_bytes(Context::new_dbus(LE, 0), session).ok()?;
    let (path, _): (ObjectPath<'_>, _) = data.deserialize().ok()?;
    Some(path.into())
}
