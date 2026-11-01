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
    let open_question: OpenQuestion = Rc::new(RefCell::new(None));
    let (portal_store, portal_calls) = (store.clone(), Rc::clone(&calls));
    let (hold, hold_any, open) = (
        hold_question.clone(),
        hold_grab.clone(),
        open_question.clone(),
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
    let dir = std::env::temp_dir().join(format!("annota-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("schemas")).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    std::fs::copy(
        root.join("data/io.github.trosinde.Annota.gschema.xml"),
        dir.join("schemas/io.github.trosinde.Annota.gschema.xml"),
    )
    .unwrap();
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
