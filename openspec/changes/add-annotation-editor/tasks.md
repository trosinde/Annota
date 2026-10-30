# Tasks

## 1. Design gate
- [ ] 1.1 `designer` agent: mockups for all scenarios in `docs/design/add-annotation-editor/` (light + dark, 1x + 2x)
- [ ] 1.2 Maintainer approval recorded in design.md

## 2. Model
- [ ] 2.1 Scene graph + Annotation trait
- [ ] 2.2 Command-based undo/redo
- [ ] 2.3 .annota read/write + JSON schema
- [ ] 2.4 Autosave/recovery

## 3. Canvas
- [ ] 3.1 Rendering, zoom, pan
- [ ] 3.2 Selection, handles, rotation
- [ ] 3.3 Snapping guides
- [ ] 3.4 Grouping and z-order

## 4. Editor UI
- [ ] 4.1 Header bar, properties sidebar, adaptive layout
- [ ] 4.2 Recent captures tray
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
