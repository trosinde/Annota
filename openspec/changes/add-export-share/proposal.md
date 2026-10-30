# Add export and sharing

**Roadmap phase:** MVP (milestone 0.1)

## Why
Annotated images must reach documents, chats and tickets in one step.

## What Changes
- Export to PNG, JPG, WebP, PDF (and `.annota`)
- Copy to clipboard, drag and drop out of the editor
- Quick save to default folder with filename pattern
- Share via the system (xdg-desktop-portal OpenURI / email)

## Benchmark features covered
- Export formats
- Copy to clipboard
- Share to applications
- Batch export with consistent settings

## Out of scope
- Cloud upload / share links (not planned)
- Office integrations (not planned; use clipboard/drag)

## Capabilities

### New Capabilities
- `export-share`

### Modified Capabilities
- none

## Impact
- New module `src/export/`
