# Design: Add quick styles and themes

## Mockups (design gate)

Mockups live in `docs/design/add-style-system/` (HTML in `src/`, PNG light/dark at 1× and 2× in `png/`).
Scenario-to-mockup mapping and redlines: [docs/design/add-style-system/index.md](../../../docs/design/add-style-system/index.md).

Screens (light, 1×; every screen also has `-dark@1x/2x`):

- [01-quick-style-grid](../../../docs/design/add-style-system/png/01-quick-style-grid-light@1x.png)
- [02-save-as-quick-style](../../../docs/design/add-style-system/png/02-save-as-quick-style-light@1x.png)
- [03-style-saved-context-menu](../../../docs/design/add-style-system/png/03-style-saved-context-menu-light@1x.png)
- [04-favourites-view](../../../docs/design/add-style-system/png/04-favourites-view-light@1x.png)
- [05-favourite-applied-callout](../../../docs/design/add-style-system/png/05-favourite-applied-callout-light@1x.png)
- [06-theme-editor](../../../docs/design/add-style-system/png/06-theme-editor-light@1x.png)
- [07-theme-applied](../../../docs/design/add-style-system/png/07-theme-applied-light@1x.png)
- [08-managed-theme-readonly](../../../docs/design/add-style-system/png/08-managed-theme-readonly-light@1x.png)
- [09-theme-import-export](../../../docs/design/add-style-system/png/09-theme-import-export-light@1x.png)
- [10-theme-import-error](../../../docs/design/add-style-system/png/10-theme-import-error-light@1x.png)

Design approved: 2026-10-30

## Technical decisions
- Styles stored in `~/.local/share/annota/styles/`; themes as JSON with schema version

### Decided during apply (2026-10-31: tasks 2.1–2.4)

**Data model, and how it hooks into the property system.**
- *A quick style is a set of `properties`/`set_property` values* — the same values the sidebar
  edits and `editing::tool::Toolbox` keeps as tool defaults (`styles::value`). Its keys are the
  tool's defaults minus what is not style: a text's words and a marker's value
  (`Toolbox::INTERNAL`), a shape's *kind* (a style switching to polygons would change how the
  tool draws) and a step sequence's *counting* (it belongs to the whole sequence, B1 of
  add-core-annotation-tools). In JSON a value is written by kind and read back by the kind the
  tool's prototype has; a value a type refuses, or a key a newer Annota added, is kept aside and
  written back unchanged — a style can never hold a value the type would not accept.
- *Applying is the existing property path*, extended to several keys:
  `ops::set_properties` makes one `Replace` with only the objects that really change (the
  phantom-edit guard of `ops::set_property`; a style already in use adds no step), undo label
  "Apply Style to <Type>" (`Action::ApplyStyle`). `Canvas::apply_style` routes like
  `Canvas::set_property`: the text being edited (part of editing's one step), else the selection,
  else the tool's defaults — which, like every tool default since add-core-annotation-tools, are
  not part of a document's history (no undo step there).
- *The selected tile is derived, not remembered*: the style whose values the current values
  contain (`value::matches`), preferring the one applied or saved last when several match. None
  matching is "Modified" and enables Save as Quick Style. So nothing goes stale when the
  selection, the tool or undo changes the values.
- *The library is application state, not document state* (`styles::library::Library`,
  `~/.local/share/annota/styles/styles.json`): saving, renaming, moving, starring, updating and
  deleting styles are not `Command`s — they would otherwise be undone with a document's edits and
  lost when it closes. Save and Delete get a toast with Undo built from the inverse operation
  (`Library::remove` → `Library::restore` puts a style back at its place, star included);
  rename/move/star/update are immediately reversible by the same control and get none. Every
  change is written at once (temporary file + rename) and announced to the views
  (`ui::editor::styles::Styles::connect_changed`).
- *Favourites* are references: a user style by id, or a theme slot `(theme id, tool, slot)`. A
  starred theme style stays a favourite when another theme is in use (it resolves against the
  theme it came from) and disappears with that theme.
- `styles.json` (schema `data/schema/styles-1.schema.json`) follows `document.json`'s versioning:
  `"format": "1.0"`; unknown top-level keys, styles of unknown tools, unknown style properties and
  favourites of unknown kinds are kept and written back; a newer *major* is not read and never
  overwritten (`Library::read_only`); a file that is not a library at all is moved aside to
  `styles.json.broken` instead of being overwritten silently.

**Themes and the generator** (`styles::theme`, `styles::generate`).
- Built in: **Annota** = the add-core-annotation-tools palette in its swatch order (Coral, Amber,
  Lime, Teal, Azure, Violet, Ink) with Adwaita Sans for both fonts; **Monochrome** = Ink, a neutral
  grey (#77767B), Coral (mockup 06's chips).
- Eight styles per tool, deterministic, built from the tool's built-in defaults by
  `set_property` (so always valid): slots 0–3 are the default look in colours 1–4, slot 4 in the
  theme's *ink* (its darkest colour after the first, if dark; Annota's Ink otherwise; colour 5 when
  colour 1 already is that dark, as in Monochrome), slots 5–7 are variations in colour 1 per tool —
  bold/dashed/no shadow for arrows, lines and pens, tinted fill/solid fill/dashed for shapes, large
  /outlined/plain body text for text, ink fill/outlined/colour 6 for callouts, square/large/
  outlined for steps; highlights are yellow, colours 1–5, light yellow, light colour 1; redactions
  are pixelate/blur at 70 and 100, solid ink, solid colour 1, ellipse pixelate/blur. The full table
  is in `generate.rs`. A theme with fewer colours than a slot needs uses tints of colour *i mod n*,
  lighter each round, so two colours still give eight different styles (tested for 2-colour,
  Monochrome, Annota and a brand theme). Why this design: slot 0 of Annota *is* every tool's
  built-in default (tested), so a fresh window shows tile 1 selected and nothing "Modified", and
  "Use Theme" can simply make slot 0 the defaults ("The first colour is the default for every
  tool", mockup 06); the first row varies only colour (what a brand theme is about), the second
  row the most-used variations, which keeps the grid predictable across themes.
- The theme editor's preview is exactly mockup 06's composition: arrow slots 0, 1; callout slots
  0, 2; step slots 0, 1; shape slots 0, 1 — colours 1–3, the second callout showing the automatic
  text colour on a light fill.
- Text on a fill (callouts, step labels) is white unless white contrasts below 3:1 (WCAG's
  minimum for large text) and ink contrasts better: Amber and yellows get dark text, Coral and
  Azure white. *Deviation:* mockup 05 shows white bars on the Teal callout; by this rule Teal gets
  dark text (white would be 2.5:1).
- Fonts: the heading font goes to the text tool, the body font to callouts (and the text tool's
  "plain body text" slot). *Deviation:* the redline's Body font subtitle says "…, step numbers",
  but step markers have no font property in the model (their label font is fixed), so the
  subtitle reads "Callout text and plain body text" instead of promising what is not done.
- Using a theme (switcher, "Use Theme", or editing the theme in use) sets every tool's defaults
  to its slot 0 and the sidebar's palette swatches to its colours (`SwatchSet::lines_with`; the
  highlighter's yellow takes Amber's place, or comes first in a theme without Amber); objects
  already in documents and user styles are untouched. The current theme is stored in
  `styles.json` and applied when the editor window is built.

**`.annota-theme` format** (`styles::theme`, schema `data/schema/annota-theme-1.schema.json`).
UTF-8 JSON: `format` ("1.0"), `requires` ("0.1"), `name`, `colours` (2–8 × `#RRGGBB` or
`#RRGGBBAA`), `fonts.heading`, `fonts.body`. Forward compatibility is `document.json`'s: a newer
minor is read, its unknown keys (also inside `fonts`) are kept and written back on save/export
with its minor version, so a theme passed through an older Annota loses nothing; an unknown major
(or a version that is not `MAJOR.MINOR`) is "unknown schema version"; no version at all is its own
reason. The file carries no id: themes are told apart by where they are (`annota`,
`monochrome`, `system/<stem>`, `user/<stem>`), so files from different people never clash.
Importing validates first (count before colours, so "It contains 11 colours" wins over a bad
colour), then copies into `~/.local/share/annota/styles/themes/<slug>.annota-theme`; a name
already in use gets " 2". Export writes the same JSON. Both schemas were validated with Python
`jsonschema` 4.19 (draft 2020-12) against files written by the code; unit tests keep the schemas'
keys in step with what is written.

**Managed themes.** `/usr/share/annota/themes/*.annota-theme` is read when the editor window is
built and whenever themes change (save, import, delete); invalid files are skipped with a message
on stderr. Managed themes are read-only (lock in the list and switcher, banner "Managed by your
administrator · read-only" with Duplicate, no Add tile, fonts disabled, no More menu); built-in
themes are read-only too but get no banner (it would be untrue) and keep the More menu with
Export and Duplicate (Delete hidden). The directory is not watched while the app runs.

**UI.** Blueprint for everything static (`editor-window.blp`: toggle group, Favourites view,
quick styles section, Save popover, rename popover, style menu; `theme-editor.blp`); tiles,
theme rows, palette swatches and Favourites groups are built in code (dynamic lists, the
precedent of the swatch buttons). Tiles are drawn by the shared renderer (`styles::tile`) — a
sample object of the tool in that style, on paper in light and dark (callouts show two text bars,
highlights and redactions lie on a small sample page). The window's capture-phase key handler
leaves arrows, Enter, Space, Delete, F2, F10 and Menu to a focused style grid
(`is_style_grid_key`), so Delete on a tile never deletes the selection. Toasts with Undo/Apply
use high priority (shown at once, not queued behind an older toast).

**Deviations from the mockups (judgement calls).**
- The sidebar heading reads "Properties" (mockups 01–05); the type's name moved into the toggle.
  Its right side shows "● Modified" when nothing matches, else add-core-annotation-tools' mode
  caption ("Tool defaults", "1 selected · 2 tails") — mockup 03 shows the applied style's name
  there, which is not shown.
- The theme dropdown shows the theme's first four colours; for Annota these are Coral, Amber,
  Lime, Teal. Mockup 01 shows Coral, Amber, Teal, Azure, mockup 06 lists the same theme as
  Coral, Amber, Lime, Teal, Azure; the swatch order of add-core-annotation-tools decided it, so
  theme 1–4 also equals the first four swatches.
- With nothing in "My Styles" the caption and the empty grid are hidden.
- "Add to Favourites" in the save popover starts checked (mockup 02) and keeps the last choice.
- Picking a favourite whose tool differs from the selection's type lets go of the selection, so
  the tool's defaults show and take the style (mockup 05); a selection of that type gets it as one
  undo step instead.
- The swatch colour chooser of the theme editor has the hex entry, Pick From Screen and Remove of
  the redline, plus "Custom Colour…" (GTK's `ColorDialog`) and Move Left/Right buttons (the
  keyboard path to drag-to-reorder; Ctrl+←/→ on a focused swatch works too). Phase B's picker
  popover is bound to the sidebar's property rows and was not reused.
- Fonts use GTK's system font chooser (`FontDialogButton`, family level), which looks like a
  button with the font name rather than a flat dropdown with a chevron.
- Delete in the style menu is coloured with CSS on the menu's last section (GTK menus have no
  destructive item style); tested by its rendered colour.
- The shortcuts window does not list the grid keys (they are contextual; the menu shows them).

**Verification.** Library: `styles::*` unit tests (value round trips and refusals, generator —
Annota slot 0 equals every default, every change accepted, eight distinct styles for 2/3/5/7
colours, brand colours on every tool, contrast —, library operations, undo of delete, file round
trip with newer-minor data, newer/broken files, theme parse/write/every import error, newer-minor
theme keys, schema drift, store: managed themes read-only, import/export/duplicate/delete, invalid
imports add nothing), `ops::a_quick_style_is_one_step_on_every_selected_object`, two goldens of
every tool's theme tiles (Annota and a brand theme; glyphs masked). Bin: `styles` (names,
descriptions, import error texts), `colour::a_theme_replaces_the_palette_swatches`,
`theme_editor` names. Live (one per process, Xvfb, `GDK_BACKEND=x11`, `WAYLAND_DISPLAY` unset,
a private session bus that activates nothing, an empty `XDG_CONFIG_HOME` so no user CSS):
`quick_styles_on_a_live_display` (scenarios Default style, Save style, Apply style to a
selection, Delete with undo, Reorder with the keyboard, Use favourite, No favourites, Apply
theme; keys not reaching the canvas; Delete's colour) and `theme_editor_on_a_live_display`
(Managed theme, Colour count, Round trip, the 11-colour alert, duplicate/rename/new/use). The
nine earlier live tests still pass. Screenshots (`ANNOTA_SHOTS`) were compared by eye with
mockups 01, 03, 04, 05, 06, 07, 08, 09, 10 (light) and 03 (dark); popovers (save, context menu, switcher, More)
are separate surfaces the widget snapshot does not capture and were checked structurally only.

**Not verified / open.**
- The theme editor's Pick From Screen calls the real `PickColor` portal (the same call as the
  sidebar's eyedropper) and was not run against a live portal; Import/Export use the real file
  chooser portal through `gtk::FileDialog` — the file handling after the chooser is tested, the
  chooser itself was not opened.
- Drag-to-reorder of palette swatches is wired (`DragSource`/`DropTarget`) but only its
  keyboard/button equivalent is tested; GTK cannot synthesise a drag in the tests.
- Of the dark renders only the sidebar (mockup 03-dark: paper tiles in a dark window) was
  compared; the theme editor in dark was not.
- Recent custom colours are still per window and not persisted (add-core-annotation-tools noted
  this change "may" persist them; it does not).

## Wayland / X11
- Behaviour is identical unless stated in a decision above.

## Risks / open questions
- To be completed during `/opsx:propose` refinement.
