# add-advanced-annotation-tools — mockups

Window 1280 × 800, editor shell from `docs/design/kit/template.html`. Every screen has
`png/<name>-{light,dark}@{1,2}x.png`. Sources: `src/*.html`. Derived canvas images (auto-fill,
transparency, flood fill, colour replace, cut-out result, wand mask) are in `assets/`, all
generated from the kit sample PNGs. Original stamp art: `stamps/*.svg` (41 files, CC-BY-SA-4.0).

## Scenario → mockup

| Requirement · Scenario | Mockup | Notes |
|---|---|---|
| Magnifier · Offset lens | `01-magnifier-offset-lens` | Dashed source circle on the code warning, solid lens at 300 %, straight connector between the two outlines. Handles on the lens (resize) and on the source (radius). |
| Spotlight · Two spots | `02-spotlight-two-spots` | Two rounded areas (Visitors KPI, Top routes) stay clear, the rest is dimmed 60 %. Area 2 selected in canvas and in the "Clear Areas" list. |
| Stamps · picker (browse) | `03-stamp-picker` | Search, category chips, grouped grids. Placed stamps on canvas: check, cross, pointing hand (selected). |
| Stamps · search | `04-stamp-search-keys` | Search "key" filters to Keys (badge 12); keyboard focus ring on "Ctrl", selection on "C". |
| Stamps · search, no result | `05-stamp-search-empty` | Status page with "Import Stamp…" fallback. |
| Stamps · Custom stamp (import) | `06-stamp-import-dialog` | My Stamps empty state behind; import dialog previews company-logo.png on a checkerboard; name, category, white-background removal. |
| Stamps · Custom stamp (placed) | `07-my-stamps-placed` | Logo appears under My Stamps (badge 1) and is placed on the canvas; toast confirms. |
| Stamps · art reference | `20-stamp-set-sheet` | Full original set: Status (8), Numbers (10), Pointers (6), Faces (5), Keys (12). |
| Cut-out · Remove middle (preview) | `08-cut-out-preview` | Horizontal strip y 1240–1690 tinted coral, zigzag seam preview on both cut lines, "− 450 px" label, Cancel / Cut Out footer. |
| Cut-out · Remove middle (result) | `09-cut-out-applied` | Image 2600 → 2150 px tall with a zigzag seam; toast with Undo. |
| Selection · Auto-fill (before) | `10-selection-before-delete` | Marching-ants rectangle around "Start backup"; Fill Vacated Area = Auto-Fill. |
| Selection · Auto-fill (after) | `11-selection-auto-filled` | Button removed, hole filled with the dialog background; toast with Undo. |
| Selection · transparent fill + move | `12-selection-moved-transparent` | Same selection moved left with Fill = Transparent: checkerboard hole stays at the old place. |
| Fill · tolerance (click) | `13-fill-tolerance` | Bookings KPI card flood-filled amber at tolerance 12; text and icons stay intact. |
| Fill · Global replace | `14-fill-global-replace` | Shift-click: every #1B7FA8 pixel (chart line, legend, route bars, device bar) becomes coral; "Last Replace" summary. |
| Eraser and magic wand · eraser | `15-eraser` | Round brush 44 px with outline cursor erasing "Storage" to the canvas colour. |
| Eraser and magic wand · Wand | `16-magic-wand` | Click in the sidebar: the uniform sidebar area is selected (marching ants + 22 % accent tint); the darker "Backup" row is excluded. |
| Text-only mode · Highlight text only | `17-text-only-highlight` | Highlighter drawn over the Locations card; only the six text lines are highlighted, folder/chevron icons are not. |
| Text-only mode · detecting | `18-text-only-detecting` | Redact (blur) with text-only on: spinner + "Detecting text…" while OCR runs. |
| Text-only mode · unavailable | `19-text-only-unavailable` | Spotlight with tesseract missing: row disabled, warning banner explains the package. |

## Redlines

**Tool strip** (52 px wide, buttons 36 × 34, gap 2, separators 28 px): Select V, Selection Q,
Magic Wand W | Arrow A, Line L, Pen P, Shape U, Text T, Callout K, Step S, Highlighter H |
Redact B, Magnify M, Spotlight G, Stamp I | Crop C, Cut-Out X, Fill F, Eraser E. Tooltip =
name + `.shortcut` key, 4 px right of the strip. Icons: kit `select, selection, magic-wand,
arrow, line, pen, shape, text, callout, step, highlighter, redact, magnify, spotlight, stamp,
crop, cut-out, fill, eraser`.

**Properties sidebar** 300 px, padding 18/12, 18 px between groups. First line is always the
tool header: 16 px icon + bold name. Segmented choices use a full-width `.toggle-group`
(13 px labels; 12 px when there are four options). Sliders: label column 84 px, value
right-aligned with tabular figures. Hints: 12 px, dim. Confirmation tools (Cut-Out) put
Cancel / primary action in a footer bar (12 px padding, top separator). Esc = Cancel,
Enter = primary.

**Magnifier.** Lens shape Circle | Rounded. Zoom 125–800 % (slider marks at 125, 200, 300,
800); scroll over the lens changes zoom. Outline: 7 Annota colours + custom, default Coral,
width 4, shadow on (soft, 25 % black, 6 px). Connector: 2.5 px, same colour as the outline, from
source edge to lens edge along the centre line. Source outline 2 px, dash 5/4 (Dashed |
Solid | None). Handles: 10 px white circles with 1.5 px accent border.

**Spotlight.** Shape Rectangle | Rounded (radius 10 px at 100 %) | Ellipse | Polygon. Dim
0–90 % (default 60), Blur 0–30 px (default 0). Clear areas get a 1.5 px white outline at 90 %.
"Clear Areas" boxed list: one row per area with a trash button; Shift-drag adds an area.

**Stamps.** Search entry (placeholder "Search stamps"), chips All · Status · Numbers ·
Pointers · Faces · Keys · My Stamps (28 px, pill, active = accent; count badge while a search
runs). Grid: 5 columns, 48 px cells, 6 px gap, stamp drawn at 36 px; selected = 2 px accent
inset + 12 % accent fill; keyboard focus = standard focus ring. Enter places the focused stamp
at the canvas centre. Default size 48 px, slider for size. My Stamps grid ends with a dashed "+"
cell (Import Stamp…). Context menu on a stamp: Rename, Move to Category, Remove.
Import dialog: 560 px, preview 120 px on checkerboard, rows Name (entry), Category
(dropdown), Remove White Background (switch, off). Stamps are copied to
`~/.local/share/annota/stamps`. "Save Selection as Stamp" is disabled without a pixel selection
(tooltip names the Selection tool Q).

**Stamp art** (`stamps/*.svg`, 48 × 48 viewBox, ship in `data/stamps/`): circles r 20; strokes
4.5 px white for glyphs; pointers use 2.4 px #1F1F24 outlines on white with a coral cuff;
faces #FFC93C with #D99A00 1.5 px rim; keys are light keycaps with a 2 px bottom lip and
Inter Bold labels. Number badges use Annota Coral (circle) and Azure (square).

**Cut-out.** Direction Horizontal | Vertical. Seam Straight | Zigzag | Wave | Torn (toggles with
40 × 16 seam glyph + caption). Seam size 2–40 px (default 10). Preview: strip tinted
coral 18 %, cut lines drawn in the chosen seam style, 2 px coral; size badge in accent
("− 450 px"). The info list shows the strip range and the result size. Undo toast after cut.

**Selection.** Mode Rectangle | Ellipse | Polygon | Lasso. Fill Vacated Area: Auto-Fill
(default) | Transparent. Marching ants: 1 px white + 1 px #1F1F24 dashed 4/4. Actions Delete,
Crop to Selection (Ctrl+Shift+X), Save as Stamp. Shift adds, Alt subtracts. After a move the
old position is outlined 1 px accent dashed 2/3 until deselected.

**Fill.** Colour swatches (default Coral), Tolerance 0–100 (default 12), Anti-Aliased Edges on.
Click = contiguous fill; Shift+click = global replace. "Last Replace" group appears after a
global replace: from/to chips with hex, pixel count, toast "Replaced #RRGGBB everywhere"
+ Undo.

**Eraser.** Brush Round | Square, Size 2–200 px (default 44, `[` / `]` change), brush outline
cursor 1.5 px dark + 1 px white halo. Row "Canvas Colour" shows the colour erased pixels get.

**Magic wand.** Tolerance 0–100 (default 8), Contiguous on, Smooth Edges on. Selection tint
accent 22 %, marching-ants border. Actions Delete, Invert.

**Apply to Text Only** (Highlighter, Redact, Spotlight): boxed row with switch; subtitle
"Restricts the effect to detected text lines". While detecting: spinner before the switch,
subtitle "Detecting text…". Without tesseract: row disabled, subtitle "Text recognition is not
installed", warning banner below. Highlighter colour default here Amber, blend Multiply.

## Open questions

1. Eraser "canvas colour": the spec does not say where it is set. The mockup shows it as a
   read-only row; proposal: Image ▸ Canvas Colour in the main menu, default = sampled
   background.
2. Fill uses Amber in `13` to read as a fill; the global default should stay Coral (as in `14`).
3. Wand vs. Selection share one selection model — confirm that the Selection sidebar actions
   (Delete, Crop to Selection, Save as Stamp) also appear for wand selections.
