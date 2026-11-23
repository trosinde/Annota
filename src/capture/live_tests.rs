//! The permission flow of a capture against a stand-in portal, on a live display.
//!
//! A real `xdg-desktop-portal` must never be called from a test (it is the maintainer's real
//! desktop), so these tests own `org.freedesktop.portal.Desktop` and the permission store on a
//! private bus and answer the way the real ones do (`src/screenshot.c` of xdg-desktop-portal 1.21,
//! `src/access.c` of xdg-desktop-portal-gnome 50). This proves what Annota sends and how it sorts
//! the answers. It does not prove how a real GNOME session answers: see `manual-checks.md`.
//!
//! Each test runs in a process of its own, on a bus with no service directories (the test
//! refuses to run if a portal could be started on it):
//! `XDG_CONFIG_HOME=$(mktemp -d) GDK_BACKEND=x11 xvfb-run -a dbus-run-session --config-file=<no
//! services> -- cargo test --bin annota <full::path> -- --ignored --exact`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use adw::prelude::*;
use gtk::{gdk, gio, glib};

use super::shortcuts::{self, ShownTrigger};
use super::{start, Request, BUSY};
use crate::cli::CaptureMode;
use crate::config;
use crate::ui::test_support::{init, pump};

const PORTAL_XML: &str = r#"<node>
  <interface name="org.freedesktop.portal.Screenshot">
    <method name="Screenshot">
      <arg type="s" direction="in"/><arg type="a{sv}" direction="in"/>
      <arg type="o" direction="out"/>
    </method>
    <property name="version" type="u" access="read"/>
  </interface>
  <interface name="org.freedesktop.host.portal.Registry">
    <method name="Register">
      <arg type="s" direction="in"/><arg type="a{sv}" direction="in"/>
    </method>
  </interface>
  <interface name="org.freedesktop.impl.portal.PermissionStore">
    <method name="Lookup">
      <arg type="s" direction="in"/><arg type="s" direction="in"/>
      <arg type="a{sas}" direction="out"/><arg type="v" direction="out"/>
    </method>
    <method name="DeletePermission">
      <arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="in"/>
    </method>
  </interface>
</node>"#;

/// What the user does with the question, if it can be shown at all.
#[derive(Clone, Copy, PartialEq)]
enum Question {
    Allow,
    Deny,
    /// Closes the question without an answer (response code 1); nothing is stored.
    Dismiss,
    /// Even with a parent window the question is not shown; nothing is stored.
    CannotShow,
}

/// The user's answer to the open question.
type OpenQuestion = Rc<RefCell<Option<Box<dyn FnOnce()>>>>;

/// One Screenshot call as the portal saw it.
#[derive(Clone, Debug)]
struct Call {
    parent: String,
    interactive: bool,
    visible_windows: usize,
    file: Option<PathBuf>,
}

/// `xdg-permission-store`'s error for a table or entry that doesn't exist.
const NOT_FOUND: &str = "org.freedesktop.portal.Error.NotFound";

const COULD_NOT_ASK: &str =
    "GNOME could not ask for screen access. Click this window and try again.";

const NOTHING_CAPTURED: &str = "Nothing was captured";
const CAPTURE_FAILED: &str = "The capture failed. Nothing was captured.";

struct Portal {
    /// The `screenshot` entry of the `screenshot` table: the answer per app ID.
    store: Rc<RefCell<HashMap<String, &'static str>>>,
    /// `Lookup` fails, as it would inside a sandbox.
    unreadable: Rc<Cell<bool>>,
    /// The arguments of every `DeletePermission`: table, id, app.
    deletes: Rc<RefCell<Vec<(String, String, String)>>>,
    /// The D-Bus error `DeletePermission` answers with instead of deleting.
    delete_error: Rc<Cell<Option<&'static str>>>,
    /// How long `DeletePermission` takes to answer.
    delete_time: Rc<Cell<Duration>>,
    calls: Rc<RefCell<Vec<Call>>>,
    /// The question stays open until [`Scene::answer`], so things can happen during it.
    hold_question: Rc<Cell<bool>>,
    /// The same for a request nobody is asked about: the portal is busy with the grab.
    hold_grab: Rc<Cell<bool>>,
    /// The interactive picker ends without an image, as it does when the user closes it.
    picker_closed: Rc<Cell<bool>>,
    /// The user's answer, while the question is open.
    open_question: OpenQuestion,
    /// `Register` answers only when the test says so.
    hold_registration: Rc<Cell<bool>>,
    /// Calls of `Register`, and the one being held.
    registrations: Rc<Cell<usize>>,
    open_registration: Rc<RefCell<Option<gio::DBusMethodInvocation>>>,
}

struct Scene {
    app: adw::Application,
    portal: Portal,
    dir: PathBuf,
}

impl Drop for Scene {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Stops the test unless the bus is private and no real portal can be reached or started on it.
fn claim(connection: &gio::DBusConnection, name: &str) {
    let dbus = |method: &str, body: Option<&glib::Variant>| {
        connection
            .call_sync(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                method,
                body,
                None,
                gio::DBusCallFlags::NONE,
                5000,
                gio::Cancellable::NONE,
            )
            .unwrap()
    };
    let activatable: Vec<String> = dbus("ListActivatableNames", None).child_get(0);
    assert!(
        !activatable.iter().any(|n| n == name),
        "{name} can be started on this bus: not an isolated session, refusing to run"
    );
    // DO_NOT_QUEUE: 1 means Annota's request goes to this process, nothing else.
    let reply = dbus("RequestName", Some(&(name, 4u32).to_variant()));
    assert_eq!(
        reply.child_get::<u32>(0),
        1,
        "{name} already has an owner: not an isolated session, refusing to run"
    );
}

fn write_png(path: &Path) {
    let surface = gtk::cairo::ImageSurface::create(gtk::cairo::Format::ARgb32, 64, 48).unwrap();
    surface
        .write_to_png(&mut std::fs::File::create(path).unwrap())
        .unwrap();
}

fn start_portal(
    app: &adw::Application,
    dir: &Path,
    stored: Option<&'static str>,
    question: Question,
) -> Portal {
    let connection = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).unwrap();
    claim(&connection, "org.freedesktop.portal.Desktop");
    claim(&connection, "org.freedesktop.impl.portal.PermissionStore");
    let info = gio::DBusNodeInfo::for_xml(PORTAL_XML).unwrap();
    let store = Rc::new(RefCell::new(HashMap::new()));
    if let Some(stored) = stored {
        store
            .borrow_mut()
            .insert(config::APP_ID.to_string(), stored);
    }
    let calls = Rc::new(RefCell::new(Vec::new()));

    let unreadable = Rc::new(Cell::new(false));

    let deletes = Rc::new(RefCell::new(Vec::new()));
    let delete_error = Rc::new(Cell::new(None));
    let delete_time = Rc::new(Cell::new(Duration::ZERO));

    let (lookup_store, lookup_fails) = (store.clone(), unreadable.clone());
    let (delete_store, delete_log, delete_fails, delete_takes) = (
        store.clone(),
        deletes.clone(),
        delete_error.clone(),
        delete_time.clone(),
    );
    connection
        .register_object(
            "/org/freedesktop/impl/portal/PermissionStore",
            &info
                .lookup_interface("org.freedesktop.impl.portal.PermissionStore")
                .unwrap(),
        )
        .method_call(move |_, _, _, _, method, parameters, invocation| {
            if method == "DeletePermission" {
                let (table, id, app): (String, String, String) = parameters.get().unwrap();
                delete_log
                    .borrow_mut()
                    .push((table.clone(), id.clone(), app.clone()));
                let (store, fails) = (delete_store.clone(), delete_fails.get());
                glib::timeout_add_local_once(delete_takes.get(), move || {
                    // xdg-permission-store as remembered, not read: no entry is an error, no app in it not.
                    let exists =
                        table == "screenshot" && id == "screenshot" && !store.borrow().is_empty();
                    if let Some(error) = fails {
                        invocation.return_dbus_error(error, "The stand-in store fails");
                    } else if exists {
                        store.borrow_mut().remove(&app);
                        invocation.return_value(None);
                    } else {
                        invocation.return_dbus_error(NOT_FOUND, "No entry for screenshot");
                    }
                });
                return;
            }
            if lookup_fails.get() {
                invocation
                    .return_dbus_error("org.freedesktop.DBus.Error.AccessDenied", "Sandboxed");
                return;
            }
            let entries: HashMap<String, Vec<String>> = lookup_store
                .borrow()
                .iter()
                .map(|(app, value)| (app.clone(), vec![(*value).to_string()]))
                .collect();
            if entries.is_empty() {
                invocation.return_dbus_error(NOT_FOUND, "No entry for screenshot");
            } else {
                invocation.return_value(Some(&(entries, 0u8.to_variant()).to_variant()));
            }
        })
        .build()
        .unwrap();

    let hold_registration = Rc::new(Cell::new(false));
    let registrations = Rc::new(Cell::new(0));
    let open_registration = Rc::new(RefCell::new(None));
    let (hold, count, open) = (
        hold_registration.clone(),
        registrations.clone(),
        open_registration.clone(),
    );
    connection
        .register_object(
            "/org/freedesktop/portal/desktop",
            &info
                .lookup_interface("org.freedesktop.host.portal.Registry")
                .unwrap(),
        )
        .method_call(move |_, _, _, _, _, _, invocation| {
            count.set(count.get() + 1);
            if hold.get() {
                open.replace(Some(invocation));
            } else {
                invocation.return_value(None);
            }
        })
        .build()
        .unwrap();

    let hold_question = Rc::new(Cell::new(false));
    let hold_grab = Rc::new(Cell::new(false));
    let picker_closed = Rc::new(Cell::new(false));
    let open_question: OpenQuestion = Rc::new(RefCell::new(None));
    let (portal_store, portal_calls) = (store.clone(), Rc::clone(&calls));
    let (hold, hold_any, open, closed_picker) = (
        hold_question.clone(),
        hold_grab.clone(),
        open_question.clone(),
        picker_closed.clone(),
    );
    let (app, dir) = (app.clone(), dir.to_path_buf());
    let counter = Rc::new(Cell::new(0));
    connection
        .register_object(
            "/org/freedesktop/portal/desktop",
            &info
                .lookup_interface("org.freedesktop.portal.Screenshot")
                .unwrap(),
        )
        .method_call(move |connection, sender, _, _, _, parameters, invocation| {
            let parent = parameters.child_value(0).str().unwrap().to_string();
            let options: HashMap<String, glib::Variant> = parameters.child_value(1).get().unwrap();
            let interactive = options
                .get("interactive")
                .and_then(|v| v.get::<bool>())
                .unwrap_or(false);
            let token = options
                .get("handle_token")
                .and_then(|v| v.get::<String>())
                .unwrap();
            let sender = sender.unwrap().to_string();

            // `check_non_interactive_permission_in_thread` and GNOME's access dialog.
            let stored = portal_store.borrow().get(config::APP_ID).copied();
            // No parent: GNOME Shell's "Only the focused app …", with Annota's windows hidden the
            // focus is on another app. Nothing is stored.
            let asks = !interactive
                && stored.is_none()
                && !parent.is_empty()
                && question != Question::CannotShow;
            // GNOME's screenshot UI, closed without an image: `InteractiveScreenshot` reports
            // `success: false`, which xdg-desktop-portal-gnome 50 answers with response code 2
            // — the same code as a real failure, not the cancel code 1.
            let closed = interactive && closed_picker.get();
            let index = portal_calls.borrow().len();
            portal_calls.borrow_mut().push(Call {
                parent,
                interactive,
                visible_windows: super::visible_windows(&app).len(),
                file: None,
            });

            let unique = sender.trim_start_matches(':').replace('.', "_");
            let handle = format!("/org/freedesktop/portal/desktop/request/{unique}/{token}");
            let path = glib::variant::ObjectPath::try_from(handle.clone()).unwrap();
            invocation.return_value(Some(&(path,).to_variant()));

            let (store, calls) = (portal_store.clone(), portal_calls.clone());
            let (dir, counter, connection) = (dir.clone(), counter.clone(), connection.clone());
            let answer = move || {
                let code = match (asks, question) {
                    (true, Question::Allow) => {
                        store.borrow_mut().insert(config::APP_ID.to_string(), "yes");
                        0u32
                    }
                    (true, Question::Deny) => {
                        store.borrow_mut().insert(config::APP_ID.to_string(), "no");
                        2
                    }
                    (true, _) => 1,
                    (false, _) if closed => 2,
                    (false, _) if interactive || stored == Some("yes") => 0,
                    (false, _) => 2,
                };
                let mut results: HashMap<String, glib::Variant> = HashMap::new();
                if code == 0 {
                    counter.set(counter.get() + 1);
                    let file = dir.join(format!("portal-{}.png", counter.get()));
                    write_png(&file);
                    let uri = gio::File::for_path(&file).uri().to_string();
                    results.insert(String::from("uri"), uri.to_variant());
                    calls.borrow_mut()[index].file = Some(file);
                }
                connection
                    .emit_signal(
                        Some(&sender),
                        &handle,
                        "org.freedesktop.portal.Request",
                        "Response",
                        Some(&(code, results).to_variant()),
                    )
                    .unwrap();
            };
            if (asks && hold.get()) || hold_any.get() {
                open.replace(Some(Box::new(answer)));
            } else {
                answer();
            }
        })
        .property(|_, _, _, _, _| 2u32.to_variant())
        .build()
        .unwrap();
    Portal {
        store,
        unreadable,
        deletes,
        delete_error,
        delete_time,
        calls,
        hold_question,
        hold_grab,
        picker_closed,
        open_question,
        hold_registration,
        registrations,
        open_registration,
    }
}

impl Portal {
    /// What the store holds for Annota.
    fn stored(&self) -> Option<&'static str> {
        self.store.borrow().get(config::APP_ID).copied()
    }
}

/// A private environment with the portal running. Nothing here touches the real session.
fn scene(name: &str, stored: Option<&'static str>, question: Question) -> Scene {
    scene_with_schemas(name, stored, question, &[])
}

/// [`scene`] with further GSettings schemas (file name, XML) beside Annota's, e.g. a stand-in for
/// GNOME Shell's keybindings.
fn scene_with_schemas(
    name: &str,
    stored: Option<&'static str>,
    question: Question,
    schemas: &[(&str, &str)],
) -> Scene {
    let dir = std::env::temp_dir().join(format!("annota-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("schemas")).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    std::fs::copy(
        root.join("data/io.github.trosinde.Annota.gschema.xml"),
        dir.join("schemas/io.github.trosinde.Annota.gschema.xml"),
    )
    .unwrap();
    for (file, xml) in schemas {
        std::fs::write(dir.join("schemas").join(file), xml).unwrap();
    }
    assert!(std::process::Command::new("glib-compile-schemas")
        .arg(dir.join("schemas"))
        .status()
        .unwrap()
        .success());
    // Edition 2021: setting the environment is safe here, before GTK starts threads.
    std::env::set_var("GSETTINGS_SCHEMA_DIR", dir.join("schemas"));
    std::env::set_var("GSETTINGS_BACKEND", "memory");
    std::env::set_var("XDG_CACHE_HOME", dir.join("cache"));
    std::env::set_var("XDG_DATA_HOME", dir.join("data"));
    let resources = std::env::var_os("ANNOTA_GRESOURCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("_build/data/resources.gresource"));
    gio::resources_register(&gio::Resource::load(&resources).unwrap());
    init();
    let app = adw::Application::builder()
        .application_id("io.github.trosinde.Annota.PermissionTest")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .resource_base_path("/io/github/trosinde/Annota")
        .build();
    app.register(gio::Cancellable::NONE).unwrap();
    let portal = start_portal(&app, &dir, stored, question);
    Scene { app, portal, dir }
}

impl Scene {
    fn capture(&self, mode: CaptureMode) {
        self.start_capture(mode, 0);
        self.capture_ends();
    }

    /// Starts a capture and returns while it runs.
    fn start_capture(&self, mode: CaptureMode, delay: u32) {
        start(
            &self.app,
            Request {
                mode,
                delay,
                include_pointer: false,
            },
        );
        pump(10);
    }

    fn capture_ends(&self) {
        let end = Instant::now() + Duration::from_secs(20);
        while BUSY.with(Cell::get) {
            assert!(Instant::now() < end, "the capture did not end");
            pump(20);
        }
        pump(300);
    }

    /// Waits until GNOME's question is open (the stand-in holds it).
    fn question_opens(&self) {
        let end = Instant::now() + Duration::from_secs(10);
        while self.portal.open_question.borrow().is_none() {
            assert!(Instant::now() < end, "no question");
            pump(20);
        }
    }

    /// The user answers the open question.
    fn answer(&self) {
        let answer = self.portal.open_question.borrow_mut().take();
        answer.expect("an open question")();
    }

    /// Opens the editor with an image, as `annota --open` does.
    fn open_editor(&self) -> gtk::Window {
        let image = self.dir.join("opened.png");
        write_png(&image);
        assert!(crate::ui::editor::open_file(&self.app, &image, false));
        pump(200);
        self.app
            .windows()
            .into_iter()
            .find(|w| w.has_css_class("annota-editor"))
            .expect("the editor window")
    }

    fn calls(&self) -> Vec<Call> {
        self.portal.calls.borrow().clone()
    }

    fn visible(&self) -> Vec<gtk::Window> {
        super::visible_windows(&self.app)
    }

    /// The "Screen Access Denied" dialog, if one is open.
    fn dialog(&self) -> Option<adw::Dialog> {
        self.app
            .windows()
            .into_iter()
            .filter_map(|w| w.downcast::<adw::ApplicationWindow>().ok())
            .find_map(|w| w.visible_dialog())
    }

    /// The dialog after a stored "no" refused a capture.
    fn denied_dialog(&self) -> DeniedDialog {
        self.capture(CaptureMode::Screen);
        let dialog = self.dialog().expect("the denied dialog");
        DeniedDialog {
            close: find(&dialog, "close_button").expect("Close"),
            ask_again: find(&dialog, "ask_again_button").expect("Ask Again"),
            failed: find(&dialog, "reset_failed_label").expect("the error line"),
            dialog,
        }
    }

    /// Waits until the dialog is gone, or says that it isn't.
    fn dialog_closes(&self) -> bool {
        let end = Instant::now() + Duration::from_secs(3);
        while self.dialog().is_some() && Instant::now() < end {
            pump(20);
        }
        self.dialog().is_none()
    }

    /// Whether `window` shows a toast with this text.
    fn shows_toast(&self, window: &gtk::Window, text: &str) -> bool {
        holds(window.upcast_ref(), &|toast| {
            toast.type_().name() == "AdwToastWidget"
                && holds(toast, &|w| {
                    w.downcast_ref::<gtk::Label>()
                        .is_some_and(|l| l.text() == text)
                })
        })
    }

    /// A toast on any window.
    fn has_toast(&self) -> bool {
        self.app.windows().iter().any(|window| {
            holds(window.upcast_ref(), &|w| {
                w.type_().name() == "AdwToastWidget"
            })
        })
    }

    fn assert_portal_files_gone(&self) {
        for file in self.calls().iter().filter_map(|c| c.file.clone()) {
            assert!(!file.exists(), "{file:?} left behind");
        }
    }
}

struct DeniedDialog {
    dialog: adw::Dialog,
    close: gtk::Button,
    ask_again: gtk::Button,
    failed: gtk::Label,
}

/// Whether `widget` or anything below it passes `test`.
fn holds(widget: &gtk::Widget, test: &dyn Fn(&gtk::Widget) -> bool) -> bool {
    test(widget)
        || std::iter::successors(widget.first_child(), |child| child.next_sibling())
            .any(|child| holds(&child, test))
}

/// The widget with this ID in the `.ui` file.
fn find<W: IsA<gtk::Widget>>(root: &impl IsA<gtk::Widget>, id: &str) -> Option<W> {
    let root = root.as_ref();
    if root.buildable_id().as_deref() == Some(id) {
        return root.clone().downcast().ok();
    }
    std::iter::successors(root.first_child(), |child| child.next_sibling())
        .find_map(|child| find(&child, id))
}

/// With `ANNOTA_SHOT_DIR` set, saves `widget` as a PNG to look at.
fn shot(widget: &impl IsA<gtk::Widget>, name: &str) {
    let Some(dir) = std::env::var_os("ANNOTA_SHOT_DIR") else {
        return;
    };
    pump(500);
    let widget = widget.as_ref();
    let paintable = gtk::WidgetPaintable::new(Some(widget));
    let snapshot = gtk::Snapshot::new();
    let (width, height) = (f64::from(widget.width()), f64::from(widget.height()));
    paintable.snapshot(&snapshot, width, height);
    let node = snapshot.to_node().unwrap();
    let texture = widget
        .native()
        .and_then(|native| native.renderer())
        .unwrap()
        .render_texture(&node, None);
    texture
        .save_to_png(Path::new(&dir).join(format!("{name}.png")))
        .unwrap();
}

fn set_after_capture_to_clipboard() {
    gio::Settings::new(config::APP_ID)
        .set_enum("after-capture", 1)
        .unwrap();
}

/// First run from a shortcut, nothing open, nothing stored, the user allows.
// spec: capture / After-capture destination / Copy to clipboard
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn first_run_allow_from_a_shortcut() {
    let scene = scene("allow", None, Question::Allow);
    set_after_capture_to_clipboard();
    scene.capture(CaptureMode::Screen);
    let calls = scene.calls();
    assert_eq!(calls.len(), 2, "{calls:?}");
    // The question: a parent the desktop can use, with its window on the screen.
    assert!(calls[0].parent.starts_with("x11:"), "{calls:?}");
    assert_eq!(calls[0].visible_windows, 1, "{calls:?}");
    // The capture: nothing of Annota on the screen.
    assert_eq!(calls[1].parent, "", "{calls:?}");
    assert_eq!(calls[1].visible_windows, 0, "{calls:?}");
    assert_eq!(scene.portal.stored(), Some("yes"));
    scene.assert_portal_files_gone();
    // "Copy to clipboard" from a shortcut opens no window, and the question's window is gone.
    assert!(scene.visible().is_empty());
    assert!(scene.app.windows().is_empty());
    assert!(scene.dialog().is_none());
    let clipboard = gdk::Display::default().unwrap().clipboard();
    assert!(clipboard
        .formats()
        .contains_type(gdk::Texture::static_type()));
}

/// The window in use asks. It comes back after the capture; the second image reaches the editor.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn first_run_allow_over_the_open_window() {
    let scene = scene("allow-open", None, Question::Allow);
    let window = crate::application::present_window(&scene.app);
    pump(100);
    scene.capture(CaptureMode::Screen);
    let calls = scene.calls();
    assert_eq!(calls.len(), 2, "{calls:?}");
    assert!(calls[0].parent.starts_with("x11:"), "{calls:?}");
    assert_eq!(calls[0].visible_windows, 1, "{calls:?}");
    assert_eq!(
        (calls[1].parent.as_str(), calls[1].visible_windows),
        ("", 0)
    );
    assert!(window.is_visible(), "the window comes back");
    assert_eq!(scene.visible().len(), 2, "with the capture in the editor");
    scene.assert_portal_files_gone();
}

/// The user chooses Deny: the denied dialog, on a stored "no", with Ask Again as default and focus.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn first_run_deny() {
    let scene = scene("deny", None, Question::Deny);
    scene.capture(CaptureMode::Screen);
    let calls = scene.calls();
    assert_eq!(calls.len(), 1, "no second try after a Deny: {calls:?}");
    assert!(calls[0].parent.starts_with("x11:"), "{calls:?}");
    assert_eq!(scene.portal.stored(), Some("no"));
    let dialog = scene.dialog().expect("the denied dialog");
    shot(&dialog, "permission-denied");
    let ask_again: gtk::Button = find(&dialog, "ask_again_button").expect("Ask Again");
    assert_eq!(dialog.focus().as_ref(), Some(ask_again.upcast_ref()));
    assert_eq!(
        dialog.default_widget().as_ref(),
        Some(ask_again.upcast_ref())
    );
    assert!(ask_again.has_visible_focus(), "the ring of mockup 08");
    assert!(ask_again.has_css_class("suggested-action"));
}

/// Ask Again deletes Annota's entry and nobody else's, closes, and captures nothing.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn ask_again_removes_only_annotas_entry_and_closes() {
    let scene = scene("ask-again", Some("no"), Question::Allow);
    scene
        .portal
        .store
        .borrow_mut()
        .insert(String::from("org.other.App"), "no");
    let denied = scene.denied_dialog();
    assert!(scene.calls().is_empty());
    denied.ask_again.emit_clicked();
    assert!(scene.dialog_closes(), "Ask Again closes the dialog");
    assert_eq!(
        *scene.portal.deletes.borrow(),
        [(
            String::from("screenshot"),
            String::from("screenshot"),
            config::APP_ID.to_string()
        )]
    );
    assert_eq!(scene.portal.stored(), None);
    assert_eq!(
        scene.portal.store.borrow().get("org.other.App"),
        Some(&"no"),
        "another app's answer stays"
    );
    pump(300);
    assert!(scene.calls().is_empty(), "nothing is captured");
    assert!(!scene.has_toast(), "no toast on success");
}

/// After Ask Again the next capture finds nothing stored, so GNOME's question comes.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn a_capture_after_ask_again_asks_again() {
    let scene = scene("asks-again", Some("no"), Question::Allow);
    let denied = scene.denied_dialog();
    // What Enter does: Ask Again is the default.
    denied
        .ask_again
        .activate_action("default.activate", None)
        .unwrap();
    assert!(scene.dialog_closes());
    scene.capture(CaptureMode::Screen);
    let calls = scene.calls();
    assert_eq!(calls.len(), 2, "asked, captured: {calls:?}");
    assert!(calls[0].parent.starts_with("x11:"), "{calls:?}");
    assert_eq!(calls[0].visible_windows, 1, "{calls:?}");
    assert_eq!(
        (calls[1].parent.as_str(), calls[1].visible_windows),
        ("", 0)
    );
    assert_eq!(scene.portal.stored(), Some("yes"));
    assert!(scene.dialog().is_none());
    scene.assert_portal_files_gone();
}

/// A failing delete: the dialog stays with the error line, and Ask Again can be tried again.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn a_failing_delete_keeps_the_dialog_with_the_error_line() {
    let scene = scene("delete-fails", Some("no"), Question::Allow);
    let denied = scene.denied_dialog();
    assert!(!denied.failed.is_visible(), "no error before a failure");
    scene
        .portal
        .delete_error
        .set(Some("org.freedesktop.DBus.Error.AccessDenied"));
    denied.ask_again.emit_clicked();
    pump(300);
    assert_eq!(scene.portal.deletes.borrow().len(), 1);
    assert_eq!(scene.dialog().as_ref(), Some(&denied.dialog), "stays open");
    assert!(denied.failed.is_visible(), "the error line shows");
    assert_eq!(denied.failed.text(), "The answer could not be reset");
    assert!(!scene.has_toast(), "the error is in the dialog");
    assert_eq!(scene.portal.stored(), Some("no"));
    assert!(denied.close.is_sensitive() && denied.ask_again.is_sensitive());
    assert_eq!(
        denied.dialog.focus().as_ref(),
        Some(denied.ask_again.upcast_ref())
    );
    shot(&denied.dialog, "permission-reset-failed");

    // The store works again: the line goes, and so does the dialog.
    scene.portal.delete_error.set(None);
    denied.ask_again.emit_clicked();
    assert!(!denied.failed.is_visible(), "a new try clears the line");
    assert!(scene.dialog_closes());
    assert_eq!(scene.portal.stored(), None);
}

/// An entry that is already gone is what Ask Again wanted.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn not_found_counts_as_success() {
    let scene = scene("not-found", Some("no"), Question::Allow);
    let denied = scene.denied_dialog();
    // Reset from elsewhere while the dialog was open.
    scene.portal.store.borrow_mut().clear();
    denied.ask_again.emit_clicked();
    assert!(scene.dialog_closes(), "NotFound is not a failure");
    assert_eq!(scene.portal.deletes.borrow().len(), 1);
    assert!(!denied.failed.is_visible());
    assert!(!scene.has_toast());
}

/// Close keeps the answer, and so does closing the way Esc does; the dialog comes back.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn close_keeps_the_answer() {
    let scene = scene("close", Some("no"), Question::Allow);
    let denied = scene.denied_dialog();
    denied.close.emit_clicked();
    assert!(scene.dialog_closes(), "Close closes the dialog");
    assert!(scene.portal.deletes.borrow().is_empty());
    assert_eq!(scene.portal.stored(), Some("no"));

    let denied = scene.denied_dialog();
    assert!(scene.calls().is_empty(), "no question: the answer is kept");
    // What libadwaita's Esc binding calls. A real key press can't be sent from here.
    denied.dialog.close();
    assert!(scene.dialog_closes(), "Esc closes the dialog");
    assert!(scene.portal.deletes.borrow().is_empty());
    assert_eq!(scene.portal.stored(), Some("no"));
}

/// While the delete runs: no second delete, and nothing can close the dialog.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn ask_again_runs_once_and_does_not_block() {
    let scene = scene("once", Some("no"), Question::Allow);
    let denied = scene.denied_dialog();
    scene.portal.delete_time.set(Duration::from_millis(800));
    denied.ask_again.emit_clicked();
    denied.ask_again.emit_clicked();
    pump(200);
    // The main loop ran: the store's slow answer doesn't block the UI.
    denied.ask_again.emit_clicked();
    assert!(!denied.ask_again.is_sensitive() && !denied.close.is_sensitive());
    assert!(!denied.dialog.close(), "Esc waits for the answer");
    pump(100);
    assert_eq!(scene.dialog().as_ref(), Some(&denied.dialog));
    assert_eq!(scene.portal.stored(), Some("no"), "not answered yet");
    assert!(scene.dialog_closes());
    assert_eq!(scene.portal.deletes.borrow().len(), 1, "one delete");
    assert_eq!(scene.portal.stored(), None);
}

/// The German dialog, both states: everything fits, Ask Again's label isn't cut off.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn the_german_dialog_fits() {
    // Edition 2021: setting the environment is safe here, before GTK starts threads. No German
    // locale is needed: gettext follows `LANGUAGE` in any locale but C and C.UTF-8.
    std::env::set_var("LC_ALL", "en_US.UTF-8");
    std::env::set_var("LANGUAGE", "de");
    let po = Path::new(env!("CARGO_MANIFEST_DIR")).join("_build/po");
    gettextrs::setlocale(gettextrs::LocaleCategory::LcAll, "en_US.UTF-8").unwrap();
    gettextrs::bindtextdomain(config::GETTEXT_PACKAGE, po).unwrap();
    gettextrs::bind_textdomain_codeset(config::GETTEXT_PACKAGE, "UTF-8").unwrap();
    gettextrs::textdomain(config::GETTEXT_PACKAGE).unwrap();

    let scene = scene("german", Some("no"), Question::Allow);
    let denied = scene.denied_dialog();
    pump(300);
    assert_eq!(denied.ask_again.label().as_deref(), Some("Erneut fragen"));
    assert_eq!(denied.close.label().as_deref(), Some("Schließen"));
    assert!(holds(denied.dialog.upcast_ref(), &|w| w
        .downcast_ref::<gtk::Label>()
        .is_some_and(|l| l.text().starts_with("GNOME hat gespeichert")
            && l.text().contains("Erneut fragen")
            && l.uses_markup())));
    // Labels only: `width()` and `height()` leave out a button's padding, `measure` doesn't.
    let fits = |label: &gtk::Widget| {
        let (_, natural, _, _) = label.measure(gtk::Orientation::Horizontal, -1);
        let (minimum, _, _, _) = label.measure(gtk::Orientation::Vertical, label.width());
        assert!(natural <= label.width(), "{natural} > {}", label.width());
        assert!(minimum <= label.height(), "{minimum} > {}", label.height());
    };
    let content_width = || {
        let buttons = denied.ask_again.parent().expect("the button row");
        buttons.parent().expect("the dialog's content").width()
    };
    let text_of = |button: &gtk::Button| button.child().expect("the button's label");
    fits(&text_of(&denied.ask_again));
    fits(&text_of(&denied.close));
    assert_eq!(
        denied.ask_again.width(),
        denied.close.width(),
        "equal width"
    );
    assert_eq!(content_width(), 372, "420 px less the margins");
    for button in [&denied.ask_again, &denied.close] {
        let bounds = button.compute_bounds(button).expect("on the screen");
        assert_eq!(
            bounds.height(),
            40.0,
            "the redline's height, padding included"
        );
    }
    shot(&denied.dialog, "permission-denied-de");

    scene
        .portal
        .delete_error
        .set(Some("org.freedesktop.DBus.Error.AccessDenied"));
    denied.ask_again.emit_clicked();
    pump(500);
    assert_eq!(
        denied.failed.text(),
        "Die Antwort konnte nicht zurückgesetzt werden"
    );
    assert!(denied.failed.is_visible());
    fits(denied.failed.upcast_ref());
    fits(&text_of(&denied.ask_again));
    assert_eq!(content_width(), 372, "420 px less the margins");
    shot(&denied.dialog, "permission-reset-failed-de");
}

/// A stored "no": no request, no question, no window hidden, the denied dialog.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn stored_no_is_denied_without_asking() {
    let scene = scene("stored-no", Some("no"), Question::Allow);
    let window = crate::application::present_window(&scene.app);
    pump(100);
    let hidden = Rc::new(Cell::new(false));
    let seen = hidden.clone();
    window.connect_hide(move |_| seen.set(true));
    scene.capture(CaptureMode::Screen);
    assert!(scene.calls().is_empty(), "{:?}", scene.calls());
    assert!(!hidden.get(), "nothing is captured, so nothing hides");
    assert_eq!(scene.portal.stored(), Some("no"));
    assert!(scene.dialog().is_some());
}

/// A stored "no" with a capture delay: the dialog comes at once, no countdown first.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn stored_no_shows_the_dialog_without_the_delay() {
    let scene = scene("stored-no-delay", Some("no"), Question::Allow);
    let started = Instant::now();
    scene.start_capture(CaptureMode::Region, 5);
    while scene.dialog().is_none() {
        assert!(started.elapsed() < Duration::from_secs(2), "it waited");
        pump(20);
    }
    assert!(!BUSY.with(Cell::get), "the capture is over");
    assert!(scene.calls().is_empty());
    assert_eq!(
        scene.visible().len(),
        1,
        "no countdown beside the window with the dialog"
    );
}

/// With only the editor open, the dialog is over the editor (mockup 08): no launcher window.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn the_denied_dialog_uses_the_open_editor() {
    let scene = scene("denied-editor", Some("no"), Question::Allow);
    let editor = scene.open_editor();
    scene.capture(CaptureMode::Screen);
    assert_eq!(
        scene.app.windows().as_slice(),
        std::slice::from_ref(&editor),
        "no new window"
    );
    let editor: adw::ApplicationWindow = editor.downcast().unwrap();
    assert!(editor.visible_dialog().is_some(), "over the editor");
    shot(&editor, "permission-denied-editor");
}

/// A capture shortcut with the dialog open: still one dialog, raised, and one Close ends it.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn a_second_capture_does_not_stack_a_second_dialog() {
    let scene = scene("no-stack", Some("no"), Question::Allow);
    let denied = scene.denied_dialog();
    scene.capture(CaptureMode::Region);
    assert_eq!(
        scene.dialog().as_ref(),
        Some(&denied.dialog),
        "the same one"
    );
    denied.close.emit_clicked();
    assert!(scene.dialog_closes(), "nothing is left behind it");
    // Closed, it can come back.
    let again = scene.denied_dialog();
    assert_ne!(again.dialog, denied.dialog);
}

/// The bug report: response code 2 with nothing stored is not "denied", and the toast says so.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn a_question_that_cannot_be_shown_is_not_a_denial() {
    let scene = scene("cannot-ask", None, Question::CannotShow);
    scene.capture(CaptureMode::Screen);
    let calls = scene.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(calls[0].parent.starts_with("x11:"), "{calls:?}");
    assert_eq!(scene.portal.stored(), None, "nothing is stored");
    assert!(scene.dialog().is_none(), "no \"Screen Access Denied\"");
    let window = scene.visible();
    assert_eq!(window.len(), 1, "the window with the toast stays");
    assert!(scene.shows_toast(&window[0], COULD_NOT_ASK));
    shot(&window[0], "could-not-ask");
}

/// With only the editor open, the question is asked over it and the toast is on it.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn could_not_ask_uses_the_open_editor() {
    let scene = scene("cannot-ask-editor", None, Question::CannotShow);
    let editor = scene.open_editor();
    scene.capture(CaptureMode::Screen);
    assert_eq!(
        scene.app.windows().as_slice(),
        std::slice::from_ref(&editor),
        "no new window"
    );
    assert!(scene.shows_toast(&editor, COULD_NOT_ASK));
}

/// A window opened while GNOME asks is not the question's window: it is hidden for the grab like
/// the others and comes back, so an edited document can't vanish without "Save Changes?".
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn a_window_opened_during_the_question_survives() {
    let scene = scene("opened-during", None, Question::Allow);
    set_after_capture_to_clipboard();
    scene.portal.hold_question.set(true);
    scene.start_capture(CaptureMode::Screen, 0);
    scene.question_opens();
    let question_window = scene.visible();
    assert_eq!(question_window.len(), 1, "the window opened to ask");
    let editor = scene.open_editor();
    assert_eq!(scene.visible().len(), 2);
    scene.answer();
    scene.capture_ends();

    let calls = scene.calls();
    assert_eq!(calls.len(), 2, "{calls:?}");
    assert_eq!(
        (calls[1].parent.as_str(), calls[1].visible_windows),
        ("", 0),
        "out of the picture like every other window"
    );
    assert_eq!(
        scene.app.windows().as_slice(),
        std::slice::from_ref(&editor),
        "the asking one is gone"
    );
    assert!(editor.is_visible(), "the editor is back");
    assert!(
        scene.shows_toast(&editor, "Screen captured · 64 × 48 · Copied to clipboard"),
        "a window is open, so the copy is confirmed there"
    );
}

/// A window closed while GNOME asks: the answer is safe, and it doesn't come back.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn closing_the_parent_during_the_question_is_safe() {
    let scene = scene("closed-during", None, Question::Allow);
    set_after_capture_to_clipboard();
    let window = crate::application::present_window(&scene.app);
    pump(100);
    scene.portal.hold_question.set(true);
    scene.start_capture(CaptureMode::Screen, 0);
    scene.question_opens();
    window.destroy();
    pump(50);
    scene.answer();
    scene.capture_ends();
    assert_eq!(scene.calls().len(), 2, "{:?}", scene.calls());
    assert!(!window.is_visible(), "a destroyed window stays closed");
    assert!(scene.visible().is_empty());
    assert!(scene.app.windows().is_empty());

    // And while the grab runs, with the window hidden: it isn't brought back from the dead.
    let window = crate::application::present_window(&scene.app);
    pump(100);
    scene.portal.hold_grab.set(true);
    scene.start_capture(CaptureMode::Screen, 0);
    scene.question_opens();
    assert!(!window.is_visible(), "hidden for the grab");
    window.destroy();
    scene.answer();
    scene.capture_ends();
    assert!(!window.is_visible(), "a destroyed window stays closed");
    assert!(scene.app.windows().is_empty());
}

/// The X11 half of the Wayland panic: the identifier goes with the window, before the surface.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn the_parent_handle_goes_with_the_window() {
    let scene = scene("handle", None, Question::Allow);
    let window = crate::application::present_window(&scene.app);
    pump(100);
    glib::MainContext::default().block_on(super::portal::held_until_the_window_closes(
        window.upcast_ref(),
    ));
}

/// The question closed without an answer: the window opened for it goes, one in use stays.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn a_dismissed_question_leaves_no_window_of_its_own() {
    let scene = scene("dismissed", None, Question::Dismiss);
    scene.capture(CaptureMode::Screen);
    assert_eq!(scene.calls().len(), 1, "no capture follows");
    assert!(scene.app.windows().is_empty(), "the asking window is gone");
    assert!(!scene.has_toast() && scene.dialog().is_none());

    let window = crate::application::present_window(&scene.app);
    pump(100);
    scene.capture(CaptureMode::Screen);
    assert_eq!(scene.calls().len(), 2, "{:?}", scene.calls());
    assert!(window.is_visible(), "a window in use stays");
    assert_eq!(scene.app.windows().len(), 1);
}

/// A cold `--capture`: the shortcut setup is registering Annota's ID, and the grab waits for it.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn a_grab_waits_for_the_registration() {
    let scene = scene("registration", Some("yes"), Question::Allow);
    set_after_capture_to_clipboard();
    scene.portal.hold_registration.set(true);
    glib::spawn_future_local(super::portal::ensure_registered());
    let end = Instant::now() + Duration::from_secs(5);
    while scene.portal.registrations.get() == 0 {
        assert!(Instant::now() < end, "no registration");
        pump(20);
    }
    scene.start_capture(CaptureMode::Screen, 0);
    pump(800);
    assert!(scene.calls().is_empty(), "it overtook the registration");
    assert!(BUSY.with(Cell::get));
    let held = scene.portal.open_registration.borrow_mut().take();
    held.expect("the held registration").return_value(None);
    scene.capture_ends();
    assert_eq!(scene.calls().len(), 1, "{:?}", scene.calls());
    assert_eq!(scene.portal.registrations.get(), 1, "once per process");
}

/// A stored "yes": exactly one grab, Annota's windows hidden from the start.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn stored_yes_grabs_once_with_the_window_hidden() {
    let scene = scene("stored-yes", Some("yes"), Question::Deny);
    let window = crate::application::present_window(&scene.app);
    pump(100);
    scene.capture(CaptureMode::Screen);
    let calls = scene.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert_eq!(
        (calls[0].parent.as_str(), calls[0].visible_windows),
        ("", 0)
    );
    assert!(window.is_visible());
    scene.assert_portal_files_gone();
}

/// A store Annota can't read: grab as if allowed, so a real permission never flashes a window;
/// when that is refused, ask once with a window.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn unreadable_store_asks_after_the_first_refusal() {
    let scene = scene("unreadable", None, Question::Allow);
    scene.portal.unreadable.set(true);
    set_after_capture_to_clipboard();
    scene.capture(CaptureMode::Screen);
    let calls = scene.calls();
    assert_eq!(calls.len(), 3, "{calls:?}");
    assert_eq!(
        (calls[0].parent.as_str(), calls[0].visible_windows),
        ("", 0)
    );
    assert!(calls[1].parent.starts_with("x11:"), "{calls:?}");
    assert_eq!(calls[1].visible_windows, 1, "{calls:?}");
    assert_eq!(
        (calls[2].parent.as_str(), calls[2].visible_windows),
        ("", 0)
    );
    assert!(scene.dialog().is_none());
    assert!(scene.app.windows().is_empty());
}

/// Window mode is interactive: the portal asks nothing, so nothing waits for a question.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn window_mode_never_asks_first() {
    let scene = scene("window-mode", None, Question::CannotShow);
    set_after_capture_to_clipboard();
    scene.capture(CaptureMode::Window);
    let calls = scene.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(calls[0].interactive);
    assert_eq!(
        (calls[0].parent.as_str(), calls[0].visible_windows),
        ("", 0)
    );
}

/// A picker that ends without an image is reported as exactly that, and not as a failure.
///
/// Both halves matter, and each of them was wrong once. GNOME's screenshot UI answers response
/// code 2 whether the user closed it or it never opened, so "The capture failed" (until
/// 2026-11-13) claimed more than Annota knows, and saying nothing at all (2026-11-14) left the
/// user pressing a menu entry that did nothing visible. One portal call, no second attempt, and
/// one message that holds either way.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn a_picker_without_an_image_says_so() {
    let scene = scene("picker-closed", None, Question::CannotShow);
    scene.portal.picker_closed.set(true);
    set_after_capture_to_clipboard();
    scene.capture(CaptureMode::Window);
    let calls = scene.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(calls[0].interactive);
    assert!(calls[0].file.is_none(), "{calls:?}");
    let windows = scene.visible();
    assert_eq!(windows.len(), 1, "no window to carry the message");
    assert!(scene.shows_toast(&windows[0], NOTHING_CAPTURED));
    assert!(!scene.shows_toast(&windows[0], CAPTURE_FAILED));
    assert!(scene.dialog().is_none());
}

// --- Window capture through the ScreenCast portal (fix-window-capture-screencast) -----------

const SCREENCAST_XML: &str = r#"<node>
  <interface name="org.freedesktop.portal.ScreenCast">
    <method name="CreateSession">
      <arg type="a{sv}" direction="in"/><arg type="o" direction="out"/>
    </method>
    <method name="SelectSources">
      <arg type="o" direction="in"/><arg type="a{sv}" direction="in"/>
      <arg type="o" direction="out"/>
    </method>
    <method name="Start">
      <arg type="o" direction="in"/><arg type="s" direction="in"/>
      <arg type="a{sv}" direction="in"/><arg type="o" direction="out"/>
    </method>
    <method name="OpenPipeWireRemote">
      <arg type="o" direction="in"/><arg type="a{sv}" direction="in"/>
      <arg type="h" direction="out"/>
    </method>
    <property name="AvailableSourceTypes" type="u" access="read"/>
    <property name="AvailableCursorModes" type="u" access="read"/>
    <property name="version" type="u" access="read"/>
  </interface>
  <interface name="org.freedesktop.portal.Session">
    <method name="Close"/>
    <signal name="Closed"><arg type="a{sv}"/></signal>
    <property name="version" type="u" access="read"/>
  </interface>
</node>"#;

/// The ScreenCast half of the stand-in portal, answering as xdg-desktop-portal 1.21 with
/// xdg-desktop-portal-gnome 50 was measured to (design.md D6).
struct ScreenCast {
    /// Every call in order: "CreateSession", "SelectSources", "Start", "OpenPipeWireRemote",
    /// "Session.Close".
    log: Rc<RefCell<Vec<String>>>,
    /// `SelectSources`' options.
    selected: Rc<RefCell<HashMap<String, glib::Variant>>>,
    /// `Start`'s parent window.
    parent: Rc<RefCell<Option<String>>>,
}

impl ScreenCast {
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
}

/// Emits a request's `Response`.
fn respond(
    connection: &gio::DBusConnection,
    sender: &str,
    handle: &str,
    code: u32,
    results: HashMap<String, glib::Variant>,
) {
    connection
        .emit_signal(
            Some(sender),
            handle,
            "org.freedesktop.portal.Request",
            "Response",
            Some(&(code, results).to_variant()),
        )
        .unwrap();
}

/// The request or session path the portal builds from a token.
fn portal_path(kind: &str, sender: &str, token: &str) -> String {
    let unique = sender.trim_start_matches(':').replace('.', "_");
    format!("/org/freedesktop/portal/desktop/{kind}/{unique}/{token}")
}

/// Adds `org.freedesktop.portal.ScreenCast` to the scene's portal. `start`: the response code
/// `Start` answers with once the chooser "closes", or `None` for a chooser that never answers.
/// The PipeWire remote handed out is `/dev/null`, so a started stream never starts:
/// `pipewiresrc` waits its own 30 s for it (a hung-up socket was tried too and behaves the same).
fn add_screencast(scene: &Scene, source_types: u32, start: Option<u32>) -> ScreenCast {
    let connection = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).unwrap();
    let info = gio::DBusNodeInfo::for_xml(SCREENCAST_XML).unwrap();
    let log = Rc::new(RefCell::new(Vec::new()));
    let selected = Rc::new(RefCell::new(HashMap::new()));
    let parent = Rc::new(RefCell::new(None));
    let session_interface = info
        .lookup_interface("org.freedesktop.portal.Session")
        .unwrap();
    let (calls, options, start_parent) = (log.clone(), selected.clone(), parent.clone());
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
                        let session = portal_path(
                            "session",
                            &sender,
                            &token("session_handle_token").unwrap(),
                        );
                        let closed = calls.clone();
                        let id = Rc::new(RefCell::new(None));
                        let own_id = id.clone();
                        let registration = connection
                            .register_object(&session, &session_interface)
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
                        let handle =
                            portal_path("request", &sender, &token("handle_token").unwrap());
                        let path = glib::variant::ObjectPath::try_from(handle.clone()).unwrap();
                        invocation.return_value(Some(&(path,).to_variant()));
                        let results =
                            HashMap::from([(String::from("session_handle"), session.to_variant())]);
                        respond(&connection, &sender, &handle, 0, results);
                    }
                    "SelectSources" | "Start" => {
                        if method == "SelectSources" {
                            options.replace(opts.clone());
                        } else {
                            let window = parameters.child_value(1).str().unwrap().to_string();
                            start_parent.replace(Some(window));
                        }
                        let handle =
                            portal_path("request", &sender, &token("handle_token").unwrap());
                        let path = glib::variant::ObjectPath::try_from(handle.clone()).unwrap();
                        invocation.return_value(Some(&(path,).to_variant()));
                        let code = if method == "SelectSources" {
                            Some(0)
                        } else {
                            start
                        };
                        let Some(code) = code else {
                            return; // GNOME's chooser that never shows and never answers.
                        };
                        let mut results = HashMap::new();
                        if method == "Start" && code == 0 {
                            let stream: HashMap<String, glib::Variant> = HashMap::from([
                                (String::from("source_type"), 2u32.to_variant()),
                                (String::from("size"), (3072i32, 1728i32).to_variant()),
                            ]);
                            results.insert(
                                String::from("streams"),
                                vec![(42u32, stream)].to_variant(),
                            );
                        }
                        respond(&connection, &sender, &handle, code, results);
                    }
                    _ => unreachable!(),
                }
            },
        )
        .property(move |_, _, _, _, name| match name {
            "AvailableSourceTypes" => source_types.to_variant(),
            // HIDDEN | EMBEDDED, as GNOME 50 offers (METADATA is not used).
            "AvailableCursorModes" => 3u32.to_variant(),
            _ => 5u32.to_variant(),
        })
        .build()
        .unwrap();
    let _ = &scene.app;
    ScreenCast {
        log,
        selected,
        parent,
    }
}

impl Scene {
    fn start_with_pointer(&self, mode: CaptureMode, include_pointer: bool) {
        start(
            &self.app,
            Request {
                mode,
                delay: 0,
                include_pointer,
            },
        );
        pump(10);
    }

    /// Waits until the stand-in portal has seen `count` calls of `call`.
    fn screencast_reaches(&self, screencast: &ScreenCast, call: &str, count: usize) {
        let end = Instant::now() + Duration::from_secs(10);
        while screencast.count(call) < count {
            assert!(Instant::now() < end, "no {call}: {:?}", screencast.calls());
            pump(20);
        }
    }
}

/// Scenario "Chooser closed": `Start` answers 1 → no image, no window, no message; the session
/// is closed and the next capture is free to start (task 4.4).
// spec: capture / Window capture / Chooser closed
// spec: capture / Window capture / Window mode from the welcome page
// spec: capture / Cursor inclusion / Pointer in a window capture
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn a_closed_chooser_is_a_silent_cancel() {
    let scene = scene("chooser-closed", Some("yes"), Question::CannotShow);
    let screencast = add_screencast(&scene, 7, Some(1));
    scene.start_with_pointer(CaptureMode::Window, true);
    scene.capture_ends();
    scene.screencast_reaches(&screencast, "Session.Close", 1);
    assert_eq!(
        screencast.calls(),
        ["CreateSession", "SelectSources", "Start", "Session.Close"]
    );
    // design.md D1: one window, no restore token, the pointer embedded when asked for.
    assert_eq!(screencast.option_u32("types"), Some(2));
    assert_eq!(screencast.option_u32("persist_mode"), Some(0));
    assert_eq!(screencast.option_u32("cursor_mode"), Some(2));
    assert_eq!(
        screencast
            .selected
            .borrow()
            .get("multiple")
            .and_then(|v| v.get::<bool>()),
        Some(false)
    );
    // The waiting OSD was the chooser's parent (design.md D8).
    let parent = screencast.parent.borrow().clone().unwrap();
    assert!(parent.starts_with("x11:"), "{parent}");
    assert!(scene.calls().is_empty(), "no Screenshot portal call");
    assert!(super::window_wait::shown().is_none());
    assert!(scene.visible().is_empty(), "no window, no message");
    assert!(!scene.has_toast());
}

/// Response code 2 from the chooser is a failure, with the failure toast (task 4.4, design.md
/// D6), and never the Screenshot portal's "Nothing was captured".
// spec: capture / Cursor inclusion / Cursor excluded
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn a_failed_chooser_says_the_capture_failed() {
    let scene = scene("chooser-failed", Some("yes"), Question::CannotShow);
    let screencast = add_screencast(&scene, 7, Some(2));
    scene.start_with_pointer(CaptureMode::Window, false);
    scene.capture_ends();
    scene.screencast_reaches(&screencast, "Session.Close", 1);
    assert_eq!(
        screencast.option_u32("cursor_mode"),
        Some(1),
        "pointer hidden"
    );
    let windows = scene.visible();
    assert_eq!(windows.len(), 1, "a window to carry the message");
    assert!(scene.shows_toast(&windows[0], CAPTURE_FAILED));
    assert!(!scene.shows_toast(&windows[0], NOTHING_CAPTURED));
    assert!(scene.dialog().is_none(), "not a permission question");
}

/// Scenarios "Waiting for the chooser" and "Cancel while waiting" (task 4.7): GNOME never
/// answers `Start`. The OSD is shown, a second window request only raises it, Cancel closes the
/// session, nothing is said, and the next capture starts normally.
// spec: capture / Window capture / Waiting for the chooser
// spec: capture / Window capture / Cancel while waiting
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn cancelling_the_wait_closes_the_session() {
    let scene = scene("wait-cancel", Some("yes"), Question::CannotShow);
    let screencast = add_screencast(&scene, 7, None);
    scene.start_with_pointer(CaptureMode::Window, false);
    scene.screencast_reaches(&screencast, "Start", 1);
    let osd = super::window_wait::shown().expect("the waiting OSD");
    assert!(osd.is_visible());
    assert!(
        scene.visible() == std::slice::from_ref(&osd),
        "the OSD is the only Annota window"
    );
    shot(&osd, "window-wait-osd");
    // Sheet 02's ring: 72 x 72 with a 6 px stroke whose outer edge stays inside, one turn per
    // 1.2 s.
    use super::window_wait::{ring_angle, RING_RADIUS, RING_STROKE};
    const { assert!(RING_RADIUS + RING_STROKE / 2.0 <= 36.0) };
    assert_eq!(ring_angle(0), 0.0);
    assert!((ring_angle(300_000) - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    assert!((ring_angle(600_000) - std::f64::consts::PI).abs() < 1e-9);
    assert_eq!(ring_angle(1_200_000), 0.0);
    assert_eq!(ring_angle(2_400_000), 0.0);
    let cancel: gtk::Button = find(&osd, "cancel_button").expect("Cancel");
    assert!(cancel.has_focus(), "Cancel has the default focus");

    // A second window-capture request raises the OSD and starts nothing new.
    scene.start_with_pointer(CaptureMode::Window, false);
    pump(100);
    assert_eq!(screencast.count("CreateSession"), 1);
    assert!(BUSY.with(Cell::get));
    assert!(osd.is_visible(), "the OSD stays up");
    assert!(
        super::window_wait::shown().as_ref() == Some(&osd),
        "the same OSD, raised"
    );

    cancel.emit_clicked();
    scene.capture_ends();
    scene.screencast_reaches(&screencast, "Session.Close", 1);
    assert!(!BUSY.with(Cell::get));
    assert!(super::window_wait::shown().is_none());
    assert!(scene.visible().is_empty());
    assert!(!scene.has_toast(), "no message");

    // A following capture starts normally; this time the OSD is closed (as Esc and the
    // window's close request do).
    scene.start_with_pointer(CaptureMode::Window, false);
    scene.screencast_reaches(&screencast, "Start", 2);
    let osd = super::window_wait::shown().expect("the waiting OSD again");
    osd.close();
    scene.capture_ends();
    scene.screencast_reaches(&screencast, "Session.Close", 2);
    assert!(!scene.has_toast());
}

/// Scenario "No window source": a ScreenCast portal that offers only monitors leaves window mode
/// on the Screenshot portal's interactive UI, with no waiting OSD (design.md D6).
// spec: capture / Window capture / No window source
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn without_a_window_source_the_screenshot_ui_is_used() {
    let scene = scene("no-window-source", None, Question::CannotShow);
    let screencast = add_screencast(&scene, 1, Some(0));
    set_after_capture_to_clipboard();
    scene.capture(CaptureMode::Window);
    assert!(screencast.calls().is_empty(), "{:?}", screencast.calls());
    let calls = scene.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(calls[0].interactive);
}

/// Scenario "No frame arrives": a window is chosen (code 0) but the stream never starts. The
/// failure toast comes at the deadline although `pipewiresrc` still blocks its worker thread, so
/// the main loop never waits for it. The OSD went at the answer, and the session closes at the
/// deadline too, without waiting for the worker (the one exception to design.md D2's order), so
/// GNOME's sharing indicator goes with the toast. Measured here with GStreamer 1.28:
/// `pipewiresrc` waits 30 s for a stream that never starts, sometimes twice; the worker lets go
/// of the pipeline and the fd only then.
// spec: capture / Window capture / No frame arrives
// spec: capture / Window capture / Waiting for the chooser
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn a_stream_without_a_frame_fails_without_blocking_annota() {
    let scene = scene("no-frame", Some("yes"), Question::CannotShow);
    let screencast = add_screencast(&scene, 7, Some(0));
    scene.start_with_pointer(CaptureMode::Window, false);
    scene.screencast_reaches(&screencast, "OpenPipeWireRemote", 1);
    assert!(
        super::window_wait::shown().is_none(),
        "the OSD went at the answer"
    );
    let started = Instant::now();
    scene.capture_ends();
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
    let windows = scene.visible();
    assert_eq!(windows.len(), 1);
    assert!(scene.shows_toast(&windows[0], CAPTURE_FAILED));
    // At the deadline, not 30-60 s later when `pipewiresrc` gives up.
    let end = Instant::now() + Duration::from_secs(2);
    while screencast.count("Session.Close") == 0 {
        assert!(
            Instant::now() < end,
            "the session was not closed with the toast"
        );
        pump(50);
    }
    assert_eq!(
        screencast.calls(),
        [
            "CreateSession",
            "SelectSources",
            "Start",
            "OpenPipeWireRemote",
            "Session.Close"
        ]
    );
}

/// Scenario "Editor unavailable", its THEN: the fallback that `deliver` falls through to saves
/// the capture as PNG into the save folder, beside a file of the same name instead of over it, and
/// the toast names the new file (review of fix-window-capture-screencast, task 7.5). The WHEN, an
/// editor that cannot be built, is not provoked here.
// spec: capture / After-capture destination / Editor unavailable
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn a_capture_the_editor_cannot_take_is_saved_beside_not_over() {
    let scene = scene("editor-unavailable", Some("yes"), Question::CannotShow);
    let folder = scene.dir.join("save");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("Shot.png"), b"keep").unwrap();
    let settings = gio::Settings::new(config::APP_ID);
    settings
        .set_string("save-folder", folder.to_str().unwrap())
        .unwrap();
    settings.set_string("filename-pattern", "Shot").unwrap();
    let texture = gdk::MemoryTexture::new(
        4,
        3,
        gdk::MemoryFormat::B8g8r8a8Premultiplied,
        &glib::Bytes::from_owned(vec![255u8; 4 * 4 * 3]),
        16,
    );
    super::delivery::save_instead(
        &scene.app,
        texture.upcast_ref(),
        &settings,
        super::delivery::Source::Window,
        "Window captured \u{b7} 4 \u{d7} 3",
    );
    pump(100);
    assert_eq!(std::fs::read(folder.join("Shot.png")).unwrap(), b"keep");
    let saved = gdk::Texture::from_filename(folder.join("Shot (2).png")).unwrap();
    assert_eq!((saved.width(), saved.height()), (4, 3));
    let windows = scene.visible();
    assert_eq!(windows.len(), 1, "a window to carry the toast");
    assert!(scene.shows_toast(
        &windows[0],
        "Window captured \u{b7} 4 \u{d7} 3 \u{b7} Saved as Shot (2).png"
    ));
}

// --- Global shortcuts through the GlobalShortcuts portal (fix-portal-shortcut-bindings) ------

const GLOBAL_SHORTCUTS_XML: &str = r#"<node>
  <interface name="org.freedesktop.portal.GlobalShortcuts">
    <method name="CreateSession">
      <arg type="a{sv}" direction="in"/><arg type="o" direction="out"/>
    </method>
    <method name="BindShortcuts">
      <arg type="o" direction="in"/><arg type="a(sa{sv})" direction="in"/>
      <arg type="s" direction="in"/><arg type="a{sv}" direction="in"/>
      <arg type="o" direction="out"/>
    </method>
    <method name="ListShortcuts">
      <arg type="o" direction="in"/><arg type="a{sv}" direction="in"/>
      <arg type="o" direction="out"/>
    </method>
    <signal name="Activated">
      <arg type="o"/><arg type="s"/><arg type="t"/><arg type="a{sv}"/>
    </signal>
    <signal name="Deactivated">
      <arg type="o"/><arg type="s"/><arg type="t"/><arg type="a{sv}"/>
    </signal>
    <signal name="ShortcutsChanged">
      <arg type="o"/><arg type="a(sa{sv})"/>
    </signal>
    <property name="version" type="u" access="read"/>
  </interface>
  <interface name="org.freedesktop.portal.Session">
    <method name="Close"/>
    <signal name="Closed"><arg type="a{sv}"/></signal>
    <property name="version" type="u" access="read"/>
  </interface>
</node>"#;

/// A stand-in for GNOME Shell's keybinding schema: `screenshot` holds `Shift+Print`, as on GNOME
/// 50 (design.md D7). Nothing of the real desktop is read.
const SHELL_KEYBINDINGS_XML: &str = r#"<schemalist>
  <schema id="org.gnome.shell.keybindings" path="/org/gnome/shell/keybindings/">
    <key name="screenshot" type="as">
      <default><![CDATA[['<Shift>Print']]]></default>
      <summary>Take a screenshot</summary>
    </key>
    <key name="screenshot-window" type="as">
      <default><![CDATA[['<Alt>Print']]]></default>
      <summary>Take a screenshot of a window</summary>
    </key>
  </schema>
</schemalist>"#;

/// The GlobalShortcuts half of the stand-in portal, answering as GNOME 50's version-1 portal was
/// measured to (design.md "Context"): the bind returns the triggers the desktop keeps, whatever
/// Annota prefers.
struct GlobalShortcutsPortal {
    /// What the desktop has bound: (id, trigger description). Bind and list answer with it.
    bound: Rc<RefCell<Vec<(&'static str, &'static str)>>>,
    /// Calls in order: "CreateSession", "BindShortcuts", "ListShortcuts", "Session.Close".
    log: Rc<RefCell<Vec<String>>>,
    /// The preferred triggers of every bind: (id, preferred trigger).
    preferred: Rc<RefCell<Vec<(String, String)>>>,
    /// Annota's bus name and session, for the signals.
    client: Rc<RefCell<Option<(String, String)>>>,
    /// `BindShortcuts` answers with response code 2 and no results (D8).
    bind_error: Rc<Cell<bool>>,
}

fn shortcut_list(bound: &[(&str, &str)]) -> glib::Variant {
    bound
        .iter()
        .map(|(id, trigger)| {
            let info: HashMap<String, glib::Variant> = HashMap::from([
                (String::from("description"), id.to_variant()),
                (String::from("trigger_description"), trigger.to_variant()),
            ]);
            (id.to_string(), info)
        })
        .collect::<Vec<_>>()
        .to_variant()
}

fn add_global_shortcuts(bound: &[(&'static str, &'static str)]) -> GlobalShortcutsPortal {
    let connection = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).unwrap();
    let info = gio::DBusNodeInfo::for_xml(GLOBAL_SHORTCUTS_XML).unwrap();
    let session_interface = info
        .lookup_interface("org.freedesktop.portal.Session")
        .unwrap();
    let portal = GlobalShortcutsPortal {
        bound: Rc::new(RefCell::new(bound.to_vec())),
        log: Rc::new(RefCell::new(Vec::new())),
        preferred: Rc::new(RefCell::new(Vec::new())),
        client: Rc::new(RefCell::new(None)),
        bind_error: Rc::new(Cell::new(false)),
    };
    let (answer, log, preferred, client, bind_error) = (
        portal.bound.clone(),
        portal.log.clone(),
        portal.preferred.clone(),
        portal.client.clone(),
        portal.bind_error.clone(),
    );
    connection
        .register_object(
            "/org/freedesktop/portal/desktop",
            &info
                .lookup_interface("org.freedesktop.portal.GlobalShortcuts")
                .unwrap(),
        )
        .method_call(
            move |connection, sender, _, _, method, parameters, invocation| {
                let sender = sender.unwrap().to_string();
                log.borrow_mut().push(method.to_string());
                let last = parameters.n_children() - 1;
                let opts: HashMap<String, glib::Variant> =
                    parameters.child_value(last).get().unwrap();
                let token = |name: &str| opts.get(name).and_then(|v| v.get::<String>());
                let handle = portal_path("request", &sender, &token("handle_token").unwrap());
                let path = glib::variant::ObjectPath::try_from(handle.clone()).unwrap();
                let mut results: HashMap<String, glib::Variant> = HashMap::new();
                let mut code = 0;
                match method {
                    "CreateSession" => {
                        let session = portal_path(
                            "session",
                            &sender,
                            &token("session_handle_token").unwrap(),
                        );
                        let closed = log.clone();
                        connection
                            .register_object(&session, &session_interface)
                            .method_call(move |_, _, _, _, _, _, invocation| {
                                closed.borrow_mut().push(String::from("Session.Close"));
                                invocation.return_value(None);
                            })
                            .property(|_, _, _, _, _| 1u32.to_variant())
                            .build()
                            .unwrap();
                        client.replace(Some((sender.clone(), session.clone())));
                        results.insert(String::from("session_handle"), session.to_variant());
                    }
                    "BindShortcuts" => {
                        let asked: Vec<(String, HashMap<String, glib::Variant>)> =
                            parameters.child_value(1).get().unwrap();
                        for (id, info) in asked {
                            let trigger = info
                                .get("preferred_trigger")
                                .and_then(|v| v.get::<String>())
                                .unwrap_or_default();
                            preferred.borrow_mut().push((id, trigger));
                        }
                        if bind_error.get() {
                            code = 2;
                        } else {
                            results
                                .insert(String::from("shortcuts"), shortcut_list(&answer.borrow()));
                        }
                    }
                    "ListShortcuts" => {
                        results.insert(String::from("shortcuts"), shortcut_list(&answer.borrow()));
                    }
                    _ => unreachable!(),
                }
                invocation.return_value(Some(&(path,).to_variant()));
                respond(&connection, &sender, &handle, code, results);
            },
        )
        .property(|_, _, _, _, _| 1u32.to_variant())
        .build()
        .unwrap();
    portal
}

impl GlobalShortcutsPortal {
    fn count(&self, call: &str) -> usize {
        self.log.borrow().iter().filter(|c| *c == call).count()
    }

    fn emit(&self, signal: &str, body: glib::Variant) {
        let connection = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).unwrap();
        let (sender, _) = self.client.borrow().clone().expect("a session");
        connection
            .emit_signal(
                Some(&sender),
                "/org/freedesktop/portal/desktop",
                "org.freedesktop.portal.GlobalShortcuts",
                signal,
                Some(&body),
            )
            .unwrap();
    }

    fn session(&self) -> glib::variant::ObjectPath {
        let (_, session) = self.client.borrow().clone().expect("a session");
        glib::variant::ObjectPath::try_from(session).unwrap()
    }

    /// GNOME Settings rebinds: `ShortcutsChanged` with this list.
    fn changed(&self, bound: &[(&str, &str)]) {
        let body =
            glib::Variant::tuple_from_iter([self.session().to_variant(), shortcut_list(bound)]);
        self.emit("ShortcutsChanged", body);
    }

    /// The user presses the trigger bound to `id`.
    fn activate(&self, id: &str) {
        let options: HashMap<String, glib::Variant> = HashMap::new();
        let body = (self.session(), id, 0u64, options).to_variant();
        self.emit("Activated", body);
    }
}

/// The settings of the bug report (proposal.md): what Annota would *like*.
fn set_reported_settings() {
    let settings = gio::Settings::new(config::APP_ID);
    for (key, accel) in [
        ("shortcut-region", "<Control>p"),
        ("shortcut-window", "<Control>w"),
        ("shortcut-screen", "<Control>q"),
    ] {
        settings.set_string(key, accel).unwrap();
    }
}

/// Starts Annota's portal shortcuts and waits until the portal's answer is in.
fn bind_through_the_portal(scene: &Scene) {
    shortcuts::start_portal_shortcuts(&scene.app);
    wait_for(
        || shortcuts::shown_trigger(CaptureMode::Region).trigger != ShownTrigger::Pending,
        "the portal's answer",
    );
}

fn wait_for(done: impl Fn() -> bool, what: &str) {
    let end = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < end, "no {what}");
        pump(20);
    }
}

/// The labels in a keycap line or caption, in order.
fn texts(widget: &gtk::Widget) -> Vec<String> {
    let mut out = Vec::new();
    let mut child = widget.first_child();
    while let Some(c) = child {
        if let Some(label) = c.downcast_ref::<gtk::Label>() {
            if c.is_visible() {
                out.push(label.text().to_string());
            }
        }
        child = c.next_sibling();
    }
    out
}

fn has_warning(keys: &gtk::Widget) -> bool {
    holds(keys, &|w| {
        w.downcast_ref::<gtk::Image>().is_some_and(|i| {
            i.is_visible() && i.icon_name().as_deref() == Some("dialog-warning-symbolic")
        })
    })
}

/// The Preferences dialog over `window`, on its Capture page.
fn open_preferences(window: &adw::ApplicationWindow) -> adw::PreferencesDialog {
    crate::preferences::present(window);
    pump(200);
    let dialog: adw::PreferencesDialog = window
        .visible_dialog()
        .and_downcast()
        .expect("the Preferences dialog");
    let page: adw::PreferencesPage = find(&dialog, "capture_page").expect("Capture page");
    dialog.set_visible_page(&page);
    pump(200);
    dialog
}

/// The bug report, end to end: the settings say Ctrl+P / Ctrl+W / Ctrl+Q, GNOME has bound
/// Shift+Ctrl+P and Shift+Print and nothing for Window. Every surface shows GNOME's triggers, and
/// GNOME's Shift+Print is marked as GNOME's own (tasks 3.1, 3.4, 4.1, 4.3, 4.4, 4.6).
// spec: capture / Global capture hotkey / Portal keeps an earlier trigger
// spec: capture / Global capture hotkey / Shortcut the portal has not bound
// spec: capture / Global capture hotkey / Shortcuts are managed by GNOME
// spec: capture / Global capture hotkey / Trigger taken by GNOME
// spec: capture / Reversible takeover of GNOME's screenshot keys / No takeover on the GlobalShortcuts portal path
// spec: app-shell / Welcome page / Keycaps show the portal's triggers
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn every_surface_shows_the_portals_triggers() {
    let scene = scene_with_schemas(
        "portal-triggers",
        Some("yes"),
        Question::CannotShow,
        &[("org.gnome.shell.gschema.xml", SHELL_KEYBINDINGS_XML)],
    );
    set_reported_settings();
    let portal = add_global_shortcuts(&[
        ("region", "Press <Shift><Control>p"),
        ("screen", "Press <Shift>Print"),
    ]);
    bind_through_the_portal(&scene);

    // What Annota asked for, and what it got instead.
    assert_eq!(
        *portal.preferred.borrow(),
        [
            (String::from("region"), String::from("CTRL+p")),
            (String::from("window"), String::from("CTRL+w")),
            (String::from("screen"), String::from("CTRL+q")),
        ]
    );
    let screen = shortcuts::shown_trigger(CaptureMode::Screen);
    assert_eq!(
        screen.trigger,
        ShownTrigger::Accel(String::from("<Shift>Print"))
    );
    assert_eq!(
        screen
            .taken_by
            .as_ref()
            .map(shortcuts::TakenBy::text)
            .as_deref(),
        Some("Used by GNOME: Take a screenshot")
    );
    let region = shortcuts::shown_trigger(CaptureMode::Region);
    assert_eq!(
        region.trigger,
        ShownTrigger::Accel(String::from("<Shift><Control>p"))
    );
    assert_eq!(region.taken_by, None);
    assert_eq!(
        shortcuts::shown_trigger(CaptureMode::Window).trigger,
        ShownTrigger::NotSet
    );
    // GNOME Shell's own bindings stay as they were.
    let shell = gio::Settings::new("org.gnome.shell.keybindings");
    assert_eq!(shell.strv("screenshot").to_vec(), ["<Shift>Print"]);
    assert_eq!(shell.strv("screenshot-window").to_vec(), ["<Alt>Print"]);

    // Welcome page (mockups 05, 06c).
    let window = crate::application::present_window(&scene.app);
    pump(200);
    let keys = |mode: &str| -> gtk::Widget {
        find(&window, &format!("welcome_{mode}_keys")).expect("the keycap line")
    };
    assert_eq!(texts(&keys("region")), ["Shift", "Ctrl", "P"]);
    assert!(texts(&keys("window")).is_empty(), "no keycaps for Window");
    assert_eq!(texts(&keys("screen")), ["Shift", "Print"]);
    assert!(has_warning(&keys("screen")) && !has_warning(&keys("region")));
    let card: gtk::Button = find(&window, "welcome_screen_card").unwrap();
    assert_eq!(
        card.tooltip_text().as_deref(),
        Some("Used by GNOME: Take a screenshot")
    );
    let region_card: gtk::Button = find(&window, "welcome_region_card").unwrap();
    assert_eq!(
        region_card.tooltip_text().as_deref(),
        Some("Capture a region")
    );
    assert!(
        region_card.has_focus(),
        "the Region card keeps the initial focus"
    );
    for mode in ["region", "window", "screen"] {
        assert!(
            keys(mode).height() >= 22,
            "{mode}: the line keeps its height"
        );
    }
    shot(&window, "welcome-portal-triggers");

    // Launcher (mockups 01, 06b): Screen selected.
    gio::Settings::new(config::APP_ID)
        .set_enum("capture-mode", CaptureMode::Screen.index())
        .unwrap();
    let menu: gtk::MenuButton = find(&window, "capture_menu_button").unwrap();
    let lists_before = portal.count("ListShortcuts");
    menu.popup();
    wait_for(
        || portal.count("ListShortcuts") == lists_before + 1,
        "ListShortcuts when the launcher shows",
    );
    pump(100);
    let caption = |mode: &str| -> gtk::Widget {
        find(&window, &format!("launcher_{mode}_caption")).expect("the caption")
    };
    let screen_button: gtk::ToggleButton = find(&window, "launcher_screen_button").unwrap();
    screen_button.set_active(true);
    pump(50);
    assert_eq!(texts(&caption("region")), ["Shift+Ctrl+P"]);
    assert!(
        !caption("window").is_visible(),
        "no caption line for Window"
    );
    assert_eq!(texts(&caption("screen")), ["Shift+Print"]);
    assert!(has_warning(&caption("screen")) && !has_warning(&caption("region")));
    assert_eq!(
        screen_button.tooltip_text().as_deref(),
        Some("Used by GNOME: Take a screenshot")
    );
    let capture_accel: gtk::Label = find(&window, "launcher_capture_accel").unwrap();
    assert!(
        !capture_accel.is_visible(),
        "no key hint for a key GNOME keeps"
    );
    let region_button: gtk::ToggleButton = find(&window, "launcher_region_button").unwrap();
    region_button.set_active(true);
    pump(50);
    assert!(capture_accel.is_visible());
    assert_eq!(capture_accel.text(), "Shift+Ctrl+P");
    assert_eq!(region_button.tooltip_text().as_deref(), Some("Region"));
    for mode in ["region", "window", "screen"] {
        let toggle: gtk::Widget = find(&window, &format!("launcher_{mode}_button")).unwrap();
        assert!(toggle.height() >= 76, "{mode}: the toggle keeps its height");
    }
    let popover: gtk::Popover = find(&window, "capture_launcher").unwrap();
    assert!(popover.is_visible());
    shot(&popover, "launcher-portal-triggers");
    popover.popdown();
    pump(100);

    // Preferences › Capture (mockups 02, 03a, 06a).
    let lists_before = portal.count("ListShortcuts");
    let dialog = open_preferences(&window);
    wait_for(
        || portal.count("ListShortcuts") > lists_before,
        "ListShortcuts when Preferences › Capture maps",
    );
    let group: adw::PreferencesGroup = find(&dialog, "shortcuts_group").unwrap();
    assert_eq!(
        group.description().as_deref(),
        Some("GNOME manages these shortcuts. Change them in GNOME Settings.")
    );
    let open: gtk::Button = find(&dialog, "open_gnome_settings_button").unwrap();
    assert_eq!(
        open.is_visible(),
        gio::DesktopAppInfo::new("org.gnome.Settings.desktop").is_some()
    );
    assert_eq!(open.label().as_deref(), Some("Open GNOME Settings"));
    let row =
        |mode: &str| -> adw::ActionRow { find(&dialog, &format!("shortcut_{mode}_row")).unwrap() };
    let keycaps = |mode: &str| -> gtk::ShortcutLabel {
        find(&dialog, &format!("shortcut_{mode}_label")).unwrap()
    };
    let text =
        |mode: &str| -> gtk::Label { find(&dialog, &format!("shortcut_{mode}_text")).unwrap() };
    let chevron =
        |mode: &str| -> gtk::Image { find(&dialog, &format!("shortcut_{mode}_chevron")).unwrap() };
    for mode in ["region", "window", "screen"] {
        assert!(!row(mode).is_activatable(), "{mode}: read-only");
        assert!(!chevron(mode).is_visible(), "{mode}: no chevron");
    }
    assert_eq!(
        keycaps("region").accelerator().unwrap_or_default().as_str(),
        "<Shift><Control>p"
    );
    assert!(keycaps("region").is_visible() && !text("region").is_visible());
    assert_eq!(row("region").subtitle().as_deref().unwrap_or_default(), "");
    assert!(!keycaps("window").is_visible());
    assert!(text("window").is_visible());
    assert_eq!(text("window").text(), "Not set");
    assert_eq!(
        keycaps("screen").accelerator().unwrap_or_default().as_str(),
        "<Shift>Print"
    );
    assert_eq!(
        row("screen").subtitle().as_deref(),
        Some("Used by GNOME: Take a screenshot")
    );
    let warning: gtk::Image = find(&dialog, "shortcut_screen_warning").unwrap();
    assert!(warning.is_visible() && warning.has_css_class("warning"));
    let region_warning: gtk::Image = find(&dialog, "shortcut_region_warning").unwrap();
    assert!(!region_warning.is_visible());
    for setting in ["Ctrl+P", "Ctrl+W", "Ctrl+Q"] {
        assert!(
            !holds(dialog.upcast_ref(), &|w| w
                .downcast_ref::<gtk::Label>()
                .is_some_and(|l| l.is_visible() && l.text() == setting)),
            "{setting} shown"
        );
    }
    shot(&dialog, "prefs-portal-triggers");

    // Activating a row opens no Set Shortcut dialog.
    row("region").emit_by_name::<()>("activated", &[]);
    pump(200);
    assert!(
        !holds(window.upcast_ref(), &|w| w.buildable_id().as_deref()
            == Some("shortcut_dialog")),
        "a Set Shortcut dialog opened on the portal path"
    );
    assert_eq!(window.visible_dialog().as_ref(), Some(dialog.upcast_ref()));
}

/// GNOME Settings rebinds `screen`: two `ShortcutsChanged` in a burst, the first without
/// `screen`. Annota shows the final list and never the gap; `ListShortcuts` on open follows a
/// rebind the signal missed (task 3.2); the rebound key starts the capture.
// spec: capture / Global capture hotkey / Trigger changed in GNOME Settings
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn a_rebind_in_gnome_settings_reaches_every_surface() {
    let scene = scene("portal-rebind", Some("yes"), Question::CannotShow);
    set_after_capture_to_clipboard();
    set_reported_settings();
    let portal = add_global_shortcuts(&[
        ("region", "Press <Shift><Control>p"),
        ("window", "Press <Alt>Print"),
        ("screen", "Press <Shift>Print"),
    ]);
    // The application's `capture` action, which `Activated` runs (application.rs).
    let capture = gio::SimpleAction::new("capture", Some(glib::VariantTy::STRING));
    let app = scene.app.clone();
    capture.connect_activate(move |_, parameter| {
        if let Some(mode) = parameter
            .and_then(|p| p.get::<String>())
            .and_then(|m| CaptureMode::parse(&m))
        {
            start(
                &app,
                Request {
                    mode,
                    delay: 0,
                    include_pointer: false,
                },
            );
        }
    });
    scene.app.add_action(&capture);
    bind_through_the_portal(&scene);
    let window = crate::application::present_window(&scene.app);
    pump(200);
    let screen_keys: gtk::Widget = find(&window, "welcome_screen_keys").unwrap();
    assert_eq!(texts(&screen_keys), ["Shift", "Print"]);

    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = seen.clone();
    shortcuts::connect_bound_changed(move || {
        log.borrow_mut()
            .push(shortcuts::shown_trigger(CaptureMode::Screen).trigger);
        glib::ControlFlow::Continue
    });
    portal.changed(&[
        ("region", "Press <Shift><Control>p"),
        ("window", "Press <Alt>Print"),
    ]);
    portal.changed(&[
        ("region", "Press <Shift><Control>p"),
        ("window", "Press <Alt>Print"),
        ("screen", "Press <Shift><Super>s"),
    ]);
    let rebound = ShownTrigger::Accel(String::from("<Shift><Super>s"));
    wait_for(
        || shortcuts::shown_trigger(CaptureMode::Screen).trigger == rebound,
        "the rebound trigger",
    );
    pump(400);
    assert_eq!(
        *seen.borrow(),
        std::slice::from_ref(&rebound),
        "the gap is never shown"
    );
    assert_eq!(texts(&screen_keys), ["Shift", "Super", "S"]);

    // A rebind the signal did not report: Preferences asks the portal when it opens.
    *portal.bound.borrow_mut() = vec![
        ("region", "Press <Shift><Control>F12"),
        ("window", "Press <Alt>Print"),
        ("screen", "Press <Shift><Super>s"),
    ];
    let dialog = open_preferences(&window);
    let region_keys: gtk::ShortcutLabel = find(&dialog, "shortcut_region_label").unwrap();
    wait_for(
        || region_keys.accelerator().as_deref() == Some("<Shift><Control>F12"),
        "the listed trigger in Preferences",
    );
    let region_card: gtk::Widget = find(&window, "welcome_region_keys").unwrap();
    assert_eq!(texts(&region_card), ["Shift", "Ctrl", "F12"]);
    // refresh_bound() itself, as the launcher calls it.
    *portal.bound.borrow_mut() = vec![("screen", "Press <Shift><Super>s")];
    shortcuts::refresh_bound();
    wait_for(
        || shortcuts::shown_trigger(CaptureMode::Region).trigger == ShownTrigger::NotSet,
        "the store following ListShortcuts",
    );
    dialog.close();
    pump(100);

    // Pressing the rebound key: the portal says `Activated screen`, a screen capture runs.
    portal.activate("screen");
    let end = Instant::now() + Duration::from_secs(10);
    while scene.calls().is_empty() {
        assert!(Instant::now() < end, "no capture");
        pump(20);
    }
    scene.capture_ends();
    let calls = scene.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(!calls[0].interactive, "a screen capture, not the region UI");
}

/// Without the GlobalShortcuts portal (GNOME 46) the cards show the settings and follow them.
// spec: app-shell / Welcome page / Shortcut keycaps follow the settings
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn without_the_portal_the_keycaps_follow_the_settings() {
    let scene = scene("fallback-keycaps", None, Question::CannotShow);
    shortcuts::start_portal_shortcuts(&scene.app);
    let window = crate::application::present_window(&scene.app);
    let keys: gtk::Widget = find(&window, "welcome_window_keys").unwrap();
    wait_for(
        || texts(&keys) == ["Alt", "Print"],
        "the configured Window keys",
    );
    gio::Settings::new(config::APP_ID)
        .set_string("shortcut-window", "<Super>w")
        .unwrap();
    pump(100);
    assert_eq!(texts(&keys), ["Super", "W"]);
    gio::Settings::new(config::APP_ID)
        .set_string("shortcut-window", "")
        .unwrap();
    pump(100);
    assert!(texts(&keys).is_empty());
}

/// D8: the portal answers the bind with an error. Nothing shows a trigger, none of the configured
/// accelerators either, and Preferences says GNOME has not reported them. `ListShortcuts` keeps
/// being asked when Preferences or the launcher opens; once it reports triggers, they show
/// (task 7.1).
// spec: capture / Global capture hotkey / Bind answered with an error
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn a_failed_bind_is_said_and_retried() {
    let scene = scene("portal-bind-error", Some("yes"), Question::CannotShow);
    set_reported_settings();
    let portal = add_global_shortcuts(&[]);
    portal.bind_error.set(true);
    shortcuts::start_portal_shortcuts(&scene.app);
    wait_for(shortcuts::unreported, "the failed bind");
    assert_eq!(portal.count("BindShortcuts"), 1);
    assert!(shortcuts::portal_active(), "the session stays open");
    for mode in CaptureMode::ALL {
        assert_eq!(
            shortcuts::shown_trigger(mode).trigger,
            ShownTrigger::Pending
        );
    }

    // Welcome page and launcher: no keycaps, no configured accelerator.
    let window = crate::application::present_window(&scene.app);
    pump(200);
    for mode in ["region", "window", "screen"] {
        let keys: gtk::Widget = find(&window, &format!("welcome_{mode}_keys")).unwrap();
        assert!(texts(&keys).is_empty(), "{mode}: no keycaps");
    }
    let menu: gtk::MenuButton = find(&window, "capture_menu_button").unwrap();
    let lists_before = portal.count("ListShortcuts");
    menu.popup();
    wait_for(
        || portal.count("ListShortcuts") == lists_before + 1,
        "ListShortcuts when the launcher shows",
    );
    pump(100);
    assert!(shortcuts::unreported(), "an empty list reports nothing");
    for mode in ["region", "window", "screen"] {
        let caption: gtk::Widget = find(&window, &format!("launcher_{mode}_caption")).unwrap();
        assert!(!caption.is_visible(), "{mode}: no caption");
    }
    let capture_accel: gtk::Label = find(&window, "launcher_capture_accel").unwrap();
    assert!(!capture_accel.is_visible(), "no key hint");
    let popover: gtk::Popover = find(&window, "capture_launcher").unwrap();
    popover.popdown();
    pump(100);

    // Preferences › Capture: the D8 line under the description, rows without a trigger.
    let dialog = open_preferences(&window);
    let group: adw::PreferencesGroup = find(&dialog, "shortcuts_group").unwrap();
    wait_for(
        || {
            group.description().as_deref()
                == Some(
                    "GNOME manages these shortcuts. Change them in GNOME Settings.\n\
                     GNOME has not reported these shortcuts.",
                )
        },
        "the unreported line",
    );
    for mode in ["region", "window", "screen"] {
        let keycaps: gtk::ShortcutLabel = find(&dialog, &format!("shortcut_{mode}_label")).unwrap();
        let text: gtk::Label = find(&dialog, &format!("shortcut_{mode}_text")).unwrap();
        let warning: gtk::Image = find(&dialog, &format!("shortcut_{mode}_warning")).unwrap();
        assert!(!keycaps.is_visible(), "{mode}: no keycaps");
        assert!(!text.is_visible(), "{mode}: no text");
        assert!(!warning.is_visible(), "{mode}: no icon");
    }
    for setting in ["Ctrl+P", "Ctrl+W", "Ctrl+Q"] {
        assert!(
            !holds(dialog.upcast_ref(), &|w| w
                .downcast_ref::<gtk::Label>()
                .is_some_and(|l| l.is_visible() && l.text() == setting)),
            "{setting} shown"
        );
    }
    shot(&dialog, "prefs-portal-unreported");
    dialog.close();
    pump(200);

    // GNOME reports triggers now: the next time Preferences opens, they show.
    *portal.bound.borrow_mut() = vec![
        ("region", "Press <Shift><Control>p"),
        ("screen", "Press <Shift><Control>F12"),
    ];
    let lists_before = portal.count("ListShortcuts");
    let dialog = open_preferences(&window);
    wait_for(
        || portal.count("ListShortcuts") > lists_before,
        "ListShortcuts when Preferences › Capture maps",
    );
    let region: gtk::ShortcutLabel = find(&dialog, "shortcut_region_label").unwrap();
    wait_for(
        || region.accelerator().as_deref() == Some("<Shift><Control>p"),
        "the listed trigger in Preferences",
    );
    assert!(!shortcuts::unreported());
    let group: adw::PreferencesGroup = find(&dialog, "shortcuts_group").unwrap();
    assert_eq!(
        group.description().as_deref(),
        Some("GNOME manages these shortcuts. Change them in GNOME Settings.")
    );
    let window_text: gtk::Label = find(&dialog, "shortcut_window_text").unwrap();
    assert_eq!(window_text.text(), "Not set");
    let screen: gtk::ShortcutLabel = find(&dialog, "shortcut_screen_label").unwrap();
    assert_eq!(screen.accelerator().as_deref(), Some("<Shift><Control>F12"));
    let keys: gtk::Widget = find(&window, "welcome_region_keys").unwrap();
    assert_eq!(texts(&keys), ["Shift", "Ctrl", "P"]);
    // No second bind: the session of the failed one was kept.
    assert_eq!(portal.count("BindShortcuts"), 1);
    assert_eq!(portal.count("CreateSession"), 1);
}

/// N2: Preferences opens while GNOME Settings rebinds. The `ListShortcuts` answer lands between
/// the burst's two `ShortcutsChanged` and holds the gap; it waits like a signal, and the gap is
/// never shown (task 7.2).
// spec: capture / Global capture hotkey / Trigger changed in GNOME Settings
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn a_refresh_inside_a_rebind_burst_shows_no_gap() {
    let scene = scene("portal-burst-refresh", Some("yes"), Question::CannotShow);
    set_reported_settings();
    let full = [
        ("region", "Press <Shift><Control>p"),
        ("window", "Press <Alt>Print"),
        ("screen", "Press <Shift>Print"),
    ];
    let gap = [
        ("region", "Press <Shift><Control>p"),
        ("window", "Press <Alt>Print"),
    ];
    let portal = add_global_shortcuts(&full);
    bind_through_the_portal(&scene);

    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = seen.clone();
    shortcuts::connect_bound_changed(move || {
        log.borrow_mut()
            .push(shortcuts::shown_trigger(CaptureMode::Screen).trigger);
        glib::ControlFlow::Continue
    });
    // The first signal of the burst, then a refresh that sees the same gap.
    *portal.bound.borrow_mut() = gap.to_vec();
    portal.changed(&gap);
    pump(20);
    let lists_before = portal.count("ListShortcuts");
    shortcuts::refresh_bound();
    wait_for(
        || portal.count("ListShortcuts") > lists_before,
        "ListShortcuts inside the burst",
    );
    pump(20);
    // The burst's last signal.
    let rebound = [
        ("region", "Press <Shift><Control>p"),
        ("window", "Press <Alt>Print"),
        ("screen", "Press <Shift><Super>s"),
    ];
    *portal.bound.borrow_mut() = rebound.to_vec();
    portal.changed(&rebound);
    let shown = ShownTrigger::Accel(String::from("<Shift><Super>s"));
    wait_for(
        || shortcuts::shown_trigger(CaptureMode::Screen).trigger == shown,
        "the rebound trigger",
    );
    pump(400);
    assert!(
        !seen
            .borrow()
            .iter()
            .any(|t| *t == ShownTrigger::NotSet || *t == ShownTrigger::Pending),
        "the gap was shown: {:?}",
        seen.borrow()
    );
    assert_eq!(seen.borrow().last(), Some(&shown));
}

/// N7: GNOME's keybinding settings change while Annota runs. The "Used by GNOME" mark follows on
/// the welcome page and in Preferences without any portal event (task 7.4).
// spec: capture / Global capture hotkey / Trigger taken by GNOME
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn a_change_in_gnomes_keybindings_moves_the_mark() {
    let scene = scene_with_schemas(
        "portal-taken-follows",
        Some("yes"),
        Question::CannotShow,
        &[("org.gnome.shell.gschema.xml", SHELL_KEYBINDINGS_XML)],
    );
    set_reported_settings();
    let _portal = add_global_shortcuts(&[
        ("region", "Press <Shift><Control>p"),
        ("screen", "Press <Shift>Print"),
    ]);
    bind_through_the_portal(&scene);
    let window = crate::application::present_window(&scene.app);
    pump(200);
    let keys: gtk::Widget = find(&window, "welcome_screen_keys").unwrap();
    let card: gtk::Button = find(&window, "welcome_screen_card").unwrap();
    assert!(has_warning(&keys));
    let dialog = open_preferences(&window);
    let row: adw::ActionRow = find(&dialog, "shortcut_screen_row").unwrap();
    let warning: gtk::Image = find(&dialog, "shortcut_screen_warning").unwrap();
    assert_eq!(
        row.subtitle().as_deref(),
        Some("Used by GNOME: Take a screenshot")
    );

    // GNOME's screenshot moves to another key: Shift+Print is free.
    let shell = gio::Settings::new("org.gnome.shell.keybindings");
    shell.set_strv("screenshot", ["<Super>F12"]).unwrap();
    wait_for(|| !has_warning(&keys), "the mark leaving the welcome card");
    assert_eq!(card.tooltip_text().as_deref(), Some("Capture the screen"));
    wait_for(|| !warning.is_visible(), "the mark leaving the row");
    assert_eq!(row.subtitle().as_deref().unwrap_or_default(), "");
    assert_eq!(texts(&keys), ["Shift", "Print"]);

    // A window screenshot takes Shift+Print: marked again, with that action's name.
    shell
        .set_strv("screenshot-window", ["<Shift>Print"])
        .unwrap();
    wait_for(|| has_warning(&keys), "the mark on the welcome card");
    wait_for(
        || row.subtitle().as_deref() == Some("Used by GNOME: Take a screenshot of a window"),
        "the mark on the row",
    );
    assert!(warning.is_visible());
    assert_eq!(
        card.tooltip_text().as_deref(),
        Some("Used by GNOME: Take a screenshot of a window")
    );
}
