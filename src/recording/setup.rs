//! The recording setup in the region overlay's Video mode (task 4.1, mockups 01 and 02, spec
//! "Recording setup").
//!
//! The widgets are part of `region-overlay.blp` (one set per monitor's overlay); the overlay
//! places them beside the selection and owns Start Recording and `Esc`. This module keeps them
//! in step with the saved choices (design.md D8: `record-source`, `record-microphone`,
//! `record-system-audio`, `record-countdown`; Include pointer is the capture's `include-pointer`),
//! shows which audio devices exist and which sources the portal offers, and runs the level
//! meters from a preview pipeline (no video, nothing written) while the setup is shown.
//!
//! Every choice is written the moment it changes, so a cancelled setup keeps it too (scenario
//! "Choices are remembered"). The device probe and the portal query run off the main loop.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gettextrs::gettext;
use gstreamer as gst;
use gstreamer::prelude::*;
use gtk::{gio, glib};

use super::audio::{AudioSource, SystemDevices};
use super::pipeline::{self, RecordingEvent};
use super::source::{self, Offered, SourceKind};
use crate::cli::CaptureMode;
use crate::config;

/// Level meter (redline 01): 8 bars of 4 × 10 px, 2 px apart.
const BARS: usize = 8;
const BAR_WIDTH: f64 = 4.0;
const BAR_GAP: f64 = 2.0;
/// The meter's scale: −60 dB unlit … 0 dB all lit; above −3 dB the last bar is "hot".
const FLOOR_DB: f64 = -60.0;
const HOT_DB: f64 = -3.0;
/// The meter while its switch is off (redline: dimmed to 60 %).
const DIMMED: f64 = 0.6;
/// How often the preview reports a level.
const LEVEL_INTERVAL: gst::ClockTime = gst::ClockTime::from_mseconds(50);
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

/// How many bars a peak of `peak_db` lights, and whether it is hot.
pub fn lit_bars(peak_db: f64) -> (usize, bool) {
    if !peak_db.is_finite() || peak_db <= FLOOR_DB {
        return (0, false);
    }
    let fraction = ((peak_db - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0);
    let lit = (fraction * BARS as f64).ceil() as usize;
    (lit.min(BARS), peak_db > HOT_DB)
}

/// The text under a row's title: the device's name, or that there is none (mockup 02).
pub fn device_subtitle(source: AudioSource, devices: &SystemDevices) -> String {
    let name = match source {
        AudioSource::Microphone => devices.microphone.as_deref(),
        AudioSource::SystemAudio => devices.output.as_deref(),
    };
    match (name, source) {
        (Some(name), _) => name.to_string(),
        (None, AudioSource::Microphone) => gettext("No microphone found"),
        (None, AudioSource::SystemAudio) => gettext("No audio output found"),
    }
}

/// The source to record: the saved one, or Region if the saved one is not offered.
pub fn effective_source(saved: CaptureMode, offered: Option<Offered>) -> CaptureMode {
    match offered {
        Some(offered) if !offered.offers(kind(saved)) => CaptureMode::Region,
        _ => saved,
    }
}

fn kind(source: CaptureMode) -> SourceKind {
    match source {
        CaptureMode::Window => SourceKind::Window,
        CaptureMode::Region | CaptureMode::Screen => SourceKind::Monitor,
    }
}

/// The setup's widgets on one monitor's overlay.
struct View {
    sources: [(CaptureMode, gtk::ToggleButton); 3],
    rows: [(AudioSource, adw::ActionRow, gtk::DrawingArea, gtk::Switch); 2],
    pointer: gtk::Switch,
    countdown: gtk::SpinButton,
}

impl View {
    fn load(builder: &gtk::Builder) -> Option<Self> {
        let get = |name: &str| {
            let found = builder.object::<glib::Object>(name);
            if found.is_none() {
                eprintln!("annota: region-overlay.ui is missing object '{name}'");
            }
            found
        };
        let toggle = |name: &str| get(name)?.downcast::<gtk::ToggleButton>().ok();
        let row = |source: AudioSource, tag: &str| {
            Some((
                source,
                get(&format!("record_{tag}_row"))?
                    .downcast::<adw::ActionRow>()
                    .ok()?,
                get(&format!("record_{tag}_meter"))?
                    .downcast::<gtk::DrawingArea>()
                    .ok()?,
                get(&format!("record_{tag}_switch"))?
                    .downcast::<gtk::Switch>()
                    .ok()?,
            ))
        };
        Some(Self {
            sources: [
                (CaptureMode::Region, toggle("record_region_button")?),
                (CaptureMode::Window, toggle("record_window_button")?),
                (CaptureMode::Screen, toggle("record_screen_button")?),
            ],
            rows: [
                row(AudioSource::Microphone, "microphone")?,
                row(AudioSource::SystemAudio, "system")?,
            ],
            pointer: get("record_pointer_switch")?
                .downcast::<gtk::Switch>()
                .ok()?,
            countdown: get("record_countdown_spin")?
                .downcast::<gtk::SpinButton>()
                .ok()?,
        })
    }

    fn switch(&self, source: AudioSource) -> &gtk::Switch {
        &self.rows[source.index()].3
    }
}

/// The level preview: `pulsesrc ! level ! fakesink` per recorded, present source.
struct Preview {
    pipeline: gst::Pipeline,
    _watch: gst::bus::BusWatchGuard,
}

impl Drop for Preview {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

/// The setup of one overlay session (all monitors).
pub struct Setup {
    settings: gio::Settings,
    views: Vec<View>,
    devices: RefCell<Option<SystemDevices>>,
    offered: Cell<Option<Offered>>,
    shown: Cell<bool>,
    preview: RefCell<Option<Preview>>,
    /// Peak dB per [`AudioSource::index`] from the preview.
    levels: Rc<RefCell<[Option<f64>; 2]>>,
    syncing: Cell<bool>,
    tasks: RefCell<Vec<glib::JoinHandle<()>>>,
    /// The overlay redraws its chrome (Start Recording's sensitivity) after a change.
    on_change: Box<dyn Fn()>,
}

impl Setup {
    /// Binds the widgets of every monitor's overlay to the saved choices and starts the device
    /// probe and the portal query. `None` if a `.ui` file lacks a widget.
    pub fn new(builders: &[gtk::Builder], on_change: impl Fn() + 'static) -> Option<Rc<Self>> {
        let views = builders
            .iter()
            .map(View::load)
            .collect::<Option<Vec<_>>>()?;
        let setup = Rc::new(Self {
            settings: gio::Settings::new(config::APP_ID),
            views,
            devices: RefCell::new(None),
            offered: Cell::new(None),
            shown: Cell::new(false),
            preview: RefCell::new(None),
            levels: Rc::new(RefCell::new([None, None])),
            syncing: Cell::new(false),
            tasks: RefCell::new(Vec::new()),
            on_change: Box::new(on_change),
        });
        setup.connect();
        setup.sync_source();
        setup.sync_countdown();

        let weak = Rc::downgrade(&setup);
        let devices = (super::hooks().devices)();
        let probe = glib::spawn_future_local(async move {
            let devices = devices.await;
            if let Some(setup) = weak.upgrade() {
                setup.apply_devices(devices);
            }
        });
        let weak = Rc::downgrade(&setup);
        let query = glib::spawn_future_local(async move {
            let offered = source::offered().await;
            if let Some(setup) = weak.upgrade() {
                setup.offered.set(Some(offered));
                setup.sync_source();
                (setup.on_change)();
            }
        });
        setup.tasks.borrow_mut().extend([probe, query]);
        Some(setup)
    }

    fn connect(self: &Rc<Self>) {
        let settings = &self.settings;
        for view in &self.views {
            settings
                .bind("include-pointer", &view.pointer, "active")
                .build();
            for (source, _, meter, switch) in &view.rows {
                let key = match source {
                    AudioSource::Microphone => "record-microphone",
                    AudioSource::SystemAudio => "record-system-audio",
                };
                settings.bind(key, switch, "active").build();
                let weak = Rc::downgrade(self);
                switch.connect_active_notify(move |_| {
                    if let Some(setup) = weak.upgrade() {
                        setup.restart_preview();
                    }
                });
                let (weak, source) = (Rc::downgrade(self), *source);
                meter.set_draw_func(move |_, cr, _, height| {
                    if let Some(setup) = weak.upgrade() {
                        setup.draw_meter(source, cr, f64::from(height));
                    }
                });
            }
            for (mode, button) in &view.sources {
                let (weak, mode) = (Rc::downgrade(self), *mode);
                button.connect_toggled(move |button| {
                    let Some(setup) = weak.upgrade() else {
                        return;
                    };
                    if button.is_active() && !setup.syncing.get() {
                        let _ = setup.settings.set_enum("record-source", mode.index());
                    }
                });
            }
            let spin = &view.countdown;
            spin.connect_output(|spin| {
                spin.set_text(&countdown_text(spin.value_as_int()));
                glib::Propagation::Stop
            });
            spin.connect_input(|spin| {
                let digits: String = spin.text().chars().filter(char::is_ascii_digit).collect();
                digits.parse::<f64>().ok().map(Ok)
            });
            let weak = Rc::downgrade(self);
            spin.connect_value_changed(move |spin| {
                if let Some(setup) = weak.upgrade() {
                    if !setup.syncing.get() {
                        let _ = setup
                            .settings
                            .set_int("record-countdown", spin.value_as_int());
                    }
                }
            });
        }
        let weak = Rc::downgrade(self);
        settings.connect_changed(None, move |_, key| {
            let Some(setup) = weak.upgrade() else {
                return;
            };
            match key {
                "record-source" => setup.sync_source(),
                "record-countdown" => setup.sync_countdown(),
                _ => {}
            }
            (setup.on_change)();
        });
    }

    /// The source toggles show the saved source (Region if it is not offered); a source the
    /// portal does not offer is insensitive with the redline's tooltip.
    fn sync_source(&self) {
        let offered = self.offered.get();
        let source = self.source();
        self.syncing.set(true);
        for view in &self.views {
            for (mode, button) in &view.sources {
                let available = offered.is_none_or(|o| o.offers(kind(*mode)));
                button.set_sensitive(available);
                let tooltip = (!available).then(source::not_offered_message);
                button.set_tooltip_text(tooltip.as_deref());
                if *mode == source {
                    button.set_active(true);
                }
            }
        }
        self.syncing.set(false);
    }

    fn sync_countdown(&self) {
        let seconds = f64::from(self.settings.int("record-countdown"));
        self.syncing.set(true);
        for view in &self.views {
            if view.countdown.value() != seconds {
                view.countdown.set_value(seconds);
            }
        }
        self.syncing.set(false);
    }

    fn apply_devices(&self, devices: SystemDevices) {
        for view in &self.views {
            for (source, row, meter, switch) in &view.rows {
                let present = match source {
                    AudioSource::Microphone => devices.microphone.is_some(),
                    AudioSource::SystemAudio => devices.output.is_some(),
                };
                row.set_subtitle(&device_subtitle(*source, &devices));
                meter.set_visible(present);
                if !present {
                    // Off and insensitive, without writing "off" into the saved choice.
                    gio::Settings::unbind(switch, "active");
                    switch.set_active(false);
                }
                switch.set_sensitive(present);
                row.set_sensitive(present);
            }
        }
        self.devices.replace(Some(devices));
        self.restart_preview();
    }

    /// The source Start Recording records.
    pub fn source(&self) -> CaptureMode {
        let saved = CaptureMode::from_index(self.settings.enum_("record-source"));
        effective_source(saved, self.offered.get())
    }

    /// Whether Start Recording can start: the source is offered (or not known yet) and a region
    /// has a selection.
    pub fn can_start(&self, has_selection: bool) -> bool {
        let source = self.source();
        let offered = self
            .offered
            .get()
            .is_none_or(|offered| offered.offers(kind(source)));
        offered && (source != CaptureMode::Region || has_selection)
    }

    /// The setup is on the screen (`true`) or not: the level preview runs only meanwhile.
    pub fn set_shown(&self, shown: bool) {
        if self.shown.replace(shown) != shown {
            self.restart_preview();
        }
    }

    /// The overlay closes: the preview is torn down, the probes are dropped.
    pub fn close(&self) {
        self.shown.set(false);
        self.preview.replace(None);
        for task in self.tasks.borrow_mut().drain(..) {
            task.abort();
        }
    }

    fn wanted(&self, source: AudioSource) -> bool {
        let present = self.devices.borrow().as_ref().is_some_and(|devices| {
            devices.microphone.is_some() && source == AudioSource::Microphone
                || devices.output.is_some() && source == AudioSource::SystemAudio
        });
        present
            && self
                .views
                .first()
                .is_some_and(|view| view.switch(source).is_active())
    }

    /// (Re)builds the preview for the sources whose switch is on, while the setup is shown.
    fn restart_preview(&self) {
        self.preview.replace(None);
        self.levels.replace([None, None]);
        let sources: Vec<AudioSource> = AudioSource::ALL
            .into_iter()
            .filter(|source| self.shown.get() && self.wanted(*source))
            .collect();
        if !sources.is_empty() {
            match self.build_preview(&sources) {
                Ok(preview) => {
                    self.preview.replace(Some(preview));
                }
                Err(err) => eprintln!("annota: recording setup: no level preview: {err}"),
            }
        }
        self.redraw_meters();
    }

    fn build_preview(&self, sources: &[AudioSource]) -> Result<Preview, String> {
        gst::init().map_err(|e| e.to_string())?;
        let pipeline = gst::Pipeline::with_name("annota-setup-preview");
        let hooks = super::hooks();
        for source in sources {
            let element = (hooks.preview_source)(*source)
                .ok_or_else(|| format!("no source element for {source:?}"))?;
            let level = gst::ElementFactory::make("level")
                .name(format!("level-{}", source.tag()))
                .property("interval", LEVEL_INTERVAL.nseconds())
                .property("post-messages", true)
                .build()
                .map_err(|_| String::from("no level element"))?;
            let sink = gst::ElementFactory::make("fakesink")
                .property("sync", false)
                .property("async", false)
                .build()
                .map_err(|_| String::from("no fakesink element"))?;
            pipeline
                .add_many([&element, &level, &sink])
                .map_err(|e| e.to_string())?;
            gst::Element::link_many([&element, &level, &sink]).map_err(|e| e.to_string())?;
        }
        let bus = pipeline.bus().ok_or("no bus")?;
        let views: Vec<[gtk::DrawingArea; 2]> = self
            .views
            .iter()
            .map(|view| [view.rows[0].2.clone(), view.rows[1].2.clone()])
            .collect();
        let shared = self.levels.clone();
        let watch = bus
            .add_watch_local(move |_, message| {
                match message.view() {
                    gst::MessageView::Element(element) => {
                        if let Some(RecordingEvent::Level {
                            source, peak_db, ..
                        }) = pipeline::level_event(message, element.structure())
                        {
                            shared.borrow_mut()[source.index()] = Some(peak_db);
                            for meters in &views {
                                meters[source.index()].queue_draw();
                            }
                        }
                    }
                    gst::MessageView::Error(err) => {
                        eprintln!(
                            "annota: recording setup: level preview failed in {}: {}",
                            err.src().map(|s| s.name().to_string()).unwrap_or_default(),
                            err.error()
                        );
                        return glib::ControlFlow::Break;
                    }
                    _ => {}
                }
                glib::ControlFlow::Continue
            })
            .map_err(|e| e.to_string())?;
        pipeline
            .set_state(gst::State::Playing)
            .map_err(|e| e.to_string())?;
        Ok(Preview {
            pipeline,
            _watch: watch,
        })
    }

    fn redraw_meters(&self) {
        for view in &self.views {
            for (_, _, meter, switch) in &view.rows {
                meter.set_opacity(if switch.is_active() { 1.0 } else { DIMMED });
                meter.queue_draw();
            }
        }
    }

    fn draw_meter(&self, source: AudioSource, cr: &gtk::cairo::Context, height: f64) {
        let peak = self.levels.borrow()[source.index()];
        let (lit, hot) = peak.map_or((0, false), lit_bars);
        let dark = adw::StyleManager::default().is_dark();
        for bar in 0..BARS {
            let x = bar as f64 * (BAR_WIDTH + BAR_GAP);
            if bar < lit {
                let (r, g, b) = if hot && bar == BARS - 1 {
                    WARNING
                } else {
                    SUCCESS
                };
                cr.set_source_rgb(r, g, b);
            } else if dark {
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.15);
            } else {
                cr.set_source_rgba(0.0, 0.0, 6.0 / 255.0, 0.15);
            }
            cr.rectangle(x, 0.0, BAR_WIDTH, height);
            let _ = cr.fill();
        }
    }
}

/// The countdown spin's text: "3 s".
pub fn countdown_text(seconds: i32) -> String {
    gettext("{seconds} s").replace("{seconds}", &seconds.to_string())
}
