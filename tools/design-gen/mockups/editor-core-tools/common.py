"""Shared building blocks for Annota editor + tool mockups (generator, not shipped)."""
import math

KIT = "../../kit"
SAMPLES = f"{KIT}/sample-content/png"

# ------------------------------------------------------------------ geometry
# Canvas: window 1280x800, header 47, tool strip 52, props 300, tray 107.
# canvas-area = x 52..980 (928) , y 47..693 (646). Image 1600x1000 at 50 % = 800x500,
# centred -> screen origin (116, 120).
IMG_X, IMG_Y, ZOOM = 116, 120, 0.5


def sx(x):
    return IMG_X + x * ZOOM


def sy(y):
    return IMG_Y + y * ZOOM


# ------------------------------------------------------------------ page-local glyphs
# Symbolic glyphs the kit does not have yet (listed in index.md as kit additions).
GLYPHS = """
<svg xmlns="http://www.w3.org/2000/svg" style="display:none">
 <symbol id="x-rotate-left" viewBox="0 0 16 16"><path fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" d="M4 6.5A5 5 0 1 1 3.4 10"/><path fill="currentColor" d="M1.5 3.5v4.5H6z"/></symbol>
 <symbol id="x-rotate-right" viewBox="0 0 16 16"><path fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" d="M12 6.5A5 5 0 1 0 12.6 10"/><path fill="currentColor" d="M14.5 3.5v4.5H10z"/></symbol>
 <symbol id="x-flip-h" viewBox="0 0 16 16"><path fill="currentColor" d="M7.2 1h1.6v14H7.2z"/><path fill="currentColor" d="M6 3.5v9L1 12.5z"/><path fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round" d="M10.7 4.6v7.2l4-.1z"/></symbol>
 <symbol id="x-flip-v" viewBox="0 0 16 16"><path fill="currentColor" d="M1 7.2h14v1.6H1z"/><path fill="currentColor" d="M3.5 6h9L12.5 1z"/><path fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round" d="M4.6 10.7h7.2l-.1 4z"/></symbol>
 <symbol id="x-rect" viewBox="0 0 16 16"><rect x="2" y="3.5" width="12" height="9" fill="none" stroke="currentColor" stroke-width="1.6"/></symbol>
 <symbol id="x-rounded" viewBox="0 0 16 16"><rect x="2" y="3.5" width="12" height="9" rx="3" fill="none" stroke="currentColor" stroke-width="1.6"/></symbol>
 <symbol id="x-ellipse" viewBox="0 0 16 16"><ellipse cx="8" cy="8" rx="6.2" ry="4.8" fill="none" stroke="currentColor" stroke-width="1.6"/></symbol>
 <symbol id="x-polygon" viewBox="0 0 16 16"><path d="M8 2l6 4.4-2.3 7.1H4.3L2 6.4z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/></symbol>
 <symbol id="x-cloud" viewBox="0 0 16 16"><path d="M4.5 12.5a3 3 0 0 1-.4-6 4 4 0 0 1 7.6-.6 3.3 3.3 0 0 1 .3 6.6z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/></symbol>
 <symbol id="x-solid" viewBox="0 0 16 16"><path d="M1.5 8h13" stroke="currentColor" stroke-width="2"/></symbol>
 <symbol id="x-dashed" viewBox="0 0 16 16"><path d="M1 8h14" stroke="currentColor" stroke-width="2" stroke-dasharray="4 2.5"/></symbol>
 <symbol id="x-dotted" viewBox="0 0 16 16"><path d="M2 8h13" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-dasharray="0 3.6"/></symbol>
 <symbol id="x-align-left" viewBox="0 0 16 16"><path fill="currentColor" d="M1 2h14v1.6H1zM1 6h9v1.6H1zM1 10h14v1.6H1zM1 14h9v1.6H1z"/></symbol>
 <symbol id="x-align-center" viewBox="0 0 16 16"><path fill="currentColor" d="M1 2h14v1.6H1zM3.5 6h9v1.6h-9zM1 10h14v1.6H1zM3.5 14h9v1.6h-9z"/></symbol>
 <symbol id="x-align-right" viewBox="0 0 16 16"><path fill="currentColor" d="M1 2h14v1.6H1zM6 6h9v1.6H6zM1 10h14v1.6H1zM6 14h9v1.6H6z"/></symbol>
 <symbol id="x-group" viewBox="0 0 16 16"><rect x="1.5" y="1.5" width="8" height="7" rx="1" fill="none" stroke="currentColor" stroke-width="1.4"/><rect x="6.5" y="7.5" width="8" height="7" rx="1" fill="currentColor"/></symbol>
 <symbol id="x-transparent" viewBox="0 0 16 16"><path fill="currentColor" d="M2 2h4v4H2zM10 2h4v4h-4zM6 6h4v4H6zM2 10h4v4H2zM10 10h4v4h-4z" opacity=".9"/></symbol>
</svg>"""


def icon(name, cls="icon"):
    return f'<svg class="{cls}"><use href="#i-{name}"/></svg>'


def glyph(name, cls="icon"):
    return f'<svg class="{cls}"><use href="#x-{name}"/></svg>'


# ------------------------------------------------------------------ page CSS (layout only)
PAGE_CSS = """
  .tool-strip { width: 52px; flex: none; display: flex; flex-direction: column; align-items: center; gap: 4px; padding: 6px 0; background: var(--window-bg); box-shadow: inset -1px 0 var(--separator); }
  .tool-strip .btn { width: 40px; min-height: 40px; }
  .tool-strip .separator-h { width: 28px; margin: 2px 0; }
  .props .scroll { padding: 18px 12px; display: flex; flex-direction: column; gap: 18px; }
  .editor-col { flex: 1; min-width: 0; display: flex; flex-direction: column; position: relative; }
  .editor-col > .canvas-area { flex: 1; }
  .canvas-area.top { align-items: flex-start; }
  /* recent-captures tray */
  .tray { flex: none; height: 107px; display: flex; align-items: flex-start; gap: 12px; padding: 9px 12px 12px; background: var(--window-bg); box-shadow: inset 0 1px var(--separator); position: relative; }
  .tray .tgroup { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .tray .tgroup.grow { flex: 1; overflow: hidden; }
  .tray .tlabel { font-size: var(--fs-caption); font-weight: var(--fw-bold); opacity: var(--dim-opacity); height: 15px; line-height: 15px; display: flex; gap: 6px; align-items: center; }
  .tray .thumbs { display: flex; gap: 6px; }
  .tray .thumb { width: 100px; height: 62px; flex: none; border-radius: var(--radius-s); overflow: hidden; position: relative; box-shadow: 0 0 0 1px var(--borders); background: var(--view-bg); }
  .tray .thumb img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .tray .thumb.current { box-shadow: 0 0 0 2px var(--window-bg), 0 0 0 4px var(--accent-bg); }
  .tray .thumb.hover { box-shadow: 0 0 0 1px var(--borders-strong), 0 2px 6px rgb(0 0 6 / 18%); }
  .tray .thumb.dragging { opacity: .45; }
  .tray .thumb .unsaved { position: absolute; top: 4px; right: 4px; width: 8px; height: 8px; border-radius: 50%; background: var(--accent-bg); box-shadow: 0 0 0 2px var(--view-bg); }
  .tray .tend { display: flex; flex-direction: column; gap: 6px; align-self: center; }
  .tray .fade { position: absolute; right: 64px; top: 0; bottom: 0; width: 48px; background: linear-gradient(to right, transparent, var(--window-bg)); pointer-events: none; }
  /* sidebar helpers */
  .props .group-title { display: flex; align-items: center; gap: 6px; padding: 0 2px; }
  .props .group-title .heading { flex: 1; }
  .props .lbl { font-size: var(--fs-small); opacity: var(--dim-opacity); padding: 0 2px; }
  .props .field { display: flex; flex-direction: column; gap: 6px; }
  .props .row-line { display: flex; align-items: center; gap: 12px; min-height: 34px; }
  .props .row-line > .name { flex: 1; font-size: var(--fs-body); }
  .props .grid2 { display: grid; grid-template-columns: 1fr 1fr; gap: 6px 12px; }
  .props .grid2 .cell { display: flex; align-items: center; gap: 6px; }
  .props .grid2 .cell .k { width: 16px; font-size: var(--fs-small); opacity: var(--dim-opacity); text-align: center; }
  .props .grid2 .entry { flex: 1; min-width: 0; font-variant-numeric: tabular-nums; }
  .props .toggle-group { width: 100%; }
  .props .toggle-group > .toggle { flex: 1; }
  .props .hint { font-size: var(--fs-small); opacity: var(--dim-opacity); line-height: 1.35; padding: 0 2px; }
  .props .card-note { display: flex; gap: 10px; align-items: flex-start; padding: 10px 12px; border-radius: var(--radius-l); background: var(--accent-a12); font-size: var(--fs-small); line-height: 1.35; }
  .props .card-note.warn { background: rgb(229 165 10 / 18%); }
  .props .card-note .icon { margin-top: 1px; }
  .props .boxed-list .row { min-height: 46px; padding: 6px 12px; }
  .props .boxed-list .row .title { font-size: var(--fs-body); }
  .props .full { width: 100%; }
  .swatch.eyedrop { background: var(--fill-1); box-shadow: none; color: inherit; }
  .swatch.checker { background-color: var(--canvas-checker-a); background-image: linear-gradient(45deg, var(--canvas-checker-b) 25%, transparent 25%, transparent 75%, var(--canvas-checker-b) 75%), linear-gradient(45deg, var(--canvas-checker-b) 25%, transparent 25%, transparent 75%, var(--canvas-checker-b) 75%); background-size: 10px 10px; background-position: 0 0, 5px 5px; }
  .swatch.none.selected { box-shadow: inset 0 0 0 1px var(--borders-strong), 0 0 0 2px var(--window-bg), 0 0 0 4px var(--accent-bg); }
  .swatch.mixed { background: conic-gradient(var(--annota-coral) 0 50%, var(--annota-azure) 50% 100%); }
  /* canvas overlays */
  .ov { position: absolute; pointer-events: none; }
  .selx { position: absolute; outline: 1px solid var(--accent-bg); }
  .selx.dashed { outline: 1px dashed var(--accent-bg); }
  .selx.thin { outline-color: var(--accent-a50); }
  .hd { position: absolute; width: 10px; height: 10px; margin: -5px 0 0 -5px; background: #ffffff; border: 1.5px solid var(--accent-bg); border-radius: 50%; box-sizing: border-box; }
  .hd.sq { border-radius: 2px; }
  .hd.big { width: 12px; height: 12px; margin: -6px 0 0 -6px; }
  .hd.curve { width: 12px; height: 12px; margin: -6px 0 0 -6px; border-radius: 2px; transform: rotate(45deg); background: var(--accent-bg); border-color: #ffffff; }
  .hd.add { width: 18px; height: 18px; margin: -9px 0 0 -9px; background: var(--accent-bg); border: 2px solid #ffffff; display: grid; place-items: center; color: #ffffff; box-shadow: 0 1px 3px rgb(0 0 6 / 30%); }
  .hd.add .icon { width: 12px; height: 12px; }
  .hd.active { background: var(--accent-bg); border-color: #ffffff; }
  .rot-stem { position: absolute; width: 1px; background: var(--accent-bg); }
  .guide { position: absolute; background: var(--annota-violet); }
  .guide.v { width: 1px; } .guide.h { height: 1px; }
  .chip { position: absolute; padding: 3px 8px; border-radius: var(--radius-s); background: var(--osd-bg-solid); color: var(--osd-fg); font-size: var(--fs-caption); font-weight: var(--fw-bold); font-variant-numeric: tabular-nums; white-space: nowrap; box-shadow: 0 1px 3px rgb(0 0 6 / 25%); }
  .chip.violet { background: var(--annota-violet); }
  .cursor { position: absolute; width: 18px; height: 18px; }
  .zoom-osd .label { min-width: 64px; text-align: center; }
"""


# ------------------------------------------------------------------ page
def page(title, body, size=(1280, 800), extra_css="", flush=True):
    w, h = size
    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport-size" content="{w}x{h}">
<title>Annota — {title}</title>
<link rel="stylesheet" href="{KIT}/tokens.css">
<link rel="stylesheet" href="{KIT}/adwaita.css">
<style>{PAGE_CSS}{extra_css}</style>
</head>
<body class="mock-frame{' flush' if flush else ''}">
{GLYPHS}
{body}
<script src="{KIT}/icons.js"></script>
</body>
</html>
"""


def headerbar(title, subtitle, undo_dis=False, redo_dis=True, narrow=False, sidebar_on=True, extra_end=""):
    u = " disabled" if undo_dis else ""
    r = " disabled" if redo_dis else ""
    side = (f'<button class="btn flat image-button{" active" if sidebar_on else ""}" title="Show Properties (F9)">{icon("sidebar-show-right")}</button>')
    copy = "" if narrow else f'<button class="btn image-button" title="Copy to Clipboard (Ctrl+C)">{icon("copy")}</button>'
    return f"""<div class="headerbar">
    <div class="start">
      <button class="btn image-button" title="New Capture">{icon("capture-region")}</button>
      <div class="linked">
        <button class="btn image-button{u}" title="Undo (Ctrl+Z)">{icon("undo")}</button>
        <button class="btn image-button{r}" title="Redo (Ctrl+Shift+Z)">{icon("redo")}</button>
      </div>
    </div>
    <div class="title-widget"><span class="title">{title}</span><span class="subtitle">{subtitle}</span></div>
    <div class="end">
      {extra_end}{side}
      {copy}
      <button class="btn text-button suggested-action">Export</button>
      <button class="btn flat image-button">{icon("open-menu")}</button>
      <div class="window-controls"><span class="wc">{icon("close")}</span></div>
    </div>
  </div>"""


# canonical tool order (Phase 1 + Phase 2); see index.md redlines
TOOLS = [
    ("select", "Select", "V"), None,
    ("arrow", "Arrow", "A"), ("line", "Line", "L"), ("pen", "Pen", "P"), ("shape", "Shape", "U"),
    ("text", "Text", "T"), ("callout", "Callout", "K"), ("step", "Step", "S"), None,
    ("highlighter", "Highlighter", "H"), ("blur", "Redact", "B"), ("magnify", "Magnify", "M"),
    ("spotlight", "Spotlight", "G"), ("stamp", "Stamp", "I"), None,
    ("crop", "Crop", "C"), ("view-more", "More Tools", ""),
]


def tool_strip(active="select", hover=None, focus=None, compact=False):
    out = ['<div class="tool-strip">']
    for t in TOOLS:
        if t is None:
            out.append('<span class="separator-h"></span>')
            continue
        name, label, key = t
        cls = "btn flat"
        if name == active:
            cls += " active"
        if name == hover:
            cls += " hover"
        if name == focus:
            cls += " focus"
        out.append(f'<button class="{cls}" title="{label}{" (" + key + ")" if key else ""}">{icon(name)}</button>')
    out.append("</div>")
    return "\n".join(out)


# ------------------------------------------------------------------ tray
TRAY_OPEN = [("b-lumen-checkout", "0% 0%", "Checkout flow", True, False),
             ("d-tidewater-analytics", "0% 0%", "September dashboard", False, True),
             ("a-harbor-backup", "50% 50%", "Backup settings", False, False)]
TRAY_RECENT = [("e-relay-chat", "30% 20%"), ("c-code-editor", "0% 0%"), ("f-field-notes", "50% 0%"),
               ("d-tidewater-analytics", "100% 20%"), ("b-lumen-checkout", "100% 30%"),
               ("a-harbor-backup", "0% 0%"), ("e-relay-chat", "100% 0%")]


def tray(current="b-lumen-checkout", hover_idx=None, drag_idx=None, open_docs=None):
    open_docs = open_docs if open_docs is not None else TRAY_OPEN
    o = []
    for i, (img, pos, name, cur, unsaved) in enumerate(open_docs):
        cls = "thumb" + (" current" if img == current else "")
        o.append(f'<div class="{cls}" title="{name}"><img src="{SAMPLES}/{img}.png" style="object-position:{pos}" alt="">'
                 + ('<span class="unsaved"></span>' if unsaved else '') + '</div>')
    r = []
    for i, (img, pos) in enumerate(TRAY_RECENT):
        cls = "thumb"
        if i == hover_idx:
            cls += " hover"
        if i == drag_idx:
            cls += " dragging"
        r.append(f'<div class="{cls}"><img src="{SAMPLES}/{img}.png" style="object-position:{pos}" alt=""></div>')
    open_html = (f'<div class="tgroup"><span class="tlabel">Open</span><div class="thumbs">{"".join(o)}</div></div>'
                 '<span class="separator-v" style="margin:21px 0 0"></span>') if o else ""
    return f"""<div class="tray">
        {open_html}
        <div class="tgroup grow"><span class="tlabel">Recent</span><div class="thumbs">{''.join(r)}</div></div>
        <span class="fade"></span>
        <div class="tend">
          <button class="btn flat image-button" title="Open Library (Ctrl+L)">{icon("library")}</button>
          <button class="btn flat image-button" title="Hide Tray">{icon("pan-down")}</button>
        </div>
      </div>"""


def zoom_osd(label="Fit · 50 %", extra_style="right:18px;bottom:18px", active=False):
    return f"""<div class="osd-toolbar zoom-osd" style="position:absolute;{extra_style}">
          <button class="btn image-button" title="Zoom Out (Ctrl+−)">{icon("zoom-out")}</button>
          <button class="btn text-button{' active' if active else ''}" style="padding:0 8px;min-width:78px" title="Zoom Levels">{label}</button>
          <button class="btn image-button" title="Zoom In (Ctrl++)">{icon("zoom-in")}</button>
        </div>"""


def canvas(sample, annotations_svg, overlays="", img_w=800, img_h=500, vb=(1600, 1000), osd=None,
           area_extra="", img_style="", image_html=None, top=False):
    osd = zoom_osd() if osd is None else osd
    img = image_html if image_html is not None else f'<img src="{SAMPLES}/{sample}.png" alt="">'
    return f"""<div class="canvas-area{' top' if top else ''}" {area_extra}>
        <div class="canvas-image" style="width:{img_w}px;height:{img_h}px;{img_style}">
          {img}
          <svg class="annotations" viewBox="0 0 {vb[0]} {vb[1]}">{SVG_DEFS}{annotations_svg}</svg>
          {overlays}
        </div>
        {osd}
      </div>"""


def editor(title, subtitle, tool, canvas_html, sidebar_html, overlays_win="", tray_html=None,
           undo_dis=False, redo_dis=True, strip_hover=None, sidebar_on=True):
    tray_html = tray() if tray_html is None else tray_html
    side = f"""<div class="sidebar-pane end props"><div class="scroll">{sidebar_html}</div></div>""" if sidebar_on else ""
    return f"""<div class="window" style="width:1280px;height:800px">
  {headerbar(title, subtitle, undo_dis, redo_dis, sidebar_on=sidebar_on)}
  <div class="split-view">
    {tool_strip(tool, hover=strip_hover)}
    <div class="editor-col">
      {canvas_html}
      {tray_html}
    </div>
    {side}
  </div>
  {overlays_win}
</div>"""


# ------------------------------------------------------------------ SVG annotation helpers (image px)
SVG_DEFS = """<defs>
  <filter id="sh" filterUnits="userSpaceOnUse" x="-500" y="-500" width="3000" height="2500"><feDropShadow dx="2.5" dy="3.5" stdDeviation="3" flood-color="#000006" flood-opacity=".32"/></filter>
  <filter id="sh-big" filterUnits="userSpaceOnUse" x="-500" y="-500" width="3000" height="2500"><feDropShadow dx="7" dy="9" stdDeviation="8" flood-color="#000006" flood-opacity=".45"/></filter>
</defs>"""

CORAL = "var(--annota-coral)"


def _head(x, y, ang, w, kind, color, size=1.0):
    """arrow end at (x,y) pointing in direction ang (radians)."""
    L = max(w * 3.2, 20) * size
    W = max(w * 2.6, 16) * size
    ca, sa = math.cos(ang), math.sin(ang)

    def p(dx, dy):  # dx along, dy perpendicular
        return (x + dx * ca - dy * sa, y + dx * sa + dy * ca)

    if kind == "triangle":
        a, b, c = p(0, 0), p(-L, W / 2), p(-L, -W / 2)
        return (f'<path d="M{a[0]:.1f} {a[1]:.1f} L{b[0]:.1f} {b[1]:.1f} L{c[0]:.1f} {c[1]:.1f}Z" fill="{color}" '
                f'stroke="{color}" stroke-width="{w*0.4:.1f}" stroke-linejoin="round"/>'), L * 0.85
    if kind == "open":
        b, a, c = p(-L, W / 2), p(0, 0), p(-L, -W / 2)
        return (f'<path d="M{b[0]:.1f} {b[1]:.1f} L{a[0]:.1f} {a[1]:.1f} L{c[0]:.1f} {c[1]:.1f}" fill="none" '
                f'stroke="{color}" stroke-width="{w:.1f}" stroke-linecap="round" stroke-linejoin="round"/>'), 0
    if kind == "circle":
        r = W * 0.38
        c = p(-r * 0.6, 0)
        return f'<circle cx="{c[0]:.1f}" cy="{c[1]:.1f}" r="{r:.1f}" fill="{color}"/>', r
    if kind == "square":
        s = W * 0.62
        pts = [p(0, s / 2), p(0, -s / 2), p(-s, -s / 2), p(-s, s / 2)]
        d = "M" + " L".join(f"{q[0]:.1f} {q[1]:.1f}" for q in pts) + "Z"
        return f'<path d="{d}" fill="{color}"/>', s * 0.5
    if kind == "bar":
        a, b = p(0, W / 2), p(0, -W / 2)
        return (f'<path d="M{a[0]:.1f} {a[1]:.1f} L{b[0]:.1f} {b[1]:.1f}" stroke="{color}" stroke-width="{w:.1f}" '
                f'stroke-linecap="round"/>'), 0
    return "", 0


def arrow(x1, y1, x2, y2, w=8, color=CORAL, end="triangle", start="none", dash=None, ctrl=None, shadow=True,
          opacity=1, size_end=1.0, size_start=1.0):
    """Straight or quadratic arrow. ctrl = point the curve passes THROUGH at t=.5 (handle position)."""
    if ctrl:
        # control point so that curve passes through ctrl at t=.5
        cx, cy = 2 * ctrl[0] - (x1 + x2) / 2, 2 * ctrl[1] - (y1 + y2) / 2
        ang_end = math.atan2(y2 - cy, x2 - cx)
        ang_start = math.atan2(y1 - cy, x1 - cx)
    else:
        ang_end = math.atan2(y2 - y1, x2 - x1)
        ang_start = ang_end + math.pi
    he, inset_e = _head(x2, y2, ang_end, w, end, color, size_end)
    hs, inset_s = _head(x1, y1, ang_start, w, start, color, size_start)
    ex, ey = x2 - inset_e * math.cos(ang_end), y2 - inset_e * math.sin(ang_end)
    bx, by = x1 - inset_s * math.cos(ang_start), y1 - inset_s * math.sin(ang_start)
    dash_attr = ""
    if dash == "dashed":
        dash_attr = f' stroke-dasharray="{w*2.5:.0f} {w*1.6:.0f}"'
    elif dash == "dotted":
        dash_attr = f' stroke-dasharray="0 {w*2:.0f}"'
    if ctrl:
        d = f"M{bx:.1f} {by:.1f} Q{cx:.1f} {cy:.1f} {ex:.1f} {ey:.1f}"
    else:
        d = f"M{bx:.1f} {by:.1f} L{ex:.1f} {ey:.1f}"
    f = ' filter="url(#sh)"' if shadow else ""
    return (f'<g{f} opacity="{opacity}"><path d="{d}" stroke="{color}" stroke-width="{w}" stroke-linecap="round" '
            f'fill="none"{dash_attr}/>{he}{hs}</g>')


def step(x, y, label, color=CORAL, r=26, shape="circle", fg="#ffffff", shadow=True, fs=None):
    f = ' filter="url(#sh)"' if shadow else ""
    fs = fs or (r * 1.05 if len(label) < 2 else r * (0.9 if len(label) == 2 else 0.72))
    if shape == "circle":
        body = f'<circle cx="{x}" cy="{y}" r="{r}" fill="{color}" stroke="#ffffff" stroke-width="3"/>'
        ty = y
    elif shape == "square":
        body = f'<rect x="{x-r}" y="{y-r}" width="{2*r}" height="{2*r}" fill="{color}" stroke="#ffffff" stroke-width="3"/>'
        ty = y
    elif shape == "rounded":
        body = f'<rect x="{x-r}" y="{y-r}" width="{2*r}" height="{2*r}" rx="{r*0.4}" fill="{color}" stroke="#ffffff" stroke-width="3"/>'
        ty = y
    else:  # pin: circle head with point at (x, y+r*1.9)
        body = (f'<path d="M{x} {y + r*1.9} C{x - r*0.5} {y + r*1.25} {x - r} {y + r*0.75} {x - r} {y} '
                f'A{r} {r} 0 1 1 {x + r} {y} C{x + r} {y + r*0.75} {x + r*0.5} {y + r*1.25} {x} {y + r*1.9}Z" '
                f'fill="{color}" stroke="#ffffff" stroke-width="3"/>')
        ty = y
    return (f'<g{f}>{body}<text x="{x}" y="{ty}" dy=".36em" text-anchor="middle" fill="{fg}" '
            f'style="font-family:var(--font-ui);font-weight:800;font-size:{fs:.0f}px">{label}</text></g>')


def rect(x, y, w, h, color=CORAL, sw=6, rx=0, fill="none", shadow=True, dash=None, opacity=1):
    f = ' filter="url(#sh)"' if shadow else ""
    da = f' stroke-dasharray="{sw*2.5:.0f} {sw*1.6:.0f}"' if dash == "dashed" else ""
    return (f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" fill="{fill}" stroke="{color}" '
            f'stroke-width="{sw}"{da}{f} opacity="{opacity}"/>')


def highlight(x, y, w, h, color="rgb(255 222 0)", opacity=.4):
    return (f'<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{color}" opacity="{opacity}" '
            f'style="mix-blend-mode:multiply"/>')


def callout(x, y, w, h, text_lines, tails, fill=CORAL, stroke="none", fg="#ffffff", rx=14, fs=30, sw=0,
            shadow=True, shape="rounded", tail_w=34, weight=700, anchor="middle"):
    """tails: list of (tip_x, tip_y, base_x, base_y) where base is on the body edge midpoint area."""
    f = ' filter="url(#sh)"' if shadow else ""
    parts = []
    for tx, ty, bx, by in tails:
        # base: two points around (bx,by) perpendicular to the tail direction
        ang = math.atan2(ty - by, tx - bx)
        px, py = -math.sin(ang) * tail_w / 2, math.cos(ang) * tail_w / 2
        parts.append(f'<path d="M{bx+px:.1f} {by+py:.1f} L{tx} {ty} L{bx-px:.1f} {by-py:.1f}Z" fill="{fill}" '
                     f'stroke="{stroke}" stroke-width="{sw}" stroke-linejoin="round"/>')
    if shape == "ellipse":
        body = f'<ellipse cx="{x+w/2}" cy="{y+h/2}" rx="{w/2}" ry="{h/2}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}"/>'
    else:
        r = 0 if shape == "rect" else rx
        body = f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}"/>'
    # fill-only cover so tail joins cleanly
    cover = ""
    if sw:
        cover = "".join(
            f'<path d="M{bx + (-math.sin(math.atan2(ty-by, tx-bx)) * (tail_w/2 - sw)):.1f} {by + (math.cos(math.atan2(ty-by, tx-bx)) * (tail_w/2 - sw)):.1f} '
            f'L{bx - (-math.sin(math.atan2(ty-by, tx-bx)) * (tail_w/2 - sw)):.1f} {by - (math.cos(math.atan2(ty-by, tx-bx)) * (tail_w/2 - sw)):.1f}" stroke="{fill}" stroke-width="{sw*2.2}"/>'
            for tx, ty, bx, by in tails)
    lh = fs * 1.3
    total = lh * len(text_lines)
    tx_ = x + w / 2 if anchor == "middle" else x + 24
    texts = "".join(
        f'<text x="{tx_}" y="{y + h/2 - total/2 + lh*(i+.5)}" dy=".35em" text-anchor="{anchor}" fill="{fg}" '
        f'style="font-family:var(--font-ui);font-weight:{weight};font-size:{fs}px">{t}</text>'
        for i, t in enumerate(text_lines))
    return f'<g{f}>{"".join(parts)}{body}{cover}</g>{texts}'


def text_obj(x, y, lines, fs=34, color=CORAL, weight=700, outline=None, ow=0, anchor="start", italic=False,
             underline=False, shadow=True, family="var(--font-ui)"):
    f = ' filter="url(#sh)"' if shadow else ""
    st = (f'font-family:{family};font-weight:{weight};font-size:{fs}px;'
          + ("font-style:italic;" if italic else "") + ("text-decoration:underline;" if underline else ""))
    if outline:
        st += f"paint-order:stroke;stroke:{outline};stroke-width:{ow}px;stroke-linejoin:round;"
    return "".join(f'<text x="{x}" y="{y + i*fs*1.25}" text-anchor="{anchor}" fill="{color}" style="{st}"{f}>{t}</text>'
                   for i, t in enumerate(lines))


# ------------------------------------------------------------------ overlay helpers (screen px relative to canvas-image)
def sel(x, y, w, h, rot=False, handles=8, dashed=False, thin=False, angle=0, ox=None, oy=None):
    """Selection box in IMAGE px (converted to screen at 50 %)."""
    X, Y, W, H = x * ZOOM, y * ZOOM, w * ZOOM, h * ZOOM
    cls = "selx" + (" dashed" if dashed else "") + (" thin" if thin else "")
    hs = []
    if handles == 8:
        for hx, hy in [(0, 0), (50, 0), (100, 0), (100, 50), (100, 100), (50, 100), (0, 100), (0, 50)]:
            hs.append(f'<span class="hd sq" style="left:{hx}%;top:{hy}%"></span>')
    if rot:
        hs.append('<span class="rot-stem" style="left:50%;top:-24px;height:24px"></span>')
        hs.append('<span class="hd" style="left:50%;top:-28px;width:14px;height:14px;margin:-7px 0 0 -7px"></span>')
    tr = f"transform:rotate({angle}deg);" if angle else ""
    return (f'<div class="ov {cls}" style="left:{X:.1f}px;top:{Y:.1f}px;width:{W:.1f}px;height:{H:.1f}px;{tr}">'
            + "".join(hs) + "</div>")


def handle(x, y, kind="", title=""):
    """single handle at IMAGE px"""
    inner = icon("plus") if kind == "add" else ""
    return f'<span class="ov hd {kind}" style="left:{x*ZOOM:.1f}px;top:{y*ZOOM:.1f}px" title="{title}">{inner}</span>'


def chip(x, y, text, cls=""):
    return f'<span class="ov chip {cls}" style="left:{x:.1f}px;top:{y:.1f}px">{text}</span>'


def cursor(x, y, kind="arrow"):
    """cursor at screen px relative to canvas-image (or window)"""
    if kind == "move":
        return (f'<svg class="ov cursor" style="left:{x-11}px;top:{y-11}px;width:22px;height:22px" viewBox="0 0 22 22">'
                '<path d="M11 1l3.5 4h-2.3v4.8H17V7.5l4 3.5-4 3.5v-2.3h-4.8V17h2.3L11 21l-3.5-4h2.3v-4.8H5v2.3L1 11l4-3.5v2.3h4.8V5H7.5z" '
                'fill="#000" stroke="#fff" stroke-width="1.2" stroke-linejoin="round"/></svg>')
    if kind == "cross":
        return (f'<svg class="ov cursor" style="left:{x-11}px;top:{y-11}px;width:22px;height:22px" viewBox="0 0 22 22">'
                '<path d="M11 1v8M11 13v8M1 11h8M13 11h8" stroke="#fff" stroke-width="3.2"/>'
                '<path d="M11 1v8M11 13v8M1 11h8M13 11h8" stroke="#000" stroke-width="1.4"/></svg>')
    if kind == "rotate":
        return (f'<svg class="ov cursor" style="left:{x-11}px;top:{y-11}px;width:22px;height:22px" viewBox="0 0 22 22">'
                '<path d="M5 12a6 6 0 1 1 6 6" fill="none" stroke="#fff" stroke-width="4.2"/>'
                '<path d="M5 12a6 6 0 1 1 6 6" fill="none" stroke="#000" stroke-width="2"/>'
                '<path d="M1.5 10.5h7L5 15.5z" fill="#000" stroke="#fff" stroke-width="1.2"/></svg>')
    if kind == "text":
        return (f'<svg class="ov cursor" style="left:{x-6}px;top:{y-11}px;width:12px;height:22px" viewBox="0 0 12 22">'
                '<path d="M2 1.5h3l1 1 1-1h3M6 2.5v17M2 20.5h3l1-1 1 1h3" fill="none" stroke="#fff" stroke-width="3.2"/>'
                '<path d="M2 1.5h3l1 1 1-1h3M6 2.5v17M2 20.5h3l1-1 1 1h3" fill="none" stroke="#000" stroke-width="1.3"/></svg>')
    if kind == "grab":
        return (f'<svg class="ov cursor" style="left:{x-4}px;top:{y-2}px;width:22px;height:24px" viewBox="0 0 22 24">'
                '<path d="M2 1.5v16l4-3.8 3 6.8 3-1.4-3-6.6h5.6z" fill="#000" stroke="#fff" stroke-width="1.3" stroke-linejoin="round"/>'
                '<rect x="11" y="13" width="10" height="10" rx="2" fill="#2ec27e" stroke="#fff" stroke-width="1.2"/>'
                '<path d="M16 15.2v5.6M13.2 18h5.6" stroke="#fff" stroke-width="1.6"/></svg>')
    if kind == "eyedropper":
        return (f'<svg class="ov cursor" style="left:{x-2}px;top:{y-20}px;width:22px;height:22px" viewBox="0 0 22 22">'
                '<path d="M2.5 19.5l1-4 9-9 3 3-9 9z" fill="#fff" stroke="#000" stroke-width="1.4" stroke-linejoin="round"/>'
                '<path d="M12 4l2.5-2.5a2 2 0 0 1 3 0l3 3a2 2 0 0 1 0 3L18 10z" fill="#000" stroke="#fff" stroke-width="1"/></svg>')
    return (f'<svg class="ov cursor" style="left:{x}px;top:{y}px" viewBox="0 0 18 18">'
            '<path d="M2 1.5v14l3.8-3.6 2.7 6 2.6-1.2-2.6-5.9h5.2z" fill="#000" stroke="#fff" stroke-width="1.3" stroke-linejoin="round"/></svg>')


# ------------------------------------------------------------------ sidebar widgets
PALETTE = ["coral", "amber", "lime", "teal", "azure", "violet", "ink"]


def swatches(selected="coral", second_row=True, none=False, paper=True, custom=True, eyedrop=True,
             focus=None, hover=None, mixed=False, custom_color=None, custom_selected=False, extra_first=None):
    row1 = []
    for c in PALETTE:
        cls = "swatch"
        if c == selected and not mixed:
            cls += " selected"
        if c == focus:
            cls += " focus"
        if c == hover:
            cls += " hover"
        row1.append(f'<span class="{cls}" style="--c:var(--annota-{c})" title="{c.title()}"></span>')
    row2 = []
    if extra_first:
        row2.append(extra_first)
    if none:
        row2.append(f'<span class="swatch none{" selected light-ink" if selected == "none" else ""}" title="No Fill"></span>')
    if paper:
        row2.append(f'<span class="swatch light-ink{" selected" if selected == "paper" else ""}" style="--c:var(--annota-paper)" title="Paper"></span>')
    if custom_color:
        row2.append(f'<span class="swatch{" selected" if custom_selected else ""}" style="--c:{custom_color}" title="{custom_color}"></span>')
    if custom:
        row2.append('<span class="swatch custom" title="Custom Colour…"></span>')
    if eyedrop:
        row2.append(f'<span class="swatch eyedrop" title="Pick Colour From Screen">{icon("eyedropper")}</span>')
    r2 = f'<div class="swatch-row">{"".join(row2)}</div>' if second_row else ""
    return f'<div class="vbox" style="--gap:6px"><div class="swatch-row">{"".join(row1)}</div>{r2}</div>'


def group(heading, body, suffix="", desc=""):
    d = f'<span class="description">{desc}</span>' if desc else ""
    return f"""<div class="preferences-group">
          <div class="group-header"><div class="text"><span class="heading">{heading}</span>{d}</div>{suffix}</div>
          {body}
        </div>"""


def field(label, body):
    return f'<div class="field"><span class="lbl">{label}</span>{body}</div>'


def scale(pct, value, disabled=False, marks=()):
    m = "".join(f'<span class="mark" style="left:{p}%"></span>' for p in marks)
    return (f'<div class="scale{" disabled" if disabled else ""}"><div class="trough"><span class="fill" style="width:{pct}%"></span>'
            f'{m}<span class="knob" style="left:{pct}%"></span></div><span class="value" style="min-width:44px">{value}</span></div>')


def spin(value, width=120, focus=False):
    return (f'<span class="spin{" focus" if focus else ""}" style="width:{width}px"><span class="value">{value}</span>'
            f'<button class="btn">{icon("minus")}</button><button class="btn">{icon("plus")}</button></span>')


def row_line(name, control, sub=""):
    s = f'<span class="lbl" style="padding:0;display:block">{sub}</span>' if sub else ""
    return f'<div class="row-line"><span class="name">{name}{s}</span>{control}</div>'


def toggles(items, active, icon_only=False, glyphs=False, focus=None):
    out = []
    for i, it in enumerate(items):
        cls = "toggle" + (" active" if i == active else "") + (" icon-only" if icon_only else "") + (" focus-ring" if i == focus else "")
        if glyphs:
            g, title = it
            content = glyph(g) if g.startswith("x:") is False and not g.startswith("i:") else (icon(g[2:]) if g.startswith("i:") else glyph(g[2:]))
            out.append(f'<span class="{cls}" title="{title}">{content}</span>')
        else:
            out.append(f'<span class="{cls}">{it}</span>')
    return f'<div class="toggle-group">{"".join(out)}</div>'


def opacity_field(pct=100):
    return field("Opacity", scale(pct, f"{pct} %"))


def shadow_row(on=True, expanded=False, inner=""):
    chev = "pan-down"
    body = ""
    if expanded:
        body = inner
    return f"""<div class="boxed-list">
            <div class="row"><div class="text"><span class="title">Shadow</span></div><div class="suffix"><span class="switch{' on' if on else ''}"></span><button class="btn flat image-button" style="transform:rotate({180 if expanded else 0}deg)">{icon(chev)}</button></div></div>
            {body}
          </div>"""


def select_all_btn(type_plural, pressed=False, hover=False):
    cls = "btn full" + (" pressed" if pressed else "") + (" hover" if hover else "")
    return f'<button class="{cls}" style="justify-content:center">Select All {type_plural}</button>'


def arrange_group(x, y, w, h, angle="0°", locked=True):
    return group("Arrange", f"""<div class="grid2">
            <div class="cell"><span class="k">X</span><span class="entry">{x}</span></div>
            <div class="cell"><span class="k">Y</span><span class="entry">{y}</span></div>
            <div class="cell"><span class="k">W</span><span class="entry">{w}</span></div>
            <div class="cell"><span class="k">H</span><span class="entry">{h}</span></div>
          </div>
          {row_line("Rotation", spin(angle, 120))}""")


def toast(text, action="Undo", bottom=130):
    return f"""<div class="toast-overlay" style="bottom:{bottom}px;left:52px;right:300px"><div class="toast"><span class="toast-title">{text}</span>
      {f'<button class="btn">{action}</button>' if action else ''}<button class="btn flat circular">{icon("close")}</button></div></div>"""


def tooltip(x, y, text, key):
    keys = "".join(f'<span class="shortcut">{k}</span>' for k in key.split("+")) if key else ""
    return f'<div class="tooltip" style="left:{x}px;top:{y}px">{text}{keys}</div>'
