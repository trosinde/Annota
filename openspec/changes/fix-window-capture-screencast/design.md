# Design

## Context

See proposal.md, "Why", for the defect and the 2026-11-14 measurements.

The current code, as read on 2026-11-14:

- `capture::run` (`src/capture/mod.rs`) handles `CaptureMode::Window` with
  `portal::grab(GrabOptions::PICKER)`, i.e. Screenshot portal `interactive: true`. It runs the
  countdown *before* the grab, like every mode does.
- `permission::after_refusal` maps every window-mode refusal to `Refusal::PickerClosed`, which
  becomes `CaptureError::NothingCaptured` → toast "Nothing was captured".
- `run_hidden` hides Annota's windows for every capture, so the ScreenCast request also has no
  visible parent. That is why `xdg-desktop-portal-gnome` logs "Failed to associate portal window
  with parent window", which it also did for the probe. The chooser was shown anyway.
- `Frame::from_argb32(width, height, stride, data)` (`src/capture/frame.rs`) already builds a
  frame from cairo-layout pixels. GStreamer's `BGRA` on little-endian hosts is the same byte
  order, so no conversion code is needed beyond `videoconvert`.
- `ashpd` 0.11.1 is locked. Its `desktop::screencast` module is behind the `screencast` feature
  and needs no further crate. `OpenPipeWireRemote` returns an `OwnedFd`.
- No GStreamer crate is in `Cargo.lock` yet. CLAUDE.md §2 already names `gstreamer-rs` for
  video.

**Wayland vs X11.** Task 2.4: no GNOME X11 session was available on 2026-11-14, so
`AvailableSourceTypes` under X11 is **unmeasured**. The D6 fallback covers both answers. On Wayland, Mutter's ScreenCast backs the portal. Whether
`xdg-desktop-portal-gnome` offers a WINDOW source in a GNOME X11 session has not been measured
(task 2.4). The fallback in D6 keys on `AvailableSourceTypes` and not on the session type, so
either answer is handled.

## Goals / Non-Goals

**Goals:**
- Window mode delivers the chosen window as an image, with the same delivery as region and screen
  mode (`delivery::deliver(…, Source::Window)`).
- The screen-sharing session is as short as the flow allows: from chooser to frame, plus the
  delay if one is set.
- Frames exist only in memory. Nothing is written to disk on the way (CLAUDE.md §7).

**Non-Goals:**
- A reusable streaming layer for add-screen-recording. `screencast.rs` stays single-frame and
  minimal. Recording may later grow it or replace it.
- Choosing a monitor through ScreenCast.

## Decisions

### D1: ScreenCast session parameters

`CreateSession`, then `SelectSources { types: WINDOW, multiple: false, cursor_mode, persist_mode:
0 }`, then `Start(parent)`, then `OpenPipeWireRemote`. No restore token is requested and none is
stored. A persisted token would skip the chooser, and the chooser *is* the window picker.
`cursor_mode` is `EMBEDDED` when include-pointer is on and `AvailableCursorModes` contains it,
otherwise `HIDDEN` (D5). `parent` is the waiting OSD's exported handle (D8). A request with no parent was measured to
leave the chooser invisible and unanswered in several runs.

*Alternative.* Screenshot portal `interactive: true` plus a hint to choose "Window" in GNOME's UI:
rejected by the maintainer on 2026-11-14 (option 2 of 3). It stays only as the fallback (D6).

### D2: One frame through GStreamer

The pipeline is `pipewiresrc fd=<fd> path=<node> num-buffers=1 always-copy=true ! videoconvert !
video/x-raw,format=BGRA ! appsink`, built in code (not `parse_launch`, so no string assembly
around the fd). The frame comes from `appsink`'s `pull_sample` on a worker thread, with a
5-second timeout (spec). Then the pipeline goes to `NULL`, the fd closes and the session closes,
in that order and on every exit path (a `Drop` guard). The one exception is a stream that never
delivers: at the deadline the session closes at once, and the worker thread releases the pipeline
and then the fd whenever it unblocks.

*Alternative.* The `pipewire` crate directly: rejected. It needs `bindgen` and libclang at build
time (Debian build-deps and offline vendoring both get heavier), while CLAUDE.md §2 already
picks GStreamer for video.

### D3: Cutting the frame to the window

Measured 2026-11-14 (GNOME 50.1, GStreamer 1.28.2, scale 1.25, `scale-monitor-framebuffer`).
The test window was a non-maximised terminal:
- The stream's `size` is (3072, 1728), the monitor's logical size, and not the window's. The
  buffer is 6144 × 3456 BGRA.
- `pipewiresrc` attaches a `GstVideoCropMeta`: (0, 0, 2524, 2032). That rectangle is the window
  **plus its shadow margin**.
- Inside it, from the outside in: the shadow has alpha 28–41, rising towards the window; then a
  1 px outline with alpha ~200; then the window body with alpha 255, 79 px in at mid-height.
- The bounding box of alpha > 0 is (125, 137, 2274, 1787). It still contains the shadow.

The cut is therefore:
1. Restrict to `GstVideoCropMeta` when present, otherwise to the whole buffer.
2. Inside that, take the bounding box of pixels with **alpha ≥ 128**. This keeps the outline and
   the window, drops the shadow, and keeps transparent rounded corners inside the box, because
   only the outer bound counts.
3. If no pixel reaches 128 (a translucent window, e.g. a terminal with background opacity), fall
   back to alpha > 0 within the crop meta. That image then keeps a faint shadow, which beats
   losing the window.

A frame with no pixel above 0 at all is a failure ("no frame"), not an empty image. The
threshold is a named constant with unit tests built from the measured alpha profile.

### D4: Delay order

`run` gets a window branch that does chooser → `countdown::run(Window, delay)` → frame. The
current order (countdown → grab) stays for region and screen. A cancelled countdown closes the
session (the D2 guard). While the countdown runs, GNOME's screen-sharing indicator is visible.
This is accepted and named in the proposal.

### D5: Pointer

Measured 2026-11-14, fully automated (D9). A maximised probe window was chosen,
`cursor_mode` EMBEDDED was set, and the pointer was moved over the window before "Share". The
pointer **is drawn into the window frame**. Relative pointer motion did not land exactly where it
was aimed, so live tests assert "pointer present / absent" and never a position. Annota does not
composite a pointer into window frames itself: it has no pointer position relative to another
app's window on Wayland.

### D6: Fallback and error mapping

`screencast::window_source_available()` reads `AvailableSourceTypes` once per capture. Without
WINDOW (or without the interface), window mode takes today's `portal::grab(PICKER)` path,
unchanged, including `after_refusal`'s `PickerClosed`. On the ScreenCast path:
- Response code 1 from `Start` (chooser closed) → `CaptureError::Cancelled`, with no message.
- Annota's own Cancel or `Esc` on the waiting indicator (D8) → closes the session →
  `CaptureError::Cancelled`, with no message.
- Response code 2, no stream, or a frame timeout → `CaptureError::Failed(cause)`, which gives the
  existing toast "The capture failed. Nothing was captured." and an `eprintln!` with the cause.

Measured 2026-11-14, afternoon. Ten runs of the probe were made in all, some without a parent,
some with a probe window as parent:
- The chooser was visible and a window was shared: three runs, each `0` with a frame in
  115-397 ms.
- The user clicked "Abbrechen" (no parent): no answer within 90 s.
- The user saw nothing at all: several runs. One of them still answered `0` after 9.6 s,
  presumably the user's Enter in the terminal confirming a chooser that had keyboard focus but
  was not visible. Another, parented, answered `1` after 4.5 s without the user doing anything.
- Restarting `xdg-desktop-portal-gnome` dropped **five backend `Start` calls that had never been
  answered** ("A backend call failed: Message recipient disconnected from message bus without
  replying"). After that, `xdg-desktop-portal` had to be restarted too, because it reported
  `AvailableSourceTypes` 0 until then.

Measured again later the same day, with a visible parent window and the chooser's "Cancel"
button pressed over AT-SPI (`tools`-side automation, no human timing involved): code `1` after
2.3 s, three runs out of three. With a parent, a cancel is therefore reported reliably. The
earlier unanswered cancel was a request without a parent. Row selection in the chooser cannot be
automated: GNOME refuses AT-SPI focus, selection and synthetic keys on Wayland, so "Share" stays
a human step in every live check.

The conclusion, decided by the maintainer: Annota does not depend on GNOME answering. Its own
cancel (D8) always ends the attempt and closes the session, so no request is left behind in the
portal. Whether "Abbrechen" yields `1` reliably is left to the live checks. If it does not, the
indicator stays until the user cancels it, which is an honest outcome. The spec's scenarios
avoid any claim about the response code.

### D8: Waiting indicator and parent window

From `Start` until the chooser answers, Annota shows a small, non-modal OSD (sheet 02),
"Waiting for a window…" with a Cancel button and `Esc`. It is a top-level Annota window placed
like the countdown OSD (`countdown.blp`). It is also the chooser's **parent**: its exported
handle (`ashpd::WindowIdentifier`) goes into `Start`. With no Annota window open (shortcut, CLI),
this OSD is the only window, so `window_for_question` is not needed. Annota's other windows stay
hidden as today (`run_hidden`), so the chooser list does not offer them. The OSD itself is
excluded from the list, because it is the parent.

On an answer, the OSD goes away. Then come the countdown (D4) if a delay is set, the frame, and
the editor. Cancel or `Esc` closes the ScreenCast session (`Session.Close`). That also ends
GNOME's pending request, which is what keeps requests from piling up. A second window-capture
request while the OSD is open raises the OSD and starts nothing new (`BUSY`, as today).

*Alternatives.* A plain timeout without UI: rejected. It blocks Annota for the timeout after a
cancel that GNOME does not report. Reusing an existing Annota window as parent: kept as a
fallback only if the OSD cannot export a handle, because hidden windows cannot parent a dialog.

### D7: Permission

The ScreenCast portal has no permission-store entry that Annota reads. The chooser is the
consent. `with_permission`'s screenshot-permission logic therefore does not apply to the
ScreenCast path: `first_step(Window, _)` already returns `Grab`, and that stays.

### D9: Automated live tests

Every scenario runs automatically on a real GNOME session, with no human in the loop:
- **Annota's own UI** (the Window card, the launcher, the waiting OSD's Cancel, the countdown):
  AT-SPI actions on Annota's widgets.
- **GNOME's chooser:** the "Share" and "Cancel" buttons through AT-SPI. Row selection through
  synthetic keys (`Tab`, `Down`, `space`) from `tools/portal-input.py`, which uses the
  RemoteDesktop portal with a restore token. The maintainer granted it once, on 2026-11-14. The
  token is stored in `~/.cache/annota-tests/remote-desktop.token` (0600) and is removed with
  `tools/portal-input.py forget`.
- **Pointer:** moved with the same tool before "Share". Assertions are presence only (D5).
- **Results:** the test reads Annota's outcome from the editor's accessible tree (document title
  and toast text) and from the delivered frame's size. It never reads image content beyond the
  alpha and presence checks.

The suite is `tools/live-window-capture.py`, outside `cargo test`. It needs a logged-in GNOME
session, Annota installed from the current build, and the token. The RemoteDesktop portal is test
tooling only; Annota itself never uses it. The scenarios with no automatic path, which are the
fresh-install check on Ubuntu 24.04 (6.2) and X11 (2.4), stay manual.

## Mockups

The chooser, GNOME's screen-sharing indicator and the screenshot UI are GNOME's. Annota adds one
screen of its own, the waiting OSD (D8). The design gate covers the flow sheet and that OSD in
`docs/design/fix-window-capture-screencast/`:

| # | Screen | Scenarios | File |
|---|---|---|---|
| 01 | Flow: Window card / launcher → GNOME window chooser (drawn as a neutral placeholder, not a copy of GNOME's dialog) → countdown 07 (only with delay) → editor with "Window captured · W × H" | Window on Wayland; Window mode from the welcome page; Delayed window capture | [png/01-window-capture-flow-light@1x.png](../../../docs/design/fix-window-capture-screencast/png/01-window-capture-flow-light@1x.png), source [src/01-window-capture-flow.html](../../../docs/design/fix-window-capture-screencast/src/01-window-capture-flow.html) |
| 02 | Waiting OSD "Waiting for a window…" with Cancel, above the (placeholder) chooser; light + dark | Waiting for the chooser; Cancel while waiting | [png/02-window-wait-osd-light@1x.png](../../../docs/design/fix-window-capture-screencast/png/02-window-wait-osd-light@1x.png), source [src/02-window-wait-osd.html](../../../docs/design/fix-window-capture-screencast/src/02-window-wait-osd.html) |

Index with scenario mapping and redlines:
[docs/design/fix-window-capture-screencast/index.md](../../../docs/design/fix-window-capture-screencast/index.md).

It replaces add-screen-capture mockup 05 (window via screenshot UI) as the reference for window
mode. The flow sheet must show the OSD between panels a and b once sheet 02 exists. "Chooser closed", "Countdown cancelled", "No frame arrives", "No window source", the
pointer scenarios and the packaging scenarios have no screen of their own. They are covered by
tests and by `manual-checks.md` rows.

Design approved: 2026-11-14

## Decided during apply

2026-11-14, tasks 3.1 and 4.1-4.7:
- **`pipewiresrc` blocks for 30 s on a stream that never starts.** Measured with GStreamer 1.28
  against the live-test stand-in: its start waits 30 s (in about one run of four the pipeline
  retries and waits 30 s more), and setting the pipeline to `NULL` from another thread blocks
  until then. D2's order is kept without ever blocking the main loop: GStreamer init, pipeline
  build, `PLAYING`, the pull, `NULL` and the fd close all run on the worker thread, and
  `Session.Close` follows only once the worker has finished. At the deadline (5 s + 2 s grace)
  Annota reports the failure at once; the session close then waits for the worker. In that one
  case GNOME's sharing indicator can stay up to ~60 s after the toast. Closing the session at the
  deadline instead would end it at once but break D2's order. **Open for the maintainer.**
- `gst::init()` (the first-run plugin-registry scan) starts on a thread when the chooser opens,
  so it is done by the time the frame is taken.
- `ashpd` 0.11.1 has no `screencast` feature; `desktop::screencast` is always compiled.
- `gstreamer-rs` 0.24.5, the newest line with rust-version <= 1.85, uses glib 0.21 while gtk4 0.9
  uses glib 0.20. Both are linked (no `links` conflict, one libglib). No GStreamer object crosses
  into GTK code. 0.23 would share glib 0.20, but it is not the newest that fits.
- The frame's BGRA bytes go into `Frame` as they are (cairo `ARGB32`, premultiplied). Whether
  Mutter's window frames are premultiplied is not measured. Only partly transparent pixels (corner
  antialiasing, the outline) would differ.
- OSD body: the countdown OSD's `rgba(36, 36, 40, 0.88)` stands in for `--osd-bg` + 16 px blur,
  as in 07, because GTK has no backdrop blur. The Cancel pill's `min-width` is 84 px of content
  plus 2 x 18 px padding = 120 px (GTK's `min-width` is the content box).
- The OSD takes pointer input only over its own rectangle. The input region is set after each paint
  when the rectangle changes.

2026-11-14, maintainer follow-ups to the entry above:
- **Never-starting stream: the session closes at the deadline.** The maintainer decided the open
  point above: at the deadline (5 s + 2 s grace) Annota calls `Session.Close` at once and no
  longer waits for the worker, so GNOME's sharing indicator goes away with the error toast. The
  worker still sets the pipeline to `NULL` and then closes the fd, in that order, whenever
  `pipewiresrc` unblocks. D2's order holds for every other path. Test:
  `screencast::tests::a_hanging_stream_closes_the_session_at_the_deadline`.
- **`gstreamer-rs` 0.23 instead of 0.24.5.** `gstreamer` 0.23.7, `gstreamer-app` 0.23.5,
  `gstreamer-video` 0.23.6 (all rust-version 1.71.1) are the gtk-rs 0.20 generation, so they
  share glib 0.20 with gtk4/libadwaita and `cargo tree -d` shows a single glib. This replaces the
  0.24.5 bullet above. The code needed no change for the older API.

2026-11-14, clipboard defect found by `tools/live-window-capture.py`:
- **Symptom.** With after-capture "Both" or "Copy to Clipboard", a window capture announced
  "Window captured · 800 × 500 · Copied to clipboard", but the clipboard kept the previous owner's
  text. It was not window-specific: region and screen captures did not land either.
- **Cause, measured on GNOME 50.1 (mutter 50.1, GTK 4.22) with `WAYLAND_DEBUG` and small GTK
  probes.** Mutter ignores `wl_data_device.set_selection` unless (1) a surface of the client has
  the keyboard focus and (2) the serial is one of the client's input events. GTK sends the serial
  of the client's last key or button press, or 0 if there was none. Keyboard focus alone and a
  pointer entering the window give no serial. A focused window that copies with serial 0 is
  ignored. A copy with no focused Annota surface is ignored. A focused window that copies with
  the serial of an earlier key press lands, even if another Annota window, since destroyed,
  received that key, and even after input to other programs since then. `delivery::deliver`
  copied before anything of Annota had the focus: the region overlay and the waiting OSD were
  already destroyed, and the editor was presented only after the copy. A capture from
  `annota --capture` or a global shortcut has no key or click in Annota at all, because the
  window chooser and the screenshot UI belong to GNOME. Such a capture cannot be copied by GTK
  on its own.
- **Not a focus defect.** The editor of a window capture does get the keyboard focus: GTK's own
  activation token works, with `wl_keyboard.enter` about 0.5 s after the editor is presented, and
  AT-SPI shows it ACTIVE. That was true before this fix as well. The live suite saw it inactive
  because every `wl-paste` call maps a surface of its own to take the focus; while the suite
  polls the clipboard, the focus moves back and forth about every 260 ms.
- **Fix.** `delivery::copy_lands(wayland, focused, input_seen)` states the rule; X11 always
  lands. Every Annota window watches for key and button presses (`delivery::watch_input`,
  hooked in `application::run`). On Wayland the copy waits until an Annota window is active, at
  most 2 s, and is made only if it can land. "Both" opens the editor, copies once the editor is
  active, and only then shows the toast (`editor::open_new_capture` + `OpenedCapture::announce`).
  "Copy to Clipboard" on Wayland presents the message window (`present_for_message`) to take the
  focus, also when no Annota window was open. The toast says "· Copied to clipboard" only when
  the copy was made under those conditions. Otherwise the toast shows the heading alone and
  offers Copy (mockup 12's editor toast), and a click on Copy supplies the serial. The quick-save
  fallback no longer copies first, so its toast now offers Copy also with "Both".
- **Measured after the fix:** region "Both" and region "Copy to Clipboard" from
  `annota --capture region` (drag and Enter in the overlay) put `image/png` on the clipboard.
  Screen and window captures from `annota --capture`, and a window capture from the Window card
  pressed through AT-SPI, show the heading with Copy and leave the clipboard alone. That holds for
  `--capture window`, the Window card, "Pointer in a window capture" and "Cursor excluded". After
  one real key press (Shift, through the RemoteDesktop portal) in the focused editor, Copy on the
  toast puts the image on the clipboard: 800 × 500, and 3840 × 2100 for a maximised window with
  and without the pointer as expected. Ctrl+C could not be used for this check on the
  maintainer's desktop: its GlobalShortcuts portal binds Annota's window capture to
  `<Control>c` (region `<Control>y`, screen `<Control>x`), so GNOME takes Ctrl+C before any
  application sees it.
- **Deviation from the archived `capture` spec, open for the maintainer.** "After-capture
  destination / Copy to clipboard" says no window opens. On Wayland a window now opens, because
  without one no copy can land. Window and screen captures from a shortcut cannot honour
  "Copy to Clipboard" or the clipboard half of "Both" automatically. Doing that would need a raw
  Wayland data source that uses the keyboard-enter serial, which is the technique `wl-copy`
  uses. That would mean an ADR and either `wayland-client` or unsafe FFI. Neither is in this
  change.

## Risks / Trade-offs

- [GNOME's chooser sometimes never becomes visible and its request never answers (measured,
  D6)] → the waiting OSD (D8) gives the user a way out and closes the session, so nothing piles
  up. If the chooser often stays invisible, users see "Waiting for a window…" with nothing to
  choose. The live checks record how often this happens with the OSD as parent.
- [GStreamer becomes an MVP dependency: bigger vendoring, two more runtime packages] → it is the
  stack CLAUDE.md §2 already fixed for video, so recording will need it anyway.
- [`gstreamer-rs` MSRV or its minimum GStreamer version does not fit 1.85 / 1.24] → task 3.1
  picks the newest version that fits both and records it. Feature flags stay at `v1_24` or lower.
- [Screen-sharing indicator during the delay may surprise users] → accepted. It is GNOME's honest
  signal that a stream is open.
- [Two active changes modify `capture` "Cursor inclusion"] → this delta is written on top of
  `fix-region-pointer-inclusion`'s version. Archive that change first. If it changes again,
  rebase this delta before archiving.

## Migration Plan

No data migration. Rollback is reverting the code and the packaging deps. The ADR is then marked
superseded.

## Open Questions

None that change the plan. D5 and D6 carry explicit stop points in tasks 2.1 and 2.3.
