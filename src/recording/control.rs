//! The Recording control (task 4.4, mockups 05 and 06, spec "Recording control", "Pause and
//! resume", "Audio").
//!
//! Annota's own small undecorated toplevel titled "Recording" (`recording-control.blp`), shown
//! when a recording starts (after the countdown) and closed when it ends, whichever way. GNOME
//! places it; it is dragged by its body and never kept above other windows or out of the
//! recording (design.md D4). It shows the elapsed time without paused time (`M:SS`, `H:MM:SS`
//! from one hour), Pause/Resume (`Space`), Stop, a mute button with a 5-bar level meter per
//! recorded audio source (`M`, `S`) and Minimize. `Space`, `M` and `S` work while the control
//! has the keyboard focus (it is a normal window). A second recording request presents it
//! ([`present`]).
//!
//! Idle cost: the time is refreshed every [`TICK`] only while the window is mapped and the
//! recording is not paused, and a label or meter is redrawn only when what it shows changed.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

use gettextrs::gettext;
use gstreamer as gst;
use gtk::{cairo, gdk, glib, prelude::*};

use super::audio::AudioSource;
use super::pipeline::RecordingEvent;
use super::session::RecordingSession;
use crate::capture::window_wait;

const UI: &str = "/io/github/trosinde/Annota/ui/recording-control.ui";
/// How often the time is refreshed while it runs.
pub const TICK: Duration = Duration::from_millis(250);
/// Level meter (redline 05): 5 bars of 3 × 12 px, 2 px apart.
const BARS: usize = 5;
const BAR_WIDTH: f64 = 3.0;
const BAR_HEIGHT: f64 = 12.0;
const BAR_GAP: f64 = 2.0;
/// The meter's scale: −60 dB unlit … 0 dB all lit; above −3 dB the last bar is "hot".
const FLOOR_DB: f64 = -60.0;
const HOT_DB: f64 = -3.0;
const SUCCESS: (f64, f64, f64) = (
    0x2e as f64 / 255.0,
    0xc2 as f64 / 255.0,
    0x7e as f64 / 255.0,
);
const WARNING: (f64, f64, f64) = (
    0xe5 as f64 / 255.0,
    0xa5 as f64 / 255.0,
    0x0a as f64 / 255.0,
);
/// An unlit bar: white at 25 %.
const UNLIT: f64 = 0.25;

/// The recorded time as the control shows it: `M:SS`, from one hour `H:MM:SS`.
pub fn format_elapsed(elapsed: gst::ClockTime) -> String {
    let total = elapsed.seconds();
    let (hours, minutes, seconds) = (total / 3600, total / 60 % 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// How many of the 5 bars a peak of `peak_db` lights, and whether the last one is hot.
pub fn meter_bars(peak_db: f64) -> (usize, bool) {
    if !peak_db.is_finite() || peak_db <= FLOOR_DB {
        return (0, false);
    }
    let fraction = ((peak_db - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0);
    let lit = (fraction * BARS as f64).ceil() as usize;
    (lit.min(BARS), peak_db > HOT_DB)
}

/// The recorded time, without paused time; `None` once the recording has ended.
pub type ElapsedSource = Rc<dyn Fn() -> Option<gst::ClockTime>>;
/// The Record shortcut's trigger as a GTK accelerator, as the portal reports it (Stop's tooltip
/// chip); `None`: no chip. `recording::Hooks::record_trigger`, filled by task 4.5.
pub type RecordTrigger = Rc<dyn Fn() -> Option<String>>;

/// One audio source's mute button.
struct AudioButton {
    source: AudioSource,
    button: gtk::Button,
    icon: gtk::Image,
    meter: gtk::DrawingArea,
    /// Lit bars and hot, as drawn.
    shown: Cell<(usize, bool)>,
}

impl AudioButton {
    fn key(&self) -> &'static str {
        match self.source {
            AudioSource::Microphone => "m",
            AudioSource::SystemAudio => "s",
        }
    }

    /// The button's text: what activating it does, or that the source is muted (redline 05–06).
    fn label(&self, muted: bool) -> String {
        match (self.source, muted) {
            (AudioSource::Microphone, false) => gettext("Mute microphone"),
            (AudioSource::Microphone, true) => gettext("Microphone muted"),
            (AudioSource::SystemAudio, false) => gettext("Mute system audio"),
            (AudioSource::SystemAudio, true) => gettext("System audio muted"),
        }
    }

    /// Kit gap: Adwaita's muted glyphs stand in for the kit's slashed ones.
    fn icon_name(&self, muted: bool) -> &'static str {
        match (self.source, muted) {
            (AudioSource::Microphone, false) => "annota-microphone-symbolic",
            (AudioSource::Microphone, true) => "microphone-disabled-symbolic",
            (AudioSource::SystemAudio, false) => "annota-speaker-symbolic",
            (AudioSource::SystemAudio, true) => "audio-volume-muted-symbolic",
        }
    }

    fn show_muted(&self, muted: bool) {
        self.icon.set_icon_name(Some(self.icon_name(muted)));
        self.button
            .update_property(&[gtk::accessible::Property::Label(&self.label(muted))]);
        if muted {
            self.set_level(f64::NEG_INFINITY);
        }
        self.button.trigger_tooltip_query();
    }

    fn set_level(&self, peak_db: f64) {
        let bars = meter_bars(peak_db);
        if self.shown.replace(bars) != bars {
            self.meter.queue_draw();
        }
    }
}

struct Control {
    window: gtk::Window,
    rec_dot: gtk::Widget,
    paused_icon: gtk::Image,
    paused_label: gtk::Label,
    time_label: gtk::Label,
    pause_button: gtk::Button,
    minimize_button: gtk::Button,
    audio: Vec<AudioButton>,
    session: Weak<RecordingSession>,
    elapsed: RefCell<ElapsedSource>,
    tick: RefCell<Option<glib::SourceId>>,
}

thread_local! {
    static SHOWN: RefCell<Option<Rc<Control>>> = const { RefCell::new(None) };
    /// Tests: how often the control was asked to minimise itself and to come to the front
    /// (neither is observable on a display without a window manager).
    #[cfg(test)]
    static REQUESTS: RefCell<(u32, u32)> = const { RefCell::new((0, 0)) };
}

fn shown_control() -> Option<Rc<Control>> {
    SHOWN.with(|shown| shown.borrow().clone())
}

/// Shows the control for `session`, which has just started writing. `record_trigger` gives
/// Stop's tooltip chip each time the tooltip is shown.
pub fn show(app: &adw::Application, session: &Rc<RecordingSession>, record_trigger: RecordTrigger) {
    close();
    let Some(control) = build(session, record_trigger) else {
        eprintln!("annota: recording-control.ui is incomplete");
        return;
    };
    SHOWN.with(|shown| shown.replace(Some(control.clone())));
    control.window.set_application(Some(app));
    control.refresh();
    control.window.present();
    // Redline: Pause has the initial focus.
    control.pause_button.grab_focus();
}

/// A second recording request: asks the desktop to bring the control to the front (GNOME
/// decides whether it is raised or shows "Recording is ready"). `false`: no control is shown.
pub fn present() -> bool {
    match shown_control() {
        Some(control) => {
            #[cfg(test)]
            REQUESTS.with(|r| r.borrow_mut().1 += 1);
            control.window.present();
            true
        }
        None => false,
    }
}

/// Takes the control away (the recording ended or is being stopped).
pub fn close() {
    let control = SHOWN.with(|shown| shown.borrow_mut().take());
    if let Some(control) = control {
        control.stop_tick();
        control.window.set_visible(false);
        control.window.destroy();
    }
}

/// Level meters while recording.
pub fn on_event(event: &RecordingEvent) {
    let RecordingEvent::Level {
        source, peak_db, ..
    } = event
    else {
        return;
    };
    let Some(control) = shown_control() else {
        return;
    };
    let muted = control
        .session
        .upgrade()
        .and_then(|s| s.with_recording(|r| r.is_muted(*source)))
        .unwrap_or(false);
    if let Some(button) = control.audio.iter().find(|b| b.source == *source) {
        button.set_level(if muted { f64::NEG_INFINITY } else { *peak_db });
    }
}

/// The control's window while it is shown, for tests.
#[cfg(test)]
pub(crate) fn shown() -> Option<gtk::Window> {
    shown_control().map(|control| control.window.clone())
}

/// Tests: replaces the control's time source (a recording that "has run for 12 s").
#[cfg(test)]
pub(crate) fn set_elapsed_source(source: ElapsedSource) {
    if let Some(control) = shown_control() {
        control.elapsed.replace(source);
        control.refresh();
    }
}

/// Tests: the lit bars (and hot) of `source`'s meter, if it has a button.
#[cfg(test)]
pub(crate) fn meter(source: AudioSource) -> Option<(usize, bool)> {
    shown_control()?
        .audio
        .iter()
        .find(|a| a.source == source)
        .map(|a| a.shown.get())
}

/// Tests: (minimise requests, present requests) so far in this thread.
#[cfg(test)]
pub(crate) fn requests() -> (u32, u32) {
    REQUESTS.with(|r| *r.borrow())
}

fn build(session: &Rc<RecordingSession>, record_trigger: RecordTrigger) -> Option<Rc<Control>> {
    let builder = gtk::Builder::from_resource(UI);
    let object = |name: &str| builder.object::<glib::Object>(name);
    let window = object("control_window")?.downcast::<gtk::Window>().ok()?;
    let pill = object("control_box")?.downcast::<gtk::Box>().ok()?;
    let button = |name: &str| object(name)?.downcast::<gtk::Button>().ok();
    let image = |name: &str| object(name)?.downcast::<gtk::Image>().ok();
    let meter = |name: &str| object(name)?.downcast::<gtk::DrawingArea>().ok();
    let recorded = session
        .with_recording(|r| r.audio_sources())
        .unwrap_or_default();
    let mut audio = Vec::new();
    for (source, tag) in [
        (AudioSource::Microphone, "microphone"),
        (AudioSource::SystemAudio, "system"),
    ] {
        let source_button = button(&format!("{tag}_button"))?;
        if recorded.contains(&source) {
            audio.push(AudioButton {
                source,
                button: source_button,
                icon: image(&format!("{tag}_icon"))?,
                meter: meter(&format!("{tag}_meter"))?,
                shown: Cell::new((0, false)),
            });
        } else {
            // A source not recorded has no button (redline 05).
            source_button.set_visible(false);
        }
    }
    // With no audio, the group and its separator go.
    object("audio_separator")?
        .downcast::<gtk::Widget>()
        .ok()?
        .set_visible(!audio.is_empty());

    let control = Rc::new(Control {
        window,
        rec_dot: object("rec_dot")?.downcast::<gtk::Widget>().ok()?,
        paused_icon: image("paused_icon")?,
        paused_label: object("paused_label")?.downcast::<gtk::Label>().ok()?,
        time_label: object("time_label")?.downcast::<gtk::Label>().ok()?,
        pause_button: button("pause_button")?,
        minimize_button: button("minimize_button")?,
        audio,
        session: Rc::downgrade(session),
        elapsed: RefCell::new(elapsed_of(session)),
        tick: RefCell::new(None),
    });
    let stop_button = button("stop_button")?;
    control.connect(&pill, &stop_button, record_trigger);
    Some(control)
}

/// The session's recorded time.
fn elapsed_of(session: &Rc<RecordingSession>) -> ElapsedSource {
    let session = Rc::downgrade(session);
    Rc::new(move || session.upgrade()?.with_recording(|r| r.elapsed()))
}

/// A tooltip "text  key" with the key as a keycap (mockup 05).
fn set_tooltip(tooltip: &gtk::Tooltip, text: &str, accel: Option<&str>) {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    content.append(&gtk::Label::new(Some(text)));
    if let Some(accel) = accel {
        content.append(&gtk::ShortcutLabel::new(accel));
    }
    tooltip.set_custom(Some(&content));
}

impl Control {
    fn with_session<R>(&self, f: impl FnOnce(&RecordingSession) -> Option<R>) -> Option<R> {
        self.session.upgrade().and_then(|session| f(&session))
    }

    fn is_paused(&self) -> bool {
        self.with_session(|s| s.with_recording(|r| r.is_paused()))
            .unwrap_or(false)
    }

    fn is_muted(&self, source: AudioSource) -> bool {
        self.with_session(|s| s.with_recording(|r| r.is_muted(source)))
            .unwrap_or(false)
    }

    fn connect(self: &Rc<Self>, pill: &gtk::Box, stop: &gtk::Button, trigger: RecordTrigger) {
        let weak = Rc::downgrade(self);
        let with = move |f: fn(&Rc<Control>)| {
            let weak = weak.clone();
            move || {
                if let Some(control) = weak.upgrade() {
                    f(&control);
                }
            }
        };

        let toggle_pause = with(|c| c.toggle_pause());
        self.pause_button.connect_clicked(move |_| toggle_pause());
        let weak = Rc::downgrade(self);
        self.pause_button
            .connect_query_tooltip(move |_, _, _, _, tooltip| {
                let paused = weak.upgrade().is_some_and(|c| c.is_paused());
                let text = if paused {
                    gettext("Resume")
                } else {
                    gettext("Pause")
                };
                set_tooltip(tooltip, &text, Some("space"));
                true
            });

        stop.connect_clicked(|_| {
            super::stop();
        });
        stop.connect_query_tooltip(move |_, _, _, _, tooltip| {
            set_tooltip(tooltip, &gettext("Stop"), trigger().as_deref());
            true
        });

        for (index, audio) in self.audio.iter().enumerate() {
            let weak = Rc::downgrade(self);
            audio.button.connect_clicked(move |_| {
                if let Some(control) = weak.upgrade() {
                    control.toggle_mute(index);
                }
            });
            let weak = Rc::downgrade(self);
            audio
                .button
                .connect_query_tooltip(move |_, _, _, _, tooltip| {
                    let Some(control) = weak.upgrade() else {
                        return false;
                    };
                    let Some(audio) = control.audio.get(index) else {
                        return false;
                    };
                    let muted = control.is_muted(audio.source);
                    set_tooltip(tooltip, &audio.label(muted), Some(audio.key()));
                    true
                });
            let weak = Rc::downgrade(self);
            audio.meter.set_draw_func(move |_, cr, _, height| {
                let shown = weak
                    .upgrade()
                    .and_then(|c| c.audio.get(index).map(|a| a.shown.get()))
                    .unwrap_or((0, false));
                let _ = draw_meter(cr, f64::from(height), shown);
            });
        }

        let minimize = with(|c| c.minimize());
        self.minimize_button.connect_clicked(move |_| minimize());

        // `Space`, `M`, `S` (redline): after the focused widget, so `Space` on a focused button
        // activates that button and on the window body pauses.
        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(control) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if modifiers.intersects(
                gdk::ModifierType::CONTROL_MASK
                    | gdk::ModifierType::ALT_MASK
                    | gdk::ModifierType::SUPER_MASK,
            ) {
                return glib::Propagation::Proceed;
            }
            let wanted = match key.to_lower() {
                gdk::Key::space => {
                    control.toggle_pause();
                    return glib::Propagation::Stop;
                }
                gdk::Key::m => AudioSource::Microphone,
                gdk::Key::s => AudioSource::SystemAudio,
                _ => return glib::Propagation::Proceed,
            };
            match control.audio.iter().position(|a| a.source == wanted) {
                Some(index) => {
                    control.toggle_mute(index);
                    glib::Propagation::Stop
                }
                None => glib::Propagation::Proceed,
            }
        });
        self.window.add_controller(keys);

        // It closes with its recording, not on its own (`Alt+F4` and the like do nothing).
        self.window
            .connect_close_request(|_| glib::Propagation::Stop);

        let start = with(|c| c.refresh());
        self.window.connect_map(move |_| start());
        let stop_tick = with(|c| c.stop_tick());
        self.window.connect_unmap(move |_| stop_tick());

        // Only the pill takes pointer input; its shadow margins let clicks through.
        let pill = pill.clone();
        self.window.connect_map(move |window| {
            let (Some(surface), Some(clock)) = (window.surface(), window.frame_clock()) else {
                return;
            };
            let (window, pill) = (window.clone(), pill.clone());
            let last = Cell::new(None);
            clock.connect_after_paint(move |_| {
                let rect = window_wait::input_rect(&window, &pill);
                if rect.is_some() && rect != last.get() {
                    last.set(rect);
                    if let Some(rect) = rect {
                        surface.set_input_region(&cairo::Region::create_rectangle(&rect));
                    }
                }
            });
        });
    }

    fn toggle_pause(self: &Rc<Self>) {
        self.with_session(|s| {
            s.with_recording(|r| {
                if r.is_paused() {
                    r.resume();
                } else {
                    r.pause();
                }
            })
        });
        self.refresh();
        self.pause_button.trigger_tooltip_query();
    }

    fn toggle_mute(&self, index: usize) {
        let Some(audio) = self.audio.get(index) else {
            return;
        };
        let muted = self
            .with_session(|s| {
                s.with_recording(|r| {
                    let muted = !r.is_muted(audio.source);
                    r.set_muted(audio.source, muted);
                    muted
                })
            })
            .unwrap_or(false);
        audio.show_muted(muted);
    }

    fn minimize(&self) {
        #[cfg(test)]
        REQUESTS.with(|r| r.borrow_mut().0 += 1);
        // The recording continues; GNOME's overview or Alt+Tab bring the control back.
        self.window.minimize();
    }

    /// Shows the state (recording or paused) and the time, and runs the clock while it moves.
    fn refresh(self: &Rc<Self>) {
        let paused = self.is_paused();
        self.rec_dot.set_visible(!paused);
        self.paused_icon.set_visible(paused);
        self.paused_label.set_visible(paused);
        let (icon, label) = if paused {
            ("annota-play-symbolic", gettext("Resume"))
        } else {
            ("annota-pause-symbolic", gettext("Pause"))
        };
        if self.pause_button.icon_name().as_deref() != Some(icon) {
            self.pause_button.set_icon_name(icon);
        }
        self.pause_button
            .update_property(&[gtk::accessible::Property::Label(&label)]);
        for audio in &self.audio {
            let muted = self.is_muted(audio.source);
            if audio.icon.icon_name().as_deref() != Some(audio.icon_name(muted)) {
                audio.show_muted(muted);
            }
        }
        self.show_time();
        if paused || !self.window.is_mapped() {
            self.stop_tick();
        } else if self.tick.borrow().is_none() {
            let weak = Rc::downgrade(self);
            let id = glib::timeout_add_local(TICK, move || {
                let Some(control) = weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                if !control.show_time() {
                    // The recording is being finished (the desktop ended it, an error): the
                    // control goes now rather than when the file is written.
                    control.tick.replace(None);
                    close();
                    return glib::ControlFlow::Break;
                }
                glib::ControlFlow::Continue
            });
            self.tick.replace(Some(id));
        }
    }

    /// Sets the time label; `false` once the recording no longer runs.
    fn show_time(&self) -> bool {
        let elapsed = (self.elapsed.borrow())();
        let Some(elapsed) = elapsed else {
            return false;
        };
        let text = format_elapsed(elapsed);
        if self.time_label.label() != text {
            self.time_label.set_label(&text);
        }
        true
    }

    fn stop_tick(&self) {
        if let Some(id) = self.tick.borrow_mut().take() {
            id.remove();
        }
    }
}

fn draw_meter(
    cr: &cairo::Context,
    height: f64,
    (lit, hot): (usize, bool),
) -> Result<(), cairo::Error> {
    let top = ((height - BAR_HEIGHT) / 2.0).max(0.0);
    for bar in 0..BARS {
        let x = bar as f64 * (BAR_WIDTH + BAR_GAP);
        if bar < lit {
            let (r, g, b) = if hot && bar == BARS - 1 {
                WARNING
            } else {
                SUCCESS
            };
            cr.set_source_rgb(r, g, b);
        } else {
            cr.set_source_rgba(1.0, 1.0, 1.0, UNLIT);
        }
        rounded_bar(cr, x, top);
        cr.fill()?;
    }
    Ok(())
}

/// A bar with 1 px corners.
fn rounded_bar(cr: &cairo::Context, x: f64, y: f64) {
    let r = 1.0;
    let (w, h) = (BAR_WIDTH, BAR_HEIGHT);
    let pi = std::f64::consts::PI;
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -pi / 2.0, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, pi / 2.0);
    cr.arc(x + r, y + h - r, r, pi / 2.0, pi);
    cr.arc(x + r, y + r, r, pi, 1.5 * pi);
    cr.close_path();
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: screen-recording / Recording control / Elapsed time
    #[test]
    fn the_time_reads_minutes_and_seconds_and_hours_from_one_hour() {
        let s = gst::ClockTime::from_seconds;
        assert_eq!(format_elapsed(s(0)), "0:00");
        assert_eq!(format_elapsed(s(12)), "0:12");
        assert_eq!(
            format_elapsed(gst::ClockTime::from_mseconds(12_999)),
            "0:12"
        );
        assert_eq!(format_elapsed(s(59 * 60 + 59)), "59:59");
        assert_eq!(format_elapsed(s(3600)), "1:00:00");
        assert_eq!(format_elapsed(s(2 * 3600 + 5 * 60 + 7)), "2:05:07");
    }
}
