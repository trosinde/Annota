# Manual checks — add-style-system

Live coverage for this change is `ui::editor::tests::quick_styles_on_a_live_display` and, for the
focus fix below (reviewer finding B1), `ui::editor::tests::style_menu_returns_focus_on_a_live_display`.
An earlier version of this file said the fix needed a real window manager. It does not: under Xvfb
with no window manager the window is active and the focus is observable (run as in
`openspec/changes/add-export-share/manual-checks.md`).

| Scenario | Status | How it is checked |
|---|---|---|
| Closing a style tile's context menu returns focus to the tile | Automated | With a canvas object selected, the test opens a tile's menu with the Menu key (asserting the open menu holds the focus), closes it by popdown, and asserts the tile has the focus and that `Delete` through the window's key handler leaves the object selected and in place. Apply, Add to Favourites, Remove from Favourites, Update and Move Left/Right are checked the same way, in the order GTK runs them (popdown, then the action). With the `connect_closed` handler removed the test fails. |

Not exercised by the test, and worth a look when the maintainer runs the other changes' manual
checks: a real right-click and a real Escape key press (the test uses the Menu key's code path and
`popdown()`), and Remove from Favourites from the Favourites view, where the tile goes away.
