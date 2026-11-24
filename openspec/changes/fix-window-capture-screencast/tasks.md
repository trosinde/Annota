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

- [x] 3.1 Add `gstreamer` (+ `gstreamer-app`, `gstreamer-video` if D3 uses crop meta) in the newest
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
      Proven: built and tested with Rust 1.85 in CI run 43612609079 (ubuntu-24.04).
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

- [x] 4.8 Whole window at any crop-meta scale (design.md D3, amendment of 2026-11-23): a crop
      meta in monitor pixels while the buffer is at scale 2 cut every window to its top-left
      62.5 %. New scenario "The whole window at any display scale" in the `capture` delta;
      `window_rect` falls back to the alpha box of the whole buffer when the meta cuts the window.
      Verify: unit tests with a four-coloured-corner window for a meta in buffer pixels and at
      1/1.6 of them (failing before the fix), and `tools/live-window-capture.py`'s target now has
      four coloured corner squares, checked in every captured image.

- [x] 4.9 Window image at screen size (design.md D3, maintainer decision 2026-11-24): a window
      stream at a higher scale than the monitor (2 on a 1.25 monitor) is scaled down to the
      monitor's scale, factor (monitor scale) ÷ (buffer px ÷ stream `size`), so a 640 × 400 window
      at 1.25 gives 800 × 500 and a maximised one on 3840 × 2160 gives 3840 × 2100. New scenario
      "Window image at screen size". Verify: unit tests (scale-2 buffer on a 1.25 monitor with
      the meta at 1.25 → 800 × 500, four corners; equal scales bit-identical; GDK monitor choice)
      and `tools/live-window-capture.py`, whose size check now expects logical size × monitor
      scale ± 2 px.

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
      2026-11-22, helper only: the v0.1.0 release `.deb` installs with apt in a plain
      `ubuntu:24.04` container and brings `gstreamer1.0-pipewire`, `gstreamer1.0-plugins-base`
      and both portals; `pipewiresrc`, `videoconvert` and `appsink` are found
      ([manual-checks.md](manual-checks.md), "Helper note for 6.2"). The GNOME part, a window
      image on a fresh 24.04 desktop, is still the maintainer's.

## 6a. Automated live tests (design.md D9)

- [x] 6.3 `tools/portal-input.py` (RemoteDesktop keys and pointer, restore token) committed with
      its `--help` text. `tools/portal-probe.py --screencast-window` gains a parent window and
      the crop-meta read. Verify: `tools/portal-input.py check` prints "input session ready".
- [x] 6.4 `tools/live-window-capture.py`: one function per scenario of `specs/capture` "Window
      capture" and "Cursor inclusion" (window part), driving Annota and GNOME's chooser as in D9.
      Each run asserts the outcome from the editor's accessible tree or from the absence of a new
      document. Verify: all scenarios pass twice in a row on the maintainer's desktop, and the
      run prints a scenario → pass/fail table.
- [x] 6.5 `manual-checks.md` keeps only what 6.4 cannot automate (6.2 fresh install, 2.4 X11)
      and links the 6.4 output for the rest.

## 7. Verify

- [x] 7.1 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `meson test -C _build`
      green.
- [ ] 7.2 `openspec validate --all --strict` passes. `fix-region-pointer-inclusion` is archived
      first, or this delta is rebased onto its final "Cursor inclusion".
      2026-11-22: `validate --all --strict` passes (22/22). Simulated on a copy of `openspec/`:
      `openspec archive fix-region-pointer-inclusion --yes`, then this change, applies
      (+1 ~1, then ~4) but the merged `capture` spec failed `--strict`: "Window capture",
      "Cursor inclusion" and "After-capture destination" exceeded 500 characters of requirement
      text (deltas are not length-checked, merged specs are). Fixed by tightening the three
      requirement texts; what they dropped is in their scenarios (a second request raising the
      indicator moved into "Waiting for the chooser", "no stream starts" into "No frame
      arrives"). After that the simulation ends with 20/20 valid, and the merged "Cursor
      inclusion" has the region change's six scenarios plus "Pointer in a window capture". The
      reverse order is refused (the region delta would drop that scenario). This delta's
      "Cursor inclusion" scenarios are the region change's six as of `main` 8fcf4ab plus "Pointer
      in a window capture" and a window clause in "Cursor excluded"; its requirement text was
      rewritten for the 500-character limit but keeps the region change's SHALLs; `fix/region-pointer-inclusion` was not pushed, so its final text
      could not be read. **Stays open** until `fix-region-pointer-inclusion` is archived (or its
      final "Cursor inclusion" is known and re-checked against this delta).
      Rebased the same day onto the region change's final text (`origin/fix/region-pointer-inclusion`
      a14d89a, PR #6): confirmation is `Enter` or the Capture button (no longer the drag
      release), a Capture-button confirmation has no pointer, and "Pointer on the selection edge"
      takes its new WHEN. The scenarios now equal a14d89a's six plus "Pointer in a window capture"
      and the window clause in "Cursor excluded". Simulation re-run with a14d89a's change dir:
      region first (+1 ~1), then this change (~4), 20/20 strict-valid. Still unticked until #6 is
      merged and archived; if its "Cursor inclusion" changes again, rebase again.
- [x] 7.3 No frame touches disk: reviewer greps `screencast.rs` for file writes and checks that
      no temp file is created during a window capture (`strace -e trace=openat` during one live
      capture, or an equivalent).
      2026-11-22, equivalent without a real portal. Static: outside `#[cfg(test)]`,
      `screencast.rs`, `window_wait.rs`, `frame.rs` and `capture/mod.rs` have no `std::fs`,
      `File`, `OpenOptions`, temp-file, `save_to_png` or file-sink use; the pipeline is
      `pipewiresrc ! videoconvert ! appsink` into memory (`Pixels::copy` → `Frame`). The only
      write on the capture path is `delivery::quick_save` into the user's save folder, the spec'd
      "Editor unavailable" fallback. Dynamic: `strace -f -e trace=openat,creat,open,rename*,link*,
      mkdir*,memfd_create` over (a) the fake-portal test
      `a_stream_without_a_frame_fails_without_blocking_annota` (CreateSession → Start →
      OpenPipeWireRemote → `pipewiresrc` started → deadline → Session.Close; isolated bus, Xvfb,
      empty `XDG_RUNTIME_DIR`) and (b) `a_stream_sample_is_cut_by_its_crop_meta_and_stride`
      (a GStreamer buffer with video and crop meta through `window_frame`). Every file opened for
      writing is one of: the test's own GSettings schema, the GStreamer plugin registry cache,
      mesa/NVIDIA GL shader caches, liborc's `orcexec.*` JIT-code file, `/proc/self/task/*/comm`
      and `/dev` GPU nodes. No image file, no file in a save or temp location with frame data.
      Limit: the fake portal's stream never delivers a frame, so (a) and (b) together, not one
      run, cover the path from the portal to a `Frame`.
- [x] 7.4 `build-aux/make-deb.sh` builds and `lintian -I --pedantic` is clean. CI run 43612609079 (ubuntu-24.04, Rust 1.85): build, tests, .deb + lintian green.
- [x] 7.5 `reviewer` agent (fresh context): specs vs. tests, ADR vs. code, no blocking findings.
      2026-11-22, pass 1: code matches the ADR and D1-D9. Blocking: (B1) the change's own
      MODIFIED scenarios "Editor destination", "Editor unavailable" and "Dependencies resolved"
      leaned on the baseline; (B2) open manual rows and tasks; (B3) `spec:` tags that claimed
      more than their tests prove. Handled: existing delivery and editor tests tagged with
      "Editor destination"; new live test `a_capture_the_editor_cannot_take_is_saved_beside_not_over`
      and `unique_path_never_overwrites` for "Editor unavailable"; "Dependencies resolved" from
      the 24.04 container install (manual row); their baseline entries removed. The OSD ring
      checks moved from two over-tagged unit tests into `cancelling_the_wait_closes_the_session`;
      "No frame arrives" dropped from the code-2 test; "Waiting for the chooser" added to the
      no-frame test; "Window mode from the welcome page" moved to the fake-portal test that
      proves window mode makes no Screenshot-portal call, with the GNOME half as a manual row
      from [live-run.md](live-run.md) (2026-11-14). New unit test for crop meta and stride read
      from a real `gst::Buffer` (N2). B2 is the maintainer's 6.1/6.2 and the X11 row, which stay
      open. Non-blocking findings: design.md "Risks / Trade-offs".
      Pass 2 (fresh reviewer, commits 8fcf4ab..73483b1): nothing blocking; it re-ran the 7.2
      simulation with the same result. Its non-blocking points on the shortened requirement
      texts are taken up (request no restore token; "no further window" in "Cancel while
      waiting"; "5 seconds of the stream starting"; pointer drawing only when included; the
      Screenshot-portal fallback left to the desktop), the second-request test now asserts the
      same OSD stays up, and the rest is in design.md.

## Workflow follow-up

- `/opsx:archive fix-window-capture-screencast` after `fix-region-pointer-inclusion` is archived.
- `fix-portal-shortcut-bindings` stays parked (2/23) until the maintainer picks it up again.
