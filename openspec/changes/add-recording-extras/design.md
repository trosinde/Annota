# Design: Add recording extras (stub)

Status: stub. Split off `add-screen-recording` on 2026-11-22. Complete this design after
`add-screen-recording` is archived and its probes (P1–P12) are measured.

## Mockups (design gate)

The screens of the first set (October 2026) that belong here were moved to
`docs/design/add-recording-extras/`; see
[`index.md`](../../../docs/design/add-recording-extras/index.md). **They predate the core redesign
and must be redrawn** against the approved core mockups before this gate can close.

- [01-recording-setup-extras](../../../docs/design/add-recording-extras/png/01-recording-setup-extras-light@1x.png)
- [02-webcam-overlay-options](../../../docs/design/add-recording-extras/png/02-webcam-overlay-options-light@1x.png)
- [03-recording-control-effects](../../../docs/design/add-recording-extras/png/03-recording-control-effects-light@1x.png)
- [04-draw-while-recording](../../../docs/design/add-recording-extras/png/04-draw-while-recording-light@1x.png)
- [05-swapped-full-webcam](../../../docs/design/add-recording-extras/png/05-swapped-full-webcam-light@1x.png)
- [06-video-editor-trim](../../../docs/design/add-recording-extras/png/06-video-editor-trim-light@1x.png)
- [07-video-editor-cut-range](../../../docs/design/add-recording-extras/png/07-video-editor-cut-range-light@1x.png)
- [08-export-gif](../../../docs/design/add-recording-extras/png/08-export-gif-light@1x.png)

Design approved: _pending_

## Direction (to be decided at this change's propose refinement)

- Everything visible in the video is **composited by the GStreamer pipeline** (`compositor`
  with the screen, webcam, pointer ring and drawing layers), never an Annota window over the
  desktop: the core change showed that Annota cannot keep a window on top on Wayland.
- Pointer position: ScreenCast `cursor_mode` Metadata (pointer position + bitmap per buffer) —
  must be probed (E1).
- Clicks: no portal exposes global pointer clicks to a native app on Wayland. Candidates: AT-SPI
  events (as planned for `add-step-capture`), X11 only, or not at all (E2, E3). The spec only
  offers click animation where clicks can be observed.
- Drawing while recording: the user needs an input surface above the recorded content, which
  Annota cannot keep on top on Wayland. Candidate: a fullscreen transparent window that takes
  input only while Draw is on, its strokes composited into the stream (E4). Decide after the
  probe; the spec hides Draw where it cannot work.
- Webcam: `v4l2src` or a PipeWire camera node; the Camera portal for Flatpak.
- Video editor: opens from the core's Export Recording window ("Edit…"); the stop flow becomes
  a MODIFIED delta on `screen-recording` once the core is archived (task 1.1). Undo through the
  existing `Command` pattern; non-destructive edit list applied at export.
- GIF: gifski is named in CLAUDE.md §2, but it is AGPL-3.0-or-later (Q1). Alternative:
  GStreamer `gifenc` (gst-plugins-rs, may not be packaged on Ubuntu 24.04) or `ffmpeg`-free
  palette quantisation in-house.

## Probes (CLAUDE.md §5)

| # | Probe |
|---|---|
| E1 | ScreenCast `cursor_mode` Metadata: pointer position units, hotspot, bitmap, frequency on GNOME 46 and 50 |
| E2 | Can a native app observe global clicks on GNOME Wayland via AT-SPI, and with what latency? |
| E3 | Same on GNOME X11 through the portal-only rule (CLAUDE.md §2) |
| E4 | A fullscreen transparent input window during a screen recording: stays on top while drawing? Excluded from nothing — appears in the stream, so only its drawings may be visible |
| E5 | Webcam: `v4l2src` vs PipeWire camera on Ubuntu 24.04, formats, latency, two consumers |

## Risks / open questions
- **Q1 (maintainer):** gifski's AGPL-3.0 licence vs. the MIT binary (CLAUDE.md §10) — accept,
  replace, or drop GIF.
- **Q2:** Is click animation worth shipping if it only works on some desktops?
- **Q3:** Video editor scope for a first version (trim + cut only, combine later?).
