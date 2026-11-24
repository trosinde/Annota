# Manual checks — fix-region-pointer-inclusion

The decision itself is fully automated. Everything that goes through `IRect::pointer_inside` —
which position is used, whether the pointer is drawn, where it lands in the crop — is covered by
unit tests that need no display, and the capture and the overlay's preview share that one
function, so they cannot drift apart. Since the 5.5 review the hot spot is mapped to an image
pixel by `geometry::image_px`, the same rounding the overlay uses for the selection's edges, so a
fractional release on the edge can no longer count as one pixel inside it.

`overlay::tests::the_preview_reaches_the_captures_verdict` is the test that earns the "cannot
disagree" claim: it sweeps positions straddling every edge of a selection by fractions of a
pixel, at 100 % and 125 %, through `preview_for` (the preview's own decision path, toggle and
monitor filter included) and compares its verdict with the image `compose_region` really produces.

The GTK plumbing — that `update_pointer_preview` actually shows and hides the chip, its suffix and
the hint segment, and that motion, `pointer_left`, a nudge and the toggle repaint them — is proven
by the live test `overlay::tests::the_overlay_shows_what_will_be_captured` (headless X11, isolated
bus, run by the scenario gate). It fails if `pointer_left` stops repainting, the defect of the
first review.

What is **not** automated is what only a person on a real session can judge: that the preview
arrow and the chip are legible, that they do not collide with the loupe in practice, that icons
recolour in dark mode and at 2x, and that the hint bar teaches the gesture well enough that a user
finds it — plus the whole chain against the real portal.

## Automated

| Scenario | Test |
|---|---|
| Cursor inclusion / Pointer inside the selection | `capture::a_region_capture_draws_the_pointer_only_inside_the_selection` (pixel in the composed image), `geometry::the_pointer_is_drawn_only_inside_the_selection`, `capture::the_selections_origin_is_subtracted`, `overlay::the_pointer_is_the_one_at_confirmation_not_the_first_seen` |
| Cursor inclusion / Pointer outside the selection | `capture::a_region_capture_draws_the_pointer_only_inside_the_selection` (image byte-identical to the crop), `geometry::the_pointer_is_drawn_only_inside_the_selection`, `capture::a_pointer_on_another_monitor_is_not_drawn`, `capture::no_pointer_position_draws_nothing`, `overlay::leaving_the_frozen_image_forgets_the_position` |
| Cursor inclusion / Pointer on the selection edge | `overlay::a_fractional_release_on_the_edge_draws_no_pointer` — a drag released at fractional positions (x.0 to x.75, 100 % and 125 %) yields no pointer, an "on the edge" chip and an image identical to the crop; it fails with the old truncating test. `geometry::the_hot_spot_decides_not_the_whole_arrow` — one pixel inside the lower-right corner still draws, exactly on the edge does not |
| Cursor inclusion / Include cursor (screen capture) | Unchanged by this change; `frame::pointer_is_visible_after_drawing` |
| Cursor inclusion / Cursor excluded | `capture::an_excluded_pointer_leaves_the_portal_frame_unmodified`: region and screen images with Include Pointer off are pixel-identical to the portal frame of the captured area, with a control that the same captures do differ with it on. Window stills hide the stream's cursor (`screencast::pointer_mode_for_all_four_combinations`). |
| Cursor inclusion / Delayed region capture places the pointer | `capture::a_delayed_region_capture_draws_the_pointer_where_the_countdown_ended` (via `capture_pointer`, the decision the countdown path now goes through); the probe itself needs a display (live check 8) |
| Pointer preview / Pointer will be included, will not be included | `overlay::the_overlay_shows_what_will_be_captured` (live: chip text, suffix and hint segment on screen), `overlay::the_preview_reaches_the_captures_verdict`, `overlay::the_edge_suffix_fires_only_just_outside` |
| Pointer preview / No indication when excluded | `overlay::the_overlay_shows_what_will_be_captured` (live: the toggle's signal hides chip and hint segment), `overlay::the_preview_is_off_while_include_pointer_is_off` |
| Pointer preview / Indication follows the selection | `overlay::the_overlay_shows_what_will_be_captured` (live: `Shift+Left` flips the chip on screen without confirming), `overlay::the_preview_follows_the_selection_before_confirming` (nudge and handle resize flip the verdict) |
| Pointer preview / no position: over the chrome, another monitor, keyboard only | `overlay::the_preview_has_no_state_without_a_position_on_this_monitor`; the hint segment stays, driven by the toggle rather than the position |
| Pointer preview / no selection yet (Phase Idle) | `overlay::without_a_selection_nothing_is_in_capture` |
| Pointer preview / verdict matches the capture | `overlay::the_preview_reaches_the_captures_verdict` (the preview's own path against the composed image) |
| Pointer preview / chip position and loupe clearance | `geometry::the_pointer_chip_sits_beside_the_hot_spot_and_clears_the_loupe` asserts the chip's bottom edge is above `loupe_position`'s top edge, so the collision this design was most at risk of cannot regress silently; `geometry::the_pointer_chip_flips_at_the_right_edge` |
| No stale position after a keyboard-only capture | `overlay::a_keyboard_only_capture_has_no_pointer_position` |
| A late `leave` does not erase the next monitor's position | `overlay::a_late_leave_does_not_erase_the_next_monitors_position` |

The regression test for the reported defect is
`overlay::the_pointer_is_the_one_at_confirmation_not_the_first_seen`: with the old
`first_pointer` logic (set once, on the first motion) its second `note_pointer` call is ignored
and the assertion fails.

## Live, still to run

Region capture with "Include pointer" on, on a real session. Reported 2026-11-10 on GNOME Shell
50.1 / Wayland / xdg-desktop-portal 1.21.1.

| # | Check | Expected | Result | Date |
|---|---|---|---|---|
| 1 | Select a region, move the pointer inside it, press `Enter` | The arrow is in the saved image at that position. This is the check the whole change exists for. | pass, 2 of 2: selection 750 × 500 px, pointer moved to 300, 200 logical px inside it, `Enter`; chip "Pointer in capture"; arrow in the saved image at x 374–391, y 249–275, hot spot expected at 375, 250 | 2026-11-23 |
| 2 | Select a region, leave the pointer outside it, press `Enter` | No arrow, and the image is otherwise exactly the selection. | pass, 2 of 2: pointer 200 px right of the selection, chip "Pointer not in capture"; image 750 × 500, every pixel the background colour | 2026-11-23 |
| 3 | Drag a region and release at the lower-right corner | Chip reads "Pointer not in capture · on the edge", hint bar says how to fix it, and the capture has no arrow. The trap this direction creates, named in place. | pass, 2 of 2: chip "Pointer not in capture · on the edge", hint bar starts "Move the pointer into the selection to include it · Enter Capture", image has no arrow | 2026-11-23 |
| 4 | While dragging, watch the chip and the loupe together | Both legible, no overlap; chip in the band above the loupe (mockup 05). | pass at 1.25, from an overlay screenshot taken mid-drag: chip "Pointer not in capture · on the edge" sits above the loupe and does not overlap it, and both read clearly. Comparing it against mockup 05 by eye stays the maintainer's call | 2026-11-23 |
| 5 | With the pointer inside the selection, nudge the selection out from under it with `Shift+→` | The preview flips to "not in capture" without confirming anything (mockup 03). | pass, 2 of 2: pointer 4 px inside the left edge, chip "Pointer in capture"; after `Shift+→` it reads "Pointer not in capture" with no capture; `Enter` then gives an image without an arrow | 2026-11-23 |
| 6 | Toggle "Include pointer" off | Arrow, chip and hint segment all disappear; the hint bar is the one from add-screen-capture (mockup 04). | pass, 3 of 3 (toggle pressed over AT-SPI): preview arrow, chip and the hint segment disappear; the hint bar reads "Enter Capture · Esc Cancel · ←↑→↓ Nudge 1 px · Shift ×10 · Space Move selection"; image has no arrow. Not compared against mockup 04 | 2026-11-23 |
| 7 | Move the pointer over the overlay toolbar, then press `Enter` | No arrow: that chrome is not in the frozen image, so the position is forgotten rather than kept stale. | pass, 2 of 2: pointer inside first ("Pointer in capture"), then over the toolbar's W field: the chip is gone; image has no arrow | 2026-11-23 |
| 8 | Region capture with a delay: select, then move the pointer into the region during the countdown | The arrow is where it stood when the countdown ended (design.md D4; the probe it relies on was fixed for this check, see design.md "Live check 8"). | pass after the fix (PR #11): `tools/live-region-pointer.py --runs 3`; region, pointer moved in during the countdown: arrow at 249, 124 (expected 250, 125), 3 of 3; pointer inside throughout: 3 of 3; screen capture: arrow at 1499, 624 (expected 1500, 625), 3 of 3. Before the fix: fail, 4 of 4 without an arrow (see "Live run 2026-11-23"). | 2026-11-23 |
| 9 | Two monitors: selection on one, pointer on the other, `Enter` | No arrow. | not run: this desktop has one monitor | |
| 10 | Chip and hint segment in dark and at 2× | Legible; icons recoloured by GTK, not stuck dark. | partly: at 1.25 the chip's icons are drawn light on the dark chip and the hint segment's icon is light on the dark bar. The overlay chrome is dark in both colour schemes, so `ADW_DEBUG_COLOR_SCHEME=prefer-dark` looks the same. 2× was not run: the desktop is at 1.25 | 2026-11-23 |
| 11 | Two monitors, screen capture with Include Pointer on and off, pointer resting on monitor 2 | Only monitor 2 is captured; with the pointer on, the arrow is where the pointer rests (the probe's cover step on two monitors, design.md "Live check 8"). | open: not run, this desktop has one monitor; X11 not run either |  |

### Live run 2026-11-23

GNOME Shell 50 on Wayland, one monitor 3840 × 2160 at scale 1.25. Annota was built from `main`
b96c775 and run with private keyfile settings (include-pointer on, after-capture "editor", save
folder in a temporary directory). A scratch driver ran it with no human. It placed a maximised
magenta window as the background, started `annota --capture region`, and moved and clicked the
pointer through `tools/portal-input.py`, from a known corner and checked against the overlay's
size label. It read the chip and hint labels over AT-SPI and pressed `Enter`, then `Ctrl+S` in
the editor, and took `original.png` out of the saved `.annota`. Overlay screenshots came from
the Screenshot portal. Every image was inspected pixel by pixel and by eye.

**Check 8 failed on this build (b96c775); fixed by PR #11, see row 8.** The selection is 750 × 500 px at 1000, 400 logical. The toolbar's delay is set
to "3 s" (the radio menu item, over AT-SPI). The pointer rests outside the selection at
1800, 600, and the capture is confirmed (Capture pressed over AT-SPI; with a delay, the
position at confirmation is not used, as `capture_pointer` decides). During the countdown the
pointer moves to 1200, 500, inside the selection. The magenta background window logged
"enter 1200.0 500.0" during the countdown, then "leave" when the probe window opened. Expected:
the arrow with its hot spot at about 250, 125 in the image. Observed: a 750 × 500 image with no
pixel other than the background colour, in 2 of 2 runs. The same happened with the pointer
resting inside the selection the whole time (1 run), and with the pointer wiggled by 1 px every
50 ms for 4 s after the move, so that motion events reached the probe window (1 run): 4 of 4 delayed runs without an arrow.
`probe_pointer` waits `PROBE_TIMEOUT_MS` = 400 ms for an enter or motion event on a freshly
presented fullscreen window. That this is too short on this desktop is a guess, not a
measurement.

Not automatable here: check 9 (needs two monitors), 2× in check 10 (the desktop is at 1.25), and
the subjective half of checks 4 and 10 (whether it matches the mockups). A side note: the delay
menu's seven radio items have empty accessible names over AT-SPI.

## Known, not covered

- **The Capture button can never include the pointer**: clicking it puts the hot spot on the
  button, so a mouse-driven capture is always pointer-free. Only `Enter` or a delay can include
  it. The hint bar teaches `Enter`; the button's tooltip was deliberately left alone (designer's
  open question, index.md).
- **The preview arrow and the system cursor share the hot spot.** Mockups 05 and 06 show the
  honest pile-up at a corner (handle + resize cursor + preview arrow + slash). It reads on the
  mockup; check 4 above is where it is judged for real. Hiding the system cursor was considered
  and not proposed, because it costs the crosshair's precision.
- The two icons are new artwork (`annota-pointer-included-symbolic`,
  `annota-pointer-excluded-symbolic`). Decision D8 wants kit icons generated by
  `tools/build-icons.py`, which does not exist; like every other icon in `data/icons/` these are
  authored files. Adding them to `docs/design/kit/icons/` stays an open D8 follow-up, shared with
  the 15 other icons D8 lists.
