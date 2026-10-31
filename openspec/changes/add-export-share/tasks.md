# Tasks

## 1. Design gate
- [x] 1.1 `designer` agent: mockups for all scenarios in `docs/design/add-export-share/` (light + dark, 1x + 2x)
- [x] 1.2 Maintainer approval recorded in design.md
- [x] 1.3 Redraw screens 01-14 with the editor's actual header (Show Properties / Copy / Export /
      menu, decision D1) instead of the pre-D1 drag-handle/Share header they still show; screen
      15 (added for task 2.6) already uses the current header — see design.md "Decided during
      apply", 2026-10-31. Export is drawn as the built `Adw.SplitButton` (Export + a dropdown
      arrow opening Share); 08/09's drag affordance moved to the window title area per D1, since
      no other drag-out control is built yet (task 2.3). Other staleness in 01-14 (tool strip
      order predates D2, tray and sidebar layout) intentionally left alone — out of this task's
      scope, see design.md for the maintainer's follow-up decision on whether it needs its own
      pass.

## 2. Implementation
- [x] 2.1 Exporters PNG/JPG/WebP/PDF (`src/export/`, not wired into the UI yet; `.annota` save
      re-exposed as `export::save_annota`; see design.md "Decided during apply", task 2.1)
- [x] 2.1a Export dialog (`Ctrl+E`, header Export; mockups 02-05; spec export-share "Export
      dialog") — not a task of its own originally: 2.1 built the encoders without UI.
      `src/ui/editor/export_dialog.rs`, `data/ui/export-dialog.blp`, error mapping in
      `src/ui/editor/saving.rs`; see design.md "Decided during apply", export dialog
- [x] 2.2 Quick save + Save As (`Ctrl+S` writes `.annota` — own file, or the filename pattern in
      the save folder; `Ctrl+Shift+S` GTK's file chooser; title becomes the file's name, autosave
      removed, mockup 06 toast; failures as a toast with Save As…; see design.md, task 2.2)
- [x] 2.3 Clipboard, DnD source (`Ctrl+C`/header Copy → flattened image on the clipboard,
      mockup 07 toast; dragging the window title area → temporary PNG named after the
      document as `text/uri-list` + `image/png`, mockups 08/09; `src/ui/editor/share.rs`,
      `src/export/handoff.rs`; see design.md, tasks 2.3/2.4)
- [x] 2.4 Share menu via portals (Email… → `Email` portal with the PNG attached, mockup 14
      toast; Open With… → `OpenURI` with the app chooser; Copy to Folder… → GTK's file
      chooser, no Email/OpenURI portal; real portal round-trips are maintainer checks,
      `manual-checks.md`)
- [x] 2.4a Object copy/paste (maintainer decision, 2026-10-31, over the "leave it for later"
      recommendation — the original mockup table assumed this existed and it never did
      anywhere in the app): `Ctrl+C` with a selection SHALL put the selected object(s) on the
      system clipboard in an Annota-specific mime type (self-contained: geometry, properties,
      embedded resources — same shape as the `.annota` format's own object serialization, so a
      copy survives the source document closing). **Correction (2026-10-31, checked against the
      actual code — the first-pass wording below wrongly assumed Ctrl+V was already bound):**
      the existing "paste a clipboard image as a new document" (`capture::import::paste`,
      `app.paste-image`) is bound to **Shift+Ctrl+V**, not Ctrl+V — plain Ctrl+V is free. So
      this task newly binds `Ctrl+V`, editor-scoped (`win.paste`), as the smart paste: with a
      document open and focused, it checks the clipboard for Annota's object mime type first
      and inserts the objects into the *currently open* document (works across documents —
      pasting into a different document than the one they were copied from, or a later-opened
      one, is supported) as one undo step, offset like a duplicate (10px, the established
      convention — see add-annotation-editor's phase 2a decisions); without that mime type, it
      falls back to the same behaviour as `app.paste-image` (a plain clipboard image opens as a
      new document). `Shift+Ctrl+V` keeps working exactly as it does today, untouched.
      **Built (2026-10-31):** clipboard type `application/x-annota-objects`, payload = a
      cut-down `.annota` (`objects.json` with the file's versioning keys + `images/<n>.png`;
      `src/document/clipboard.rs`), paste = `editing::ops::paste` (Duplicate's shape, images
      re-numbered or shared, newer-minor data kept via `Command::SetFormatMinor`), `win.paste`
      as a window key, shortcuts-window row "Paste"; see design.md "Decided during apply
      (2026-10-31: task 2.4a object copy/paste)". Cross-process paste: `manual-checks.md` rows
      9-10.
      **Fixed (2026-10-31, maintainer decision):** Copy with a selection now puts the objects
      *and* the whole flattened image (`image/png` for other apps) on the clipboard as one
      content (`share::selection_content`), behind the redaction warning ("Copy Anyway"; Convert
      to Solid applies to both); toast "Image and object copied to clipboard". See design.md
      "Decided during apply (2026-10-31: selection copy includes the image)".
- [x] 2.5 Batch export dialog (mockups 11–13, built against the real tray: "Select" at the
      right end of its label row, a selection bar in that row's place — Cancel · "<n> Selected"
      · Select All · Export <n>…; open documents and recent files both selectable;
      `src/ui/editor/batch.rs`, `data/ui/batch-export-dialog.blp`, the export dialog's format
      rows shared as `FormatRows`/`Encoding`; pattern `%name`/`%n`/strftime, If a file exists;
      redaction warning once for the whole selection; toast of mockup 13; see design.md
      "Decided during apply (2026-10-31: task 2.5 batch export)")
- [x] 2.6 "Save changes before closing?" — add-annotation-editor keeps every unsaved document's
      autosave when the editor window closes or Annota quits, because there was no Save yet
      (maintainer decision; see add-annotation-editor's design.md, phase 2b-ii, and its
      document-format requirement "Closing keeps autosaves"). Once Save exists: ask before closing
      a document with unsaved changes (Save / Discard / Cancel; mockup 15, approved 2026-10-31,
      redlines in `docs/design/add-export-share/index.md` — one alert per unsaved document, tray
      Open order), then revert closing to "an autosave is removed when its document is saved or
      the user discards it" — in the code (`Session::close`, `Editor::close_all`,
      `editor::shut_down`) and as a MODIFIED delta of `document-format` ("Autosave and recovery",
      "Closing keeps autosaves", "Recovery dialog").
      **Built (2026-10-31):** `src/ui/editor/closing.rs`, `data/ui/save-changes-dialog.blp`
      (AdwAlertDialog, 372 px), `Editor::confirm_close` on the window's close request and on
      `app.quit` (`editor::quit`); Save = quick save then close, a failed save or Cancel stops
      everything, Discard removes the autosave; ending without asking (logout, a signal) still
      keeps autosaves. `specs/document-format/spec.md`: the three requirements MODIFIED
      (copied from add-annotation-editor's delta, not archived yet — archive it first) plus
      ADDED "Save changes before closing". No "Quit" main-menu item exists or was added (open
      question). See design.md "Decided during apply (2026-10-31: task 2.6 save changes before
      closing)".
- [x] 2.7 Redaction export warning — add-core-annotation-tools builds blur, pixelate and solid
      redactions (`annota::tools::redaction::Redaction`) but has no way out of Annota to guard;
      its requirement "Redaction export warning" (spec annotation-tools, decision D7) is only a
      placeholder scenario there (no export path exists yet when that change archives) —
      completed here as a `## MODIFIED Requirements` delta on the `annotation-tools` capability
      (`specs/annotation-tools/spec.md`), full requirement block copied from the archived spec
      per the project's established MODIFIED-delta convention. Before Export (and, once built,
      Copy, drag-out, Share and batch export — not quick save or Save As, see below) of a
      document with blur or pixelate redactions (at any depth, groups included): the alert
      "Make Redactions Solid?" (mockup 10 — the canonical version, maintainer decision
      2026-10-31, see design.md; add-core-annotation-tools mockup 19 gets a follow-up redraw to
      match) with **Convert to Solid** (suggested), **Export Anyway**/**Copy Anyway** and
      **Cancel**. Converting is one undo step: `set_property("redaction", Choice("solid"))` on
      each, as one `Command::Replace`. Faces found by smart redaction default to Solid (D7,
      add-smart-redaction).
      **Partly built (2026-10-31):** wired in front of Export (`Ctrl+E`) through
      `Editor::guard_redactions`. **Deliberately not wired in front of quick save or Save As**
      (maintainer decision, 2026-10-31, see design.md): `.annota` keeps the original image
      under every redaction regardless of the choice made here, so the warning does not apply
      to a save — `Purpose::Save` was removed. **Copy, drag-out and Share wired too
      (2026-10-31, tasks 2.3/2.4):** `Purpose::Copy` ("Copy Anyway"), `Purpose::Drag` ("Drag
      Anyway"), `Purpose::Share` ("Share Anyway"). **Batch export wired too (2026-10-31, task
      2.5) — now complete on every way out of Annota** (Export, Copy, drag-out, Share, batch
      export; never on a save): batch export asks **once for the whole selection** (one alert
      counting every selected document's blur/pixelate redactions, "Export Anyway" —
      `Purpose::Export` reused; Convert to Solid converts in each open document as its own undo
      step and in a recent file's exported copy only), through `Editor::ask_redactions`, the
      alert half of `guard_redactions`; scenario "Warning before batch export". See design.md,
      task 2.5.

## 3. Verify
- [ ] 3.1 Tests for every scenario (unit, golden-image or documented manual check)
- [ ] 3.2 `reviewer` agent: specs, mockup fidelity, clippy/fmt/tests green
- [ ] 3.3 `.deb` builds and installs on Ubuntu 24.04, lintian clean
- [ ] 3.4 German translation updated
