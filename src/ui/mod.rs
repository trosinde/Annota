//! Editor UI: the canvas (with the Select tool's editing, tasks 3.1–3.4) and the editor window
//! around it (tasks 4.1, 4.2). Debug builds reach it through `--dev-canvas FILE` and startup
//! recovery until `--open` and captures open it (tasks 4.4, 4.5).

pub mod canvas;
pub mod editor;
pub mod labels;

/// Helpers for the `#[ignore]`d tests that need a display. Run each such test in a process of
/// its own (GTK can only be initialised on one thread per process), headless:
/// `GDK_BACKEND=x11 xvfb-run -a dbus-run-session cargo test --bin annota -- --ignored --exact <test>`.
#[cfg(test)]
pub mod test_support {
    use std::time::{Duration, Instant};

    use gtk::glib;

    /// Initialises GTK and libadwaita. Without a display this *fails* the test: a live test
    /// that silently does nothing is worse than none.
    pub fn init() {
        gtk::init().expect("no display: run under xvfb-run with GDK_BACKEND=x11");
        adw::init().expect("libadwaita did not initialise");
    }

    /// Runs the main loop for `ms` milliseconds.
    pub fn pump(ms: u64) {
        let end = Instant::now() + Duration::from_millis(ms);
        let ctx = glib::MainContext::default();
        while Instant::now() < end {
            while ctx.iteration(false) {}
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
