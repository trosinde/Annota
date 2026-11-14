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
