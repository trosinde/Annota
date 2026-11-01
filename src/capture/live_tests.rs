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
  <interface name="org.freedesktop.impl.portal.PermissionStore">
    <method name="Lookup">
      <arg type="s" direction="in"/><arg type="s" direction="in"/>
      <arg type="a{sas}" direction="out"/><arg type="v" direction="out"/>
    </method>
  </interface>
</node>"#;

/// What the user does with the question, if it can be shown at all.
#[derive(Clone, Copy, PartialEq)]
enum Question {
    Allow,
    Deny,
    /// Even with a parent window the question is not shown; nothing is stored.
    CannotShow,
}

/// One Screenshot call as the portal saw it.
#[derive(Clone, Debug)]
struct Call {
    parent: String,
    interactive: bool,
    visible_windows: usize,
    file: Option<PathBuf>,
}

struct Portal {
    store: Rc<RefCell<Option<&'static str>>>,
    /// `Lookup` fails, as it would inside a sandbox.
    unreadable: Rc<Cell<bool>>,
    calls: Rc<RefCell<Vec<Call>>>,
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
    let store = Rc::new(RefCell::new(stored));
    let calls = Rc::new(RefCell::new(Vec::new()));

    let unreadable = Rc::new(Cell::new(false));

    let (lookup_store, lookup_fails) = (store.clone(), unreadable.clone());
    connection
        .register_object(
            "/org/freedesktop/impl/portal/PermissionStore",
            &info
                .lookup_interface("org.freedesktop.impl.portal.PermissionStore")
                .unwrap(),
        )
        .method_call(move |_, _, _, _, _, _, invocation| {
            if lookup_fails.get() {
                invocation
                    .return_dbus_error("org.freedesktop.DBus.Error.AccessDenied", "Sandboxed");
                return;
            }
            match *lookup_store.borrow() {
                None => invocation.return_dbus_error(
                    "org.freedesktop.portal.Error.NotFound",
                    "No entry for screenshot",
                ),
                Some(value) => {
                    let entries =
                        HashMap::from([(config::APP_ID.to_string(), vec![value.to_string()])]);
                    invocation.return_value(Some(&(entries, 0u8.to_variant()).to_variant()));
                }
            }
        })
        .build()
        .unwrap();

    let (portal_store, portal_calls) = (store.clone(), Rc::clone(&calls));
    let (app, dir) = (app.clone(), dir.to_path_buf());
    let counter = Cell::new(0);
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
            let stored = *portal_store.borrow();
            let granted = interactive
                || match stored {
                    Some("yes") => true,
                    Some(_) => false,
                    // No parent: GNOME Shell's "Only the focused app …", with Annota's windows
                    // hidden the focus is on another app. Nothing is stored.
                    None if parent.is_empty() || question == Question::CannotShow => false,
                    None => {
                        let allow = question == Question::Allow;
                        *portal_store.borrow_mut() = Some(if allow { "yes" } else { "no" });
                        allow
                    }
                };

            let file = granted.then(|| {
                counter.set(counter.get() + 1);
                let path = dir.join(format!("portal-{}.png", counter.get()));
                write_png(&path);
                path
            });
            portal_calls.borrow_mut().push(Call {
                parent,
                interactive,
                visible_windows: super::visible_windows(&app).len(),
                file: file.clone(),
            });

            let unique = sender.trim_start_matches(':').replace('.', "_");
            let handle = format!("/org/freedesktop/portal/desktop/request/{unique}/{token}");
            let path = glib::variant::ObjectPath::try_from(handle.clone()).unwrap();
            invocation.return_value(Some(&(path,).to_variant()));
            let mut results: HashMap<String, glib::Variant> = HashMap::new();
            if let Some(file) = &file {
                let uri = gio::File::for_path(file).uri().to_string();
                results.insert(String::from("uri"), uri.to_variant());
            }
            let code = if granted { 0u32 } else { 2u32 };
            connection
                .emit_signal(
                    Some(&sender),
                    &handle,
                    "org.freedesktop.portal.Request",
                    "Response",
                    Some(&(code, results).to_variant()),
                )
                .unwrap();
        })
        .property(|_, _, _, _, _| 2u32.to_variant())
        .build()
        .unwrap();
    Portal {
        store,
        unreadable,
        calls,
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
        start(
            &self.app,
            Request {
                mode,
                delay: 0,
                include_pointer: false,
            },
        );
        let end = Instant::now() + Duration::from_secs(20);
        pump(10);
        while BUSY.with(Cell::get) {
            assert!(Instant::now() < end, "the capture did not end");
            pump(20);
        }
        pump(300);
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

    fn assert_portal_files_gone(&self) {
        for file in self.calls().iter().filter_map(|c| c.file.clone()) {
            assert!(!file.exists(), "{file:?} left behind");
        }
    }
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
    assert_eq!(*scene.portal.store.borrow(), Some("yes"));
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

/// The user chooses Deny: the denied dialog, on a stored "no", with a working copy button.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn first_run_deny() {
    let scene = scene("deny", None, Question::Deny);
    scene.capture(CaptureMode::Screen);
    let calls = scene.calls();
    assert_eq!(calls.len(), 1, "no second try after a Deny: {calls:?}");
    assert!(calls[0].parent.starts_with("x11:"), "{calls:?}");
    assert_eq!(*scene.portal.store.borrow(), Some("no"));
    let dialog = scene.dialog().expect("the denied dialog");
    shot(&dialog, "permission-denied");
    let copy = gtk::prelude::GtkWindowExt::focus(
        &dialog
            .root()
            .and_downcast::<gtk::Window>()
            .expect("the dialog's window"),
    )
    .and_downcast::<gtk::Button>()
    .expect("Copy Command has the focus");
    copy.emit_clicked();
    let read = Rc::new(RefCell::new(None));
    let clipboard = gdk::Display::default().unwrap().clipboard();
    glib::spawn_future_local(glib::clone!(
        #[strong]
        read,
        async move {
            read.replace(clipboard.read_text_future().await.ok().flatten());
        }
    ));
    pump(300);
    let text = read.borrow().clone().expect("the clipboard answered");
    assert_eq!(text.as_str(), super::permission::reset_command());
}

/// A stored "no": no question, no window for one, the denied dialog.
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn stored_no_is_denied_without_asking() {
    let scene = scene("stored-no", Some("no"), Question::Allow);
    scene.capture(CaptureMode::Screen);
    let calls = scene.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert_eq!(
        (calls[0].parent.as_str(), calls[0].visible_windows),
        ("", 0)
    );
    assert_eq!(*scene.portal.store.borrow(), Some("no"));
    assert!(scene.dialog().is_some());
}

/// The bug report: response code 2 with nothing stored is not "denied".
#[test]
#[ignore = "opens windows and owns portal names; needs an isolated bus, see the module docs"]
fn a_question_that_cannot_be_shown_is_not_a_denial() {
    let scene = scene("cannot-ask", None, Question::CannotShow);
    scene.capture(CaptureMode::Screen);
    let calls = scene.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert!(calls[0].parent.starts_with("x11:"), "{calls:?}");
    assert_eq!(*scene.portal.store.borrow(), None, "nothing is stored");
    assert!(scene.dialog().is_none(), "no \"Screen Access Denied\"");
    let window = scene.visible();
    assert_eq!(window.len(), 1, "the window with the toast stays");
    shot(&window[0], "could-not-ask");
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
