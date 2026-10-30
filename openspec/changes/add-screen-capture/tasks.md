# Tasks

## 1. Design gate
- [x] 1.1 `designer` agent: mockups for all scenarios in `docs/design/add-screen-capture/` (light + dark, 1x + 2x)
- [x] 1.2 Maintainer approval recorded in design.md

## 2. Portal access
- [ ] 2.1 Screenshot portal wrapper with error mapping
- [ ] 2.2 Permission-denied dialog

## 3. Overlay
- [ ] 3.1 Frozen-image overlay per monitor
- [ ] 3.2 Selection, handles, keyboard nudging, size field
- [ ] 3.3 Loupe + window-edge snapping

## 4. Modes and triggers
- [ ] 4.1 Window and full-screen modes
- [ ] 4.2 Delay with countdown
- [ ] 4.3 Global shortcuts + fallback keybinding
- [ ] 4.4 Clipboard/file import
- [ ] 4.5 `--capture` actually starts a capture: add `specs/app-shell/spec.md` with a
      `## MODIFIED Requirements` delta completing bootstrap-app-shell's "Single application
      instance" requirement (its "Second launch forwards request" scenario currently stops at
      "handles the request" — extend it to "starts a region/window/screen capture" for this
      case; "Command-line interface" and its "Invalid mode" scenario are already correct as
      they are and don't need to change)

## 5. Verify
- [ ] 5.1 Tests for every scenario (unit, golden-image or documented manual check)
- [ ] 5.2 `reviewer` agent: specs, mockup fidelity, clippy/fmt/tests green
- [ ] 5.3 `.deb` builds and installs on Ubuntu 24.04, lintian clean
- [ ] 5.4 German translation updated
