# Tasks

## 1. Design gate
- [x] 1.1 `designer` agent: mockups for all scenarios in `docs/design/add-style-system/` (light + dark, 1x + 2x)
- [x] 1.2 Maintainer approval recorded in design.md

## 2. Implementation
- [x] 2.1 Quick style grid + save/rename/delete
- [x] 2.2 Favourites view
- [x] 2.3 Theme editor + generator
- [x] 2.4 Import/export + system theme dir

## 3. Verify
- [x] 3.1 Tests for every scenario (unit, golden-image or documented manual check) — see
      `manual-checks.md` for the one row needing a real display/window manager
- [x] 3.2 `reviewer` agent: specs, mockup fidelity, clippy/fmt/tests green — findings B1/N1
      fixed, see design.md "Fixed during review"
- [x] 3.3 `.deb` builds and installs on Ubuntu 24.04, lintian clean — verified in this dev
      container (`build-aux/make-deb.sh`, `sudo apt install ../annota_*.deb`), 2026-10-31
- [x] 3.4 German translation updated — `msgfmt --statistics po/de.po`: 682 translated, 0
      fuzzy/untranslated
