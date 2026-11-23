# Live run of tools/live-window-capture.py

Date: 2026-11-14. GNOME Shell 50.1 (Wayland, scale 1.25), xdg-desktop-portal 1.21.1,
Annota built from the working tree after commit 1355c07 plus the clipboard-truth fix
(design.md "Decided during apply", 2026-11-14). Command:
`python3 -I tools/live-window-capture.py --annota _build/src/annota` (two runs).

| Scenario | Run 1 | Run 2 |
|---|---|---|
| capture / Window capture / Window on Wayland | pass | pass |
| capture / Window capture / Window mode from the welcome page | pass | pass |
| capture / Window capture / Waiting for the chooser | pass | pass |
| capture / Window capture / Cancel while waiting | pass | pass |
| capture / Window capture / Chooser closed | pass | pass |
| capture / Window capture / Delayed window capture | pass | pass |
| capture / Window capture / Countdown cancelled | pass | pass |
| capture / Cursor inclusion / Pointer in a window capture | pass | pass |
| capture / Cursor inclusion / Cursor excluded | pass | pass |
| capture / Window capture / No frame arrives | covered by src/capture/live_tests.rs (fake portal) | covered by src/capture/live_tests.rs (fake portal) |
| capture / Window capture / No window source | covered by src/capture/live_tests.rs (fake portal) | covered by src/capture/live_tests.rs (fake portal) |

Earlier runs the same day failed 1 of 18 times ("Pointer in a window capture", no image on the
clipboard after the toast's Copy); 5 repeats of that scenario alone and the two full runs above
passed. Watch for it.

## 2026-11-23: whole window at any crop-meta scale (design.md D3 amendment)

GNOME Shell 50.1 (Wayland, one monitor at scale 1.25). The target now has four coloured corner
squares, and every captured image is checked for all four (`expect_corners`). Command:
`python3 -I tools/live-window-capture.py --annota _build/src/annota --runs 1 --save <dir>`,
Annota built from `fix/window-capture-crop-scale`.

| Scenario | Before the fix (b96c775) | After the fix |
|---|---|---|
| capture / Window capture / Window on Wayland | (not run) | pass, 1280 × 800 |
| capture / Window capture / The whole window at any display scale | FAIL: "the top-left corner square is 95 × 95, expected about 60: cut off" (an 800 × 500 image of a 1280 × 800 window) | pass: small 1280 × 800, small decorated 1284 × 804, maximised decorated 6144 × 3360, all four corners, title centred |
| capture / Window capture / Window mode from the welcome page | (not run) | pass, 1280 × 800 |
| capture / Window capture / Waiting for the chooser | (not run) | pass |
| capture / Window capture / Cancel while waiting | (not run) | pass |
| capture / Window capture / Chooser closed | (not run) | pass |
| capture / Window capture / Delayed window capture | (not run) | pass |
| capture / Window capture / Countdown cancelled | (not run) | pass |
| capture / Cursor inclusion / Pointer in a window capture | (not run) | pass (twice) |
| capture / Cursor inclusion / Cursor excluded | (not run) | pass (twice) |

The images were looked at, not only checked by script. The 2026-11-14 runs above passed with
800 × 500 images that were the top-left 62.5 % of the window: a uniformly magenta target could
not tell a part from the whole.

After the review (the cut now grows along the window past a cutting meta instead of searching
the whole buffer), the full suite ran once more against the rebuilt binary: all 10 scenarios
pass, with the same sizes (1280 × 800, 1284 × 804, 6144 × 3360) and all four corners.
