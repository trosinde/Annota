# Tasks

## 1. Design gate
- [ ] 1.1 `designer` agent: mockups for all scenarios in `docs/design/bootstrap-app-shell/` (light + dark, 1x + 2x)
- [ ] 1.2 Maintainer approval recorded in design.md

## 2. Application skeleton

> Packaging scaffold built on 2026-10-30 before design approval: only non-visual parts and a
> placeholder window. UI items from the mockups (welcome page, preferences, shortcuts window)
> start after the design gate.

- [x] 2.1 Cargo + Meson project, AdwApplication, main window placeholder
- [x] 2.2 CLI parsing and D-Bus forwarding
- [ ] 2.3 Preferences window + GSettings schema _(schema done; window waits for design approval)_
- [ ] 2.4 About dialog, shortcuts window _(About done; shortcuts window waits for design approval)_
- [x] 2.5 gettext setup with `en` and `de`

## 3. Packaging
- [x] 3.1 `debian/` directory, vendoring script, offline build
- [x] 3.2 Desktop file, metainfo, icons from design kit
- [x] 3.3 GitHub Actions CI incl. .deb artifact

## 4. Verify
- [ ] 4.1 Tests for every scenario (unit, golden-image or documented manual check)
- [ ] 4.2 `reviewer` agent: specs, mockup fidelity, clippy/fmt/tests green
- [ ] 4.3 `.deb` builds and installs on Ubuntu 24.04, lintian clean
- [ ] 4.4 German translation updated
