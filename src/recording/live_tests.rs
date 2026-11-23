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
use gtk::{gdk, gio, glib};

use super::*;
use crate::capture::live_tests::{
    find, portal_path, respond, scene, shot, Question, Scene, SCREENCAST_XML,
};
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
    countdown: RefCell<Vec<(Target, Instant)>>,
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
        hooks.countdown = Rc::new(move |_, target| {
            countdown
                .countdown
                .borrow_mut()
                .push((target, Instant::now()));
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
    assert!(countdown[0].1 > gone);
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

/// Scenario "Source request cancelled": Cancel in the indicator (the same handler as `Esc` and
/// the window's close request) closes the session, writes nothing and says nothing.
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
    let cancel: gtk::Button = find(&osd, "cancel_button").unwrap();
    cancel.emit_clicked();
    cast.reaches("Session.Close", 1);
    wait_for(|| !outcomes(&seen).is_empty(), "the outcome");
    assert_eq!(outcomes(&seen), [StartOutcome::Cancelled]);
    assert!(window_wait::shown().is_none());
    assert!(files_in(&folder).is_empty());
    assert!(seen.built.get().is_none());
    assert!(!is_active());

    // Closing the indicator (as `Esc` does) cancels just the same.
    start(
        &scene.app,
        RecordRequest {
            mode: RecordMode::Screen,
        },
    );
    cast.reaches("Start", 2);
    window_wait::shown().unwrap().close();
    cast.reaches("Session.Close", 2);
    wait_for(|| outcomes(&seen).len() == 2, "the second outcome");
    assert_eq!(outcomes(&seen)[1], StartOutcome::Cancelled);
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
