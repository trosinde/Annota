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
- `annotation-tools` — "Redaction export warning": add-core-annotation-tools left it a placeholder
  (no way out of Annota existed yet); this change makes it the real warning (mockup 10, D7) on
  every way out it builds. Archive add-core-annotation-tools before this change: the MODIFIED
  delta needs its `annotation-tools` spec to exist.
- `document-format` — "Autosave and recovery", "Closing keeps autosaves" and "Recovery dialog"
  (add-annotation-editor) modified, "Save changes before closing" added (task 2.6): now that
  Annota can save, closing asks and an autosave goes only when its document is saved or
  discarded. Archive add-annotation-editor before this change: the MODIFIED delta needs its
  `document-format` spec to exist.

## Impact
- New module `src/export/` (encoders, `.annota` save); new crates `jpeg-encoder`, `libwebp-sys`
  (bundled libwebp, statically linked)
- `src/editing/redactions.rs`: finding and converting blur/pixelate redactions
- Editor UI: `src/ui/editor/{export_dialog,saving,redaction_warning,share}.rs`,
  `data/ui/{export-dialog,redaction-warning}.blp`; the editor's Export, Save, Save As, Copy and
  Share actions (placeholders until now), the window title as drag source, and `Session` (a
  saved document's file, name and autosave)
- `src/export/handoff.rs`: the temporary PNG handed to other apps (drag-out, Email, Open With),
  in `$XDG_RUNTIME_DIR/annota/handoff`
- "Save Changes?" when closing or quitting: `src/ui/editor/closing.rs`,
  `data/ui/save-changes-dialog.blp`, the editor's close request and `app.quit`, `Session::close`
- Batch export: the tray's selection mode (`src/ui/editor/tray.rs`, `editor-window.blp`),
  `src/ui/editor/batch.rs` + `data/ui/batch-export-dialog.blp`, sharing the export dialog's
  format rows
