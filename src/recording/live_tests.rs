//! The recording's ScreenCast session against a stand-in portal, on a live display.
//!
//! Same rules as `capture::live_tests` (whose isolated scene this reuses): a real
//! `xdg-desktop-portal` is never called, each test runs in a process of its own on a private bus
//! (`openspec/scenario-gate.toml`, run "live"). The stand-in answers `Start` when the test says so
//! and can emit `Session.Closed` at any time, as GNOME was measured to (P1, P5, P6). The PipeWire
//! remote it hands out is `/dev/null`; the recording's video source is a test source instead
//! (`Hooks::video_source`), so these tests prove what Annota sends, how it sorts the answers and
//! what it writes, never what a real portal does (CLAUDE.md §5).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use adw::prelude::*;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;
use gtk::glib::translate::IntoGlib;
use gtk::{gdk, gio, glib};

use super::*;
use crate::capture::frame::Frame;
use crate::capture::live_tests::{
    find, portal_path, respond, scene, shot, Question, Scene, SCREENCAST_XML,
};
use crate::capture::overlay::{self, Desktop, Mode, Outcome};
use crate::capture::window_wait;
use crate::recording::pipeline::tests::{analyse, near, rms_range, AChunk, GREEN, RED};
use crate::ui::test_support::pump;

const DESKTOP_ENDED: &str = "The desktop ended the screen-sharing request. Nothing was recorded.";
const NOT_ON_SCREEN: &str =
    "The selected area is not on the screen you chose. Nothing was recorded.";

/// The stream the stand-in grants.
#[derive(Clone, Copy)]
struct StreamProps {
    source_type: u32,
    position: Option<(i32, i32)>,
    size: (i32, i32),
}

type PendingStart = Rc<RefCell<Option<Box<dyn FnOnce(u32)>>>>;

/// The ScreenCast half of the stand-in portal for recordings.
struct FakeCast {
    connection: gio::DBusConnection,
    log: Rc<RefCell<Vec<String>>>,
    selected: Rc<RefCell<HashMap<String, glib::Variant>>>,
    parent: Rc<RefCell<Option<String>>>,
    /// `Start` answers at once with this code; `None`: it waits for [`FakeCast::answer`].
    auto: Rc<Cell<Option<u32>>>,
    pending: PendingStart,
    /// The open session: its owner and path.
    session: Rc<RefCell<Option<(String, String)>>>,
}

impl FakeCast {
    fn calls(&self) -> Vec<String> {
        self.log.borrow().clone()
    }

    fn count(&self, call: &str) -> usize {
        self.log.borrow().iter().filter(|c| *c == call).count()
    }

    fn option_u32(&self, name: &str) -> Option<u32> {
        self.selected
            .borrow()
            .get(name)
            .and_then(|v| v.get::<u32>())
    }

    /// Answers the waiting `Start`.
    fn answer(&self, code: u32) {
        let answer = self.pending.borrow_mut().take().expect("a waiting Start");
        answer(code);
    }

    /// The portal ends the session: `Session.Closed`, as GNOME's ■ or a closed window do.
    fn close_session(&self) {
        let (sender, path) = self.session.borrow().clone().expect("an open session");
        let details: HashMap<String, glib::Variant> = HashMap::new();
        self.connection
            .emit_signal(
                Some(&sender),
                &path,
                "org.freedesktop.portal.Session",
                "Closed",
                Some(&(details,).to_variant()),
            )
            .unwrap();
        self.log.borrow_mut().push(String::from("Closed"));
    }

    fn reaches(&self, call: &str, count: usize) {
        wait_for(|| self.count(call) >= count, &format!("{count} × {call}"));
    }
}

/// Adds `org.freedesktop.portal.ScreenCast` to the scene's portal.
fn add_cast(scene: &Scene, source_types: u32, stream: StreamProps) -> FakeCast {
    let connection = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).unwrap();
    let info = gio::DBusNodeInfo::for_xml(SCREENCAST_XML).unwrap();
    let log = Rc::new(RefCell::new(Vec::<String>::new()));
    let selected = Rc::new(RefCell::new(HashMap::new()));
    let parent = Rc::new(RefCell::new(None));
    let auto = Rc::new(Cell::new(None));
    let pending: PendingStart = Rc::new(RefCell::new(None));
    let session = Rc::new(RefCell::new(None));
    let session_interface = info
        .lookup_interface("org.freedesktop.portal.Session")
        .unwrap();
    let (calls, options, start_parent, answer_now, held, open) = (
        log.clone(),
        selected.clone(),
        parent.clone(),
        auto.clone(),
        pending.clone(),
        session.clone(),
    );
    connection
        .register_object(
            "/org/freedesktop/portal/desktop",
            &info
                .lookup_interface("org.freedesktop.portal.ScreenCast")
                .unwrap(),
        )
        .method_call(
            move |connection, sender, _, _, method, parameters, invocation| {
                let sender = sender.unwrap().to_string();
                calls.borrow_mut().push(method.to_string());
                let last = parameters.n_children() - 1;
                let opts: HashMap<String, glib::Variant> =
                    parameters.child_value(last).get().unwrap();
                let token = |name: &str| opts.get(name).and_then(|v| v.get::<String>());
                match method {
                    "OpenPipeWireRemote" => {
                        let list = gio::UnixFDList::new();
                        let null = std::fs::File::open("/dev/null").unwrap();
                        let index = list.append(null).unwrap();
                        invocation.return_value_with_unix_fd_list(
                            Some(&(glib::variant::Handle(index),).to_variant()),
                            Some(&list),
                        );
                    }
                    "CreateSession" => {
                        let path = portal_path(
                            "session",
                            &sender,
                            &token("session_handle_token").unwrap(),
                        );
                        let closed = calls.clone();
                        let id = Rc::new(RefCell::new(None));
                        let own_id = id.clone();
                        let registration = connection
                            .register_object(&path, &session_interface)
                            .method_call(move |connection, _, _, _, _, _, invocation| {
                                closed.borrow_mut().push(String::from("Session.Close"));
                                invocation.return_value(None);
                                if let Some(id) = own_id.borrow_mut().take() {
                                    let _ = connection.unregister_object(id);
                                }
                            })
                            .property(|_, _, _, _, _| 1u32.to_variant())
                            .build()
                            .unwrap();
                        id.replace(Some(registration));
                        open.replace(Some((sender.clone(), path.clone())));
                        let handle =
                            portal_path("request", &sender, &token("handle_token").unwrap());
                        let reply = glib::variant::ObjectPath::try_from(handle.clone()).unwrap();
                        invocation.return_value(Some(&(reply,).to_variant()));
                        let results =
                            HashMap::from([(String::from("session_handle"), path.to_variant())]);
                        respond(&connection, &sender, &handle, 0, results);
                    }
                    "SelectSources" | "Start" => {
                        let handle =
                            portal_path("request", &sender, &token("handle_token").unwrap());
                        let reply = glib::variant::ObjectPath::try_from(handle.clone()).unwrap();
                        invocation.return_value(Some(&(reply,).to_variant()));
                        if method == "SelectSources" {
                            options.replace(opts.clone());
                            respond(&connection, &sender, &handle, 0, HashMap::new());
                            return;
                        }
                        let window = parameters.child_value(1).str().unwrap().to_string();
                        start_parent.replace(Some(window));
                        let connection = connection.clone();
                        let answer = move |code: u32| {
                            let mut results = HashMap::new();
                            if code == 0 {
                                let mut props: HashMap<String, glib::Variant> = HashMap::from([
                                    (String::from("source_type"), stream.source_type.to_variant()),
                                    (String::from("size"), stream.size.to_variant()),
                                ]);
                                if let Some(position) = stream.position {
                                    props.insert(String::from("position"), position.to_variant());
                                }
                                results.insert(
                                    String::from("streams"),
                                    vec![(42u32, props)].to_variant(),
                                );
                            }
                            respond(&connection, &sender, &handle, code, results);
                        };
                        match answer_now.get() {
                            Some(code) => answer(code),
                            None => {
                                held.replace(Some(Box::new(answer)));
                            }
                        }
                    }
                    _ => unreachable!(),
                }
            },
        )
        .property(move |_, _, _, _, name| match name {
            "AvailableSourceTypes" => source_types.to_variant(),
            // HIDDEN | EMBEDDED | METADATA, as GNOME 50 offers (P4).
            "AvailableCursorModes" => 7u32.to_variant(),
            _ => 5u32.to_variant(),
        })
        .build()
        .unwrap();
    let _ = &scene.app;
    FakeCast {
        connection,
        log,
        selected,
        parent,
        auto,
        pending,
        session,
    }
}

fn wait_for(done: impl Fn() -> bool, what: &str) {
    let end = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        pump(20);
    }
}

/// What the flow reported, and when.
#[derive(Default)]
struct Seen {
    outcomes: RefCell<Vec<StartOutcome>>,
    ended: RefCell<Vec<(Ended, Instant)>>,
    /// When the video source was built.
    built: Cell<Option<Instant>>,
    /// When its first buffer left it.
    first_buffer: Arc<Mutex<Option<Instant>>>,
    countdown: RefCell<Vec<(Target, u32, Instant)>>,
}

/// `videotestsrc` (live, 320×240 BGRx, 30 fps) in place of `pipewiresrc`.
fn test_screen(seen: &Rc<Seen>) -> VideoSource {
    let seen = seen.clone();
    Rc::new(move |_fd, node| {
        assert_eq!(node, 42, "the granted stream's node");
        seen.built.set(Some(Instant::now()));
        let bin = gst::Bin::new();
        let src = gst::ElementFactory::make("videotestsrc")
            .property("is-live", true)
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
        let first = seen.first_buffer.clone();
        pad.add_probe(gst::PadProbeType::BUFFER, move |_, _| {
            first.lock().unwrap().get_or_insert_with(Instant::now);
            gst::PadProbeReturn::Ok
        });
        bin.add_pad(&gst::GhostPad::with_target(&pad).unwrap())
            .unwrap();
        Ok(bin.upcast())
    })
}

/// A scene with the recording hooks pointed at `seen` and no audio device involved.
fn recording_scene(name: &str, include_pointer: bool) -> (Scene, Rc<Seen>, PathBuf) {
    let scene = scene(name, Some("yes"), Question::CannotShow);
    let folder = scene.dir.join("recordings");
    let settings = gio::Settings::new(config::APP_ID);
    settings
        .set_string("record-folder", folder.to_str().unwrap())
        .unwrap();
    settings.set_boolean("record-microphone", false).unwrap();
    settings.set_boolean("record-system-audio", false).unwrap();
    settings
        .set_boolean("include-pointer", include_pointer)
        .unwrap();
    gst::init().unwrap();
    let seen = Rc::new(Seen::default());
    let (outcomes, ended, countdown) = (seen.clone(), seen.clone(), seen.clone());
    let video = test_screen(&seen);
    set_hooks(move |hooks| {
        hooks.video_source = video;
        hooks.audio_sources = Rc::new(|_, _| Vec::new());
        hooks.devices = Rc::new(|| {
            Box::pin(async {
                SystemDevices::new(Some("Built-in Audio"), Some("Built-in Speakers"))
            })
        });
        hooks.preview_source = Rc::new(|_| {
            gst::ElementFactory::make("audiotestsrc")
                .property("is-live", true)
                .property("volume", 0.4)
                .build()
                .ok()
        });
        hooks.countdown = Rc::new(move |_, target, seconds| {
            countdown
                .countdown
                .borrow_mut()
                .push((target, seconds, Instant::now()));
            Box::pin(async { true })
        });
        hooks.on_outcome = Rc::new(move |_, outcome| {
            outcomes.outcomes.borrow_mut().push(outcome.clone());
        });
        hooks.on_ended = Rc::new(move |_, done| {
            ended.ended.borrow_mut().push((done, Instant::now()));
        });
    });
    (scene, seen, folder)
}

fn files_in(folder: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(folder)
        .map(|dir| dir.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default()
}

fn the_monitor() -> gdk::Rectangle {
    let monitors = gdk::Display::default().unwrap().monitors();
    assert_eq!(monitors.n_items(), 1, "one monitor");
    monitors
        .item(0)
        .and_downcast::<gdk::Monitor>()
        .unwrap()
        .geometry()
}

fn screen_stream() -> StreamProps {
    let monitor = the_monitor();
    StreamProps {
        source_type: 1,
        position: Some((monitor.x(), monitor.y())),
        size: (monitor.width(), monitor.height()),
    }
}

/// The indicator's label and icon.
fn indicator_text(osd: &gtk::Window) -> (String, String) {
    let label: gtk::Label = find(osd, "wait_label").expect("the label");
    let icon: gtk::Image = find(osd, "wait_icon").expect("the icon");
    (
        label.label().to_string(),
        icon.icon_name().map(|n| n.to_string()).unwrap_or_default(),
    )
}

fn outcomes(seen: &Seen) -> Vec<StartOutcome> {
    seen.outcomes.borrow().clone()
}

/// Scenarios "Waiting indicator with one monitor", "Waiting indicator is not recorded" and
/// "Record the screen": `--record screen` on one monitor shows "Waiting for a screen…" until
/// Start answers; the indicator is unmapped before the video source exists and before its first
/// frame, with no countdown; the recording has the stream's full size. Stop then closes the
/// session and delivers the finished file once; a second request meanwhile starts nothing.
// spec: screen-recording / Recording source / Waiting indicator with one monitor
// spec: screen-recording / Recording countdown / Waiting indicator is not recorded
// spec: screen-recording / Recording source / Record the screen
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn rec_screen_waits_for_start_then_records_the_whole_stream() {
    let (scene, seen, folder) = recording_scene("rec-screen", false);
    let cast = add_cast(&scene, 7, screen_stream());
    start(
        &scene.app,
        RecordRequest {
            mode: RecordMode::Screen,
        },
    );
    cast.reaches("Start", 1);
    let osd = window_wait::shown().expect("the waiting indicator");
    shot(&osd, "rec-waiting-for-screen");
    assert_eq!(
        indicator_text(&osd),
        (
            String::from("Waiting for a screen…"),
            String::from("annota-capture-screen-symbolic")
        )
    );
    let cancel: gtk::Button = find(&osd, "cancel_button").unwrap();
    assert!(cancel.has_focus(), "Cancel has the initial focus");
    assert_eq!(cast.option_u32("types"), Some(1), "a monitor source");
    assert_eq!(cast.option_u32("persist_mode"), Some(0), "no restore token");
    assert!(!cast.selected.borrow().contains_key("restore_token"));
    assert_eq!(
        cast.selected
            .borrow()
            .get("multiple")
            .and_then(|v| v.get::<bool>()),
        Some(false)
    );
    assert!(cast.parent.borrow().clone().unwrap().starts_with("x11:"));
    // Shown until Start answers, however long the chooser stays open.
    pump(800);
    assert!(osd.is_mapped(), "still waiting");
    assert!(is_active());
    let unmapped = Rc::new(Cell::new(None));
    osd.connect_unmap({
        let unmapped = unmapped.clone();
        move |_| unmapped.set(Some(Instant::now()))
    });
    assert!(seen.built.get().is_none());

    cast.answer(0);
    wait_for(|| seen.first_buffer.lock().unwrap().is_some(), "a frame");
    let gone = unmapped.get().expect("the indicator was unmapped");
    assert!(window_wait::shown().is_none());
    let built = seen.built.get().unwrap();
    let first = seen.first_buffer.lock().unwrap().unwrap();
    assert!(
        built > gone,
        "the source exists only after the indicator went"
    );
    assert!(
        first >= gone + crate::capture::countdown::SETTLE,
        "first frame {:?} after the indicator went",
        first - gone
    );
    // The countdown slot (0 s here) comes after the indicator too.
    let countdown = seen.countdown.borrow().clone();
    assert_eq!(countdown.len(), 1);
    assert_eq!(countdown[0].0, Target::Screen);
    assert!(countdown[0].2 > gone);
    assert_eq!(outcomes(&seen), [StartOutcome::Recording]);

    // A second request while recording starts no second session.
    start(
        &scene.app,
        RecordRequest {
            mode: RecordMode::Screen,
        },
    );
    pump(700);
    assert_eq!(cast.count("CreateSession"), 1);
    assert!(running().is_some());

    assert!(stop());
    // The file is finished off the main thread; the flow is active until it is.
    wait_for(
        || !seen.ended.borrow().is_empty(),
        "the recording to finish",
    );
    assert!(!is_active());
    cast.reaches("Session.Close", 1);
    assert_eq!(
        cast.calls(),
        [
            "CreateSession",
            "SelectSources",
            "Start",
            "OpenPipeWireRemote",
            "Session.Close"
        ]
    );
    let ended = std::mem::take(&mut *seen.ended.borrow_mut());
    assert_eq!(ended.len(), 1, "one finished recording");
    let (done, _) = &ended[0];
    assert_eq!(done.reason, EndReason::Stopped);
    assert!(!done.by_error());
    assert_eq!(files_in(&folder), std::slice::from_ref(&done.finished.path));
    let analysis = analyse(&done.finished.path);
    assert!(analysis.frames.len() >= 10, "{}", analysis.frames.len());
    for frame in &analysis.frames {
        assert_eq!((frame.width, frame.height), (320, 240), "the full frame");
    }
    pump(300);
    assert!(seen.ended.borrow().is_empty(), "delivered once");
}

/// Scenarios "Pointer in the recording" and "Chooser cancelled in the desktop": the cursor mode
/// follows Include pointer; Start answering 1 ends the request without `Session.Close`, without
/// a file and without a message. The portal offers monitors only here, so a window recording is
/// refused before any session (the setup's "Source not offered" reads the same `offered`).
// spec: screen-recording / Recording source / Pointer in the recording
// spec: screen-recording / Recording source / Chooser cancelled in the desktop
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn rec_pointer_mode_and_a_chooser_cancelled_in_the_desktop() {
    let (scene, seen, folder) = recording_scene("rec-pointer", true);
    let cast = add_cast(&scene, 1, screen_stream());
    cast.auto.set(Some(1));
    let offered = glib::MainContext::default().block_on(source::offered());
    assert_eq!(
        offered,
        source::Offered {
            monitor: true,
            window: false
        }
    );
    start(
        &scene.app,
        RecordRequest {
            mode: RecordMode::Screen,
        },
    );
    wait_for(|| !outcomes(&seen).is_empty(), "the outcome");
    assert_eq!(cast.option_u32("cursor_mode"), Some(2), "embedded");
    assert_eq!(outcomes(&seen), [StartOutcome::Cancelled]);
    assert_eq!(StartOutcome::Cancelled.message(), None, "no message");
    pump(500);
    assert_eq!(cast.count("Session.Close"), 0, "{:?}", cast.calls());
    assert!(files_in(&folder).is_empty(), "nothing written");
    assert!(window_wait::shown().is_none());
    assert!(!is_active());

    gio::Settings::new(config::APP_ID)
        .set_boolean("include-pointer", false)
        .unwrap();
    start(
        &scene.app,
        RecordRequest {
            mode: RecordMode::Screen,
        },
    );
    wait_for(|| outcomes(&seen).len() == 2, "the second outcome");
    assert_eq!(cast.option_u32("cursor_mode"), Some(1), "hidden");
    assert_eq!(cast.count("Session.Close"), 0);

    start(
        &scene.app,
        RecordRequest {
            mode: RecordMode::Window,
        },
    );
    wait_for(|| outcomes(&seen).len() == 3, "the third outcome");
    assert!(matches!(outcomes(&seen)[2], StartOutcome::Failed(_)));
    assert_eq!(cast.count("CreateSession"), 2, "no session for a window");
    assert!(files_in(&folder).is_empty());
}

/// Scenario "Source request cancelled": `Esc` in the indicator (through its key controller, as
/// GTK delivers the key) closes the session, writes nothing and says nothing; so do Cancel and
/// closing the indicator. The request ending from Annota's side in any other way (its future
/// dropped, as when Annota quits while waiting) closes the session too.
// spec: screen-recording / Recording source / Source request cancelled
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn rec_cancel_in_the_indicator_closes_the_session() {
    let (scene, seen, folder) = recording_scene("rec-cancel", false);
    let cast = add_cast(&scene, 7, screen_stream());
    start(
        &scene.app,
        RecordRequest {
            mode: RecordMode::Screen,
        },
    );
    cast.reaches("Start", 1);
    let osd = window_wait::shown().expect("the waiting indicator");
    assert!(press(&osd, gdk::Key::Escape), "Esc is handled");
    cast.reaches("Session.Close", 1);
    wait_for(|| !outcomes(&seen).is_empty(), "the outcome");
    assert_eq!(outcomes(&seen), [StartOutcome::Cancelled]);
    assert_eq!(StartOutcome::Cancelled.message(), None, "no message");
    assert!(window_wait::shown().is_none());
    assert!(files_in(&folder).is_empty());
    assert!(seen.built.get().is_none());
    assert!(!is_active());

    // Cancel, and closing the indicator, cancel just the same.
    for (n, how) in [(2, "cancel"), (3, "close")] {
        start(
            &scene.app,
            RecordRequest {
                mode: RecordMode::Screen,
            },
        );
        cast.reaches("Start", n);
        let osd = window_wait::shown().unwrap();
        if how == "cancel" {
            find::<gtk::Button>(&osd, "cancel_button")
                .unwrap()
                .emit_clicked();
        } else {
            osd.close();
        }
        cast.reaches("Session.Close", n);
        wait_for(|| outcomes(&seen).len() == n, "the outcome");
        assert_eq!(outcomes(&seen)[n - 1], StartOutcome::Cancelled);
        assert!(files_in(&folder).is_empty());
    }

    // The request's future dropped while waiting: the session is closed, the indicator gone.
    let app = scene.app.clone();
    let request = glib::spawn_future_local(async move {
        let _ = source::request(&app, source::SourceKind::Monitor, false).await;
    });
    cast.reaches("Start", 4);
    assert!(window_wait::shown().is_some());
    request.abort();
    drop(request);
    cast.reaches("Session.Close", 4);
    wait_for(|| window_wait::shown().is_none(), "the indicator to go");
    assert!(files_in(&folder).is_empty());

    // Started from the setup: Annota's window, hidden for the setup, stays hidden while the
    // indicator waits and comes back with the cancel.
    gio::Settings::new(config::APP_ID)
        .set_enum("record-source", CaptureMode::Screen.index())
        .unwrap();
    let main = crate::application::present_window(&scene.app);
    pump(200);
    capture::start_recording_setup(&scene.app);
    let window = wait_for_overlay(&scene.app);
    let start_button = setup_view(&window).start;
    wait_for(|| start_button.is_sensitive(), "Start Recording");
    start_button.emit_clicked();
    cast.reaches("Start", 5);
    let osd = window_wait::shown().expect("the waiting indicator");
    assert!(!main.is_visible(), "hidden while waiting");
    assert!(press(&osd, gdk::Key::Escape));
    cast.reaches("Session.Close", 5);
    wait_for(|| outcomes(&seen).len() == 4, "the outcome");
    assert_eq!(outcomes(&seen)[3], StartOutcome::Cancelled);
    assert!(main.is_visible(), "back after the cancel");
    assert!(files_in(&folder).is_empty());
}

/// Scenario "Session closed while waiting": `Session.Closed` before Start answers takes the
/// indicator away at once; the outcome carries the message; Start's later answer is ignored.
// spec: screen-recording / Recording source / Session closed while waiting
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn rec_a_session_closed_while_waiting_ends_the_request_at_once() {
    let (scene, seen, folder) = recording_scene("rec-closed-waiting", false);
    let cast = add_cast(&scene, 7, screen_stream());
    start(
        &scene.app,
        RecordRequest {
            mode: RecordMode::Screen,
        },
    );
    cast.reaches("Start", 1);
    assert!(window_wait::shown().is_some());
    let closed = Instant::now();
    cast.close_session();
    wait_for(|| window_wait::shown().is_none(), "the indicator to close");
    assert!(closed.elapsed() < Duration::from_secs(1));
    wait_for(|| !outcomes(&seen).is_empty(), "the outcome");
    assert_eq!(outcomes(&seen), [StartOutcome::DesktopEnded]);
    assert_eq!(
        StartOutcome::DesktopEnded.message().as_deref(),
        Some(DESKTOP_ENDED)
    );
    assert!(!is_active());
    // The late answer changes nothing.
    cast.answer(0);
    pump(800);
    assert_eq!(cast.count("OpenPipeWireRemote"), 0);
    assert_eq!(cast.count("Session.Close"), 0);
    assert_eq!(outcomes(&seen).len(), 1);
    assert!(seen.built.get().is_none());
    assert!(files_in(&folder).is_empty());
}

/// Scenario "Region not on the chosen screen": a selection outside the granted monitor closes
/// the session before a working file exists, and the outcome carries the message.
// spec: screen-recording / Recording source / Region not on the chosen screen
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn rec_a_region_off_the_chosen_screen_closes_the_session() {
    let (scene, seen, folder) = recording_scene("rec-region-off", false);
    let stream = screen_stream();
    let cast = add_cast(&scene, 7, stream);
    cast.auto.set(Some(0));
    let (w, h) = stream.size;
    start_region(&scene.app, IRect::new(w - 100, h / 2, 400, 300));
    wait_for(|| !outcomes(&seen).is_empty(), "the outcome");
    assert_eq!(outcomes(&seen), [StartOutcome::RegionNotOnScreen]);
    assert_eq!(
        StartOutcome::RegionNotOnScreen.message().as_deref(),
        Some(NOT_ON_SCREEN)
    );
    cast.reaches("Session.Close", 1);
    assert_eq!(cast.option_u32("types"), Some(1), "a monitor source");
    assert!(seen.built.get().is_none(), "no stream opened");
    assert!(files_in(&folder).is_empty(), "nothing written");
    assert!(!is_active());
}

/// Scenarios "Record a window" and "Recorded window closed": "Waiting for a window…" goes when a
/// window is chosen; the recording shows only the window (its shadow and the buffer's undefined
/// pixels around it cut away); closing the window reaches Annota only as `Session.Closed`, and
/// the recording is finished at once, kept, without `Session.Close`.
// spec: screen-recording / Recording source / Record a window
// spec: screen-recording / Stop recording / Recorded window closed
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn rec_a_window_recording_shows_only_the_window_until_it_closes() {
    let (scene, seen, folder) = recording_scene("rec-window", false);
    let monitor = the_monitor();
    let (mw, mh) = (monitor.width(), monitor.height());
    let cast = add_cast(
        &scene,
        7,
        StreamProps {
            source_type: 2,
            position: None,
            size: (mw, mh),
        },
    );
    // A window stream: buffers of the monitor's size (scale 1 here), crop meta per buffer.
    let appsrc = Rc::new(RefCell::new(None::<gst_app::AppSrc>));
    let made = appsrc.clone();
    let built = seen.clone();
    set_hooks(move |hooks| {
        hooks.video_source = Rc::new(move |_, _| {
            built.built.set(Some(Instant::now()));
            let caps =
                gst_video::VideoInfo::builder(gst_video::VideoFormat::Bgra, mw as u32, mh as u32)
                    .fps(gst::Fraction::new(0, 1))
                    .build()
                    .unwrap()
                    .to_caps()
                    .unwrap();
            let src = gst_app::AppSrc::builder()
                .caps(&caps)
                .format(gst::Format::Time)
                .is_live(true)
                .do_timestamp(true)
                .build();
            made.replace(Some(src.clone()));
            Ok(src.upcast())
        });
    });
    start(
        &scene.app,
        RecordRequest {
            mode: RecordMode::Window,
        },
    );
    cast.reaches("Start", 1);
    let osd = window_wait::shown().expect("the waiting indicator");
    assert_eq!(
        indicator_text(&osd),
        (
            String::from("Waiting for a window…"),
            String::from("annota-capture-window-symbolic")
        )
    );
    assert_eq!(
        osd.title().as_deref(),
        Some("Record Window"),
        "its accessible name"
    );
    assert_eq!(cast.option_u32("types"), Some(2), "a window source");
    cast.answer(0);
    wait_for(|| appsrc.borrow().is_some(), "the video source");
    assert!(window_wait::shown().is_none(), "the indicator went");
    assert!(!osd.is_mapped());

    // The window: 400×300 green at (100, 80), a faint shadow 8 px around it, the crop meta
    // around both; red "undefined" opaque pixels elsewhere in the buffer.
    let window = IRect::new(100, 80, 400, 300);
    let shadow = IRect::new(92, 72, 416, 316);
    let mut data = vec![0u8; (mw * mh * 4) as usize];
    for y in 0..mh {
        for x in 0..mw {
            let px: [u8; 4] = if window.contains(x, y) {
                [0, 255, 0, 255]
            } else if shadow.contains(x, y) {
                [0, 0, 0, 30]
            } else {
                [0, 0, 255, 255]
            };
            let i = ((y * mw + x) * 4) as usize;
            data[i..i + 4].copy_from_slice(&px);
        }
    }
    let src = appsrc.borrow().clone().unwrap();
    for _ in 0..12 {
        let mut buffer = gst::Buffer::from_slice(data.clone());
        gst_video::VideoCropMeta::add(
            buffer.get_mut().unwrap(),
            (
                shadow.x as u32,
                shadow.y as u32,
                shadow.w as u32,
                shadow.h as u32,
            ),
        );
        src.push_buffer(buffer).unwrap();
        pump(40);
    }
    pump(300);
    assert!(seen.ended.borrow().is_empty());

    // The window is closed: only `Session.Closed`, no end of stream.
    let closed = Instant::now();
    cast.close_session();
    wait_for(
        || !seen.ended.borrow().is_empty(),
        "the recording to finish",
    );
    let ended = std::mem::take(&mut *seen.ended.borrow_mut());
    let (done, at) = &ended[0];
    assert!(*at - closed < Duration::from_secs(2), "{:?}", *at - closed);
    assert_eq!(done.reason, EndReason::DesktopEnded);
    assert!(control::shown().is_none(), "the control went with it");
    assert!(!done.by_error());
    pump(500);
    assert_eq!(cast.count("Session.Close"), 0, "{:?}", cast.calls());
    assert!(!is_active());
    assert_eq!(files_in(&folder), std::slice::from_ref(&done.finished.path));
    let analysis = analyse(&done.finished.path);
    assert!(analysis.frames.len() >= 5, "{}", analysis.frames.len());
    for (i, frame) in analysis.frames.iter().enumerate() {
        assert_eq!((frame.width, frame.height), (400, 300), "the window only");
        assert!(
            frame
                .grid()
                .iter()
                .all(|c| near(*c, GREEN) && !near(*c, RED)),
            "frame {i} shows more than the window: {:?}",
            frame
                .grid()
                .iter()
                .filter(|c| !near(**c, GREEN))
                .collect::<Vec<_>>()
        );
    }
}

/// Scenario "Session ended by the desktop": GNOME's ■ sends `Session.Closed` while the stream
/// keeps running (no end of stream ever): the recording is finished at once and kept, and no
/// `Session.Close` is sent. A pipeline error then ends a second recording with what was written,
/// the session closed by Annota; the Record shortcut ends a third one like Stop.
// spec: screen-recording / Stop recording / Session ended by the desktop
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn rec_the_desktop_ending_the_session_finishes_the_recording_at_once() {
    let (scene, seen, folder) = recording_scene("rec-desktop-ended", false);
    let cast = add_cast(&scene, 7, screen_stream());
    cast.auto.set(Some(0));
    start(
        &scene.app,
        RecordRequest {
            mode: RecordMode::Screen,
        },
    );
    wait_for(|| seen.first_buffer.lock().unwrap().is_some(), "a frame");
    pump(1000);
    let closed = Instant::now();
    cast.close_session();
    wait_for(
        || !seen.ended.borrow().is_empty(),
        "the recording to finish",
    );
    let ended = std::mem::take(&mut *seen.ended.borrow_mut());
    let (done, at) = &ended[0];
    assert!(*at - closed < Duration::from_secs(2), "{:?}", *at - closed);
    assert_eq!(done.reason, EndReason::DesktopEnded);
    assert!(control::shown().is_none(), "the control went with it");
    assert!(!done.by_error());
    pump(500);
    assert_eq!(cast.count("Session.Close"), 0, "{:?}", cast.calls());
    assert!(!is_active());
    let analysis = analyse(&done.finished.path);
    assert!(
        analysis.duration() > 0.7,
        "what was recorded is kept: {} s",
        analysis.duration()
    );

    // A failing source: the recording ends by error, keeps the file and closes the session.
    set_hooks(|hooks| {
        let screen = hooks.video_source.clone();
        hooks.video_source = Rc::new(move |fd, node| {
            let bin = screen(fd, node)?.downcast::<gst::Bin>().unwrap();
            let pad = bin.static_pad("src").unwrap();
            let failing = bin.downgrade();
            let seen_buffers = Arc::new(Mutex::new(0u32));
            pad.add_probe(gst::PadProbeType::BUFFER, move |_, _| {
                let mut n = seen_buffers.lock().unwrap();
                *n += 1;
                if *n == 20 {
                    if let Some(bin) = failing.upgrade() {
                        gst::element_error!(
                            bin,
                            gst::StreamError::Failed,
                            ("the stand-in stream failed")
                        );
                    }
                }
                gst::PadProbeReturn::Ok
            });
            Ok(bin.upcast())
        });
    });
    start(
        &scene.app,
        RecordRequest {
            mode: RecordMode::Screen,
        },
    );
    wait_for(|| !seen.ended.borrow().is_empty(), "the failed recording");
    let ended = std::mem::take(&mut *seen.ended.borrow_mut());
    let (done, _) = &ended[0];
    assert_eq!(done.reason, EndReason::Error);
    assert!(done.by_error());
    assert!(control::shown().is_none(), "the control went with it");
    cast.reaches("Session.Close", 1);
    assert!(done.finished.path.exists(), "the file is kept");
    assert_eq!(files_in(&folder).len(), 2);
    assert!(!is_active());

    // The Record shortcut (`--record toggle`) while recording stops it like Stop.
    set_hooks(|hooks| hooks.video_source = test_screen(&seen));
    start(
        &scene.app,
        RecordRequest {
            mode: RecordMode::Screen,
        },
    );
    wait_for(|| running().is_some(), "the third recording");
    pump(500);
    start(
        &scene.app,
        RecordRequest {
            mode: RecordMode::Toggle,
        },
    );
    wait_for(
        || !seen.ended.borrow().is_empty(),
        "the third recording to finish",
    );
    assert!(!is_active());
    cast.reaches("Session.Close", 2);
    let ended = std::mem::take(&mut *seen.ended.borrow_mut());
    assert_eq!(ended.len(), 1);
    assert_eq!(ended[0].0.reason, EndReason::Stopped);
    assert_eq!(files_in(&folder).len(), 3);
}

// --- Phase C1: the recording setup (task 4.1) and the countdown (task 4.3) ------------------

const NOT_OFFERED: &str = "Screen sharing for this source is not available on this desktop";

/// Presses `key` the way GTK delivers it: through the `key-pressed` signal of the
/// `EventControllerKey`s on `widget`, in order, until one handles it.
fn press(widget: &impl IsA<gtk::Widget>, key: gdk::Key) -> bool {
    let controllers = widget.as_ref().observe_controllers();
    (0..controllers.n_items()).any(|i| {
        controllers
            .item(i)
            .and_downcast::<gtk::EventControllerKey>()
            .is_some_and(|keys| {
                keys.emit_by_name::<bool>(
                    "key-pressed",
                    &[&key.into_glib(), &0u32, &gdk::ModifierType::empty()],
                )
            })
    })
}

/// The visible region overlay (the one carrying the recording setup), if one is open.
fn overlay_window(app: &adw::Application) -> Option<gtk::Window> {
    app.windows()
        .into_iter()
        .find(|w| w.is_visible() && find::<gtk::Box>(w, "record_setup").is_some())
}

fn wait_for_overlay(app: &adw::Application) -> gtk::Window {
    wait_for(|| overlay_window(app).is_some(), "the region overlay");
    let window = overlay_window(app).unwrap();
    fill_the_monitor(&window);
    pump(200);
    window
}

/// Xvfb has no window manager to honour fullscreen or maximise (GTK leaves such a window at
/// 200 × 200): where `xdotool` is installed the test sizes the X window to the monitor itself,
/// as GNOME would. `true`: the window fills the monitor, so positions can be checked.
fn fill_the_monitor(window: &gtk::Window) -> bool {
    let monitor = the_monitor();
    let fills = || window.width() == monitor.width() && window.height() == monitor.height();
    if fills() {
        return true;
    }
    let Some(id) = x_window(window) else {
        return false;
    };
    let (w, h) = (monitor.width().to_string(), monitor.height().to_string());
    let (x, y) = (monitor.x().to_string(), monitor.y().to_string());
    if !(xdotool(&["windowsize", &id, &w, &h]) && xdotool(&["windowmove", &id, &x, &y])) {
        return false;
    }
    wait_for(fills, "the window to fill the monitor");
    pump(300);
    true
}

/// `window`'s X window id.
fn x_window(window: &gtk::Window) -> Option<String> {
    glib::MainContext::default()
        .block_on(ashpd::WindowIdentifier::from_native(window))
        .map(|id| id.to_string())
        .and_then(|id| id.strip_prefix("x11:").map(String::from))
}

fn xdotool(args: &[&str]) -> bool {
    std::process::Command::new("xdotool")
        .args(args)
        .status()
        .is_ok_and(|status| status.success())
}

/// Types `key` (an X keysym name) into `window` as a real key press through the X server
/// (`xdotool`, XTEST): GTK delivers it as any typed key, to the focused widget and its key
/// bindings first, then the window's controllers. `false`: no `xdotool`, nothing typed.
fn type_key(window: &gtk::Window, key: &str) -> bool {
    let Some(id) = x_window(window) else {
        return false;
    };
    if !xdotool(&["windowfocus", &id]) {
        return false;
    }
    pump(200);
    let typed = xdotool(&["key", key]);
    pump(300);
    typed
}

/// The recording countdown's window, if one is open.
fn countdown_window(app: &adw::Application) -> Option<gtk::Window> {
    app.windows()
        .into_iter()
        .find(|w| w.title().as_deref() == Some("Recording Countdown"))
}

/// The setup's widgets in an overlay window.
struct SetupView {
    setup: gtk::Box,
    arrow: gtk::Box,
    region: gtk::ToggleButton,
    window: gtk::ToggleButton,
    screen: gtk::ToggleButton,
    microphone: adw::ActionRow,
    microphone_meter: gtk::DrawingArea,
    microphone_switch: gtk::Switch,
    system: adw::ActionRow,
    system_switch: gtk::Switch,
    pointer: adw::ActionRow,
    pointer_switch: gtk::Switch,
    countdown: gtk::SpinButton,
    countdown_row: adw::ActionRow,
    start: gtk::Button,
    video: gtk::ToggleButton,
    screenshot: gtk::ToggleButton,
    hint_video: gtk::Box,
    toolbar: gtk::Box,
}

fn setup_view(window: &gtk::Window) -> SetupView {
    let get = |id: &str| find::<gtk::Widget>(window, id).unwrap_or_else(|| panic!("no {id}"));
    let cast = |id: &str| get(id);
    SetupView {
        setup: cast("record_setup").downcast().unwrap(),
        arrow: cast("record_setup_arrow").downcast().unwrap(),
        region: cast("record_region_button").downcast().unwrap(),
        window: cast("record_window_button").downcast().unwrap(),
        screen: cast("record_screen_button").downcast().unwrap(),
        microphone: cast("record_microphone_row").downcast().unwrap(),
        microphone_meter: cast("record_microphone_meter").downcast().unwrap(),
        microphone_switch: cast("record_microphone_switch").downcast().unwrap(),
        system: cast("record_system_row").downcast().unwrap(),
        system_switch: cast("record_system_switch").downcast().unwrap(),
        pointer: cast("record_pointer_row").downcast().unwrap(),
        pointer_switch: cast("record_pointer_switch").downcast().unwrap(),
        countdown: cast("record_countdown_spin").downcast().unwrap(),
        countdown_row: cast("record_countdown_row").downcast().unwrap(),
        start: cast("record_start_button").downcast().unwrap(),
        video: cast("mode_video_button").downcast().unwrap(),
        screenshot: cast("mode_screenshot_button").downcast().unwrap(),
        hint_video: cast("hint_video").downcast().unwrap(),
        toolbar: cast("toolbar").downcast().unwrap(),
    }
}

/// Whether `widget` and all its ancestors are visible (it is on the screen).
fn on_screen(widget: &impl IsA<gtk::Widget>) -> bool {
    widget.as_ref().is_mapped()
}

/// The mockups' dashboard as the frozen desktop, at the monitor's size.
fn dashboard_desktop() -> Desktop {
    let monitor = the_monitor();
    let (w, h) = (monitor.width(), monitor.height());
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs/design/kit/sample-content/png/d-tidewater-analytics.png");
    let picture =
        gtk::cairo::ImageSurface::create_from_png(&mut std::fs::File::open(path).unwrap()).unwrap();
    let mut surface = gtk::cairo::ImageSurface::create(gtk::cairo::Format::ARgb32, w, h).unwrap();
    {
        let cr = gtk::cairo::Context::new(&surface).unwrap();
        cr.scale(
            f64::from(w) / f64::from(picture.width()),
            f64::from(h) / f64::from(picture.height()),
        );
        cr.set_source_surface(&picture, 0.0, 0.0).unwrap();
        cr.paint().unwrap();
    }
    let stride = surface.stride() as usize;
    let data = surface.data().unwrap().to_vec();
    Desktop::new(Frame::from_argb32(w, h, stride, data).unwrap())
}

/// Opens the region overlay directly (no portal), in `mode`; its outcome lands in the cell.
fn open_overlay(app: &adw::Application, mode: Mode) -> Rc<RefCell<Option<Outcome>>> {
    let outcome = Rc::new(RefCell::new(None));
    let (app, slot) = (app.clone(), outcome.clone());
    let desktop = dashboard_desktop();
    glib::spawn_future_local(async move {
        let result = overlay::select_region(&app, &desktop, false, mode).await;
        slot.replace(Some(result));
    });
    outcome
}

fn set_scheme(dark: bool) {
    adw::StyleManager::default().set_color_scheme(if dark {
        adw::ColorScheme::ForceDark
    } else {
        adw::ColorScheme::ForceLight
    });
    pump(200);
}

/// Scenario "Switch the overlay to video": the region overlay opens as for a screenshot; Video in
/// the mode pill shows the setup beside the selection (source, both audio rows with their device
/// names and meters, Include pointer bound to the capture setting, Countdown, Start Recording
/// with the initial focus) and the Video hints instead of the capture toolbar; Screenshot brings
/// the toolbar back. Along the way: the redline's heights (source group 34 px, rows 50 px, setup
/// 334 px), where the setup goes for other selections (above, or without an arrow) and that it
/// leaves the mode pill free, the pill at the top for a selection near the bottom, the arrows
/// nudging only from the selection area, and Tab order. `Enter` typed in the countdown spin then
/// starts a region recording of the selection, which ends the overlay with the selection in
/// desktop px; `Enter` on a source toggle starts one too; the setup's Include pointer shows in
/// the capture toolbar and the screenshot.
// spec: screen-recording / Recording setup / Switch the overlay to video
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn rec_video_in_the_mode_pill_shows_the_setup() {
    let (scene, _seen, _folder) = recording_scene("rec-mode-pill", false);
    let _cast = add_cast(&scene, 7, screen_stream());
    let settings = gio::Settings::new(config::APP_ID);
    settings.set_boolean("record-microphone", true).unwrap();
    settings.set_boolean("include-pointer", true).unwrap();
    let outcome = open_overlay(&scene.app, Mode::Screenshot);
    let window = wait_for_overlay(&scene.app);
    let view = setup_view(&window);
    assert!(view.screenshot.is_active(), "opens for a screenshot");
    assert!(!on_screen(&view.setup));
    assert!(overlay::test_select(0, IRect::new(130, 92, 700, 470)));
    pump(200);
    assert!(on_screen(&view.toolbar), "the capture toolbar");

    view.video.set_active(true);
    pump(400);
    assert!(on_screen(&view.setup), "the setup is shown");
    assert!(!on_screen(&view.toolbar), "instead of the capture toolbar");
    assert!(on_screen(&view.hint_video));
    for widget in [
        view.region.upcast_ref::<gtk::Widget>(),
        view.window.upcast_ref(),
        view.screen.upcast_ref(),
        view.microphone.upcast_ref(),
        view.system.upcast_ref(),
        view.pointer.upcast_ref(),
        view.countdown.upcast_ref(),
        view.start.upcast_ref(),
    ] {
        assert!(
            on_screen(widget),
            "{:?} on the screen",
            widget.buildable_id()
        );
    }
    wait_for(
        || view.microphone.subtitle().as_deref() == Some("Built-in Audio"),
        "the device names",
    );
    assert_eq!(view.system.subtitle().as_deref(), Some("Built-in Speakers"));
    assert_eq!(
        view.pointer.subtitle().as_deref(),
        Some("Same setting as for captures")
    );
    assert!(view.pointer_switch.is_active(), "the capture's setting");
    assert!(view.microphone_switch.is_active());
    assert!(on_screen(&view.microphone_meter));
    assert_eq!(view.countdown.text(), "3 s");
    assert!(
        view.start.has_focus(),
        "Start Recording has the initial focus"
    );
    // Beside the selection: 18 px right of it, top-aligned, the arrow on its left edge.
    let bounds = view.setup.compute_bounds(&window).unwrap();
    assert_eq!(bounds.x().round() as i32, 130 + 700 + 18);
    assert_eq!(bounds.y().round() as i32, 92);
    assert_eq!(bounds.width().round() as i32, 380);
    let arrow = view.arrow.compute_bounds(&window).unwrap();
    assert_eq!(
        (arrow.x() + arrow.width() / 2.0).round() as i32,
        130 + 700 + 18
    );
    // Redline 01: the source toggle group 34 px, the rows 50 px, the setup 334 px.
    let height = |widget: &gtk::Widget| widget.compute_bounds(&window).unwrap().height();
    let sources: gtk::Box = find(&window, "record_source_group").unwrap();
    let measured = [
        height(sources.upcast_ref()),
        height(view.microphone.upcast_ref()),
        height(view.system.upcast_ref()),
        height(view.pointer.upcast_ref()),
        height(view.countdown_row.upcast_ref()),
        bounds.height(),
    ];
    eprintln!("measured heights (sources, 4 rows, setup): {measured:?}");
    assert_eq!(measured[0].round() as i32, 34, "the source toggle group");
    for row in &measured[1..5] {
        assert_eq!(row.round() as i32, 50, "a row");
    }
    assert!(
        (measured[5] - 334.0).abs() <= 2.0,
        "the setup: {}",
        measured[5]
    );
    // Groups for assistive technology: "Source" and the mode pill's "Mode".
    assert_eq!(sources.accessible_role(), gtk::AccessibleRole::Group);
    let modes = view.screenshot.parent().unwrap();
    assert_eq!(modes.accessible_role(), gtk::AccessibleRole::Group);
    // The countdown row is no focus stop: Tab goes from Include pointer to the spin.
    assert!(!view.countdown_row.is_focusable());
    view.pointer_switch.grab_focus();
    assert!(window.child_focus(gtk::DirectionType::TabForward));
    let focus = gtk::prelude::GtkWindowExt::focus(&window).unwrap();
    assert!(
        focus.is_ancestor(&view.countdown),
        "Tab reaches the spin: {focus:?}"
    );

    // Arrows in the setup move the focus, never the selection; on the selection area they nudge.
    let selected = IRect::new(130, 92, 700, 470);
    view.region.grab_focus();
    assert!(!press(&window, gdk::Key::Left), "not the overlay's key");
    assert_eq!(overlay::test_selection(0), Some(selected));
    let canvas: gtk::DrawingArea = find(&window, "canvas").unwrap();
    canvas.grab_focus();
    assert!(press(&window, gdk::Key::Left));
    assert_eq!(overlay::test_selection(0).map(|r| r.x), Some(129));
    assert!(press(&window, gdk::Key::Right));
    assert_eq!(overlay::test_selection(0), Some(selected));
    view.start.grab_focus();

    // No room beside the selection and none below it: above, the arrow on the setup's bottom
    // edge facing it (never an arrow left at the monitor's (0, 0)).
    the_setup_is_placed_clear_of_the_pill();
    let monitor = the_monitor();
    assert!(overlay::test_select(0, IRect::new(100, 450, 1000, 300)));
    pump(200);
    let bounds = view.setup.compute_bounds(&window).unwrap();
    assert_eq!(
        (bounds.y() + bounds.height()).round() as i32,
        450 - 18,
        "above the selection"
    );
    assert!(on_screen(&view.arrow));
    let arrow = view.arrow.compute_bounds(&window).unwrap();
    assert_eq!(
        (arrow.y() + arrow.height() / 2.0).round() as i32,
        450 - 18,
        "on the setup's bottom edge"
    );
    // The whole monitor: no room anywhere, no arrow at all.
    assert!(overlay::test_select(
        0,
        IRect::new(0, 0, monitor.width(), monitor.height())
    ));
    pump(200);
    assert!(on_screen(&view.setup));
    assert!(!on_screen(&view.arrow), "no arrow without room");
    // A selection whose bottom handles the pill would cover: the pill goes to the top.
    let pill: gtk::Box = find(&window, "mode_pill").unwrap();
    let bottom = monitor.height() - 40;
    assert!(overlay::test_select(
        0,
        IRect::new(300, bottom - 260, 700, 260)
    ));
    pump(200);
    assert_eq!(pill.compute_bounds(&window).unwrap().y().round() as i32, 24);
    let setup = view.setup.compute_bounds(&window).unwrap();
    let at = pill.compute_bounds(&window).unwrap();
    assert!(
        setup.y() >= at.y() + at.height() || setup.y() + setup.height() <= at.y(),
        "the setup leaves the pill free"
    );
    assert!(overlay::test_select(0, selected));
    pump(200);
    let at = pill.compute_bounds(&window).unwrap();
    assert_eq!(
        (at.y() + at.height()).round() as i32,
        monitor.height() - 24,
        "back at the bottom"
    );

    // The pointer row is the capture setting itself.
    view.pointer_switch.set_active(false);
    assert!(!settings.boolean("include-pointer"));
    view.pointer_switch.set_active(true);
    set_scheme(false);
    shot(&window, "01-recording-setup-light");
    set_scheme(true);
    shot(&window, "01-recording-setup-dark");
    set_scheme(false);

    // Back to Screenshot and to Video again.
    view.screenshot.set_active(true);
    pump(200);
    assert!(!on_screen(&view.setup));
    assert!(on_screen(&view.toolbar));
    view.video.set_active(true);
    pump(200);
    assert!(on_screen(&view.setup));

    // Enter in the countdown spin, as typed: the spin takes "7" and Start Recording starts a
    // region recording of the selection.
    view.countdown.grab_focus();
    view.countdown.set_text("7");
    let ok = type_key(&window, "Return");
    if !ok {
        eprintln!("no xdotool: Enter emitted on the window's controllers instead");
        view.countdown.update();
        assert!(press(&window, gdk::Key::Return));
    }
    wait_for(|| outcome.borrow().is_some(), "the overlay's outcome");
    assert_eq!(settings.int("record-countdown"), 7, "the typed countdown");
    match outcome.borrow_mut().take().unwrap() {
        Outcome::Record { source, selection } => {
            assert_eq!(source, CaptureMode::Region);
            assert_eq!(
                selection,
                Some(IRect::new(monitor.x() + 130, monitor.y() + 92, 700, 470))
            );
        }
        other => panic!("not a recording: {other:?}"),
    }
    pump(200);
    assert!(overlay_window(&scene.app).is_none(), "the overlay closed");

    // Enter on a source toggle starts the recording too (Space stays the toggle's own key).
    let outcome = open_overlay(&scene.app, Mode::Video);
    let window = wait_for_overlay(&scene.app);
    let view = setup_view(&window);
    assert!(overlay::test_select(0, selected));
    pump(200);
    view.window.grab_focus();
    if !type_key(&window, "Return") {
        assert!(press(&window, gdk::Key::Return));
    }
    wait_for(|| outcome.borrow().is_some(), "the overlay's outcome");
    assert!(matches!(
        outcome.borrow_mut().take().unwrap(),
        Outcome::Record {
            source: CaptureMode::Region,
            ..
        }
    ));
    assert_eq!(settings.enum_("record-source"), CaptureMode::Region.index());
    pump(200);

    // Include pointer switched off in Video shows in the capture toolbar, and the screenshot
    // takes it.
    let outcome = open_overlay(&scene.app, Mode::Video);
    let window = wait_for_overlay(&scene.app);
    let view = setup_view(&window);
    assert!(overlay::test_select(0, selected));
    pump(200);
    let toolbar_pointer: gtk::ToggleButton = find(&window, "pointer_button").unwrap();
    view.pointer_switch.set_active(false);
    assert!(!toolbar_pointer.is_active());
    view.pointer_switch.set_active(true);
    assert!(
        toolbar_pointer.is_active(),
        "the toolbar follows the setting"
    );
    view.pointer_switch.set_active(false);
    view.screenshot.set_active(true);
    pump(200);
    assert!(
        !toolbar_pointer.is_active(),
        "the toolbar follows the setting"
    );
    assert!(press(&window, gdk::Key::Return));
    wait_for(|| outcome.borrow().is_some(), "the overlay's outcome");
    match outcome.borrow_mut().take().unwrap() {
        Outcome::Region {
            include_pointer, ..
        } => assert!(!include_pointer),
        other => panic!("not a screenshot: {other:?}"),
    }
    pump(200);
}

/// Placement of the setup (redline 01) on a 1280 × 800 monitor with the mode pill at the
/// bottom: right of the selection, else left, else below, else above, else at the monitor's
/// right edge without an arrow; never on the pill. The pill goes to the top edge while it would
/// cover a bottom handle of the selection.
fn the_setup_is_placed_clear_of_the_pill() {
    use crate::capture::geometry::FRect;
    use overlay::{pill_at_top, pill_rect, setup_placement};
    let area = (1280.0, 800.0);
    let size = (380.0, 334.0);
    let pill_size = (430.0, 42.0);
    let pill = pill_rect(false, pill_size, area);
    assert_eq!((pill.x, pill.y), (425.0, 734.0));
    let sel = |x, y, w, h| Some(FRect { x, y, w, h });
    let place = |selection| setup_placement(selection, size, area, Some(pill));
    let clear_of_the_pill = |p: (f64, f64)| {
        p.1 + size.1 + 8.0 <= pill.y
            || p.1 >= pill.y + pill.h + 8.0
            || p.0 + size.0 + 8.0 <= pill.x
            || p.0 >= pill.x + pill.w + 8.0
    };
    // Mockup 01: 18 px right of the selection, top-aligned, the arrow on the setup's left
    // edge near its top and within the selection's height.
    let placed = place(sel(130.0, 92.0, 700.0, 470.0));
    assert_eq!(placed.position, (848.0, 92.0));
    assert_eq!(placed.arrow, Some((848.0, 132.0)));
    // No room on the right: on the left, the arrow on its right edge.
    let placed = place(sel(700.0, 100.0, 500.0, 300.0));
    assert_eq!(placed.position, (302.0, 100.0));
    assert_eq!(placed.arrow.map(|a| a.0), Some(682.0));
    // Neither side: below, the arrow on its top edge.
    let placed = place(sel(100.0, 40.0, 1080.0, 300.0));
    assert_eq!(placed.position, (450.0, 358.0));
    assert_eq!(placed.arrow, Some((640.0, 358.0)));
    // Neither side nor below (the review's repro): above, the arrow on its bottom edge.
    let placed = place(sel(100.0, 450.0, 1000.0, 300.0));
    assert_eq!(placed.position, (410.0, 98.0));
    assert_eq!(placed.arrow, Some((600.0, 432.0)));
    // Nowhere (the whole monitor): inside the monitor, no arrow, the pill free.
    let placed = place(sel(0.0, 0.0, 1280.0, 800.0));
    assert!(placed.arrow.is_none());
    assert_eq!(placed.position, (892.0, 8.0));
    assert!(clear_of_the_pill(placed.position));
    // Right of the selection but down on the pill: moved up to clear it, the arrow still
    // within the selection's height.
    let placed = place(sel(100.0, 450.0, 400.0, 300.0));
    assert_eq!(placed.position, (518.0, 392.0));
    assert_eq!(placed.arrow, Some((518.0, 462.0)));
    assert!(clear_of_the_pill(placed.position));
    // Near the bottom with no room beside it that clears the pill: above.
    let placed = place(sel(100.0, 700.0, 200.0, 90.0));
    assert_eq!(placed.position, (10.0, 348.0));
    assert_eq!(placed.arrow, Some((200.0, 682.0)));
    // Without a selection: centred.
    let placed = place(None);
    assert_eq!(placed.position, (450.0, 233.0));
    assert!(placed.arrow.is_none());
    // Without a pill the old rules hold.
    let placed = setup_placement(sel(100.0, 450.0, 400.0, 300.0), size, area, None);
    assert_eq!(placed.position, (518.0, 450.0));

    // The pill: at the top while it would cover a bottom handle, unless a top handle is then
    // covered too.
    assert!(pill_at_top(
        sel(300.0, 600.0, 700.0, 150.0),
        pill_size,
        area
    ));
    assert!(!pill_at_top(
        sel(300.0, 40.0, 700.0, 720.0),
        pill_size,
        area
    ));
    assert!(!pill_at_top(
        sel(130.0, 92.0, 700.0, 470.0),
        pill_size,
        area
    ));
    assert!(!pill_at_top(sel(0.0, 0.0, 1280.0, 800.0), pill_size, area));
    assert!(!pill_at_top(None, pill_size, area));
    assert_eq!(pill_rect(true, pill_size, area).y, 24.0);
}

/// Scenarios "Shortcut opens the setup" and "Cancel the setup": `--record toggle` with nothing
/// running (the action the GNOME-keybinding fallback runs, and the Record shortcut's entry)
/// opens the region overlay in Video mode with the setup; `Esc` closes it with no
/// screen-sharing session requested and nothing written. `--record region` opens the same.
// spec: screen-recording / Record shortcut / Shortcut opens the setup
// spec: screen-recording / Recording setup / Cancel the setup
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn rec_the_record_shortcut_opens_the_setup_and_esc_cancels_it() {
    let (scene, seen, folder) = recording_scene("rec-shortcut-setup", false);
    let cast = add_cast(&scene, 7, screen_stream());
    crate::application::setup_actions(&scene.app);
    let main = crate::application::present_window(&scene.app);
    pump(200);
    scene
        .app
        .activate_action("record", Some(&RecordMode::Toggle.as_str().to_variant()));
    let window = wait_for_overlay(&scene.app);
    let view = setup_view(&window);
    assert!(view.video.is_active(), "opens in Video mode");
    assert!(on_screen(&view.setup), "with the setup");
    assert!(on_screen(&view.hint_video));
    assert!(!is_active(), "nothing requested yet");

    assert!(press(&window, gdk::Key::Escape));
    wait_for(|| !capture::busy(), "the overlay to close");
    assert!(overlay_window(&scene.app).is_none());
    pump(500);
    assert!(cast.calls().is_empty(), "no session: {:?}", cast.calls());
    assert!(files_in(&folder).is_empty(), "nothing written");
    assert!(outcomes(&seen).is_empty());
    assert!(!is_active());
    assert!(main.is_visible(), "Annota's window is back");

    // `--record region` opens the same setup.
    scene
        .app
        .activate_action("record", Some(&RecordMode::Region.as_str().to_variant()));
    let window = wait_for_overlay(&scene.app);
    assert!(setup_view(&window).video.is_active());
    assert!(press(&window, gdk::Key::Escape));
    wait_for(|| !capture::busy(), "the overlay to close");
    assert!(overlay_window(&scene.app).is_none());
    pump(300);
    assert!(cast.calls().is_empty());
}

/// Scenario "Choices are remembered": System audio on, countdown 5 s and source Screen, then
/// `Esc`; the setup opens again with exactly those. Start Recording with Screen then closes the
/// overlay and requests a monitor source with the waiting indicator (the selection plays no
/// part), and the countdown gets the saved 5 s. Annota's main window is hidden for the setup,
/// back after `Esc`, and stays hidden from Start Recording until the recording has ended.
// spec: screen-recording / Recording setup / Choices are remembered
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn rec_setup_choices_are_remembered() {
    let (scene, seen, _folder) = recording_scene("rec-remembered", false);
    let cast = add_cast(&scene, 7, screen_stream());
    let settings = gio::Settings::new(config::APP_ID);
    // Annota's main window, hidden for the frozen desktop like for a capture.
    let main = crate::application::present_window(&scene.app);
    pump(200);
    capture::start_recording_setup(&scene.app);
    let window = wait_for_overlay(&scene.app);
    assert!(!main.is_visible(), "hidden for the setup");
    let view = setup_view(&window);
    wait_for(
        || view.system.subtitle().is_some_and(|s| !s.is_empty()),
        "the probe",
    );
    assert!(!view.system_switch.is_active(), "off by default");
    assert_eq!(view.countdown.value_as_int(), 3);
    assert!(view.region.is_active());
    view.system_switch.set_active(true);
    view.countdown.set_value(5.0);
    view.screen.set_active(true);
    pump(100);
    assert!(press(&window, gdk::Key::Escape));
    wait_for(|| !capture::busy(), "the overlay to close");
    assert!(overlay_window(&scene.app).is_none());
    assert!(cast.calls().is_empty());
    assert!(settings.boolean("record-system-audio"));
    assert_eq!(settings.int("record-countdown"), 5);
    assert!(main.is_visible(), "back after the cancelled setup");

    capture::start_recording_setup(&scene.app);
    let window = wait_for_overlay(&scene.app);
    let view = setup_view(&window);
    assert!(view.system_switch.is_active(), "System audio is on");
    assert_eq!(view.countdown.text(), "5 s", "the countdown reads 5 s");
    assert!(view.screen.is_active(), "the source is Screen");
    // Screen needs no selection: Start Recording closes the overlay and requests a monitor.
    assert!(view.start.is_sensitive());
    view.start.emit_clicked();
    cast.reaches("Start", 1);
    assert!(overlay_window(&scene.app).is_none());
    assert_eq!(cast.option_u32("types"), Some(1), "a monitor source");
    let osd = window_wait::shown().expect("the waiting indicator");
    assert_eq!(indicator_text(&osd).0, "Waiting for a screen…");
    assert_eq!(osd.title().as_deref(), Some("Record Screen"));
    // Annota stays off the screen from Start Recording on: waiting, counting down, recording
    // (design.md, "Decided during apply", Phase C1 review).
    assert!(!main.is_visible(), "hidden while waiting for the source");
    cast.answer(0);
    wait_for(|| !seen.countdown.borrow().is_empty(), "the countdown");
    assert!(!main.is_visible(), "hidden during the countdown");
    let countdown = seen.countdown.borrow().clone();
    assert_eq!((countdown[0].0, countdown[0].1), (Target::Screen, 5));
    wait_for(|| running().is_some(), "the recording");
    pump(300);
    assert!(!main.is_visible(), "hidden while recording");
    assert!(stop());
    wait_for(
        || !seen.ended.borrow().is_empty(),
        "the recording to finish",
    );
    assert!(main.is_visible(), "back once the recording has ended");
}

/// Scenarios "Source not offered" and "No microphone": a portal that offers monitors only and a
/// desktop without an audio input. The Window segment is insensitive with the redline's
/// tooltip, and a saved Window source falls back to Region (the saved choice untouched); the
/// Microphone row is insensitive, reads "No microphone found", has no meter and its switch is
/// off and insensitive, without turning the saved choice off.
// spec: screen-recording / Recording source / Source not offered
// spec: screen-recording / Audio / No microphone
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn rec_setup_without_a_window_source_and_without_a_microphone() {
    let (scene, _seen, _folder) = recording_scene("rec-unavailable", true);
    let _cast = add_cast(&scene, 1, screen_stream());
    set_hooks(|hooks| {
        hooks.devices =
            Rc::new(|| Box::pin(async { SystemDevices::new(None, Some("Built-in Speakers")) }));
    });
    let settings = gio::Settings::new(config::APP_ID);
    settings
        .set_enum("record-source", CaptureMode::Window.index())
        .unwrap();
    settings.set_boolean("record-microphone", true).unwrap();
    settings.set_boolean("record-system-audio", true).unwrap();
    let _outcome = open_overlay(&scene.app, Mode::Video);
    let window = wait_for_overlay(&scene.app);
    assert!(overlay::test_select(0, IRect::new(130, 92, 700, 470)));
    let view = setup_view(&window);
    wait_for(|| !view.window.is_sensitive(), "the portal's answer");
    assert_eq!(view.window.tooltip_text().as_deref(), Some(NOT_OFFERED));
    assert!(
        view.region.is_active(),
        "the saved Window falls back to Region"
    );
    assert!(view.screen.is_sensitive() && view.region.is_sensitive());
    assert_eq!(
        settings.enum_("record-source"),
        CaptureMode::Window.index(),
        "the saved choice is kept"
    );
    wait_for(
        || view.microphone.subtitle().as_deref() == Some("No microphone found"),
        "the probe",
    );
    assert!(!view.microphone.is_sensitive(), "the row is insensitive");
    assert!(!view.microphone_switch.is_active() && !view.microphone_switch.is_sensitive());
    assert!(!view.microphone_meter.is_visible(), "no meter");
    assert!(
        settings.boolean("record-microphone"),
        "the saved choice is kept"
    );
    assert!(view.system.is_sensitive() && view.system_switch.is_active());
    assert!(view.start.is_sensitive());
    pump(600);
    set_scheme(false);
    shot(&window, "02-setup-unavailable-light");
    set_scheme(true);
    shot(&window, "02-setup-unavailable-dark");
    set_scheme(false);
    assert!(press(&window, gdk::Key::Escape));
    wait_for(|| !capture::busy(), "the overlay to close");
    assert!(overlay_window(&scene.app).is_none());

    // Without any output either: "No audio output found".
    let devices = SystemDevices::new(None, None);
    assert_eq!(
        setup::device_subtitle(AudioSource::SystemAudio, &devices),
        "No audio output found"
    );
}

/// The countdown's OSD window and when it was mapped and unmapped.
#[derive(Default)]
struct OsdTimes {
    window: RefCell<Option<gtk::Window>>,
    mapped: Cell<Option<Instant>>,
    unmapped: Rc<Cell<Option<Instant>>>,
}

/// Watches for the recording countdown's window and notes when it is mapped and unmapped.
fn watch_countdown(app: &adw::Application) -> Rc<OsdTimes> {
    let times = Rc::new(OsdTimes::default());
    let seen = times.clone();
    app.connect_window_added(move |_, window| {
        if window.title().as_deref() != Some("Recording Countdown") {
            return;
        }
        let unmapped = seen.unmapped.clone();
        window.connect_unmap(move |_| unmapped.set(Some(Instant::now())));
        let at = seen.clone();
        window.connect_map(move |_| at.mapped.set(Some(Instant::now())));
        seen.window.replace(Some(window.clone()));
    });
    times
}

/// The real countdown OSD in the flow (the scene's stand-in skips it).
fn real_countdown(seconds: i32) {
    gio::Settings::new(config::APP_ID)
        .set_int("record-countdown", seconds)
        .unwrap();
    set_hooks(|hooks| hooks.countdown = Rc::new(countdown_osd));
}

/// Scenario "Countdown is not recorded": a region recording with a 3 s countdown shows the OSD
/// ("Recording starts in 3 s", red ring, the region's dashed outline) for 3 s; the video source
/// is built and its first frame taken only after the OSD has been unmapped plus `SETTLE`.
// spec: screen-recording / Recording countdown / Countdown is not recorded
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn rec_the_countdown_is_not_recorded() {
    let (scene, seen, folder) = recording_scene("rec-countdown", false);
    let cast = add_cast(&scene, 7, screen_stream());
    cast.auto.set(Some(0));
    real_countdown(3);
    let osd = watch_countdown(&scene.app);
    start_region(&scene.app, IRect::new(130, 92, 700, 470));
    wait_for(|| osd.mapped.get().is_some(), "the countdown OSD");
    let window = osd.window.borrow().clone().unwrap();
    fill_the_monitor(&window);
    let message: gtk::Label = find(&window, "message").unwrap();
    assert_eq!(message.text(), "Recording starts in 3 s");
    let hints: gtk::Box = find(&window, "recording_hints").unwrap();
    assert!(hints.is_visible());
    assert!(!find::<gtk::Box>(&window, "capture_hints")
        .unwrap()
        .is_visible());
    assert!(seen.built.get().is_none(), "no stream during the countdown");
    set_scheme(false);
    shot(&window, "04-countdown-light");
    set_scheme(true);
    shot(&window, "04-countdown-dark");
    set_scheme(false);
    wait_for(|| seen.first_buffer.lock().unwrap().is_some(), "a frame");
    let (mapped, gone) = (osd.mapped.get().unwrap(), osd.unmapped.get().unwrap());
    let shown = gone - mapped;
    assert!(
        shown >= Duration::from_millis(2900) && shown < Duration::from_millis(3600),
        "the OSD was shown for {shown:?}"
    );
    let built = seen.built.get().unwrap();
    let first = seen.first_buffer.lock().unwrap().unwrap();
    assert!(
        built >= gone + crate::capture::countdown::SETTLE,
        "built {:?} after",
        built - gone
    );
    assert!(first >= gone + crate::capture::countdown::SETTLE);
    assert!(countdown_window(&scene.app).is_none());
    assert_eq!(outcomes(&seen), [StartOutcome::Recording]);
    assert!(stop());
    wait_for(
        || !seen.ended.borrow().is_empty(),
        "the recording to finish",
    );
    assert_eq!(files_in(&folder).len(), 1);
}

/// Scenarios "Cancel the countdown" and "Shortcut cancels the countdown": `Esc` in the OSD, and
/// the Record shortcut's `--record toggle` during the countdown, each end it: the session is
/// closed, nothing is written, no message. `Enter` starts at once instead. The desktop closing
/// the session during the countdown ends it too, with nothing sent back and nothing written.
// spec: screen-recording / Recording countdown / Cancel the countdown
// spec: screen-recording / Record shortcut / Shortcut cancels the countdown
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn rec_the_countdown_is_cancelled_by_esc_and_by_the_shortcut() {
    let (scene, seen, folder) = recording_scene("rec-countdown-cancel", false);
    let cast = add_cast(&scene, 7, screen_stream());
    cast.auto.set(Some(0));
    crate::application::setup_actions(&scene.app);
    real_countdown(5);
    let screen = || {
        start(
            &scene.app,
            RecordRequest {
                mode: RecordMode::Screen,
            },
        )
    };

    // Esc.
    screen();
    wait_for(|| countdown_window(&scene.app).is_some(), "the countdown");
    pump(300);
    assert!(press(
        &countdown_window(&scene.app).unwrap(),
        gdk::Key::Escape
    ));
    wait_for(|| !outcomes(&seen).is_empty(), "the outcome");
    assert_eq!(outcomes(&seen), [StartOutcome::Cancelled]);
    assert_eq!(StartOutcome::Cancelled.message(), None, "no message");
    cast.reaches("Session.Close", 1);
    assert!(countdown_window(&scene.app).is_none());
    assert!(seen.built.get().is_none() && files_in(&folder).is_empty());
    assert!(!is_active());

    // The Record shortcut (`--record toggle`).
    screen();
    wait_for(|| countdown_window(&scene.app).is_some(), "the countdown");
    pump(300);
    scene
        .app
        .activate_action("record", Some(&RecordMode::Toggle.as_str().to_variant()));
    wait_for(|| outcomes(&seen).len() == 2, "the outcome");
    assert_eq!(outcomes(&seen)[1], StartOutcome::Cancelled);
    cast.reaches("Session.Close", 2);
    pump(300);
    assert!(countdown_window(&scene.app).is_none(), "the OSD is gone");
    assert!(overlay_window(&scene.app).is_none(), "no setup opened");
    assert!(seen.built.get().is_none() && files_in(&folder).is_empty());
    assert!(!is_active());

    // The desktop ends the session during the countdown.
    screen();
    wait_for(|| countdown_window(&scene.app).is_some(), "the countdown");
    cast.close_session();
    wait_for(|| outcomes(&seen).len() == 3, "the outcome");
    assert_eq!(outcomes(&seen)[2], StartOutcome::DesktopEnded);
    pump(300);
    assert!(countdown_window(&scene.app).is_none());
    assert_eq!(cast.count("Session.Close"), 2, "nothing sent back");
    assert!(seen.built.get().is_none() && files_in(&folder).is_empty());
    assert!(!is_active());

    // Enter starts at once.
    screen();
    wait_for(|| countdown_window(&scene.app).is_some(), "the countdown");
    let pressed = Instant::now();
    assert!(press(
        &countdown_window(&scene.app).unwrap(),
        gdk::Key::Return
    ));
    wait_for(|| running().is_some(), "the recording");
    assert!(pressed.elapsed() < Duration::from_secs(2));
    assert!(stop());
    wait_for(
        || !seen.ended.borrow().is_empty(),
        "the recording to finish",
    );
    assert_eq!(files_in(&folder).len(), 1);
}

/// Scenario "No countdown": with 0 s no OSD appears and the recording starts once the source is
/// granted.
// spec: screen-recording / Recording countdown / No countdown
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn rec_no_countdown_starts_at_once() {
    let (scene, seen, _folder) = recording_scene("rec-no-countdown", false);
    let cast = add_cast(&scene, 7, screen_stream());
    real_countdown(0);
    let osd = watch_countdown(&scene.app);
    start(
        &scene.app,
        RecordRequest {
            mode: RecordMode::Screen,
        },
    );
    cast.reaches("Start", 1);
    let granted = Instant::now();
    cast.answer(0);
    wait_for(|| seen.first_buffer.lock().unwrap().is_some(), "a frame");
    let built = seen.built.get().unwrap();
    assert!(
        built - granted < Duration::from_millis(1500),
        "started {:?} after the grant",
        built - granted
    );
    assert!(osd.window.borrow().is_none(), "no OSD");
    assert!(stop());
    wait_for(
        || !seen.ended.borrow().is_empty(),
        "the recording to finish",
    );
}

/// Scenario "Record a screen from the command line": `--record screen` (its action) with
/// nothing running requests a monitor source with "Waiting for a screen…", and once it is
/// granted the countdown follows, after the indicator has gone.
// spec: screen-recording / Recording on the command line / Record a screen from the command line
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn rec_record_screen_from_the_command_line() {
    let (scene, seen, _folder) = recording_scene("rec-cli-screen", false);
    let cast = add_cast(&scene, 7, screen_stream());
    crate::application::setup_actions(&scene.app);
    real_countdown(1);
    let osd = watch_countdown(&scene.app);
    scene
        .app
        .activate_action("record", Some(&RecordMode::Screen.as_str().to_variant()));
    cast.reaches("Start", 1);
    assert_eq!(cast.option_u32("types"), Some(1), "a monitor source");
    let indicator = window_wait::shown().expect("the waiting indicator");
    assert_eq!(indicator_text(&indicator).0, "Waiting for a screen…");
    let unmapped = Rc::new(Cell::new(None));
    indicator.connect_unmap({
        let unmapped = unmapped.clone();
        move |_| unmapped.set(Some(Instant::now()))
    });
    pump(300);
    assert!(
        osd.window.borrow().is_none(),
        "no countdown before the grant"
    );
    cast.answer(0);
    wait_for(|| osd.mapped.get().is_some(), "the countdown");
    assert!(osd.mapped.get().unwrap() > unmapped.get().unwrap());
    wait_for(|| running().is_some(), "the recording");
    assert!(stop());
    wait_for(
        || !seen.ended.borrow().is_empty(),
        "the recording to finish",
    );
}

// --- Phase C2: the Recording control (task 4.4) ---------------------------------------------

/// [`recording_scene`] with `audiotestsrc` stand-ins for the wanted audio sources: the
/// microphone louder (0.5) than the system audio (0.1), so their tracks can be told apart.
fn audio_scene(name: &str, microphone: bool, system_audio: bool) -> (Scene, Rc<Seen>, PathBuf) {
    let (scene, seen, folder) = recording_scene(name, false);
    want_audio(microphone, system_audio);
    set_hooks(|hooks| {
        hooks.audio_sources = Rc::new(|microphone, system_audio| {
            let tone = |volume: f64| {
                gst::ElementFactory::make("audiotestsrc")
                    .property("is-live", true)
                    .property("volume", volume)
                    .build()
                    .unwrap()
            };
            let mut sources = Vec::new();
            if microphone {
                sources.push((AudioSource::Microphone, tone(0.5)));
            }
            if system_audio {
                sources.push((AudioSource::SystemAudio, tone(0.1)));
            }
            sources
        });
    });
    (scene, seen, folder)
}

fn want_audio(microphone: bool, system_audio: bool) {
    let settings = gio::Settings::new(config::APP_ID);
    settings
        .set_boolean("record-microphone", microphone)
        .unwrap();
    settings
        .set_boolean("record-system-audio", system_audio)
        .unwrap();
}

/// The control's widgets.
struct ControlView {
    window: gtk::Window,
    rec_dot: gtk::Widget,
    paused_icon: gtk::Image,
    time: gtk::Label,
    paused: gtk::Label,
    pause: gtk::Button,
    stop: gtk::Button,
    audio_separator: gtk::Widget,
    microphone: gtk::Button,
    microphone_icon: gtk::Image,
    system: gtk::Button,
    system_icon: gtk::Image,
    minimize: gtk::Button,
}

fn wait_for_control() -> ControlView {
    wait_for(|| control::shown().is_some(), "the Recording control");
    let window = control::shown().unwrap();
    wait_for(|| window.is_mapped(), "the control on the screen");
    pump(200);
    let get = |id: &str| find::<gtk::Widget>(&window, id).unwrap_or_else(|| panic!("no {id}"));
    ControlView {
        rec_dot: get("rec_dot"),
        paused_icon: get("paused_icon").downcast().unwrap(),
        time: get("time_label").downcast().unwrap(),
        paused: get("paused_label").downcast().unwrap(),
        pause: get("pause_button").downcast().unwrap(),
        stop: get("stop_button").downcast().unwrap(),
        audio_separator: get("audio_separator"),
        microphone: get("microphone_button").downcast().unwrap(),
        microphone_icon: get("microphone_icon").downcast().unwrap(),
        system: get("system_button").downcast().unwrap(),
        system_icon: get("system_icon").downcast().unwrap(),
        minimize: get("minimize_button").downcast().unwrap(),
        window,
    }
}

fn icon(image: &gtk::Image) -> String {
    image.icon_name().map(|n| n.to_string()).unwrap_or_default()
}

fn button_icon(button: &gtk::Button) -> String {
    button
        .icon_name()
        .map(|n| n.to_string())
        .unwrap_or_default()
}

/// The recorded time of the running recording, in seconds.
fn elapsed_now() -> f64 {
    running()
        .and_then(|s| s.with_recording(|r| r.elapsed()))
        .expect("a running recording")
        .nseconds() as f64
        / 1e9
}

/// With `ANNOTA_SHOT_DIR` set, saves the control's window on the kit's `--desktop-bg`, in the
/// light and the dark colour scheme (`<name>-light.png`, `<name>-dark.png`).
fn control_shots(window: &gtk::Window, name: &str) {
    let Some(dir) = std::env::var_os("ANNOTA_SHOT_DIR") else {
        return;
    };
    for (dark, scheme, background) in [
        (false, "light", (0xc7, 0xc9, 0xd1)),
        (true, "dark", (0x3d, 0x3f, 0x47)),
    ] {
        set_scheme(dark);
        pump(400);
        let (width, height) = (window.width() as f32, window.height() as f32);
        let snapshot = gtk::Snapshot::new();
        snapshot.append_color(
            &gdk::RGBA::new(
                background.0 as f32 / 255.0,
                background.1 as f32 / 255.0,
                background.2 as f32 / 255.0,
                1.0,
            ),
            &gtk::graphene::Rect::new(0.0, 0.0, width, height),
        );
        gtk::WidgetPaintable::new(Some(window)).snapshot(
            &snapshot,
            f64::from(width),
            f64::from(height),
        );
        let texture = window
            .renderer()
            .unwrap()
            .render_texture(snapshot.to_node().unwrap(), None);
        texture
            .save_to_png(Path::new(&dir).join(format!("{name}-{scheme}.png")))
            .unwrap();
    }
    set_scheme(false);
}

/// Scenarios "Elapsed time" and "Minimize the control": once the recording runs, the undecorated
/// "Recording" window shows rec dot, time, Pause, Stop, both audio buttons and Minimize, with
/// Pause focused and `Tab` going Pause → Stop → Microphone → System audio → Minimize. Its time
/// follows the recording's own elapsed clock (`0:01` after a second of real recording) and,
/// with that clock standing at 12 s, reads `0:12` on the next refresh. Minimize asks for the
/// window to be minimised (a display without a window manager cannot show more) and the
/// recording goes on. Stop then closes the session and takes the control away at once.
// spec: screen-recording / Recording control / Elapsed time
// spec: screen-recording / Recording control / Minimize the control
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn rec_the_control_shows_the_time_and_minimizes() {
    let (scene, seen, _folder) = audio_scene("rec-control", true, true);
    let cast = add_cast(&scene, 7, screen_stream());
    cast.auto.set(Some(0));
    start(
        &scene.app,
        RecordRequest {
            mode: RecordMode::Screen,
        },
    );
    let view = wait_for_control();
    assert_eq!(view.window.title().as_deref(), Some("Recording"));
    assert!(!view.window.is_decorated());
    assert!(view.rec_dot.is_visible() && !view.paused.is_visible());
    assert!(!view.paused_icon.is_visible());
    assert!(view.audio_separator.is_visible());
    assert!(view.microphone.is_visible() && view.system.is_visible());
    assert_eq!(icon(&view.microphone_icon), "annota-microphone-symbolic");
    assert_eq!(icon(&view.system_icon), "annota-speaker-symbolic");
    assert_eq!(button_icon(&view.pause), "annota-pause-symbolic");
    assert_eq!(button_icon(&view.minimize), "window-minimize-symbolic");
    assert_eq!(view.minimize.tooltip_text().as_deref(), Some("Minimize"));
    assert!(view.pause.has_focus(), "Pause has the initial focus");
    for next in [&view.stop, &view.microphone, &view.system, &view.minimize] {
        view.window.child_focus(gtk::DirectionType::TabForward);
        assert!(next.has_focus(), "tab order");
    }
    view.pause.grab_focus();
    // Both meters are fed by the recording's level messages (0.5 → 5 bars, 0.1 → 4 bars).
    wait_for(
        || {
            control::meter(AudioSource::Microphone) == Some((5, false))
                && control::meter(AudioSource::SystemAudio) == Some((4, false))
        },
        "both meters lit",
    );

    // The real clock: the label follows the recording's elapsed time.
    wait_for(|| view.time.label() == "0:01", "0:01");
    let width = view.time.width();
    control_shots(&view.window, "05-recording-control");

    // A recording that has run for 12 s: the clock reports 12 s, the next tick shows it.
    let clock = Rc::new(Cell::new(gst::ClockTime::from_mseconds(11_900)));
    control::set_elapsed_source({
        let clock = clock.clone();
        Rc::new(move || Some(clock.get()))
    });
    assert_eq!(view.time.label(), "0:11");
    clock.set(gst::ClockTime::from_seconds(12));
    pump(control::TICK.as_millis() as u64 + 150);
    assert_eq!(view.time.label(), "0:12");
    assert_eq!(view.time.width(), width, "tabular, no jump");
    clock.set(gst::ClockTime::from_seconds(3600 + 12));
    pump(control::TICK.as_millis() as u64 + 150);
    assert_eq!(view.time.label(), "1:00:12");

    // Minimize: requested for the control's window; the recording continues.
    let (minimized, _) = control::requests();
    view.minimize.emit_clicked();
    assert_eq!(control::requests().0, minimized + 1);
    let before = elapsed_now();
    pump(800);
    assert!(elapsed_now() > before + 0.5, "the recording continues");
    assert!(running().is_some());
    assert!(control::shown().is_some(), "minimised, not closed");
    assert_eq!(seen.ended.borrow().len(), 0);

    // Stop: the control goes at once, the session is closed, the file finished.
    view.stop.emit_clicked();
    assert!(control::shown().is_none(), "the control disappears");
    wait_for(
        || !seen.ended.borrow().is_empty(),
        "the recording to finish",
    );
    cast.reaches("Session.Close", 1);
    let ended = std::mem::take(&mut *seen.ended.borrow_mut());
    assert_eq!(ended.len(), 1);
    assert_eq!(ended[0].0.reason, EndReason::Stopped);
    assert_eq!(ended[0].0.finished.audio.len(), 2);
    assert!(!is_active());
}

/// Scenario "Second recording request": while a recording runs, `--record screen`, `--record
/// region`, `--record window` (through `app.record`, which the Record card and the shortcut use
/// too) and a region from the setup each present the control and request nothing: one
/// `CreateSession` in all, no overlay. A microphone-only recording has no System audio button;
/// one without audio has neither button nor their separator. The Record shortcut (toggle) still
/// stops the recording, while it runs and while it is paused, and the control goes with it.
// spec: screen-recording / Recording control / Second recording request
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn rec_a_second_recording_request_presents_the_control() {
    let (scene, seen, _folder) = audio_scene("rec-second-request", true, false);
    let cast = add_cast(&scene, 7, screen_stream());
    cast.auto.set(Some(0));
    crate::application::setup_actions(&scene.app);
    scene
        .app
        .activate_action("record", Some(&RecordMode::Screen.as_str().to_variant()));
    let view = wait_for_control();
    assert!(view.microphone.is_visible());
    assert!(
        !view.system.is_visible(),
        "a source not recorded has no button"
    );
    assert!(view.audio_separator.is_visible());
    wait_for(|| view.time.label() == "0:01", "0:01");
    control_shots(&view.window, "05-microphone-only");

    let (_, presented) = control::requests();
    for mode in [RecordMode::Screen, RecordMode::Region, RecordMode::Window] {
        scene
            .app
            .activate_action("record", Some(&mode.as_str().to_variant()));
    }
    start_region(&scene.app, IRect::new(10, 10, 100, 100));
    pump(700);
    assert_eq!(control::requests().1, presented + 4, "presented each time");
    assert_eq!(cast.count("CreateSession"), 1, "no second session");
    assert!(overlay_window(&scene.app).is_none(), "no overlay");
    assert!(window_wait::shown().is_none());
    assert!(running().is_some());
    assert_eq!(control::shown().as_ref(), Some(&view.window));

    // The Record shortcut while recording: ends it as Stop.
    scene
        .app
        .activate_action("record", Some(&RecordMode::Toggle.as_str().to_variant()));
    assert!(control::shown().is_none(), "the control disappears");
    wait_for(
        || !seen.ended.borrow().is_empty(),
        "the recording to finish",
    );
    cast.reaches("Session.Close", 1);
    assert_eq!(
        seen.ended.borrow_mut().remove(0).0.reason,
        EndReason::Stopped
    );

    // Without audio: no audio buttons, no separator. Paused, the shortcut still stops it.
    want_audio(false, false);
    scene
        .app
        .activate_action("record", Some(&RecordMode::Screen.as_str().to_variant()));
    let view = wait_for_control();
    assert!(!view.microphone.is_visible() && !view.system.is_visible());
    assert!(
        !view.audio_separator.is_visible(),
        "the group's separator goes"
    );
    wait_for(|| view.time.label() == "0:01", "0:01");
    control_shots(&view.window, "05-no-audio");
    assert!(press(&view.window, gdk::Key::space));
    assert!(running()
        .unwrap()
        .with_recording(|r| r.is_paused())
        .unwrap());
    scene
        .app
        .activate_action("record", Some(&RecordMode::Toggle.as_str().to_variant()));
    assert!(control::shown().is_none());
    wait_for(
        || !seen.ended.borrow().is_empty(),
        "the paused recording to finish",
    );
    cast.reaches("Session.Close", 2);
    let (done, _) = seen.ended.borrow_mut().remove(0);
    assert_eq!(done.reason, EndReason::Stopped);
    assert!(done.finished.audio.is_empty());
    assert_eq!(cast.count("CreateSession"), 2);
    assert!(!is_active());
}

/// Scenarios "Paused state", "Mute the microphone" (its control part) and "Mute system audio":
/// `M` in the control mutes the microphone (slashed icon, unlit meter) until `M` again, `S` the
/// system audio until `S` again; the file is silent on that track for exactly that time and has
/// sound before and after. `Space` pauses: the rec dot becomes the amber pause glyph, "Paused"
/// appears, Pause becomes Resume (play) and keeps the focus, and the time stands still; `Space`
/// resumes. Stop ends it.
// spec: screen-recording / Pause and resume / Paused state
// spec: screen-recording / Audio / Mute the microphone
// spec: screen-recording / Audio / Mute system audio
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn rec_pause_and_mutes_in_the_control() {
    let (scene, seen, _folder) = audio_scene("rec-control-pause", true, true);
    let cast = add_cast(&scene, 7, screen_stream());
    cast.auto.set(Some(0));
    start(
        &scene.app,
        RecordRequest {
            mode: RecordMode::Screen,
        },
    );
    let view = wait_for_control();
    wait_for(
        || control::meter(AudioSource::Microphone) == Some((5, false)),
        "the microphone meter",
    );
    pump(800);

    // `M`: microphone muted until `M` again.
    let mic_muted = elapsed_now();
    assert!(press(&view.window, gdk::Key::m));
    assert!(running()
        .unwrap()
        .with_recording(|r| r.is_muted(AudioSource::Microphone))
        .unwrap());
    assert_eq!(icon(&view.microphone_icon), "microphone-disabled-symbolic");
    pump(300);
    assert_eq!(
        control::meter(AudioSource::Microphone),
        Some((0, false)),
        "unlit"
    );
    assert_eq!(icon(&view.system_icon), "annota-speaker-symbolic");
    pump(800);
    let mic_back = elapsed_now();
    assert!(press(&view.window, gdk::Key::m));
    assert_eq!(icon(&view.microphone_icon), "annota-microphone-symbolic");
    wait_for(
        || control::meter(AudioSource::Microphone) == Some((5, false)),
        "the microphone meter again",
    );
    pump(400);

    // `S`: system audio muted until `S` again.
    let system_muted = elapsed_now();
    assert!(press(&view.window, gdk::Key::s));
    assert_eq!(icon(&view.system_icon), "audio-volume-muted-symbolic");
    assert!(running()
        .unwrap()
        .with_recording(|r| r.is_muted(AudioSource::SystemAudio))
        .unwrap());
    pump(300);
    assert_eq!(control::meter(AudioSource::SystemAudio), Some((0, false)));
    pump(900);
    let system_back = elapsed_now();
    assert!(press(&view.window, gdk::Key::s));
    assert_eq!(icon(&view.system_icon), "annota-speaker-symbolic");
    pump(600);

    // `Space`: paused, with the time from the moment of pausing.
    view.pause.grab_focus();
    assert!(press(&view.window, gdk::Key::space));
    assert!(running()
        .unwrap()
        .with_recording(|r| r.is_paused())
        .unwrap());
    assert!(view.paused.is_visible());
    assert_eq!(view.paused.label(), "Paused");
    assert!(view.paused_icon.is_visible() && !view.rec_dot.is_visible());
    assert_eq!(icon(&view.paused_icon), "annota-pause-symbolic");
    assert_eq!(button_icon(&view.pause), "annota-play-symbolic", "Resume");
    assert!(view.pause.has_focus(), "the focus stays on that button");
    let frozen = view.time.label();
    let frozen_elapsed = elapsed_now();
    // The time from the moment of pausing, not 0.
    assert!(frozen_elapsed > system_back, "{frozen_elapsed} s");
    assert_eq!(
        frozen,
        control::format_elapsed(gst::ClockTime::from_nseconds((frozen_elapsed * 1e9) as u64))
    );
    // The mockup's paused state: the microphone muted as well.
    assert!(press(&view.window, gdk::Key::m));
    control_shots(&view.window, "06-paused-mic-muted");
    assert!(press(&view.window, gdk::Key::m));
    pump(1200);
    assert_eq!(view.time.label(), frozen, "the time no longer advances");
    assert!((elapsed_now() - frozen_elapsed).abs() < 0.01);

    // Resume (the focused button), then Stop.
    view.pause.emit_clicked();
    assert!(!running()
        .unwrap()
        .with_recording(|r| r.is_paused())
        .unwrap());
    assert!(!view.paused.is_visible() && view.rec_dot.is_visible());
    assert_eq!(button_icon(&view.pause), "annota-pause-symbolic");
    pump(800);
    assert!(elapsed_now() > frozen_elapsed + 0.4, "running again");
    view.stop.emit_clicked();
    wait_for(
        || !seen.ended.borrow().is_empty(),
        "the recording to finish",
    );
    let (done, _) = seen.ended.borrow_mut().remove(0);
    assert_eq!(done.reason, EndReason::Stopped);
    let analysis = analyse(&done.finished.path);
    assert_eq!(analysis.audio.len(), 2);
    let loudest = |track: &[AChunk]| rms_range(track, 0.1, mic_muted - 0.1).1;
    let (mic, system) = if loudest(&analysis.audio[0]) > loudest(&analysis.audio[1]) {
        (&analysis.audio[0], &analysis.audio[1])
    } else {
        (&analysis.audio[1], &analysis.audio[0])
    };
    let margin = 0.15;
    assert!(
        rms_range(mic, 0.1, mic_muted - margin).0 > 0.1,
        "mic before"
    );
    assert!(
        rms_range(mic, mic_muted + margin, mic_back - margin).1 < 0.001,
        "mic silent while muted"
    );
    assert!(
        rms_range(mic, mic_back + margin, system_muted).0 > 0.1,
        "mic after"
    );
    assert!(
        rms_range(system, 0.1, system_muted - margin).0 > 0.02,
        "system before"
    );
    assert!(
        rms_range(system, system_muted + margin, system_back - margin).1 < 0.001,
        "system silent until unmuted"
    );
    assert!(
        rms_range(system, system_back + margin, frozen_elapsed - margin).0 > 0.02,
        "system after unmuting"
    );
    // The tracks keep their length.
    let end = |track: &[AChunk]| track.last().map_or(0.0, |c| c.pts);
    assert!(end(mic) > frozen_elapsed && end(system) > frozen_elapsed);
}
