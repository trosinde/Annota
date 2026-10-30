# Tasks

## 1. Design gate
- [x] 1.1 `designer` agent: mockups for all scenarios in `docs/design/bootstrap-app-shell/` (light + dark, 1x + 2x)
- [x] 1.2 Maintainer approval recorded in design.md

## 2. Application skeleton

> Packaging scaffold built on 2026-10-30 before design approval: only non-visual parts and a
> placeholder window. UI items from the mockups (welcome page, preferences, shortcuts window)
> start after the design gate.

- [x] 2.1 Cargo + Meson project, AdwApplication, main window placeholder
- [x] 2.2 CLI parsing and D-Bus forwarding
- [x] 2.3 Preferences window + GSettings schema
- [x] 2.4 About dialog, shortcuts window
- [x] 2.5 gettext setup with `en` and `de`

> Welcome page (mockup 01, mode cards) and the full main-menu redline (D1) are not in this
> pass: D1 is shared with editor screens that don't exist yet (`add-annotation-editor`,
> `add-export-share`), and the mode cards' Print/Alt+Print/Shift+Print hotkeys depend on the
> still-open Q1 decision (`docs/design/decisions.md`). The window keeps its placeholder content
> area; the main menu gained Preferences and Keyboard Shortcuts only. Revisit when
> `add-screen-capture` lands.

## 3. Packaging
- [x] 3.1 `debian/` directory, vendoring script, offline build
- [x] 3.2 Desktop file, metainfo, icons from design kit
- [x] 3.3 GitHub Actions CI incl. .deb artifact

## 4. Verify
- [x] 4.1 Tests for every scenario (unit, golden-image or documented manual check) — see `manual-checks.md`
- [ ] 4.2 `reviewer` agent: specs, mockup fidelity, clippy/fmt/tests green
- [ ] 4.3 `.deb` builds and installs on Ubuntu 24.04, lintian clean _(verified by CI on ubuntu-24.04; this sandbox only has cargo/rustc 1.93, so `build-aux/make-deb.sh` can't be exercised locally)_
- [x] 4.4 German translation updated
