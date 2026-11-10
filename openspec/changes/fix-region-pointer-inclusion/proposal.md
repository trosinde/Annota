# Proposal

**Roadmap phase:** MVP (milestone 0.1)
**Benchmark feature (docs/feature-catalog.md):** "Include/exclude cursor"

## Why

With "include pointer" on, a region capture never shows the pointer, while the `capture`
capability's "Cursor inclusion" requirement says Annota draws the pointer into the image for
*region and screen* captures. Verified live on the maintainer's desktop (GNOME Shell 50.1,
Wayland, xdg-desktop-portal 1.21.1) on 2026-11-10: full-screen captures contain the pointer,
region captures do not.

The cause is which position the region overlay reports. `Session::note_pointer`
(`src/capture/overlay.rs:190`), fed from `MonitorOverlay::motion` (`overlay.rs:406`), keeps only
the *first* pointer position the overlay ever sees — where the mouse stood when the overlay
opened, in practice over the capture button in the header bar or wherever the shortcut was
pressed. The user then drags the selection somewhere else, so `region()` (`src/capture/mod.rs`)
draws the arrow at `at.x * scale - rect.x`, which lands outside the crop, and cairo clips it away.
Unlike `screen()`, `region()` has no `contains` guard, so the drop is silent: no toast, no log,
nothing in the image.

One path already works and is worth keeping: a region capture *with* a delay re-grabs the desktop
and re-probes the pointer after the countdown, so the user places the mouse during the countdown
and it lands in the image. That is a deliberate, useful gesture — it is simply undiscoverable.

## What Changes

- A region capture SHALL use the pointer position at the moment the selection is **confirmed**,
  not the position the overlay first saw. The frozen image never contained a pointer anyway
  (the Screenshot portal has no pointer option for non-interactive grabs), so Annota composites
  it either way; the question is only which position is the honest one to use, and "where the
  pointer is when the user takes the shot" matches what the user sees on screen at that moment.
- `region()` SHALL gain the `contains` guard that `screen()` already has: the pointer is drawn
  only when its hot spot lies inside the selection, and otherwise nothing is drawn. This replaces
  silent clipping with a stated rule, and makes the behaviour testable.
- The region overlay SHALL make the outcome visible while "include pointer" is on, so the user can
  tell **before** confirming whether the pointer will end up in the image, and can learn the
  gesture that puts it there.
- The delay path keeps its current behaviour (re-grab and re-probe after the countdown).
- Screen and window modes are untouched. Not a breaking change: the setting, its default
  (exclude) and the overlay's pointer toggle all keep their meaning.

**Known limitation this change must design around, not hide:** confirming by releasing the drag
leaves the pointer exactly on the selection edge, where the arrow body extends outside the crop
and the `contains` guard drops it. The gesture that reliably includes the pointer is therefore
*select the region → move the pointer inside it → press Enter* (or use a delay). The maintainer
chose this direction (option A of three) with that caveat known; the overlay work above is what
keeps it from being a trap.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `capture`: "Cursor inclusion" — which pointer position a region capture uses, and the condition
  under which the pointer is drawn at all.

## Impact

**Code**
- `src/capture/overlay.rs` — `Session::first_pointer` / `note_pointer` are replaced by the
  position at confirmation; the region toolbar's pointer toggle gains its preview state.
- `src/capture/mod.rs` — `region()` gains the `contains` guard, mirroring `screen()`.
- `data/ui/region-overlay.blp` — whatever the approved mockups add.
- Tests: the silent-clipping case and the confirm-time position need unit coverage; today nothing
  fails when the pointer is dropped.

**Docs and design**
- `docs/design/fix-region-pointer-inclusion/` — mockups for the overlay's pointer preview
  (light + dark, 1x + 2x), required before implementation (CLAUDE.md §5).

**Not affected**
- No new dependency, no portal call, no change to `.annota`, export or packaging.

## Out of scope

- Screen and window capture modes — screen mode is verified working and keeps its code path.
- The delay path's re-probe, which already does the right thing.
- The drawn pointer's appearance (shape, size, scaling) — `frame::pointer_path` stays as it is.
- Cursor highlight and click ripple (Phase 2, `add-screen-recording`).
- Any X11-specific pointer picking; CLAUDE.md §2 rules out direct X11 window access without an ADR.
- Capturing the *real* system cursor bitmap instead of Annota's drawn arrow.
