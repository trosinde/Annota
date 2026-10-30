# Tasks

## 1. Design gate
- [ ] 1.1 `designer` agent: mockups for all scenarios in `docs/design/bootstrap-app-shell/` (light + dark, 1x + 2x)
- [ ] 1.2 Maintainer approval recorded in design.md

## 2. Application skeleton
- [ ] 2.1 Cargo + Meson project, AdwApplication, main window placeholder
- [ ] 2.2 CLI parsing and D-Bus forwarding
- [ ] 2.3 Preferences window + GSettings schema
- [ ] 2.4 About dialog, shortcuts window
- [ ] 2.5 gettext setup with `en` and `de`

## 3. Packaging
- [ ] 3.1 `debian/` directory, vendoring script, offline build
- [ ] 3.2 Desktop file, metainfo, icons from design kit
- [ ] 3.3 GitHub Actions CI incl. .deb artifact

## 4. Verify
- [ ] 4.1 Tests for every scenario (unit, golden-image or documented manual check)
- [ ] 4.2 `reviewer` agent: specs, mockup fidelity, clippy/fmt/tests green
- [ ] 4.3 `.deb` builds and installs on Ubuntu 24.04, lintian clean
- [ ] 4.4 German translation updated
