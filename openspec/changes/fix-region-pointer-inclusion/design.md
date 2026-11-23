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

**Corrected during apply (2026-11-10).** D1 said to read "the active overlay's current
`state.pointer`". That is wrong: `Session::active` is set only from `drag_begin` and `select_all`,
so it names the monitor holding the *selection*, never the one the pointer is on, and
`state.pointer` is never cleared once set. Reading it would have used a stale position from the
selection's monitor and stamped `monitor: self.index` on it, so the existing monitor filter in
`region()` could never reject it. What was built instead: `Session::first_pointer` became
`Session::pointer`, which `note_pointer` overwrites on every motion, so it is session-wide and
always carries the monitor the pointer last moved on. A `leave` handler on each canvas clears it
(`Session::clear_pointer`), which also covers the pointer moving onto the toolbar or the hint bar
— chrome that is not in the frozen image, so no honest position is left. The clear only applies to
a position still belonging to that monitor, because GTK does not order the old surface's leave
before the new surface's enter.

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

**Built as a shared decision (2026-11-10).** Rather than inlining the test in `region()`, it is
`IRect::pointer_inside(logical, scale) -> Option<(f64, f64)>` in `capture::geometry`, wrapped by
`capture::region_pointer`, which applies the monitor filter first. Task 4.2 requires preview and
result to agree; sharing one function makes that structural instead of a thing to keep in step.

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

`docs/design/fix-region-pointer-inclusion/index.md` — 7 screens, light + dark, 1x + 2x, with the full
scenario map, the redlines and the EN/DE strings. The preview is three parts on existing anchors: a
preview arrow at the hot spot (the same `frame::pointer_path` outline), a chip 30 px right of the hot
spot in the band above the loupe, and a segment prepended to the hint bar that teaches
*move the pointer inside → `Enter`*.

| Spec scenario | Mockup |
|---|---|
| Pointer preview — Pointer will be included | [01-pointer-included](../../../docs/design/fix-region-pointer-inclusion/png/01-pointer-included-light@1x.png) |
| Pointer preview — Pointer will not be included | [02-pointer-not-included](../../../docs/design/fix-region-pointer-inclusion/png/02-pointer-not-included-light@1x.png) |
| Pointer preview — Indication follows the selection | [03-indication-follows-selection](../../../docs/design/fix-region-pointer-inclusion/png/03-indication-follows-selection-light@1x.png) |
| Pointer preview — No indication when excluded | [04-pointer-excluded-no-indication](../../../docs/design/fix-region-pointer-inclusion/png/04-pointer-excluded-no-indication-light@1x.png) |
| Cursor inclusion — Pointer on the selection edge | [05-drag-pointer-on-edge](../../../docs/design/fix-region-pointer-inclusion/png/05-drag-pointer-on-edge-light@1x.png), [06-released-on-edge](../../../docs/design/fix-region-pointer-inclusion/png/06-released-on-edge-light@1x.png) |
| Implementation redlines (sizes, colours, strings, states) | [07-preview-states-redlines](../../../docs/design/fix-region-pointer-inclusion/png/07-preview-states-redlines-light@1x.png) |

Design approved: 2026-11-10

## Review findings, resolved during apply (2026-11-10)

**B1, blocking — `pointer_left` cleared the position but never repainted.** The canvas keeps its
cached render node and the `GtkOverlay` only re-runs `child_position` on `queue_allocate`, so
moving the pointer onto the toolbar, onto the hint bar or to another monitor left the arrow and
the chip frozen where they were. In one reachable case that made the preview state the opposite of
the truth: `geometry::toolbar_position` has a third branch that puts the toolbar *inside* the
selection when it fits neither below nor above (reachable after `Ctrl+A`), so moving from inside
the selection onto that toolbar left the chip reading "Pointer in capture" while the capture would
have contained none — exactly the disagreement D3 claims is structural. Fixed by calling
`update_ui()` at the end of `pointer_left`.

Hiding behind the same function: `update_pointer_preview` hid all three hint segments whenever
there was no position, while the redlines require the segment to **stay** and keep teaching the
gesture. The chip now follows the position and the hint segment follows the toggle.

**Deliberate deviations from the redlines, left as built:**
- The in-capture arrow has no drop shadow (redline `0 1px 2px rgb(0 0 6 / 45 %)`). Cairo has no
  cheap blur, and the 2.8 px halo it is stroked on already carries the contrast the shadow was for.
- The chip has no `box-shadow` (redline `0 1px 4px rgb(0 0 6 / 35 %)`). `.annota-chip` carries
  none, so this is a kit-level gap shared with the size, loupe and snap chips, not something this
  change should fix alone.
- Mockup 05 is stale, not the code: it shows the chip without the "on the edge" suffix at the same
  hot-spot-on-the-corner geometry where mockup 06 shows it. The code shows it in both.

## Review findings, task 5.5 (2026-11-22)

Independent reviewer pass. Two blocking findings, both fixed; the scenario gate
(`tools/scenario_gate.py verify fix-region-pointer-inclusion`) exits 0 afterwards.

**B1, blocking — a fractional release on the right or bottom edge still drew a sliver of
pointer.** The selection's edges come from `to_image`, which *rounds*; `IRect::pointer_inside`
*truncated* the hot spot. A release at logical 499.6 made the right edge 500 (exclusive) and the
hot spot 499, "inside": the chip read "Pointer in capture" and the image got a cut-off arrow,
against "Pointer on the selection edge". Wayland positions are fractional, at 125 % almost always.
Fixed by one mapping, `geometry::image_px`, used by `to_image` and `pointer_inside` alike, so a
released hot spot lands exactly on the edge. `overlay::a_fractional_release_on_the_edge_draws_no_pointer`
drives a `Selection` from fractional positions at 100 % and 125 % and fails with the old code.
The drawn offset stays fractional; only the inside/outside decision is on the pixel grid. The
same rounding makes a hot spot up to half a pixel left of or above the selection count as inside;
it is drawn at a negative sub-pixel offset and loses that fraction of its tip — accepted, it
follows "the hot spot decides" and is invisible.

**B2, blocking — the preview scenarios' tags claimed the on-screen part, which nothing tested.**
The tagged tests exercised the value function `preview_for` only. Added the live test
`overlay::the_overlay_shows_what_will_be_captured`: it builds the overlay exactly as
`select_region` does (construction extracted into `build_overlay`), drives `motion`, a
`Shift+Left` nudge, `pointer_left` and the toggle's own signal, and asserts chip text, edge
suffix and hint segment as shown. It fails if `pointer_left` stops repainting (the first review's
B1). `the_preview_reaches_the_captures_verdict` now compares against the image `compose_region`
really produces, at fractional positions around every edge and at two scales; the old
`capture::the_preview_and_the_capture_cannot_disagree`, a near-tautology by its own description,
is gone. `capture::a_region_capture_draws_the_pointer_only_inside_the_selection` adds the no-delay
pixel proof for "Pointer inside / outside the selection" (non-blocking N3).

**Also fixed (non-blocking):** the spec delta named "releasing the drag" as a way to confirm; it
only enters Adjusting, confirmation is `Enter` or the Capture button (mockup 06 shows exactly
that). The requirement text and the "Pointer on the selection edge" WHEN now say so, and the
requirement states that a Capture-button confirmation never contains a pointer (the click puts
the pointer on the toolbar, `pointer_left` clears the position). A second reviewer pass on the
fixes, 2026-11-22, found nothing blocking. Two stale
test comments (arrow size, window mode's pointer).

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
- **Review 5.5, left open (non-blocking):**
  - *Gate prerequisites are undeclared.* `scenario_gate.py verify` does not build
    `_build/data/resources.gresource`; on a fresh `_build` every live test panics loading it. And
    the maintainer's desktop lacks `libgstreamer1.0-dev` (in CLAUDE.md §4's list; GStreamer came
    with `fix-window-capture-screencast`), so cargo fails before any test. The review ran with a
    pkg-config shim in a scratch directory. Fix belongs to the gate tooling, not this change.
  - *Archive order and a copied sentence.* `fix-window-capture-screencast` MODIFIES "Cursor
    inclusion" too and copies the old wording: "(`Enter`, or releasing the drag)" in the
    requirement (`specs/capture/spec.md:57` of that change) and "confirms by releasing the drag"
    in the WHEN of "Pointer on the selection edge" (`:74`). This change must be archived first (that
    change's tasks already say so), and that change's delta should take over the corrected
    wording before it is archived, or the main spec regresses to the old sentence.
  - *Mockup deviations, minor (by reading, not by screenshot):* the chip's keyline is a
    `border-left` (+3 px width) rather than an inset keyline; the suffix opacity is 0.72, not
    70 %; chip and loupe flip at their own thresholds instead of together (redline §5) — no
    overlap results, only the default placement is tested. Decorative glyph images in the chip
    and hint could carry `accessible-role: presentation`.
  - *Screen mode still truncates its hot spot* (`compose_screen`, `x as i32`). Out of this
    change's scope (screen mode is unchanged by design); its edges are the monitor's, so the
    release trap does not exist there.
  - *Kit icons (D8)* and live check 10 (symbolic recolouring of the `#222222` fills) stay open, as
    `manual-checks.md` already notes.
