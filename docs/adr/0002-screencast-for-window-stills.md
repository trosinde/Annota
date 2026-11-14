# ADR 0002: ScreenCast portal for window stills

Status: accepted by the maintainer 2026-11-14 (OpenSpec change `fix-window-capture-screencast`)

## Context

CLAUDE.md §2 fixed screen access as "Screenshot portal for stills, ScreenCast portal + PipeWire
for video". The Screenshot portal has no window mode. With `interactive: true` it opens the
desktop's general screenshot UI, which on GNOME always starts in "Selection". Annota's window
mode therefore showed a region selection and never delivered a window image. This was reported
by the maintainer on 2026-11-14, and CLAUDE.md §5 records that it never worked.

The ScreenCast portal with source type WINDOW shows the desktop's own window chooser. It was
measured on 2026-11-14 (GNOME Shell 50.1, xdg-desktop-portal 1.21.1, ScreenCast version 5,
GStreamer 1.28.2): the chooser appeared, the chosen window's first frame arrived 397 ms after the
stream started, and it was clean window content without a shadow, inside a larger
transparent-margin buffer.

## Decision

Window stills use the ScreenCast portal: one session per capture with `types` WINDOW,
`multiple` false and `persist_mode` 0 (no restore token, so the chooser appears every time).
Annota takes exactly one frame through GStreamer (`pipewiresrc` → `videoconvert` → `appsink`),
cuts it to the window, and closes the session at once. Frames stay in memory.

Region and screen stills stay on the Screenshot portal. Where the ScreenCast portal offers no
window source, window mode falls back to the Screenshot portal's interactive UI.

The §2 row becomes: "Screenshot portal for region and screen stills, ScreenCast portal +
PipeWire for window stills (ADR 0002) and video."

## Alternatives considered

- **Keep the Screenshot UI and tell the user to pick "Window" in it.** This needs no new
  dependency, but it keeps a two-step detour through a UI Annota cannot configure. Rejected by
  the maintainer.
- **Remove window mode on Wayland.** Rejected: window capture is a core benchmark feature.
- **The `pipewire` crate instead of GStreamer.** It needs `bindgen`/libclang at build time and
  adds a second media stack next to the GStreamer one §2 already picks for video.
- **Annota's own window picker.** Wayland gives clients no window list or geometry, and
  compositor-private APIs are excluded by §2.

## Consequences

- GStreamer (`gstreamer-rs`, MIT/Apache-2.0, linked dynamically against LGPL GStreamer) becomes
  an MVP dependency, not only a screen-recording one. Build-Depends gain `libgstreamer1.0-dev`
  and `libgstreamer-plugins-base1.0-dev`. Depends gain `gstreamer1.0-pipewire` and
  `gstreamer1.0-plugins-base`.
- GNOME's screen-sharing indicator is visible from the chosen window until the frame is taken,
  including any capture delay.
- The ScreenCast portal has no permission-store entry for Annota. The chooser is the consent.
