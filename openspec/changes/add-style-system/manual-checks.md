# Manual checks — add-style-system

Everything in this change's design.md is covered by unit tests except one focus-management
detail that needs a real GTK display and window manager, not just a headless `xvfb-run`
session (keyboard focus delivery is unreliable under a bare Xvfb without a window manager
giving the popover window focus in the first place).

| Scenario | Status | How to check |
|---|---|---|
| Closing a style tile's context menu returns focus to the tile | Implemented (fix for reviewer finding B1) | In the editor, select a canvas object, then open a style tile's context menu (right-click or the tile's menu button) and close it with `Escape`, or by choosing "Apply", "Add to Favourites"/"Remove from Favourites", "Move left"/"Move right" or "Update from selection". Press `Delete` immediately after: it must do nothing to the selected canvas object (focus is back on the tile, not the canvas). Before the fix, focus landed on the canvas and `Delete` removed the selected object instead. |

Automated coverage: `src/styles/library.rs` unit tests cover the quick-style/favourite/theme
model, including `fresh_ids_skip_collisions_with_unknown_styles` (reviewer finding N1).
`src/ui/editor/{colour,panel,sidebar,styles,theme_editor}.rs` and `src/ui/canvas.rs` cover the
pure logic behind the UI (number parsing/formatting, the phantom-edit guard, theme/file
naming, etc.) with ordinary unit tests; none of this change's UI code needed a live-display
test beyond the one row above.
