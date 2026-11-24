# Tasks

Scenario names below are `screen-recording / <Requirement> / <Scenario>`. Every test carries a
`spec:` tag for the scenarios it proves. No code before group 1 is complete; group 2 (probes)
runs before any code that relies on desktop behaviour. Blocked until `add-screen-recording` is
archived (its pipeline, control window and Export Recording window are the base).

## 1. Design gate
- [x] 1.1 Proposal, spec, design and tasks rewritten for the maintainer's decisions of 2026-11-22 (Q1 no gifski, Q2 no click animation, Q3 editor v1 = trim, cut range, save frame; combine clips out of scope); `openspec validate --all --strict` and `python3 tools/scenario_gate.py check` pass
- [x] 1.2 `designer` agent: redraw `docs/design/add-recording-extras/` against the approved core mockups (light + dark, 1x + 2x), scenario → mockup table in index.md
- [x] 1.3 Maintainer answers the open questions O1–O6 of design.md (O1 drawing surface vs. composited strokes, O2 where saved frames go, O3 own PipeWire consumer, O4–O6 from the designer)
- [x] 1.4 Maintainer approval recorded in design.md (`Design approved: 2026-11-22 (maintainer)`); ADR 0004 "GIF export via GStreamer and the `gif` crate, not gifski" and the CLAUDE.md §2 Video row updated; O4 outline added to spec and mockup 06
- [x] 1.5 Probe list E1–E7 in `manual-checks.md` (planned commands, dialogs, what each measures), as the core did for P1–P12; no probe script yet
- [ ] 1.6 After `add-screen-recording` is archived: rebase the deltas on `openspec/specs/screen-recording/spec.md` — "Recording setup", "Recording control" and "Export after recording" become MODIFIED deltas (setup rows, Swap/Draw buttons, Edit… and GIF format), the rest stays ADDED; `openspec validate --all --strict`

## 2. Foundation (probes before code)
- [ ] 2.1 `tools/recording-probe.py` gains an `extras` group for E1–E5 (syntax-checked first; the maintainer runs it on GNOME 50 and, where possible, GNOME 46 incl. "GNOME on Xorg"); logs committed in `probe-results/`
- [ ] 2.2 Spec and design corrected from E1–E5 (or "no contradiction" noted in design.md); O3 settled (own PipeWire consumer or `pipewiresrc`) with the maintainer if E1 requires it; Draw removed (new design gate) if E3 fails
- [ ] 2.3 E7: check `compositor`, `cairooverlay`, `alpha`, `v4l2src`, `gtk4paintablesink`, `avenc_gif` and their packages on clean Ubuntu 24.04, 26.04 and Debian 13 containers; `debian/control` unchanged or extended
- [ ] 2.4 Crates: `gif` 0.14 (+ `color_quant`, `weezl`), and `pipewire` only if 2.2 needs it; `Cargo.lock` resolved and built with the project MSRV (Rust 1.91 once `fix/zip-security-update` is merged, else 1.85); `build-aux/update-copyright.py`
- [ ] 2.5 GSettings keys of design.md D9 with defaults and ranges

## 3. Compositor, webcam and pointer
- [ ] 3.1 Compositor stage in the recording pipeline (screen sink + overlay), tests with `videotestsrc` — Webcam overlay / Webcam in the recording
- [ ] 3.2 Camera source and device presence (`GstDeviceMonitor`), failure keeps recording — Webcam overlay / No camera, Camera cannot be opened
- [ ] 3.3 Layout maths and shape masks — Webcam layout / Webcam placement, Square in the top left
- [ ] 3.4 Swap at runtime — Swap to webcam / Swap, No swap without webcam
- [ ] 3.5 Metadata cursor mode, pointer image and ring via `cairooverlay` (synthetic metadata in tests) — Pointer highlight / Highlight ring, Pointer drawn from metadata, Highlight without pointer image, Metadata cursor not offered (fake portal)

## 4. UI chrome and drawing
- [ ] 4.1 Setup popover rows Webcam (expander: shape, 3×3 grid, size) and Highlight pointer, placement preview in the overlay — Webcam layout / Layout preview (mockups per index.md)
- [ ] 4.2 Recording control: Swap, Draw, "Webcam unavailable", Draw insensitive for window recordings — Draw while recording / Draw in a window recording
- [ ] 4.3 Drawing surface (full-screen on the recorded monitor, input, keys), stroke model shared with the pipeline — Draw while recording / Arrow in the recording, Input returns after drawing, Region outline while drawing, Strokes outside the region
- [ ] 4.4 Tools, colours, fade on recording time, Clear, step counter — Drawing tools and fade / Drawing fades, Keep until cleared, Step numbers count up

## 5. Video editor and GIF
- [ ] 5.1 Edit… in the Export Recording window, Edit Recording window, Done/Cancel, edit list hand-over — Edit a recording / Open the editor, Done returns to export, Working file unchanged, Cancel asks first
- [ ] 5.2 Preview and playback, timeline (thumbnails, waveform, handles, range), undo/redo via `Command` — Trim and cut / Undo a cut, Keep at least half a second
- [ ] 5.3 Export applies the edit list (MP4 re-encode when edited, WebM) — Trim and cut / Trim, Cut out a range
- [ ] 5.4 Save Frame + toast + Open in Annota — Save a frame / Save frame, Open a saved frame
- [ ] 5.5 GIF format: options, decode → `gif` crate encoder, progress/Cancel, settings persisted — GIF export / GIF, GIF plays once, GIF of an edited recording, No sound in a GIF
- [ ] 5.6 Shortcuts window (editor and control keys), metainfo/README feature text, `docs/feature-catalog.md` status

## 6. Verify
- [ ] 6.1 A tagged test for every scenario (unit, pipeline tests with test sources, fake-portal and live-GTK tests); `manual-checks.md` rows only where automation is impossible (real camera, real drawing surface on GNOME), each run with result and date
- [ ] 6.2 Live run on the maintainer's desktop: webcam, highlight, drawing in region and screen recordings, editor, GIF
- [ ] 6.3 `reviewer` agent: specs, mockup fidelity, clippy/fmt/tests green
- [ ] 6.4 `.deb` builds and installs on Ubuntu 24.04 (project MSRV toolchain), lintian clean, elements of 2.3 present
- [ ] 6.5 German translation updated
- [ ] 6.6 `python3 tools/scenario_gate.py verify add-recording-extras` exits 0, `scenario-report.md` committed
