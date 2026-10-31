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
- [ ] 2.3 Clipboard, DnD source
- [ ] 2.4 Share menu via portals
- [ ] 2.5 Batch export dialog
- [ ] 2.6 "Save changes before closing?" — add-annotation-editor keeps every unsaved document's
      autosave when the editor window closes or Annota quits, because there was no Save yet
      (maintainer decision; see add-annotation-editor's design.md, phase 2b-ii, and its
      document-format requirement "Closing keeps autosaves"). Once Save exists: ask before closing
      a document with unsaved changes (Save / Discard / Cancel; mockup 15, approved 2026-10-31,
      redlines in `docs/design/add-export-share/index.md` — one alert per unsaved document, tray
      Open order), then revert closing to "an autosave is removed when its document is saved or
      the user discards it" — in the code (`Session::close`, `Editor::close_all`,
      `editor::shut_down`) and as a MODIFIED delta of `document-format` ("Autosave and recovery",
      "Closing keeps autosaves", "Recovery dialog").
- [ ] 2.7 Redaction export warning — add-core-annotation-tools builds blur, pixelate and solid
      redactions (`annota::tools::redaction::Redaction`) but has no way out of Annota to guard;
      its requirement "Redaction export warning" (spec annotation-tools, decision D7, that
      change's mockup 19 and this change's mockup 10) is only a placeholder scenario there (no
      export path exists yet when that change archives) — complete it here as a `## MODIFIED
      Requirements` delta on the `annotation-tools` capability, full requirement block copied
      from the archived spec per the project's established MODIFIED-delta convention. Before
      every export,
      Copy, drag-out, share and quick save of a document with blur or pixelate redactions (at
      any depth, groups included): the alert "Redactions May Be Reversible" with **Convert to
      Solid and Export** (suggested; "… and Copy" for Copy), **Export As Is** and **Cancel**.
      Converting is one undo step: `set_property("redaction", Choice("solid"))` on each, as one
      `Command::Replace`. Faces found by smart redaction default to Solid (D7, add-smart-redaction).
      **Partly built (2026-10-31):** the alert follows mockup 10 ("Make Redactions Solid?",
      Cancel · Export Anyway / Save Anyway · **Convert to Solid**), not the wording above or
      add-core-annotation-tools mockup 19 (open question, design.md). The MODIFIED delta is
      `specs/annotation-tools/spec.md`. Wired in front of Export (`Ctrl+E`), quick save
      (`Ctrl+S`) and Save As (`Ctrl+Shift+S`) through `Editor::guard_redactions`; still to call it
      from Copy and drag-out (2.3), Share (2.4) and batch export (2.5) — then tick this.

## 3. Verify
- [ ] 3.1 Tests for every scenario (unit, golden-image or documented manual check)
- [ ] 3.2 `reviewer` agent: specs, mockup fidelity, clippy/fmt/tests green
- [ ] 3.3 `.deb` builds and installs on Ubuntu 24.04, lintian clean
- [ ] 3.4 German translation updated
