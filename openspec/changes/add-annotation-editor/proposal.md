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
- none

## Impact
- New modules `src/document/`, `src/render/`, `src/ui/editor/`
