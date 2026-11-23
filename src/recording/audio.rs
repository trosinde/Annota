//! Audio sources of a recording (design.md D6): the default microphone and the monitor of the
//! default output, both through `pulsesrc` (pipewire-pulse). No portal is involved for audio.

use gstreamer as gst;
use gstreamer::prelude::*;

/// Which audio source a level reading, a mute or a track belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AudioSource {
    Microphone,
    SystemAudio,
}

impl AudioSource {
    pub const ALL: [AudioSource; 2] = [Self::Microphone, Self::SystemAudio];

    /// Element-name suffix inside the recording pipelines.
    pub(super) fn tag(self) -> &'static str {
        match self {
            Self::Microphone => "mic",
            Self::SystemAudio => "system",
        }
    }

    pub(super) fn index(self) -> usize {
        match self {
            Self::Microphone => 0,
            Self::SystemAudio => 1,
        }
    }
}

/// Which audio devices exist, behind a trait so tests (and the setup's "No microphone found",
/// Phase C) can inject an empty desktop.
pub trait AudioDevices {
    /// An audio input that is not the monitor of an output.
    fn has_microphone(&self) -> bool;
    /// An audio output, whose monitor is "System audio".
    fn has_output(&self) -> bool;

    fn has(&self, source: AudioSource) -> bool {
        match source {
            AudioSource::Microphone => self.has_microphone(),
            AudioSource::SystemAudio => self.has_output(),
        }
    }
}

/// The devices `GstDeviceMonitor` reports right now (`Audio/Source`, `Audio/Sink`).
pub struct SystemDevices {
    microphone: bool,
    output: bool,
}

impl SystemDevices {
    /// Lists the devices once. Without any device provider both are absent.
    pub fn probe() -> Self {
        let _ = gst::init();
        let monitor = gst::DeviceMonitor::new();
        let _ = monitor.add_filter(Some("Audio/Source"), None);
        let _ = monitor.add_filter(Some("Audio/Sink"), None);
        if monitor.start().is_err() {
            return Self {
                microphone: false,
                output: false,
            };
        }
        let devices = monitor.devices();
        monitor.stop();
        let mut found = Self {
            microphone: false,
            output: false,
        };
        for device in devices {
            let class = device.device_class();
            if class.contains("Audio/Sink") {
                found.output = true;
            } else if class.contains("Audio/Source") && !is_monitor(&device) {
                found.microphone = true;
            }
        }
        found
    }
}

impl AudioDevices for SystemDevices {
    fn has_microphone(&self) -> bool {
        self.microphone
    }

    fn has_output(&self) -> bool {
        self.output
    }
}

/// pipewire-pulse lists each output's monitor as an `Audio/Source` with `device.class=monitor`.
fn is_monitor(device: &gst::Device) -> bool {
    device
        .properties()
        .and_then(|p| p.get::<String>("device.class").ok())
        .is_some_and(|class| class == "monitor")
}

/// The sources to record: wanted and present.
pub fn sources_to_record(
    devices: &dyn AudioDevices,
    microphone: bool,
    system_audio: bool,
) -> Vec<AudioSource> {
    AudioSource::ALL
        .into_iter()
        .filter(|source| match source {
            AudioSource::Microphone => microphone,
            AudioSource::SystemAudio => system_audio,
        })
        .filter(|source| devices.has(*source))
        .collect()
}

/// The production element of a source: `pulsesrc` on the default source, or on the monitor of
/// the default output (`@DEFAULT_MONITOR@`).
pub fn source_element(source: AudioSource) -> Result<gst::Element, gst::glib::BoolError> {
    let builder = gst::ElementFactory::make("pulsesrc");
    match source {
        AudioSource::Microphone => builder.build(),
        AudioSource::SystemAudio => builder.property("device", "@DEFAULT_MONITOR@").build(),
    }
}
