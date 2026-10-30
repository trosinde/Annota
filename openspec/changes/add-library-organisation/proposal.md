# Add library organisation (search, tags, collections, filters)

**Roadmap phase:** Phase 2 (milestone 0.2–0.4)

## Why
Once hundreds of captures pile up, chronological browsing is not enough: users need to find a capture by what is in it, where it came from, or which project it belongs to.

## What Changes
- Full-text search over name, tags, application, window title, URL, annotation text and recognised text
- Tags with tag sidebar, collections (manual groups), favourites
- Filter bar: date range, type, application, website, tag, has annotations
- Saved searches (smart collections)
- Import existing folders, retention and duplicate detection

## Benchmark features covered
- Search on metadata
- Filter and sort by date, application, webpage
- Tags
- Collections
- Step capture category

## Out of scope
- Cloud sync and shared team collections

## Capabilities

### New Capabilities
- none

### Modified Capabilities
- `capture-library`

## Impact
- Depends on add-capture-library and add-text-recognition (for OCR search)
