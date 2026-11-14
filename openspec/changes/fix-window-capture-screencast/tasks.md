# Tasks

## 1. Design gate

- [x] 1.1 `docs/adr/0002-screencast-for-window-stills.md`: decision, alternatives (Screenshot UI
      + hint, removing window mode, `pipewire` crate), consequences (GStreamer in the MVP,
      indicator). Update CLAUDE.md §2's "Screen access" and "Video" rows to point at it. Done
      when the maintainer accepts the ADR (`Status: accepted`).
- [x] 1.2 `designer` agent: flow sheet 01 (design.md "Mockups") in
      `docs/design/fix-window-capture-screencast/` with HTML + PNG (light + dark, 1× + 2×) and an
      `index.md` mapping every scenario of both deltas to the sheet, a test or a live check. The
      GNOME chooser is drawn as a neutral placeholder. Done when
      `python3 tools/render-mockups.py docs/design/fix-window-capture-screencast` renders it.
- [x] 1.4 `designer` agent: sheet 02, the waiting OSD "Waiting for a window…" with Cancel
      (design.md D8), styled like the countdown OSD 07, over a placeholder chooser, light + dark,
      1× + 2×. Add the OSD to flow sheet 01 between panels a and b, and update `index.md` for the
      scenarios "Waiting for the chooser" and "Cancel while waiting".
- [x] 1.3 Maintainer approval recorded in design.md (`Design approved: <date>`). No code before.

## 2. Portal measurements (CLAUDE.md §5, before code relies on them)

- [x] 2.1 Turn the session probe into `tools/portal-probe.py --screencast-window`. Measure and
      record in design.md D6: the response code and time when the chooser is closed, when a
      window is chosen, and when the probe runs with no focused window (started from a GNOME
      keybinding). **Stop point:** if "closed" and "failed" share a code, revise the spec first.
- [x] 2.2 Same probe: record whether `pipewiresrc` attaches `GstVideoCropMeta` (GStreamer 1.28
      here; 1.24 on Ubuntu 24.04 if a machine or container is available) and what the stream's
      `size` and the buffer size are for a non-maximised window at scale 1.25 and 1. Record it
      in D3.
- [x] 2.3 Same probe with `cursor_mode` EMBEDDED and the pointer over the chosen window: record
      whether the pointer is in the frame. **Stop point** per design.md D5.
- [x] 2.4 If a GNOME X11 session is available: `AvailableSourceTypes` there. Otherwise record
      "unmeasured" in design.md and keep the D6 fallback.

## 3. Dependencies

- [x] 3.1 Add `gstreamer` (+ `gstreamer-app`, `gstreamer-video` if D3 uses crop meta) in the newest Built and tested with Rust 1.85 in CI run 43612609079 (ubuntu-24.04).
      version whose MSRV ≤ 1.85 and whose feature level ≤ `v1_24`. Enable `ashpd` feature
      `screencast`. Verify: `cargo +1.85 build` (or `/usr/lib/rust-1.85/bin/cargo`) succeeds and
      `Cargo.lock` stays 1.85-compatible.
      2026-11-14: added `gstreamer` 0.23.7, `gstreamer-app` 0.23.5, `gstreamer-video` 0.23.6
      (rust-version 1.71.1), no feature flags (GStreamer >= 1.14 API). 0.23 is the gtk-rs 0.20
      generation: it shares glib 0.20 with gtk4/libadwaita, and `cargo tree -d` shows one glib
      (0.24.5, tried first, pulled in glib 0.21 as a second copy). `ashpd` 0.11.1 has no
      `screencast` feature; the module is always built. Lock resolved with
      `CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback`; no locked crate declares a
      rust-version above 1.85. `build-aux/vendor.sh` re-run (241 crates). **Open:** no Rust 1.85
      toolchain on the dev machine (built with 1.93); the 1.85 build proof comes from CI on
      ubuntu-24.04.
- [x] 3.2 `debian/control`: Build-Depends `libgstreamer1.0-dev`,
      `libgstreamer-plugins-base1.0-dev`. Depends `gstreamer1.0-pipewire`,
      `gstreamer1.0-plugins-base`. Run `build-aux/vendor.sh` and `build-aux/update-copyright.py`.
      Verify: `lintian -I --pedantic` is clean (in 7.4).

## 4. Window capture through ScreenCast

- [x] 4.1 `src/capture/screencast.rs`: `window_source_available()`, `choose_window(cursor, parent)
      -> Chosen` (session + node + fd, closes on drop), `Chosen::grab_frame() -> Frame` (D2, 5 s
      timeout). Verify: unit test for the drop order (pipeline `NULL` → fd → session) with fakes.
- [x] 4.2 Crop (D3): crop meta when present, else alpha bounding box. Verify: unit tests for an
      alpha box with a window in the top-left, with transparent rounded corners inside, with no
      opaque pixel (→ error), and for the crop-meta branch.
- [x] 4.3 `capture::run` window branch: availability check, chooser → countdown (D4) → frame →
      `Source::Window`. Fallback to `portal::grab(PICKER)` as today (D6). Verify: unit test of
      the pure step order (`window_steps(delay)`), and an existing `delivery` test still covering
      `Source::Window`.
- [x] 4.4 Error mapping (D6, as measured in 2.1): closed → `Cancelled` (silent), no stream /
      timeout / no opaque pixel → `Failed(cause)`. Verify: `live_tests.rs` case where the fake
      portal answers `Start` with each code and the right `CaptureError` comes back.
- [x] 4.5 Pointer (D1/D5): `cursor_mode` from include-pointer and `AvailableCursorModes`. Verify:
      unit test of the choice function for all four combinations.
- [x] 4.7 Waiting OSD (D8): `data/ui/window-wait.blp` + `src/capture/window_wait.rs`, shown
      from `Start` to the answer. Its exported handle is the `Start` parent. Cancel/`Esc` closes
      the session. A second request raises it. Matches sheet 02. Verify: `live_tests.rs` case where
      the fake portal never answers `Start`, cancel is triggered, `Session.Close` is called and
      `BUSY` is cleared.
- [x] 4.6 Strings via gettext, `po/de.po` updated in the same commit ("Waiting for a window…" and
      "Cancel" for the OSD).

## 5. Spec and docs follow-through

- [x] 5.1 Update `docs/design/add-screen-capture/index.md` so that mockup 05 points to this
      change's flow sheet as superseded. Verify: the link resolves.
- [x] 5.2 CLAUDE.md §4 status: window mode now through ScreenCast, and the defect class ("spec
      described the portal without a probe") noted once.

## 6. Live checks

- [ ] 6.1 `manual-checks.md`, every row run before archive:
      - Window card → chooser → editor with "Window captured · W × H", image has no transparent
        margin and no shadow.
      - Launcher "Capture Window", then `annota --capture window` from a terminal and from a
        GNOME keybinding.
      - Chooser closed: nothing happens.
      - Delay 3 s: chooser first, then countdown, then frame.
      - Countdown cancelled: indicator disappears.
      - Include pointer on and off.
      - Scale 1.25 and 1.
      - A maximised and a small window.
- [ ] 6.2 Fresh Ubuntu 24.04 (VM or container with GNOME): install the `.deb` with apt, window
      capture delivers an image (debian-packaging "Window capture works on a fresh install").

## 6a. Automated live tests (design.md D9)

- [ ] 6.3 `tools/portal-input.py` (RemoteDesktop keys and pointer, restore token) committed with
      its `--help` text. `tools/portal-probe.py --screencast-window` gains a parent window and
      the crop-meta read. Verify: `tools/portal-input.py check` prints "input session ready".
- [ ] 6.4 `tools/live-window-capture.py`: one function per scenario of `specs/capture` "Window
      capture" and "Cursor inclusion" (window part), driving Annota and GNOME's chooser as in D9.
      Each run asserts the outcome from the editor's accessible tree or from the absence of a new
      document. Verify: all scenarios pass twice in a row on the maintainer's desktop, and the
      run prints a scenario → pass/fail table.
- [ ] 6.5 `manual-checks.md` keeps only what 6.4 cannot automate (6.2 fresh install, 2.4 X11)
      and links the 6.4 output for the rest.

## 7. Verify

- [x] 7.1 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `meson test -C _build`
      green.
- [ ] 7.2 `openspec validate --all --strict` passes. `fix-region-pointer-inclusion` is archived
      first, or this delta is rebased onto its final "Cursor inclusion".
- [ ] 7.3 No frame touches disk: reviewer greps `screencast.rs` for file writes and checks that
      no temp file is created during a window capture (`strace -e trace=openat` during one live
      capture, or an equivalent).
- [x] 7.4 `build-aux/make-deb.sh` builds and `lintian -I --pedantic` is clean. CI run 43612609079 (ubuntu-24.04, Rust 1.85): build, tests, .deb + lintian green.
- [ ] 7.5 `reviewer` agent (fresh context): specs vs. tests, ADR vs. code, no blocking findings.

## Workflow follow-up

- `/opsx:archive fix-window-capture-screencast` after `fix-region-pointer-inclusion` is archived.
- `fix-portal-shortcut-bindings` stays parked (2/23) until the maintainer picks it up again.
