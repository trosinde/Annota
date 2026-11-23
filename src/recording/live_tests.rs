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
use crate::recording::pipeline::tests::{analyse, near, GREEN, RED};
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
    let Some(id) = glib::MainContext::default()
        .block_on(ashpd::WindowIdentifier::from_native(window))
        .map(|id| id.to_string())
        .and_then(|id| id.strip_prefix("x11:").map(String::from))
    else {
        return false;
    };
    let xdotool = |args: &[&str]| {
        std::process::Command::new("xdotool")
            .args(args)
            .status()
            .is_ok_and(|status| status.success())
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
/// the toolbar back. `Enter` then starts a region recording of the selection, which ends the
/// overlay with the selection in desktop px.
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

    // Enter: Start Recording with Region ends the overlay with the selection.
    assert!(press(&window, gdk::Key::Return));
    wait_for(|| outcome.borrow().is_some(), "the overlay's outcome");
    match outcome.borrow_mut().take().unwrap() {
        Outcome::Record { source, selection } => {
            assert_eq!(source, CaptureMode::Region);
            let monitor = the_monitor();
            assert_eq!(
                selection,
                Some(IRect::new(monitor.x() + 130, monitor.y() + 92, 700, 470))
            );
        }
        other => panic!("not a recording: {other:?}"),
    }
    pump(200);
    assert!(overlay_window(&scene.app).is_none(), "the overlay closed");
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
/// part), and the countdown gets the saved 5 s.
// spec: screen-recording / Recording setup / Choices are remembered
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn rec_setup_choices_are_remembered() {
    let (scene, seen, _folder) = recording_scene("rec-remembered", false);
    let cast = add_cast(&scene, 7, screen_stream());
    let settings = gio::Settings::new(config::APP_ID);
    capture::start_recording_setup(&scene.app);
    let window = wait_for_overlay(&scene.app);
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
    cast.answer(0);
    wait_for(|| !seen.countdown.borrow().is_empty(), "the countdown");
    let countdown = seen.countdown.borrow().clone();
    assert_eq!((countdown[0].0, countdown[0].1), (Target::Screen, 5));
    wait_for(|| running().is_some(), "the recording");
    assert!(stop());
    wait_for(
        || !seen.ended.borrow().is_empty(),
        "the recording to finish",
    );
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
