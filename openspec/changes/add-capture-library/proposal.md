# Add capture library (browse and manage captures)

**Roadmap phase:** MVP (milestone 0.1)

## Why
Every capture must be kept and found again without the user thinking about files. A screenshot tool without a library loses its captures after the first week, so the basic library ships in the MVP.

## What Changes
- Every capture is stored automatically in the library folder with metadata
- Library window: thumbnail grid and list view, grouped by day, sort, zoomable thumbnails
- Preview pane with metadata; open, rename, duplicate, delete (trash), show in Files, copy, drag out
- Multi-select with bulk actions
- Library follows external file changes in the library folder
- Recent-captures tray in the editor is fed from the library

## Benchmark features covered
- Library (all captures in one place)
- Recent captures tray
- Sort by date
- Open/edit from library
- Drag and drop out of the library

## Out of scope
- Tags, collections, favourites, full-text search, smart filters (add-library-organisation, Phase 2)
- Cloud sync

## Capabilities

### New Capabilities
- `capture-library`

### Modified Capabilities
- none

## Impact
- New module `src/library/`; dependencies `rusqlite`, `notify`
