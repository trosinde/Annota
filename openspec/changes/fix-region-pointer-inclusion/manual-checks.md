# Manual checks — fix-region-pointer-inclusion

The decision itself is fully automated. Everything that goes through `IRect::pointer_inside` —
which position is used, whether the pointer is drawn, where it lands in the crop — is covered by
unit tests that need no display, and the capture and the overlay's preview share that one
function, so they cannot drift apart.

Two tests make that claim, and they are not the same claim.
`capture::tests::the_preview_and_the_capture_cannot_disagree` sweeps 3111 positions through the
shared predicate and `region_pointer`; on its own it is close to a tautology, since
`region_pointer` is a thin wrapper over the predicate.
`overlay::tests::the_preview_reaches_the_captures_verdict` is the one that earns the claim: it
sweeps the same grid through `preview_for`, the preview's own decision path including the toggle
and the monitor filter, and asserts its verdict against what `region_pointer` would draw.

What is **not** automated is everything that only exists on screen: that the preview arrow and the
chip are legible, that they do not collide with the loupe in practice, that the widgets are
actually shown and hidden as `update_pointer_preview` asks, and that the hint bar teaches the
gesture well enough that a user finds it. Those need a real session. The decision behind each of
those is unit-tested; the GTK plumbing that acts on it is not.

## Automated

| Scenario | Test |
|---|---|
| Cursor inclusion / Pointer inside the selection | `geometry::the_pointer_is_drawn_only_inside_the_selection`, `capture::the_selections_origin_is_subtracted`, `overlay::the_pointer_is_the_one_at_confirmation_not_the_first_seen` |
| Cursor inclusion / Pointer outside the selection | `geometry::the_pointer_is_drawn_only_inside_the_selection`, `capture::a_pointer_on_another_monitor_is_not_drawn`, `capture::no_pointer_position_draws_nothing`, `overlay::leaving_the_frozen_image_forgets_the_position` |
| Cursor inclusion / Pointer on the selection edge | `geometry::the_hot_spot_decides_not_the_whole_arrow` — one pixel inside the lower-right corner still draws, exactly on the edge does not |
| Cursor inclusion / Include cursor (screen capture) | Unchanged by this change; `frame::pointer_is_visible_after_drawing` |
| Cursor inclusion / Cursor excluded | `capture::an_excluded_pointer_leaves_the_portal_frame_unmodified`: region and screen images with Include Pointer off are pixel-identical to the portal frame of the captured area, with a control that the same captures do differ with it on. Window mode never draws a pointer. |
| Cursor inclusion / Delayed region capture places the pointer | `capture::a_delayed_region_capture_draws_the_pointer_where_the_countdown_ended` (via `capture_pointer`, the decision the countdown path now goes through); the probe itself needs a display (live check 8) |
| Pointer preview / Pointer will be included, will not be included | `overlay::the_preview_reaches_the_captures_verdict`, `overlay::the_edge_suffix_fires_only_just_outside` |
| Pointer preview / No indication when excluded | `overlay::the_preview_is_off_while_include_pointer_is_off` |
| Pointer preview / Indication follows the selection | `overlay::the_preview_follows_the_selection_before_confirming` (nudge and handle resize flip the verdict); that every selection edit reaches `update_pointer_preview` through `update_ui` is read from the code, the on-screen update is live check 5 |
| Pointer preview / no position: over the chrome, another monitor, keyboard only | `overlay::the_preview_has_no_state_without_a_position_on_this_monitor`; the hint segment stays, driven by the toggle rather than the position |
| Pointer preview / no selection yet (Phase Idle) | `overlay::without_a_selection_nothing_is_in_capture` |
| Pointer preview / verdict matches the capture | `overlay::the_preview_reaches_the_captures_verdict` (the preview's own path), `capture::the_preview_and_the_capture_cannot_disagree` (the shared predicate) |
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
