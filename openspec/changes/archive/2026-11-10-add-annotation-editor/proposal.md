# Add annotation editor canvas and document format

**Roadmap phase:** MVP (milestone 0.1)

## Why
Every capture opens in an editor where annotations are separate, editable objects on top of the original image. The document model, undo history and file format are the foundation for all tools.

## What Changes
- Editor window with tool bar, canvas, properties sidebar and recent-captures tray
- Object model: select, move, resize, rotate, duplicate, delete, z-order, group
- Zoom, pan, fit, canvas extension, snapping guides
- Undo/redo for every change
- `.annota` editable file format and autosave

## Benchmark features covered
- Objects stay editable until export
- Group/ungroup, bring forward/send backward
- Canvas snapping
- Select all objects of one type
- Recent captures tray
- Cross-session editable format (.snagx equivalent)
- Flatten / flatten all
- Crop, resize, rotate image
- Extend canvas, canvas colour incl. transparent

## Out of scope
- Annotation tools themselves (separate change)
- Capture library with tags/search (Phase 2)

## Capabilities

### New Capabilities
- `editor-canvas`
- `document-format`

### Modified Capabilities
- `app-shell`: `--open <file>` now actually opens the editor; the welcome page's mode cards
  (task 4.4)
- `capture`: captures, pasted and opened/dropped images now open in the editor instead of being
  quick-saved as a PNG stand-in (task 4.5)

## Impact
- New modules `src/document/`, `src/render/`, `src/editing/`, `src/tools/`, `src/ui/editor/`
- Until add-export-share brings Save, closing the editor keeps every unsaved document's autosave
  (offered again at the next start); add-export-share adds "Save changes before closing?"
- Archive after add-screen-capture (its `capture` and `app-shell` specs are the base of this
  change's deltas)
