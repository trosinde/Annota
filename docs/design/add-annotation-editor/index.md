# add-annotation-editor — mockups

Canonical editor window. All screens 1280×800 unless noted; light + dark at 1× and 2× in `png/`
(`<name>-{light,dark}@{1,2}x.png`). HTML sources in `src/` are generated from one shared shell so every
screen has identical chrome; edit the shell first, then the screen.

Sample captures: `kit/sample-content/png/` (b-lumen-checkout, d-tidewater-analytics, e-relay-chat).

## Scenario → mockup

| Spec | Scenario | Mockup | Notes |
|---|---|---|---|
| editor-canvas · Editor layout | (layout, wide) | [01-editor-overview](png/01-editor-overview-light@1x.png) | Nothing selected → sidebar shows **document** properties (Image, Canvas, Objects). |
| editor-canvas · Editor layout | Narrow window | [13-narrow-bottom-sheet](png/13-narrow-bottom-sheet-light@1x.png) (700×800) | < 900 px: sidebar becomes a bottom sheet over the canvas; tray hidden; Copy button moves into main menu. |
| editor-canvas · Object selection | Keyboard nudge | [03-rotate-shift-snap](png/03-rotate-shift-snap-light@1x.png) | Nudge rule shown in the sidebar note; X/Y fields update by 10 on Shift+→. No separate visual state on canvas. |
| editor-canvas · Object selection | Rotate snapping | [03-rotate-shift-snap](png/03-rotate-shift-snap-light@1x.png) | Rotation handle + live angle chip “−15° · Shift: 15° steps”. |
| editor-canvas · Object selection | (8 handles, rubber-band / multi) | [02](png/02-move-snap-centre-light@1x.png), [05](png/05-context-menu-light@1x.png) | 8 square handles + round rotation handle. |
| editor-canvas · Select all of a type | Recolour all arrows | [04-select-all-arrows](png/04-select-all-arrows-light@1x.png) | Each arrow has a thin outline, the union box carries handles. Toast confirms with Undo. |
| editor-canvas · Z-order and grouping | Group moves together | [05-context-menu](png/05-context-menu-light@1x.png), [06-group-and-undo](png/06-group-and-undo-light@1x.png) | Group = **dashed** selection box. Ghost shows old position. |
| editor-canvas · Snapping and guides | Centre snap | [02-move-snap-centre](png/02-move-snap-centre-light@1x.png) | Guides in Annota Violet 1 px; chip names the target (“Canvas centre”). |
| editor-canvas · View control | Fit | [01](png/01-editor-overview-light@1x.png), [07-zoom-levels](png/07-zoom-levels-light@1x.png) | Zoom OSD label “Fit · 50 %” when fitted, plain “200 %” otherwise. |
| editor-canvas · Undo and redo | Undo move | [06-group-and-undo](png/06-group-and-undo-light@1x.png) | Undo tooltip names the action: “Undo Move Group  Ctrl Z”. Redo disabled when stack empty. |
| editor-canvas · Image operations | Crop | [08-crop-mode](png/08-crop-mode-light@1x.png), [09-cropped-resize-dialog](png/09-cropped-resize-dialog-light@1x.png) | Crop is a mode (tool C), not a dialog; 09 shows the cropped result, rounded-corner handle and Resize dialog. |
| editor-canvas · Image operations | (rotate/flip) | [01](png/01-editor-overview-light@1x.png), [17-main-menu](png/17-main-menu-light@1x.png) | Linked buttons in sidebar + menu items. |
| editor-canvas · Canvas extension | Extend for callout | [10-canvas-extension-transparent](png/10-canvas-extension-transparent-light@1x.png) | Transparent canvas = checkerboard; dashed line marks original image edge. |
| editor-canvas · Flatten | Flatten all | [05](png/05-context-menu-light@1x.png), [17](png/17-main-menu-light@1x.png), [18-flatten-all-result](png/18-flatten-all-result-light@1x.png) | No confirmation dialog (undoable) — toast with Undo instead. |
| editor-canvas · Recent captures tray | Picture in picture | [11-tray-drag-to-canvas](png/11-tray-drag-to-canvas-light@1x.png), [12-inserted-image-object](png/12-inserted-image-object-light@1x.png) | Drop target highlight on canvas area; inserted image is a normal object (border, shadow, arrange). |
| document-format · Editable file | Round trip | [16-many-objects](png/16-many-objects-light@1x.png) | No UI of its own; 16 shows a dense document (64 objects) and the per-type object summary. |
| document-format · Format versions | Newer major | [15-newer-format-error](png/15-newer-format-error-light@1x.png) | Alert names file, file format, required Annota version, current version. |
| document-format · Autosave and recovery | Crash recovery | [14-crash-recovery](png/14-crash-recovery-light@1x.png) | Shown at start before any window content; one row per autosaved document, all checked by default. |

## Redlines

### Window and shell
- Window 1280×800 default, min 360 wide. Header bar 47 px (kit `--headerbar-h`).
- Header start: **New Capture** (`capture-region`, image-button) · linked **Undo** (`undo`, Ctrl+Z) / **Redo** (`redo`, Ctrl+Shift+Z).
- Title = document name, subtitle = `W × H · Edited` (or `· 200 %` while not fitted, `Crop · W × H` in crop mode).
- Header end: **Show Properties** toggle (`sidebar-show-right`, F9, flat, `.active` when shown) · **Copy** (`copy`, Ctrl+C) · **Export** (suggested, Ctrl+E) · main menu (`open-menu`) · close.
- Narrow (< 900 px): Copy moves into the main menu; sidebar toggle opens/closes the bottom sheet.
- Tool strip 52 px wide, buttons 40×40 flat, gap 4, separators 28 px. Order and shortcuts:
  Select V | Arrow A, Line L, Pen P, Shape U, Text T, Callout K, Step S | Highlighter H, Redact B, Magnify M,
  Spotlight G, Stamp I | Crop C, More Tools (`view-more`: Cut-out X, Fill F, Eraser E, Magic wand W, Selection Q).
  Phase-2 tools appear only when their change ships.
- Properties sidebar 300 px (`.sidebar-pane.end.props`), padding 18/12, groups 18 apart.
- Tray 107 px: label row 15 px (“Open”, “Recent”, caption-heading dim), thumbnails 100×62 radius 6, gap 6.
  Current doc: 2 px accent ring with 2 px window-bg gap. Unsaved: 8 px accent dot top-right. Right end:
  **Open Library** (`library`, Ctrl+L) and **Hide Tray** (`pan-down`). Right edge fades 48 px.

### Canvas
- Canvas background `--canvas-bg`; image shadow per kit `.canvas-image`. Image centred.
- Zoom OSD bottom-right, 18 px inset: zoom-out · level button (min 78 px) · zoom-in. Level button opens the
  zoom popover (07): Fit to Window Ctrl+0, Fit Width, 25/50/100 (Ctrl+1)/200/400/800/1600 %, Zoom In Ctrl++, Zoom Out Ctrl+−.
  Range 10–1600 %. Scrollbars are overlay (6 px) only when content exceeds the view.
- Selection: 1 px `--accent-bg` box, 8 square handles 10×10 (white, 1.5 px accent border, radius 2),
  rotation handle 14 px circle 24 px above top-centre on a 1 px stem. Multi-selection: thin box per object (accent 50 %) +
  union box with handles. Group: same but **dashed** 1 px.
- Snap guides: 1 px `--annota-violet`, full canvas length, snap distance 6 screen px. Chip names target.
- Measurement chips: `--osd-bg-solid`, 12 px bold, radius 6, padding 3/8, 10 px below/right of the cursor.
- Crop mode: outside dimmed `rgb(0 0 6 / 55%)`, 2 px white frame, rule-of-thirds 1 px white 60 %,
  L-shaped corner grips 22×6 and edge grips 22×6, size chip under frame. Sidebar: X/Y/W/H entries,
  Aspect ratio dropdown (Free, Original, 1:1, 4:3, 16:9), **Cancel** (Esc) / **Crop** (Enter, suggested).
- Rounded corners: single filled handle at the top-left inner corner, offset = radius; chip “Corner radius N px”.
- Canvas extension: grabber 12×36 radius 6 at the edge midpoint; dashed 1 px accent line at the original image edge;
  chip “+N px”.
- Drag from tray: canvas area gets an inset 3 px `--accent-a50` ring, drag image 240×150 with 2 px accent ring;
  source thumbnail at 45 % opacity; chip “Drop to insert image”.

### Sidebar contents
- **No selection** (Document): groups Image (Size + Resize…, Rotate & Flip linked buttons: Rotate Left Ctrl+Shift+R,
  Rotate Right Ctrl+R, Flip Horizontally, Flip Vertically; Corner radius spin), Canvas (Background swatches:
  7 palette + Transparent checker + Paper + Custom + Eyedropper; hint text), Objects (Snap to guides switch with
  subtitle “Ctrl+; · hold Alt to suspend”; “N editable objects” + Flatten All).
- **Selection**: heading = type (“Rectangle”, “5 Arrows”, “Group”, “3 Objects”) with dim caption right
  (“1 selected”, “Shared properties”). Type properties (see add-core-annotation-tools), then **Arrange**
  (X, Y, W, H entries in a 2-column grid, Rotation spin in °), then **Select All <Type>** full-width button.
  Mixed values show “Mixed” and the half-split swatch.
- **Group**: member list (boxed list, icon + name), **Ungroup** button with Ctrl+Shift+G hint.
- **Many objects** (16): when > 40 objects and nothing selected, the document page adds a per-type summary list
  (icon, name, count badge, flat “Select” button).

### Context menu (right-click on selection, also Shift+F10 / Menu key)
Copy Ctrl+C · Duplicate Ctrl+D · Delete Del | Bring to Front Ctrl+Shift+] · Bring Forward Ctrl+] · Send Backward Ctrl+[ ·
Send to Back Ctrl+Shift+[ | Group Ctrl+G · Ungroup Ctrl+Shift+G (disabled unless a group is selected) |
Flatten Selection Ctrl+T · Flatten All Ctrl+Shift+T. Opens at the pointer, no arrow.

### Main menu
New Capture Ctrl+N · Open… Ctrl+O · Open Library Ctrl+L | Save Ctrl+S · Save As… Ctrl+Shift+S · Export… Ctrl+E |
Resize Image… · Rotate Right Ctrl+R · Rotate Left Ctrl+Shift+R · Flip Horizontally · Flip Vertically · Flatten All Ctrl+Shift+T |
Preferences Ctrl+, · Keyboard Shortcuts Ctrl+? · About Annota.

### Dialogs, toasts
- Resize Image (AdwDialog 420 px): Cancel | “Resize Image” | Resize (suggested). Width/Height spins, Keep aspect ratio
  switch (on), Unit toggle px/%, separate list: Scale annotations switch (on) with subtitle.
- Restore Unsaved Captures? (460 px): body text, boxed list rows (check, 64×40 thumbnail, name, “Autosaved today at HH:MM · N objects”),
  buttons **Discard** (destructive text) / **Restore** (suggested). Autosave every 30 s to `~/.cache/annota/autosave/`.
- Newer Annota Version Needed (alert 420 px): “<file> uses document format X.0. It was saved with Annota A.B or newer.”
  + “This is Annota 0.1, which opens format 1.x. Install Annota A.B or newer to open this file. The file was not changed.”
  Buttons: Show in Files / OK (suggested). Newer *minor* versions open silently (no UI).
- Toasts (bottom, above tray, centred on canvas): “Colour changed on 5 arrows”, “Image inserted”,
  “7 objects flattened into the image”, each with **Undo**.

### Kit additions needed (improvised as page-local glyphs in these mockups)
`rotate-left`, `rotate-right`, `flip-horizontal`, `flip-vertical`, `transparent` — to be added to `kit/tools/build-icons.py`.
Cursors (move, rotate, crosshair, grab-with-plus) are drawn inline; the app uses the system cursor theme.
