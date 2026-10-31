# Tasks

## 1. Design gate
- [ ] 1.1 `designer` agent: mockups for all scenarios in `docs/design/add-export-share/` (light + dark, 1x + 2x)
- [ ] 1.2 Maintainer approval recorded in design.md

## 2. Implementation
- [ ] 2.1 Exporters PNG/JPG/WebP/PDF
- [ ] 2.2 Quick save + Save As
- [ ] 2.3 Clipboard, DnD source
- [ ] 2.4 Share menu via portals
- [ ] 2.5 Batch export dialog
- [ ] 2.6 "Save changes before closing?" — add-annotation-editor keeps every unsaved document's
      autosave when the editor window closes or Annota quits, because there was no Save yet
      (maintainer decision; see add-annotation-editor's design.md, phase 2b-ii, and its
      document-format requirement "Closing keeps autosaves"). Once Save exists: ask before closing
      a document with unsaved changes (Save / Discard / Cancel; needs a designer mockup and
      approval — no redline exists), then revert closing to "an autosave is removed when its
      document is saved or the user discards it" — in the code (`Session::close`,
      `Editor::close_all`, `editor::shut_down`) and as a MODIFIED delta of `document-format`
      ("Autosave and recovery", "Closing keeps autosaves", "Recovery dialog").
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

## 3. Verify
- [ ] 3.1 Tests for every scenario (unit, golden-image or documented manual check)
- [ ] 3.2 `reviewer` agent: specs, mockup fidelity, clippy/fmt/tests green
- [ ] 3.3 `.deb` builds and installs on Ubuntu 24.04, lintian clean
- [ ] 3.4 German translation updated
