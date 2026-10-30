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

## 5. Verify
- [ ] 5.1 Tests for every scenario (unit, golden-image or documented manual check)
- [ ] 5.2 `reviewer` agent: specs, mockup fidelity, clippy/fmt/tests green
- [ ] 5.3 `.deb` builds and installs on Ubuntu 24.04, lintian clean
- [ ] 5.4 German translation updated
