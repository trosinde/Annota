# Design

## Context

See proposal.md — Why, for the defect and its root cause.

Three constraints shape the approach:

- **The frozen image has no pointer in it.** The Screenshot portal offers no pointer option for
  non-interactive grabs, so Annota composites a drawn arrow (`frame::draw_pointer`,
  `frame::pointer_path`, 22 logical px tall, hot spot at the origin). Any position is therefore a
  choice, not a property of the captured pixels.
- **Wayland gives no global pointer position.** Annota learns it only from GTK events on its own
  surfaces: `overlay::probe_pointer` maps a fullscreen window per monitor and waits for the first
  `enter`/`motion` (400 ms timeout), and the region overlay gets the position from
  `MonitorOverlay::motion`. Measured on the maintainer's machine 2026-11-10: a mapped fullscreen
  window receives its first `enter` after ~285–295 ms, and `probe_pointer` as a whole returned in
  104–117 ms during a real capture. **The same code path is used on X11** — Annota does not read
  the X11 pointer directly (CLAUDE.md §2), so there is no Wayland/X11 behaviour difference in this
  change. Both get the position from GTK events on the overlay's own windows.
- **The overlay already tracks the pointer continuously.** `MonitorOverlay::motion` keeps
  `state.pointer` for the loupe and the snapping guides. The position needed at confirmation time
  is already in hand; only `Session::first_pointer` throws it away by keeping the first value.

## Goals / Non-Goals

**Goals:**
- A region capture uses the pointer position at confirmation.
- A dropped pointer becomes a stated rule (`contains`) instead of silent cairo clipping.
- The overlay tells the user, before confirming, whether the pointer will be in the image.

**Non-Goals:**
- Changing `screen()` or window mode. `screen()` already probes immediately before cropping and
  already has the `contains` guard; it is the model this change copies, not a target.
- Changing the drawn pointer's shape or size (`frame::pointer_path`).
- Capturing the real system cursor bitmap or its theme.
- A keyboard shortcut for the overlay's pointer toggle — it stays a toolbar control.

## Decisions

### D1: Replace `first_pointer` with the position at confirmation

`Session::first_pointer` (`overlay.rs:156`, set in `note_pointer` at `:190`, read at `:512`) keeps
the first position seen and is the direct cause of the defect. It is replaced by reading the
active overlay's current `state.pointer` when the outcome is built, converted to image coordinates
the same way the selection rectangle is.

*Alternative considered — keep `first_pointer` and re-probe with `probe_pointer` at confirmation:*
rejected. The overlay is on screen and already receives motion events, so mapping another
fullscreen window per monitor would cost ~100–300 ms per capture and could fail its timeout, for a
value the overlay already holds.

*Alternative considered — keep `first_pointer` entirely (option B of the proposal's three):* the
maintainer chose this direction instead. Recorded here because the trade-off below is real.

### D2: `region()` gets `screen()`'s `contains` guard

`region()` draws unconditionally and lets cairo clip. The guard makes the rule explicit and gives
the tests something to assert. The guard is evaluated against the selection rectangle in image
coordinates, matching `screen()`'s `rect.contains(...)` on the crop.

The hot spot alone decides. The arrow extends right and down from it, so a hot spot one pixel
inside the lower-right corner still produces a nearly fully clipped arrow — accepted, because any
other rule (requiring the whole arrow to fit, nudging the arrow inwards) either suppresses legible
pointers or draws the pointer somewhere it never was.

### D3: The overlay shows the outcome before confirmation

Required by the spec's new "Pointer preview in the region overlay". The visual form is the
designer's decision; this design fixes only what it must be true about:

- It is driven by the same test as D2, so preview and result cannot disagree.
- It updates on pointer motion and on selection changes, both of which already trigger a redraw.
- It is absent while include-cursor is off.

The overlay already draws a loupe near the pointer and snapping guides, so the design must not
produce a third thing competing for the same screen area — a note for the designer, not a
decision made here.

### D4: The delay path is left alone

`region()` re-grabs and re-probes after the countdown. That already yields the position at capture
time and is the one gesture that reliably places the pointer. Its scenario is written down so a
later refactor cannot drop it silently.

## Mockups

To be produced by the `designer` agent in `docs/design/fix-region-pointer-inclusion/`
(light + dark, 1x + 2x) and linked here with a scenario map before implementation starts
(CLAUDE.md §5). Scenarios needing a screen:

| Spec scenario | Mockup |
|---|---|
| Pointer preview — Pointer will be included | _pending_ |
| Pointer preview — Pointer will not be included | _pending_ |
| Pointer preview — Indication follows the selection | _pending_ |
| Pointer preview — No indication when excluded | _pending_ |

Design approved: _pending_

## Risks / Trade-offs

- **Releasing the drag leaves the pointer on the selection edge, so the common gesture still
  yields no pointer.** → This is the known cost of the chosen direction (option A). D3 is the
  mitigation: the overlay has to make the outcome visible and the working gesture (move inside,
  then `Enter`) learnable. If the mockups cannot make that discoverable, this is the point to
  reconsider option B rather than ship a toggle that looks broken.
- **A capture confirmed from the keyboard without the pointer ever entering the overlay has no
  position at all** (`state.pointer` is `None` — e.g. `Enter` straight after a nudge, or a capture
  driven entirely by keyboard). → Treat it exactly as "outside": no pointer drawn, preview shows
  none. No fallback to a stale position.
- **Multi-monitor:** the selection lives on one monitor, and the pointer may be on another. → The
  existing `pointer.filter(|p| p.monitor == monitor)` already handles this and stays; the
  `contains` guard then never sees a foreign-monitor position.
- **The preview is a fourth thing drawn on the overlay** (selection, handles, loupe, guides). →
  Flagged to the designer in D3; the mockups decide.
