# Manual checks — add-style-system

Live coverage for this change is `ui::editor::tests::quick_styles_on_a_live_display`. The focus fix
below (reviewer finding B1) has no automated regression test yet: an earlier version of this file
said it needed a real window manager, but that was never tried, and it wrongly implied the project
has no live widget tests. A live test is tracked for it; until it lands, check it by hand.

| Scenario | Status | How to check |
|---|---|---|
| Closing a style tile's context menu returns focus to the tile | Implemented, no regression test yet | In the editor, select a canvas object, then open a style tile's context menu (right-click or the Menu key) and close it with `Escape`, or by choosing Apply, Add to Favourites/Remove from Favourites, Move Left/Move Right or Update. Press `Delete` immediately after: the selected canvas object must stay. Before the fix, focus landed on the canvas and `Delete` removed it. |
