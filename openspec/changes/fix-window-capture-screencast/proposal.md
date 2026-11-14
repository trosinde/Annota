# Proposal

**Roadmap phase:** MVP (milestone 0.1). This is a defect fix in `add-screen-capture`'s archived
window mode.
**Benchmark feature (docs/feature-catalog.md):** "Window capture"

## Why

Window mode does not capture a window. The maintainer reported on 2026-11-14: "Capture Window" on
the welcome page brings up a region overlay. The overlay is GNOME Shell's own screenshot UI,
which Annota opens through the Screenshot portal with `interactive: true`. The Screenshot portal
has no mode option, and GNOME's UI always starts in "Selection". Annota cannot preselect
"Window". If the user closes the UI, no image comes back (journal, 07:15:10: "the window picker
ended without an image"). The archived `capture` spec prescribes exactly this flow ("chosen
through the portal's interactive picker"), so the code does what the spec says and the spec is
wrong. CLAUDE.md §5 already records that this mode "had never once delivered an image" when it
was archived.

Measured on the maintainer's desktop on 2026-11-14 (GNOME Shell 50.1, Wayland, xdg-desktop-portal
1.21.1, ScreenCast portal version 5, `AvailableSourceTypes` = 7, GStreamer 1.28.2, display scale
1.25 with `scale-monitor-framebuffer`). The probe was registered as `io.github.trosinde.Annota`
and had no parent window:

- ScreenCast `SelectSources` with `types` = WINDOW (2), `multiple` = false, then `Start`: GNOME
  showed its own window chooser. The user picked a window, and `Start` answered `0` after
  11.9 s with one stream (`source_type` 2, `size` (3072, 1728)).
- `OpenPipeWireRemote` + `pipewiresrc fd=… path=<node> num-buffers=1`: one frame after 397 ms.
  It was clean window content, without a shadow.
- The frame buffer was 6144 × 3456 with alpha. The window occupied only its top-left part, and the
  rest was transparent. A frame therefore has to be cut to the window before use.
- The first attempt got no answer within 120 s. The maintainer could not say whether a chooser
  had appeared. This is not explained yet and is measured again (design.md, task 2).
- `xdg-desktop-portal-gnome` logged "Failed to associate portal window with parent window" (the
  probe passes no parent). The chooser was shown anyway.

## What Changes

- Window mode SHALL pick its window through the **ScreenCast portal's window chooser**. It takes
  exactly one frame from the stream, cuts it to the window, and closes the session at once.
  Nothing is recorded, and no restore token is requested or stored: the chooser appears for every
  window capture.
- With a delay, the chooser comes **first** and the countdown runs after it. The frame is taken
  when the countdown ends, so the user picks the window and then has time to bring it into the
  state they want.
- "Include pointer" asks the stream to embed the pointer (`cursor_mode` embedded) when the portal
  offers that, and to hide it otherwise.
- Closing the chooser is a cancel: no image, no window, no toast. A failure (no stream, no frame
  within 5 s) says "The capture failed. Nothing was captured." and logs the cause.
- Where the ScreenCast portal offers no window source (`AvailableSourceTypes` without WINDOW),
  window mode keeps today's Screenshot-portal UI, with its current "Nothing was captured"
  handling. On GNOME, whether X11 sessions offer a window source has not been measured yet
  (task 2.4).
- **Fixed decision (CLAUDE.md §2) changes:** "Screenshot portal for stills, ScreenCast portal +
  PipeWire for video" becomes "… for video and for window stills". This is recorded as
  `docs/adr/0002-screencast-for-window-stills.md`. GStreamer (`gstreamer-rs`), so far planned only
  for screen recording, becomes an MVP dependency.

**Out of scope**
- Region and screen modes. They stay on the Screenshot portal.
- Multi-window or monitor selection through ScreenCast.
- Annota's own X11 click-to-pick overlay (mockup 06 of add-screen-capture) stays excluded.
- Window decorations and shadow options. The stream delivers what GNOME renders for the window.

## Capabilities

### New Capabilities
- none

### Modified Capabilities
- `capture`: "Window capture" (ScreenCast chooser, one frame, crop, cancel/failure, delay
  order, fallback) and "Cursor inclusion" (window mode asks the stream for the pointer instead of
  "the desktop's picker decides").
- `debian-packaging`: runtime dependencies gain `gstreamer1.0-pipewire` and
  `gstreamer1.0-plugins-base` for the MVP, no longer only with screen recording.

## Impact

- Code: new `src/capture/screencast.rs` (ashpd `desktop::screencast` session, PipeWire fd,
  GStreamer one-frame pipeline, crop), `src/capture/mod.rs` (`run()` window branch, delay order),
  `src/capture/permission.rs` (window-mode refusal sorting), `src/capture/countdown.rs` (label),
  `src/capture/portal.rs` (`PICKER` only for the fallback).
- Dependencies: `gstreamer` and `gstreamer-app`/`gstreamer-video` crates, in a version whose
  MSRV is ≤ 1.85 and whose minimum GStreamer is ≤ 1.24 (Ubuntu 24.04). Their crate licenses are
  MIT/Apache-2.0. GStreamer itself is LGPL-2.1+ and dynamically linked, so it puts no copyleft on
  the MIT binary (CLAUDE.md §10). `ashpd` feature `screencast` (no new crate).
- Packaging: `debian/control` Build-Depends `libgstreamer1.0-dev`,
  `libgstreamer-plugins-base1.0-dev`. Depends `gstreamer1.0-pipewire`,
  `gstreamer1.0-plugins-base`. Plus vendored crates and `debian/copyright` refresh.
- Privacy: frames stay in memory. Only the cropped image reaches the after-capture destination
  (CLAUDE.md §7). GNOME shows its screen-sharing indicator while the stream is open, which lasts
  for the chooser-to-frame time plus any delay.
- Platform: Wayland on GNOME ≥ 46 gets a working window mode. X11 depends on task 2.4.
