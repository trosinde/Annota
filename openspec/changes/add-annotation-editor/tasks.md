# Tasks

## 1. Design gate
- [x] 1.1 `designer` agent: mockups for all scenarios in `docs/design/add-annotation-editor/` (light + dark, 1x + 2x)
- [x] 1.2 Maintainer approval recorded in design.md

## 2. Model
- [x] 2.1 Scene graph + Annotation trait (`src/document/{annotation,scene,registry}.rs`; R-tree
      above 100 objects; generic `Image` object type in `src/tools/image.rs`)
- [x] 2.2 Command-based undo/redo (`src/document/command.rs`; 200 steps, dirty tracking)
- [x] 2.3 .annota read/write + JSON schema (`src/document/archive.rs`,
      `data/schema/document-1.schema.json`)
- [x] 2.4 Autosave/recovery — data layer (`src/document/autosave.rs`: write, list, restore,
      discard, 30 s schedule). The timer and the recovery dialog (mockup 14) are part of 4.1.

## 3. Canvas
- [x] 3.1 Rendering, zoom, pan (`src/render/`, `src/ui/canvas.rs`; golden tests in
      `tests/render/`; debug-only preview `annota --dev-canvas FILE`). The zoom OSD/popover
      (mockup 07) is part of 4.1.
      Reviewer fixes (phase 1): viewport-sized scrollable canvas (culling, fit on shrink),
      command validation keeps documents saveable, unknown data kept at any depth.
- [x] 3.2 Selection, handles, rotation (`src/editing/{selection,drag,ops}.rs`,
      `src/render/overlay.rs`, `src/ui/canvas.rs`): click / Shift+click / Ctrl+click / rubber
      band, select all of a type (model; the sidebar button is 4.1), move, 8 resize handles and
      the rotation handle (15° steps with Shift) via `Annotation::handles`/`drag_handle`,
      multi-selection as one box, nudge, delete, duplicate; one undo step per gesture; chips.
- [x] 3.3 Snapping guides (`src/editing/snap.rs`): canvas edges/centre and object
      edges/centres within 6 screen px, guides and a chip naming the target, Alt suspends,
      Ctrl+; toggles (persisted with the sidebar switch in 4.1).
- [x] 3.4 Grouping and z-order (`src/tools/group.rs`, `src/editing/ops.rs`): `Group` object type
      (dashed box, golden image), group/ungroup keeping ids, front/forward/backward/back;
      document-wide id checks on insert/replace.

## 4. Editor UI
- [x] 4.1 Header bar, properties sidebar, adaptive layout; zoom OSD and popover; autosave timer
      and "Restore Unsaved Captures?" dialog (mockup 14); "Newer Annota Version Needed" alert
      (mockup 15); replace the `--dev-canvas` preview window
      (`src/ui/editor/{mod,sidebar,session,dialogs,text}.rs`, `data/ui/editor-window.blp`,
      `restore-dialog.blp`, `newer-version-dialog.blp`; header bar per D1 with export-share
      placeholders, tool strip per D2 with Select only, GSettings keys `snap-to-guides`,
      `show-properties`, `show-tray`). Reachable in debug builds only (`--dev-canvas`, startup
      recovery) until 4.4/4.5. Decisions and open points in design.md, phase 2b-i.
- [x] 4.2 Recent captures tray (`src/ui/editor/tray.rs`): open documents (switch, current ring,
      unsaved dot) and the newest 50 `.annota`/`.png` files of the save folder until
      add-capture-library provides the index; drag a thumbnail onto the canvas to insert it.
- [ ] 4.3 Crop/resize/rotate/flip, canvas extension, flatten
- [ ] 4.4 `--open <file>` actually opens the editor: add `specs/app-shell/spec.md` with a
      `## MODIFIED Requirements` delta completing bootstrap-app-shell's "Command-line
      interface" requirement's open-file scenario (see the note left in
      `openspec/specs/app-shell/spec.md` after that change archives)
- [ ] 4.5 Captures open in the editor: `add-screen-capture` delivers "Open in Editor" (and "Both"),
      pasted (`Shift+Ctrl+V`) and opened/dropped images as a quick-saved PNG plus toast because no
      editor existed yet (`src/capture/delivery.rs`, `deliver`). Add `specs/capture/spec.md` with a
      `## MODIFIED Requirements` delta on the `capture` capability — "After-capture destination",
      "Image from clipboard or file" and the "Select and confirm" / "Window on Wayland" scenarios of
      "Region capture" / "Window capture" — so these open a new editor tab (mockups 12, 13 of
      add-screen-capture), and route `deliver` there. Also pick up bootstrap-app-shell's welcome page
      mode cards (its mockup 01), whose copy promises the editor.

## 5. Verify
- [ ] 5.1 Tests for every scenario (unit, golden-image or documented manual check)
- [ ] 5.2 `reviewer` agent: specs, mockup fidelity, clippy/fmt/tests green
- [ ] 5.3 `.deb` builds and installs on Ubuntu 24.04, lintian clean
- [ ] 5.4 German translation updated
- [ ] 5.5 Before archiving: resolve the phase-2a follow-ups tracked in `design.md`'s "Risks /
      open questions" (non-uniform group resize, snap guides after a ratio-locked resize, the
      unreliable live-GTK-test harness, maintainer sign-off on SHALL lines added during apply,
      nudge-history coalescing, the German-keyboard z-order shortcut, unknown-member groups
      hiding known content)
