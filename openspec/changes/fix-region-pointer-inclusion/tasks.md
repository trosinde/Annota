# Tasks

## 1. Design gate

- [x] 1.1 `designer` agent: mockups for the region overlay's pointer preview in
      `docs/design/fix-region-pointer-inclusion/` (light + dark, 1x + 2x) covering all four
      scenarios of the "Pointer preview in the region overlay" requirement. Brief: the overlay
      already draws the selection, its handles, the loupe near the pointer and the snapping
      guides — the preview must not compete with those for the same screen area (design.md D3).
      It must also make the working gesture discoverable: select the region, move the pointer
      inside it, press `Enter`.
- [x] 1.2 Link each mockup to its spec scenario in design.md's "Mockups" table
- [x] 1.3 Maintainer approval recorded in design.md (`Design approved: 2026-11-10`)

## 2. Pointer position at confirmation

- [x] 2.1 Replace `Session::first_pointer` / `note_pointer` (`src/capture/overlay.rs:156`, `:190`,
      read at `:512`) with the active overlay's current `state.pointer` at the moment the outcome
      is built (design.md D1). No `probe_pointer` call on this path.
- [x] 2.2 No position at all (`state.pointer` is `None`, e.g. `Enter` straight after a keyboard
      nudge) is treated as "outside": no pointer drawn, no fallback to a stale position
- [x] 2.3 Unit test: the position reported for a confirmed selection is the one at confirmation,
      not the first one the overlay saw — this is the regression test for the reported defect
- [x] 2.4 Unit test: a keyboard-only region capture reports no pointer position

## 3. Draw only inside the selection

- [x] 3.1 Add the `contains` guard to `region()` (`src/capture/mod.rs`), mirroring `screen()`:
      the pointer is drawn only when its hot spot lies inside the selection rectangle
      (design.md D2)
- [x] 3.2 Unit test: hot spot inside → the pointer is in the frame (assert the pixel, as
      `frame::pointer_is_visible_after_drawing` does); hot spot outside → the frame is byte-identical
      to the un-drawn crop
- [x] 3.3 Unit test: hot spot one pixel inside the lower-right corner still draws (the guard tests
      the hot spot, not the whole arrow), and a release exactly on the edge does not
- [x] 3.4 Unit test: the existing multi-monitor filter still rejects a pointer on another monitor
      before the guard sees it

## 4. Overlay preview

- [x] 4.1 Implement the preview in `data/ui/region-overlay.blp` and `src/capture/overlay.rs`
      exactly as approved in 1.3. The two icons (`annota-pointer-included-symbolic`,
      `annota-pointer-excluded-symbolic`) were drawn by the `designer` agent (CLAUDE.md §9) and
      registered in `data/resources.gresource.xml`; `gresource list` confirms both are embedded.
      `tools/build-icons.py` named by D8 does not exist, so like every other icon in
      `data/icons/` these are authored files; the kit addition stays a D8 follow-up
- [x] 4.2 The preview is driven by the same inside/outside test as task 3.1, so preview and
      result cannot disagree (design.md D3)
- [x] 4.3 The preview updates on pointer motion and on selection move/resize, and is absent while
      include-cursor is off
- [x] 4.4 Unit test: the preview's verdict equals the drawing guard's verdict for the same
      selection and pointer position
- [x] 4.5 German translation updated in the same commit (CLAUDE.md §7) for any new string

## 5. Verify

- [x] 5.1 Every scenario of both requirements has a test or a documented manual check; live-only
      checks go into `manual-checks.md` with what the automated tests do and do not cover
- [x] 5.2 `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `meson test -C _build` green
- [x] 5.3 Delayed region capture still places the pointer after the countdown (design.md D4) —
      verified by reading `region()`: with a delay the position still comes from a fresh
      `probe_pointer` after the countdown, untouched by the `Session::pointer` rework. The only
      difference is that the probed position now passes the `contains` guard, which removes just
      the case that was invisible anyway (drawn outside the crop, clipped by cairo). Live check 8
      in `manual-checks.md`
- [x] 5.4 Screen and window modes unchanged — `screen()` keeps the pointer it draws today;
      verified: `git diff` over this change's commits shows no line in `screen()` added or removed
- [x] 5.5 `reviewer` agent (fresh context, did not write the code): specs, mockup fidelity,
      tests, quality gates — 2026-11-22. First pass: blocking B1 (fractional release on the
      edge drew a sliver) and B2 (preview tags without an on-screen test), both fixed. Second
      pass on the fixes, same day: no blocking findings; its non-blocking notes (Capture-button
      wording, proposal wording, sub-pixel note) are folded in. See design.md "Review findings,
      task 5.5". Mockup fidelity compared by reading, not by screenshot; the `.deb` (5.6) was
      not rebuilt
- [x] 5.6 `.deb` builds via `build-aux/make-deb.sh`, `lintian -I --pedantic` clean —
      `../annota_0.1.0~dev1_amd64.deb` (2.4 MB, 2026-11-10), lintian silent. Built on the
      maintainer's Ubuntu 26.04 with cargo 1.93, not the pinned 1.85; the 24.04 gap is the
      CI job's, shared with every other change
- [ ] 5.7 Maintainer: live check on a real desktop — region capture with include-cursor on,
      pointer inside the selection, confirmed with `Enter`; and the same with the pointer outside

## 6. Follow-ups from the 5.5 review (not part of this change)

Recorded so they are not lost; none blocks archiving this change, so they are plain notes, not
tasks. Details in design.md "Risks / Trade-offs".

- Scenario gate: build `_build/data/resources.gresource` (or fail with a clear message) before
  the live run; install `libgstreamer1.0-dev` on the maintainer's desktop.
- `fix-window-capture-screencast`: take over the corrected "Cursor inclusion" wording in its
  delta (`specs/capture/spec.md:57`, the requirement, and `:74`, the WHEN of "Pointer on the
  selection edge"), and archive it after this change.
- Minor mockup deviations of the pointer chip (keyline, suffix opacity, joint flip with the
  loupe) and `accessible-role: presentation` on its decorative glyphs.
