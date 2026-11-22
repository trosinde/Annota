# Tasks

Blocked until `add-screen-recording` is archived.

## 1. Design gate
- [ ] 1.1 Rebase the deltas on the archived `screen-recording` spec: turn the post-stop flow into a MODIFIED delta ("Export after recording" gains "Edit…"), recheck every ADDED requirement against the core
- [ ] 1.2 Run probes E1–E5 of design.md; correct spec and design where results contradict them
- [ ] 1.3 Maintainer decisions Q1–Q3 recorded in design.md
- [ ] 1.4 `designer` agent: redraw `docs/design/add-recording-extras/` against the approved core mockups (light + dark, 1x + 2x), scenario → mockup table in index.md
- [ ] 1.5 Maintainer approval recorded in design.md

## 2. Webcam overlay
- [ ] 2.1 Camera source, device presence — Webcam overlay / No camera
- [ ] 2.2 Compositor layer, shape mask, position, size — Webcam overlay / Webcam in the recording
- [ ] 2.3 Swap — Webcam overlay / Swap

## 3. Cursor effects
- [ ] 3.1 Highlight ring from cursor metadata — Cursor effects / Highlight ring
- [ ] 3.2 Click animation where observable — Cursor effects / Click ripple, Clicks not observable

## 4. Draw while recording
- [ ] 4.1 Drawing surface and compositing — Draw while recording / Arrow overlay, Drawing fades

## 5. Video editor and GIF
- [ ] 5.1 Editor window, timeline, undo — Video editor / Trim, Cut out a range
- [ ] 5.2 Combine clips — Video editor / Combine clips
- [ ] 5.3 Save frame — Video editor / Save frame
- [ ] 5.4 GIF export (per Q1) — GIF export / GIF

## 6. Verify
- [ ] 6.1 Tests for every scenario (unit, pipeline, golden-image or documented manual check)
- [ ] 6.2 `reviewer` agent: specs, mockup fidelity, clippy/fmt/tests green
- [ ] 6.3 `.deb` builds and installs on Ubuntu 24.04, lintian clean
- [ ] 6.4 German translation updated
- [ ] 6.5 `python3 tools/scenario_gate.py verify add-recording-extras` exits 0, `scenario-report.md` committed
