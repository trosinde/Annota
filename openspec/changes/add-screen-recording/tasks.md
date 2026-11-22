# Tasks

Scenario names below are `screen-recording / <Requirement> / <Scenario>` unless another
capability is named. Every test carries a `spec:` tag for the scenarios it proves.

## 1. Design gate
- [x] 1.1 `designer` agent: core mockups in `docs/design/add-screen-recording/` (14 screens, light + dark, 1x + 2x); out-of-scope screens moved to `docs/design/add-recording-extras/`
- [ ] 1.2 Maintainer decisions recorded in design.md: Q1 post-recording flow (D1), Q2 H.264 encoder, Q3 AAC encoder, Q4 working-file location, Q5 shortcut, Q6 control details
- [ ] 1.3 Maintainer approval recorded in design.md (`Design approved: <date>`)

## 2. Foundation (probes before code)
- [ ] 2.1 Extend `tools/portal-probe.py` for monitor sources, stream properties, cursor modes and session close; run P1–P12 of design.md on GNOME 46 (Ubuntu 24.04) and GNOME 50, results with date and versions in `probe-results.md`; correct spec and design wherever a result contradicts them before continuing
- [ ] 2.2 Measure `pulsesrc` default source, `@DEFAULT_MONITOR@` and `GstDeviceMonitor` on Ubuntu 24.04 (pipewire-pulse)
- [ ] 2.3 Verify every element of "Recording runtime dependencies" and its package on clean Ubuntu 24.04, Ubuntu 26.04 and Debian 13 containers; settle Q2/Q3 with the result
- [ ] 2.4 Crates: `gstreamer-pbutils`/`gstreamer-audio` if needed, MSRV 1.85 and GStreamer 1.24 checked, `Cargo.lock` with Rust 1.85, `build-aux/update-copyright.py`
- [ ] 2.5 GSettings keys of design.md D8 with defaults; `cli.rs` `--record region|window|screen|toggle` — Recording on the command line / Invalid record mode

## 3. Recording pipeline
- [ ] 3.1 `screencast.rs`: keep a stream for recording (monitor and window sources, no restore token, cursor mode) — Recording source / Pointer in the recording, Source request cancelled, Source not offered (fake portal)
- [ ] 3.2 Region crop mapping from probe P2/P3 — Recording source / Record a region, Region not on the chosen screen
- [ ] 3.3 Window and screen recording incl. per-frame window crop and scaling to the first frame — Recording source / Record a window, Record the screen
- [ ] 3.4 Pipeline to the Matroska working file (videotestsrc/audiotestsrc in tests) — Working file / Recording survives a crash, Name already taken, Recording error
- [ ] 3.5 Pause/resume via drop probe + running-time offset — Pause and resume / Paused time is not recorded
- [ ] 3.6 Audio: device presence, mute, level — Audio / Mute the microphone, Mute system audio, No microphone, No audio selected
- [ ] 3.7 Stop paths: button, shortcut, `Session::Closed`, window stream end — Stop recording / Stop by button, Stop by hotkey, Session ended by the desktop, Recorded window closed

## 4. UI chrome
- [ ] 4.1 Overlay mode pill + recording setup (Blueprint), settings persisted — Recording setup / Switch the overlay to video, Choices are remembered, Cancel the setup (mockups 01, 02)
- [ ] 4.2 Waiting indicator "Waiting for a screen…" reusing the window-wait OSD (mockup 03)
- [ ] 4.3 Countdown label and recording start after the OSD has gone — Recording countdown / Countdown is not recorded, Cancel the countdown, No countdown (mockup 04)
- [ ] 4.4 Control window: time, pause, stop, mutes, meters, `Space`/`M`/`S`, second request raises it — Recording control / Elapsed time, Second recording request; Pause and resume / Paused state (mockups 05, 06)
- [ ] 4.5 Record shortcut (portal ID `record` + GNOME-keybinding fallback), Preferences › Capture row — Record shortcut / all four scenarios (mockup 14)
- [ ] 4.6 Welcome page Record card — `app-shell` / Welcome page / Record card starts the recording setup, and the existing Welcome page scenarios stay green (mockup 12); region-not-on-screen toast (mockup 13)

## 5. Export and integration
- [ ] 5.1 Export Recording window: poster frame and duration (discoverer), name, folder, format, include audio (mockup 07)
- [ ] 5.2 MP4 remux + AAC mix, WebM VP9 + Opus transcode, progress and Cancel — Export after recording / Export as MP4, Export as WebM, Export without audio, Cancel the export, Target name taken (mockups 07, 08)
- [ ] 5.3 "Saved" state, Show in Files via `org.freedesktop.FileManager1` — Export after recording / Show in Files (mockup 09)
- [ ] 5.4 Discard + close confirmation — Discard a recording / Discard, Close asks first (mockup 10); error banner (mockup 11)
- [ ] 5.5 `debian/control` Depends per design.md "Dependencies" — `debian-packaging` / Recording runtime dependencies / Recording elements installed
- [ ] 5.6 Shortcuts window, metainfo/README feature text, `docs/feature-catalog.md` status

## 6. Verify
- [ ] 6.1 Tests for every scenario (unit, pipeline tests with test sources, fake-portal live tests); `manual-checks.md` rows only where automation is impossible (real chooser, real audio devices), each run with result and date
- [ ] 6.2 Live run on the maintainer's desktop for region, window and screen with audio, including the probe-dependent scenarios
- [ ] 6.3 `reviewer` agent: specs, mockup fidelity, clippy/fmt/tests green
- [ ] 6.4 `.deb` builds and installs on Ubuntu 24.04 (Rust 1.85), lintian clean, recording elements present
- [ ] 6.5 German translation updated
- [ ] 6.6 `python3 tools/scenario_gate.py verify add-screen-recording` exits 0, `scenario-report.md` committed
