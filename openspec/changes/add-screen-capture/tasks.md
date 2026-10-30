# Tasks

## 1. Design gate
- [x] 1.1 `designer` agent: mockups for all scenarios in `docs/design/add-screen-capture/` (light + dark, 1x + 2x)
- [x] 1.2 Maintainer approval recorded in design.md

## 2. Portal access

> Written against `ashpd` 0.11's API but never run against a live xdg-desktop-portal: development
> happened on the maintainer's own desktop, where a real call would capture the actual screen.
> `manual-checks.md` lists the live checks still to do.

- [x] 2.1 Screenshot portal wrapper with error mapping (`src/capture/portal.rs`)
- [x] 2.2 Permission-denied dialog (`data/ui/permission-dialog.blp`, mockup 08)

## 3. Overlay
- [x] 3.1 Frozen-image overlay per monitor (`src/capture/overlay.rs`, `data/ui/region-overlay.blp`)
- [x] 3.2 Selection, handles, keyboard nudging, size field
- [x] 3.3 Loupe + window-edge snapping (`src/capture/snap.rs`)

> Window edges are detected in the frozen image (Wayland exposes no window geometry), so mockup 04's
> dashed window outline is not drawn; the violet guide and chip are. See design.md.

## 4. Modes and triggers
- [x] 4.1 Window and full-screen modes
- [x] 4.2 Delay with countdown (`src/capture/countdown.rs`, mockup 07)
- [x] 4.3 Global shortcuts + fallback keybinding (`src/capture/shortcuts.rs`, Preferences › Capture,
      Set Shortcut dialog — mockups 09–11)
- [x] 4.4 Clipboard/file import (`src/capture/import.rs`, `Shift+Ctrl+V`, `Ctrl+O`, drop — mockups 13, 14)
- [x] 4.5 `--capture` actually starts a capture: add `specs/app-shell/spec.md` with a
      `## MODIFIED Requirements` delta completing bootstrap-app-shell's "Single application
      instance" requirement (its "Second launch forwards request" scenario currently stops at
      "handles the request" — extend it to "starts a region/window/screen capture" for this
      case; "Command-line interface" and its "Invalid mode" scenario are already correct as
      they are and don't need to change)
- [ ] 4.6 First-launch takeover prompt (D9: "Annota offers at first launch to take over the Print
      keys"). No mockup exists for it in `docs/design/add-screen-capture/`, so it needs a `designer`
      mockup and maintainer approval before it can be built. Until then the takeover is only
      offered in Preferences › Capture (banner, mockup 09), and only takes effect via the
      GNOME-custom-keybinding fallback (no GlobalShortcuts portal). On GNOME ≥ 48 (the portal
      path — Debian 13, Ubuntu 26.04), there is no takeover yet: Annota registers its preferred
      triggers with the portal while GNOME Shell keeps its own Print bindings, so which one
      actually fires depends on the compositor; GNOME's own GlobalShortcuts consent dialog
      governs whether Annota's registration succeeds at all, it is not a takeover prompt. Needs a
      live check on a GNOME ≥ 48 system (see `manual-checks.md`) and, if the portal path needs
      its own takeover, a follow-up task once that's designed.

> Deferred on purpose, not forgotten:
> - **Editor destination.** The editor does not exist yet (`add-annotation-editor`). "Open in Editor"
>   and "Both" quick-save the capture as PNG into the save folder with a toast; pasted and opened
>   images do the same. Mockups 12 and 13 (capture inside the editor, tray, sidebar) are therefore
>   not implemented here. `add-annotation-editor` task 4.5 replaces this with a MODIFIED delta on
>   the `capture` capability. *(Done there: captures, pasted and opened images now open in the
>   editor; that change's deltas build on this change's specs, so archive this change first.)*
> - **X11 click-to-pick (mockup 06).** Window mode uses the portal picker on X11 too; Annota-drawn
>   X11 picking needs direct X11 window enumeration, which CLAUDE.md §2 rules out without an ADR.
> - **Portal shortcuts while Annota is closed.** With the GlobalShortcuts portal the shortcuts work
>   while Annota runs; background/autostart is not part of this change. The GNOME-keybinding
>   fallback works with Annota closed.
> - **Welcome page mode cards** (bootstrap-app-shell mockup 01): their copy promises the editor
>   ("The capture opens here, ready to mark up"), so they stay with `add-annotation-editor`. This
>   change adds the capture launcher (mockup 01 of this change) to the main window's header bar.

## 5. Verify
- [x] 5.1 Tests for every scenario (unit, golden-image or documented manual check) — unit tests in
      `src/capture/*.rs` and `src/cli.rs`; live checks in `manual-checks.md` (not yet run, see there)
- [x] 5.2 `reviewer` agent: specs, mockup fidelity, clippy/fmt/tests green — found 2 blockers
      (a cold-start bug that broke the GNOME-keybinding hotkey entirely, and a spec/tasks
      honesty gap about D9 not applying on the GlobalShortcuts portal path) plus 2 non-blocking
      robustness gaps in the GNOME-keybinding takeover's write ordering; all fixed and verified
- [x] 5.3 `.deb` builds, `lintian -I --pedantic` clean (verified by the reviewer in this dev
      container — rustc 1.93, not the pinned 1.85, same caveat as bootstrap-app-shell). Actual
      install on Ubuntu 24.04 not separately verified.
- [x] 5.4 German translation updated
