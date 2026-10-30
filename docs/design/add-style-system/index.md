# add-style-system — mockups

Source HTML: `src/`, renders: `png/<name>-{light,dark}@{1,2}x.png`. Render with
`python3 tools/render-mockups.py docs/design/add-style-system`.

| # | Screen | Light 1× |
|---|---|---|
| 01 | Quick style grid in the properties sidebar (Arrow tool, hover tooltip) | [png](png/01-quick-style-grid-light@1x.png) |
| 02 | Changed properties → "Save as Quick Style" popover | [png](png/02-save-as-quick-style-light@1x.png) |
| 03 | New style appended at the end of My Styles + context menu (rename, reorder, delete) + toast | [png](png/03-style-saved-context-menu-light@1x.png) |
| 04 | Favourites view (starred styles across tools) | [png](png/04-favourites-view-light@1x.png) |
| 05 | After picking the callout favourite: Callout tool active with that style | [png](png/05-favourite-applied-callout-light@1x.png) |
| 06 | Theme editor dialog (palette, fonts, preview) | [png](png/06-theme-editor-light@1x.png) |
| 07 | Theme applied — theme switcher open, all theme styles use brand colours | [png](png/07-theme-applied-light@1x.png) |
| 08 | Managed theme from /usr/share/annota/themes (read-only) | [png](png/08-managed-theme-readonly-light@1x.png) |
| 09 | Theme More menu (Export / Duplicate / Delete) + "imported" toast | [png](png/09-theme-import-export-light@1x.png) |
| 10 | Import error (invalid theme file) | [png](png/10-theme-import-error-light@1x.png) |

All screens exist as `-dark@1x/2x`.

## Scenario → mockup

| Scenario | Mockup | Notes |
|---|---|---|
| Save style | 02 → 03 | Arrow changed to Teal / 8 px → Save as Quick Style → new tile "Teal bold" appears as the **last** tile of My Styles, selected, with a 4 px accent halo for 2 s; toast with Undo. |
| (Rename / reorder / delete user styles) | 03 | Context menu on a user tile (right-click, long-press, or `Menu` key). |
| Use favourite | 04 → 05 | Picking the starred coral callout switches the tool strip to Callout and selects that tile. |
| Apply theme | 07 (06 for editing) | Selecting "Northwind Ferries" regenerates the Theme section for every tool; Properties palette shows the theme colours; canvas defaults follow. User styles are untouched. |
| Managed theme | 08, 07 | Lock icon in the theme list and switcher, banner "Managed by your administrator · read-only", no Add colour, fonts disabled, Delete absent; Duplicate makes an editable copy under My Themes. |
| (Theme import/export) | 09, 10 | Import via folder button in the Themes list header (portal file chooser, filter `*.annota-theme`). Export from the header button or More menu. |

## Redlines

### Properties sidebar (01–05, 07)
- Sidebar 300 px, padding 18/12, 18 px between blocks (kit template).
- Top: toggle group full width, two toggles: *<Tool name>* | `star` *Favourites*.
- "Quick Styles" heading (15 px bold). Row below: caption "Theme" left, flat dropdown right showing 4 colour chips (10×10, radius 3, 2 px gap) + theme name (13 px).
- Grid: 4 columns × 63×48 px tiles, gap 8 px, radius 9, tile background always `--annota-paper` (white, also in dark — it shows ink as it will appear on a typical capture), 1 px `--borders` ring.
  - selected: 2 px accent ring; hover: `--borders-strong` + row-hover tint; keyboard focus: 2 px accent-50 % outline offset 2 px; newly saved: selected + 4 px accent-25 % halo (fades after 2 s).
  - favourite: `star` icon 12 px in `--annota-amber`, top-right, 2 px inset.
- Sections: unlabeled theme grid (8 generated styles per tool), then caption "My Styles" + user grid. Tooltip on hover: style description (e.g. "Coral · 7 px · shadow").
- Keyboard: arrow keys move in the grid, `Enter` applies, `F2` rename, `Ctrl+←/→` move, `Delete` deletes, `Menu` opens the context menu.
- Properties block: heading "Properties" + "Modified" (accent dot 8 px + 13 px label) when current values differ from the selected style; colour swatches; tool rows; full-width button `plus` "Save as Quick Style" — disabled when nothing changed.

### Save as Quick Style popover (02)
276 px, arrow down to the button, padding 12. Heading; preview tile + caption (summary, "Added to the end of <Tool>"); entry (focused, prefilled with a suggested name); check "Add to Favourites"; buttons Cancel / **Save** (suggested). `Enter` saves, `Esc` cancels.

### User style context menu (03)
Apply `Enter` · Rename… `F2` · — · Move Left `Ctrl+←` · Move Right `Ctrl+→` (disabled at ends) · — · Add to / Remove from Favourites · Update from Current Properties · — · Delete `Delete` (destructive text colour, no confirmation; toast with Undo instead).
Built-in theme tiles get only: Apply · Add to/Remove from Favourites.

### Favourites view (04)
Grouped by tool: tool icon 14 px + tool name as caption heading, then a 4-column grid of the starred tiles. Footer hint text (13 px dim). Empty state (none starred): show only the hint.

### Theme editor (06, 08, 09)
- Dialog 900×700, split: left list 260 px (sidebar bg), right detail.
- List header: `plus` (New Theme), title "Themes", `folder` (Import Theme…). Sections: *Built-in* (Annota — "Default" label, Monochrome), *Installed by administrator* (lock icon), *My Themes*. Each row: up to 5 colour chips (8×16, radius 2) + name.
- Detail header: start = **Use Theme** (suggested; replaced by dim "In use" label for the current theme), title = theme name, end = `export` (Export Theme…), `view-more`, close.
- Name entry row; group "Palette" with description "2–8 colours. The first colour is the default for every tool."; swatches 40×40 radius 9 + hex below (11 px mono); dashed "Add" tile hidden at 8 colours. Click a swatch → colour chooser popover (eyedropper, hex entry, Remove). Drag to reorder.
- Rows Heading font / Body font: font dropdowns (system font chooser).
- Group "Preview": 8 sample tiles (arrow ×2, callout ×2, step ×2, shape ×2) generated from colours 1–3, then a font sample line ("Delivery address" in heading font, colour 1 · body text sample) with "<heading> / <body>" caption right.
- More menu: Export Theme… · Duplicate · — · Delete Theme (destructive; confirms with an alert).
- Read-only (08): blue banner "Managed by your administrator · read-only" + **Duplicate** button; name row shows lock; no Add tile; font dropdowns disabled; no More menu, no Delete.

### Theme switcher (07)
Dropdown popover 250 px: each theme with chips + name; managed themes show a lock; current theme has a check; separator; "Manage Themes…" opens the editor (06).

### Messages
- Toast: “Quick style “<name>” saved” + Undo.
- Toast: “Theme “<name>” imported” + Apply.
- Alert "Can’t Import Theme": “"<file>" is not a valid Annota theme. It contains 11 colours; a theme needs 2–8.” (other reasons: "unknown schema version", "not a JSON file"). Single **OK**.
