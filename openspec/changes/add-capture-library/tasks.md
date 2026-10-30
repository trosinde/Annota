# Tasks

## 1. Design gate
- [ ] 1.1 `designer` agent: mockups for all scenarios in `docs/design/add-capture-library/` (light + dark, 1x + 2x)
- [ ] 1.2 Maintainer approval recorded in design.md

## 2. Storage & index
- [ ] 2.1 Automatic storage + preference
- [ ] 2.2 Metadata model + extraction
- [ ] 2.3 SQLite index, rebuild on mismatch
- [ ] 2.4 Folder watcher

## 3. Library UI
- [ ] 3.1 Grid + list view, day grouping, thumbnail zoom
- [ ] 3.2 Sorting
- [ ] 3.3 Preview pane + quick look
- [ ] 3.4 Item actions + undo toast
- [ ] 3.5 Multi-select + bulk actions
- [ ] 3.6 Empty state
- [ ] 3.7 Editor tray fed from library

## 4. Performance
- [ ] 4.1 Benchmark with 10 000 generated items

## 5. Verify
- [ ] 5.1 Tests for every scenario (unit, golden-image or documented manual check)
- [ ] 5.2 `reviewer` agent: specs, mockup fidelity, clippy/fmt/tests green
- [ ] 5.3 `.deb` builds and installs on Ubuntu 24.04, lintian clean
- [ ] 5.4 German translation updated
