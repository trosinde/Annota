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

| # | Check | Expected |
|---|---|---|
| 1 | Select a region, move the pointer inside it, press `Enter` | The arrow is in the saved image at that position. This is the check the whole change exists for. |
| 2 | Select a region, leave the pointer outside it, press `Enter` | No arrow, and the image is otherwise exactly the selection. |
| 3 | Drag a region and release at the lower-right corner | Chip reads "Pointer not in capture · on the edge", hint bar says how to fix it, and the capture has no arrow. The trap this direction creates, named in place. |
| 4 | While dragging, watch the chip and the loupe together | Both legible, no overlap; chip in the band above the loupe (mockup 05). |
| 5 | With the pointer inside the selection, nudge the selection out from under it with `Shift+→` | The preview flips to "not in capture" without confirming anything (mockup 03). |
| 6 | Toggle "Include pointer" off | Arrow, chip and hint segment all disappear; the hint bar is the one from add-screen-capture (mockup 04). |
| 7 | Move the pointer over the overlay toolbar, then press `Enter` | No arrow: that chrome is not in the frozen image, so the position is forgotten rather than kept stale. |
| 8 | Region capture with a delay: select, then move the pointer into the region during the countdown | The arrow is where it stood when the countdown ended — the path this change deliberately left alone (design.md D4). |
| 9 | Two monitors: selection on one, pointer on the other, `Enter` | No arrow. |
| 10 | Chip and hint segment in dark and at 2× | Legible; icons recoloured by GTK, not stuck dark. |

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
