//! A running recording and its screen-sharing session, and the ways it ends (task 3.7, spec
//! "Stop recording").
//!
//! - [`RecordingSession::stop`] (Stop button, Record shortcut): finishes the file and closes the
//!   session;
//! - `Session.Closed` from the portal (GNOME's ■, the recorded window closed): no end of stream
//!   ever arrives (P5/P6), so the file is finished at once and nothing is sent back;
//! - an error in the pipeline, or the stream's own end: finished with what was written, the
//!   session closed.
//!
//! Each ends exactly once, with one [`Ended`] for the caller (the Export Recording window, D1).
//! Finishing the file runs off the main thread; the [`Ended`] follows when it is written.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use futures_channel::mpsc;
use futures_util::StreamExt;
use gtk::{gio, glib};

use super::pipeline::{self, Finished, Recording, RecordingEvent};
use super::source::Granted;

/// Why a recording ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndReason {
    /// Stop in the control or the Record shortcut.
    Stopped,
    /// The portal closed the session (GNOME's stop, the recorded window closed).
    DesktopEnded,
    /// The video stream sent its own end of stream.
    SourceEnded,
    /// The pipeline failed; the file keeps what was written.
    Error,
}

/// A finished recording, for the Export Recording window.
#[derive(Clone, Debug)]
pub struct Ended {
    pub finished: Finished,
    pub reason: EndReason,
}

impl Ended {
    /// Whether it ended because of an error (mockup 11's banner).
    pub fn by_error(&self) -> bool {
        self.finished.error.is_some()
    }
}

type OnEnded = Box<dyn FnOnce(Ended)>;
type OnEvent = Rc<dyn Fn(&RecordingEvent)>;

pub struct RecordingSession {
    running: RefCell<Option<(Recording, Granted)>>,
    on_ended: RefCell<Option<OnEnded>>,
    watch: RefCell<Option<glib::JoinHandle<()>>>,
    finishing: Cell<bool>,
    app: adw::Application,
    _hold: gio::ApplicationHoldGuard,
}

impl RecordingSession {
    /// Takes over a started recording and its source. `on_event` gets level meters (the
    /// control, Phase C); `on_ended` is called once when the recording has ended.
    pub fn begin(
        app: &adw::Application,
        recording: Recording,
        events: mpsc::UnboundedReceiver<RecordingEvent>,
        mut granted: Granted,
        on_event: OnEvent,
        on_ended: impl FnOnce(Ended) + 'static,
    ) -> Rc<Self> {
        let closed = granted.take_closed();
        let session = Rc::new(Self {
            running: RefCell::new(Some((recording, granted))),
            on_ended: RefCell::new(Some(Box::new(on_ended))),
            watch: RefCell::new(None),
            finishing: Cell::new(false),
            app: app.clone(),
            _hold: app.hold(),
        });
        if let Some(mut closed) = closed {
            let weak = Rc::downgrade(&session);
            let watch = glib::spawn_future_local(async move {
                if closed.next().await.is_some() {
                    if let Some(session) = weak.upgrade() {
                        session.finish(EndReason::DesktopEnded);
                    }
                }
            });
            session.watch.replace(Some(watch));
        }
        let weak = Rc::downgrade(&session);
        pipeline::forward_events(events, move |event| {
            on_event(&event);
            let Some(session) = weak.upgrade() else {
                return;
            };
            match event {
                RecordingEvent::Error(_) => session.finish(EndReason::Error),
                RecordingEvent::SourceEnded => session.finish(EndReason::SourceEnded),
                RecordingEvent::Level { .. } => {}
            }
        });
        session
    }

    /// Runs `f` with the recording while it runs (pause, mute, elapsed time).
    pub fn with_recording<R>(&self, f: impl FnOnce(&Recording) -> R) -> Option<R> {
        self.running
            .borrow()
            .as_ref()
            .map(|(recording, _)| f(recording))
    }

    /// Stop in the control or the Record shortcut.
    pub fn stop(&self) {
        self.finish(EndReason::Stopped);
    }

    pub fn is_running(&self) -> bool {
        self.running.borrow().is_some()
    }

    fn finish(&self, reason: EndReason) {
        if self.finishing.replace(true) {
            return;
        }
        // Each `take` is its own statement, so no `RefMut` is held while code runs that could
        // reach back into this session (the watch's drop, the caller's `on_ended`).
        let watch = self.watch.borrow_mut().take();
        if let Some(watch) = watch {
            watch.abort();
        }
        let running = self.running.borrow_mut().take();
        let Some((recording, granted)) = running else {
            return;
        };
        // Never waits for the stream's end: the encoder gets its end of stream from here.
        let finishing = recording.stop();
        let on_ended = self.on_ended.borrow_mut().take();
        // Until the Ended is delivered, even if this session is gone by then.
        let hold = self.app.hold();
        glib::spawn_future_local(async move {
            let finished = finishing.await;
            // The PipeWire remote is released once the sources have stopped with the file.
            match reason {
                // The portal has closed it already: release the remote, send nothing.
                EndReason::DesktopEnded => granted.closed_by_desktop(),
                _ => {
                    glib::spawn_future_local(granted.close());
                }
            }
            if let Some(on_ended) = on_ended {
                on_ended(Ended { finished, reason });
            }
            drop(hold);
        });
    }
}
