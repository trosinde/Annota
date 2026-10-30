# Annota design kit

Static HTML/CSS building blocks that mirror **libadwaita 1.6** so mockups look like a real GNOME app.
Every mockup uses only `tokens.css`, `adwaita.css`, `icons.js` and the assets in this folder.

Showcase of every component in light and dark: `components.html` → `png/components-light@1x.png`,
`png/components-dark@1x.png`.

| File | Purpose |
|---|---|
| `tokens.css` | Colours (libadwaita named colours, light + dark), Annota palette, radii, spacing, type, shadows |
| `adwaita.css` | Component classes (see reference below) |
| `icons.js` | Inlines the icon sprite into the page. **Required for icons** (see "Icons") |
| `icons/*.svg`, `icons/sprite.svg` | 72 symbolic 16×16 icons (single files + sprite) |
| `app-icon/annota.svg`, `annota-symbolic.svg` | App icon (128 px grid) and symbolic variant |
| `sample-content/*.html` → `sample-content/png/*.png` | Invented app screens to annotate |
| `template.html` | Starter mockup: editor window skeleton |
| `tools/build-icons.py` | Source of truth for icons; regenerates `icons/`, `sprite.svg`, `icons.js` |
| `tools/render-samples.py` | Re-renders sample PNGs and the component showcase |

## 1. Build a mockup

1. Copy `template.html` to `docs/design/<change-id>/src/NN-slug.html`
   (`NN` = two-digit order, slug = kebab-case state, for example `03-export-dialog-error.html`).
2. Edit the content. Keep the boilerplate below unchanged.
3. Render: `python3 tools/render-mockups.py docs/design/<change-id>` → `png/NN-slug-{light,dark}@{1,2}x.png`.
4. Open the light and dark PNGs and fix anything that looks wrong before you hand them over.

### Required boilerplate

```html
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport-size" content="1280x800">        <!-- 700x800 for narrow -->
<title>Annota — <screen></title>
<link rel="stylesheet" href="../../kit/tokens.css">
<link rel="stylesheet" href="../../kit/adwaita.css">
</head>
<body class="mock-frame flush">                          <!-- flush: window fills viewport -->
  <div class="window" style="width:1280px;height:800px">
    <div class="headerbar">
      <div class="start">…</div>
      <div class="title-widget"><span class="title">Title</span><span class="subtitle">Subtitle</span></div>
      <div class="end">… <div class="window-controls"><span class="wc"><svg class="icon"><use href="#i-close"/></svg></span></div></div>
    </div>
    …content…
  </div>
  <script src="../../kit/icons.js"></script>             <!-- last element in body -->
</body>
</html>
```

- **Theme:** do not set `data-theme` yourself. The renderer sets `data-theme="dark"`/`"light"` on
  `<html>` and the matching `prefers-color-scheme`. Use `data-theme="light"` only for a sample
  that must stay light in both renders.
- **Floating window on a desktop** (dialogs over a window, screenshots for marketing): use `<body class="mock-frame">`
  without `flush`, and a window smaller than the viewport.
- **Fonts:** Inter stands in for Adwaita Sans (installed locally). Do not load web fonts; there is no network.

### Icons

Chromium **blocks external `<use href="sprite.svg#…">` on `file://` pages** (we tested this:
"file: URLs are treated as unique security origins"). So always:

```html
<svg class="icon"><use href="#i-arrow"/></svg>      <!-- 16 px, inherits text colour -->
<svg class="icon large"><use href="#i-arrow"/></svg> <!-- 32 px -->
```

…and include `<script src="../../kit/icons.js"></script>` at the end of `<body>`. The script inlines
the sprite and rewrites any `…/sprite.svg#i-x` hrefs to `#i-x`. It sets `data-icons-ready="1"` on
`<html>`, and the renderer waits for that.

Icon names: select, arrow, line, pen, shape, text, callout, step, highlighter, blur, crop, magnify,
spotlight, stamp, cut-out, fill, eraser, magic-wand, selection, record, stop, pause, play,
capture-region, capture-window, capture-screen, timer, camera, microphone, speaker, cursor,
library, template, image, folder, trash, tag, star, grid-view, list-view, undo, redo, zoom-in,
zoom-out, search, copy, share, export, save, open-menu, view-more, effects, ocr-text, redact,
settings, keyboard, plus, minus, close, check, warning, info, go-previous, go-next, pan-down,
sidebar-show, sidebar-show-right, eyedropper, delete-text, drag-handle, pin, edit-clear.
To add an icon, edit `tools/build-icons.py` and run it. Do not hand-edit `icons.js`.

### Sample content

Put captures on the canvas with
`<div class="canvas-image" style="width:…;height:…"><img src="../../kit/sample-content/png/<name>.png"></div>`
and draw annotations in an absolutely positioned `<svg class="annotations" viewBox="…">`.

| PNG | What | Good for |
|---|---|---|
| `a-harbor-backup` | Local backup app preferences (sidebar, boxed lists, switch, buttons) | Arrows, steps, callouts on UI |
| `b-lumen-checkout` | Web shop checkout with a form error | Highlight, spotlight, callout on the error |
| `c-code-editor` | Dark Rust editor with a warning and tooltip | Magnify, line, text on dark content |
| `d-tidewater-analytics` | Dashboard with KPIs, a chart and a table | Shapes, crop, step numbering |
| `e-relay-chat` | Chat with email addresses, phone numbers, IP address and booking reference | Redaction, blur, smart redact |
| `f-field-notes` | 1600×2600 article | Scrolling capture, long images |

All 1600×1000 except `f` (1600×2600). The names, brands and addresses are invented
(`.example` domains, 555 phone numbers).

## 2. Class reference

**Tokens** (`var(--…)`): `window-bg/fg`, `view-bg/fg`, `headerbar-bg`, `sidebar-bg`,
`secondary-sidebar-bg`, `card-bg`, `dialog-bg`, `popover-bg`, `borders`, `separator`, `fill-1..3`,
`fill-flat-hover/active`, `row-hover/selected`, `accent-bg`, `accent-fg`, `accent` (text),
`accent-a50` (focus ring), `destructive-bg`/`destructive`, `success-bg`/`success`,
`warning-bg`/`warning`, `dim-opacity` (.55), `dim-label`, `osd-bg`, `canvas-bg`.
Annota ink: `annota-coral` (default), `annota-amber`, `annota-lime`, `annota-teal`, `annota-azure`,
`annota-violet`, `annota-ink`, `annota-paper`, `annota-highlight`.
Radii: `radius-s` 6, `radius-m` 9, `radius-l` 12, `radius-xl` 18. Spacing: `space-1..4` = 6/12/18/24.
Sizes: `headerbar-h` 47, `control-h` 34, `row-h` 50, `sidebar-w` 280, `props-w` 300.
Type: `fs-caption` 12, `fs-small` 13, `fs-body` 15, `fs-title-4` 20, `fs-title-2` 26, `fs-title-1` 32.

| Area | Classes |
|---|---|
| Text | `.title-1` `.title-2` `.title-3` `.title-4` `.heading` `.body` `.caption` `.caption-heading` `.monospace` `.numeric` `.dim-label` `.accent` `.success` `.warning` `.error` `.link` |
| Frame | `.mock-frame` (`.flush`), `.window` (`.maximized`), `.backdrop-dim` |
| Header bar | `.headerbar` (`.flat`) › `.start` `.title-widget` (`.title` `.subtitle`) `.end`; `.window-controls` › `.wc` |
| Buttons | `.btn` + `.text-button` / `.image-button`, `.flat`, `.suggested-action`, `.destructive-action`, `.pill`, `.circular`, `.osd`, `.small`, `.opaque`; states `.hover` `.pressed` `.active` `.focus` `[disabled]` |
| Groups | `.linked` (`.vertical`), `.toggle-group` (`.flat`) › `.toggle` (`.active`, `.icon-only`) |
| Inputs | `.dropdown` (`.flat`), `.entry` (`.focus` `.error` `.success`, `.placeholder`, `.caret`), `.search-entry`, `.spin` › `.value` + 2 `.btn`, `.switch` (`.on` `.disabled` `.focus`), `.check` / `.radio` (`.checked`), `.check-label`, `.scale` › `.trough` › `.fill` `.knob` `.mark` + `.value` |
| Colour | `.swatch-row` › `.swatch` (`style="--c:…"`, `.selected` `.focus` `.square` `.small` `.none` `.custom` `.light-ink`) |
| Lists | `.boxed-list` › `.row` (`.property` `.entry-row` `.button-row` `.suggested`/`.destructive` `.selected` `.hover` `.focus` `.disabled`) › `.prefix` `.text` (`.title` `.subtitle`) `.suffix`; `.preferences-page`, `.preferences-group` › `.group-header` › `.text` (`.heading` `.description`) |
| Layout | `.split-view` › `.sidebar-pane` (`.end`, `.props`) + `.content-pane` (`.view`); `.sidebar` › `.sidebar-list` › `.nav-row` (`.selected` `.hover` `.focus`, `.label` `.count`), `.section-title`, `.separator` |
| Surfaces | `.view`, `.card`, `.separator-h`, `.separator-v` |
| Feedback | `.toast-overlay` › `.toast` (`.toast-title`), `.banner` (`.warning` `.error`), `.progress` › `.bar`, `.spinner` (`.large`), `.badge` (`.neutral` `.coral` `.success` `.warning` `.destructive` `.dot`), `.tag-chip` |
| Overlays | `.popover` (`--arrow-x`/`--arrow-y`, `.arrow-down` `.arrow-left` `.arrow-right` `.no-arrow` `.static`) › `.menu-item` (`.label` `.accel`), `.menu-sep`, `.menu-section-title`; `.tooltip` (`.static`) › `.shortcut`; `kbd.key` |
| Pages and dialogs | `.status-page` (`.compact`) › `.status-icon` `.status-title` `.status-description`; `.dialog-host` › `.dialog` (`.alert` › `.alert-body` `.alert-heading` `.alert-actions`; else `.headerbar` + `.dialog-body`); `.bottom-sheet` |
| Tabs | `.tabs` › `.tab` (`.selected`) › `.tab-title` `.tab-close` |
| Editor | `.canvas-area` › `.canvas-image` (checker) › `img` + `svg.annotations`; `.sel-box` › `.h`; `.osd-toolbar` (`.pill`) › `.btn` `.sep` `.label` `.rec-dot`; `.checker` |
| Utilities | `.hbox` / `.vbox` (`--gap`), `.grow`, `.spacer`, `.ellipsize`, `.pad-1..4`, `.focus-ring` |

## 3. Do / don't

**Do**
- Use the 6 px rhythm: gaps and paddings of 6/12/18/24.
- Use header bar actions in this order: start = navigation and undo/redo, end = primary action
  (`suggested-action`), then the main menu (`open-menu`), then the close button.
- Put **one** `suggested-action` per view. Destructive actions confirm in an `.alert` dialog.
- Put tool properties in the right `.sidebar-pane.end.props`. Keep the canvas free of floating panels
  (the zoom `.osd-toolbar` is the only exception).
- Show keyboard shortcuts in tooltips (`.tooltip .shortcut`) and menus (`.accel`).
- Draw every state: default, hover, `.focus` (visible ring), selected, disabled, empty
  (`.status-page`), loading (`.spinner`/`.progress`), error (`.entry.error`, `.banner.error`), narrow, dark.
- Use Annota Coral for annotation defaults only. UI accent stays Adwaita blue.
- Use title-case labels on buttons and in menus ("Save a Copy", "Keyboard Shortcuts").

**Don't**
- Hard-code colours. If a token is missing, add it to `tokens.css` (light **and** both dark blocks).
- Use raw hex for UI chrome, borders thicker than 1 px, or drop shadows on buttons.
- Show accounts, sign-in, avatars of the user, cloud sync or team servers. Annota is local-only.
- Use real brands, logos, people or lorem ipsum.
- Copy icons, stamps, layouts or wording from Snagit or any other product.
- Load external fonts or scripts.

## 4. Known limitations

- Inter replaces Adwaita Sans. Metrics are close, but text runs ~1–2 % wider.
- The CSD window shadow is approximate. Backdrop (unfocused) window states are not styled.
- Swatch checkmarks are always white. Use `.light-ink` for light colours (paper, amber) to get a dark check.
- `.dropdown`, `.check` and the checkmark in `.swatch` use CSS masks, so they do not need icons.js.
