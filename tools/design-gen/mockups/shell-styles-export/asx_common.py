"""Private builders for Annota mockups (designer: app-shell, style-system, export-share)."""
import pathlib

ROOT = pathlib.Path("/home/claude/annota/docs/design")
SP = "../../kit/sample-content/png/"
APPICON = "../../kit/app-icon/annota.svg"


def ic(name, cls="icon"):
    return f'<svg class="{cls}"><use href="#i-{name}"/></svg>'


BASE_CSS = """
  .tool-strip { width: 52px; flex: none; display: flex; flex-direction: column; align-items: center; gap: 4px; padding: 6px 0; background: var(--window-bg); box-shadow: inset -1px 0 var(--separator); position: relative; }
  .tool-strip .btn { width: 40px; min-height: 40px; }
  .tool-strip .separator-h { width: 28px; margin: 4px 0; }
  .props .scroll { padding: 18px 12px; display: flex; flex-direction: column; gap: 18px; }
  .editor-col { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .editor-col > .split-view { flex: 1; }
  .content-pane { position: relative; }
  .tray { flex: none; height: 132px; background: var(--window-bg); box-shadow: inset 0 1px var(--separator); display: flex; flex-direction: column; }
  .tray-bar { height: 38px; display: flex; align-items: center; gap: 6px; padding: 0 12px; }
  .tray-items { display: flex; gap: 12px; padding: 0 12px; }
  .tray-item { width: 120px; display: flex; flex-direction: column; gap: 4px; position: relative; }
  .tray-item .thumb { width: 120px; height: 68px; border-radius: var(--radius-s); overflow: hidden; box-shadow: 0 0 0 1px var(--borders); background: var(--view-bg); }
  .tray-item .thumb img { width: 100%; height: 100%; object-fit: cover; object-position: top; display: block; }
  .tray-item .name { font-size: var(--fs-caption); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; text-align: center; }
  .tray-item.open .thumb { box-shadow: 0 0 0 2px var(--borders-strong); }
  .tray-item.sel .thumb { box-shadow: 0 0 0 3px var(--accent-bg); }
  .tray-item .tick { position: absolute; top: 6px; right: 6px; }
  .tray-item .tick .check { box-shadow: inset 0 0 0 2px #fff, 0 1px 3px rgb(0 0 6 / 40%); background: rgb(0 0 6 / 25%); }
  .tray-item .tick .check.checked { background: var(--accent-bg); box-shadow: 0 0 0 2px #fff; }
  .kbd-row { display: inline-flex; gap: 4px; }
  .toast-overlay.over-canvas { left: 52px; right: 300px; }
  .toast-overlay.over-canvas.with-tray { bottom: 156px; }
"""


def head(title, size="1280x800", extra_css=""):
    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport-size" content="{size}">
<title>Annota — {title}</title>
<link rel="stylesheet" href="../../kit/tokens.css">
<link rel="stylesheet" href="../../kit/adwaita.css">
<style>{BASE_CSS}{extra_css}</style>
</head>
"""


TAIL = '<script src="../../kit/icons.js"></script>\n</body>\n</html>\n'

TOOLS = [("select", "Select", "V"), ("arrow", "Arrow", "A"), ("shape", "Shape", "U"),
         ("text", "Text", "T"), ("callout", "Callout", "K"), ("step", "Step", "S"),
         ("highlighter", "Highlighter", "H"), None, ("blur", "Redact", "B"),
         ("magnify", "Magnify", "M"), ("spotlight", "Spotlight", "G"), ("stamp", "Stamp", "I"),
         ("crop", "Crop", "C")]


def tool_strip(active="arrow", hover=None):
    out = ['<div class="tool-strip">']
    for t in TOOLS:
        if t is None:
            out.append('<span class="separator-h"></span>')
            continue
        name, label, key = t
        cls = "btn flat" + (" active" if name == active else "") + (" hover" if name == hover else "")
        out.append(f'<button class="{cls}" title="{label} ({key})">{ic(name)}</button>')
    out.append("</div>")
    return "\n".join(out)


def headerbar(title, subtitle, *, end_extra="", copy_state="", export_state="", menu_state="",
              share=False, drag=False, drag_state="", share_state="", undo_disabled=False, minimal=False):
    ud = " disabled" if undo_disabled else ""
    if minimal:
        start = f'<button class="btn image-button" title="New Capture">{ic("capture-region")}</button>'
        end = (f'{end_extra}<button class="btn flat image-button {menu_state}">{ic("open-menu")}</button>'
               f'<div class="window-controls"><span class="wc">{ic("close")}</span></div>')
    else:
        start = (f'<button class="btn image-button" title="New Capture">{ic("capture-region")}</button>'
                 f'<div class="linked"><button class="btn image-button"{ud}>{ic("undo")}</button>'
                 f'<button class="btn image-button"{ud}>{ic("redo")}</button></div>')
        extra = ""
        if drag:
            extra += f'<button class="btn flat image-button {drag_state}" title="Drag to Another App">{ic("drag-handle")}</button>'
        if share:
            extra += f'<button class="btn image-button {share_state}" title="Share">{ic("share")}</button>'
        end = (f'{end_extra}{extra}<button class="btn image-button {copy_state}" title="Copy to Clipboard (Ctrl+C)">{ic("copy")}</button>'
               f'<button class="btn text-button suggested-action {export_state}">Export</button>'
               f'<button class="btn flat image-button {menu_state}">{ic("open-menu")}</button>'
               f'<div class="window-controls"><span class="wc">{ic("close")}</span></div>')
    sub = f'<span class="subtitle">{subtitle}</span>' if subtitle else ""
    return (f'<div class="headerbar"><div class="start">{start}</div>'
            f'<div class="title-widget"><span class="title">{title}</span>{sub}</div>'
            f'<div class="end">{end}</div></div>')


CHECKOUT_ARROW = """<path d="M372 388 L276 292" stroke="var(--annota-coral)" stroke-width="6" stroke-linecap="round" fill="none"/>
<path d="M270 316 L270 287 L299 287" stroke="var(--annota-coral)" stroke-width="6" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"""

CHECKOUT_EXTRA = """<rect x="50" y="248" width="206" height="27" rx="5" fill="none" stroke="var(--annota-coral)" stroke-width="3"/>
<circle cx="50" cy="249" r="13" fill="var(--annota-coral)"/><text x="50" y="254" font-size="15" font-weight="700" fill="#fff" text-anchor="middle" font-family="Inter">1</text>"""


def canvas(img="b-lumen-checkout", annotations=CHECKOUT_ARROW + CHECKOUT_EXTRA, w=768, h=480,
           zoom="48 %", extra="", vb=None, area_extra=""):
    vb = vb or f"0 0 {w} {h}"
    return (f'<div class="canvas-area"><div class="canvas-image" style="width:{w}px;height:{h}px">'
            f'<img src="{SP}{img}.png" alt=""><svg class="annotations" viewBox="{vb}">{annotations}</svg>{extra}</div>'
            f'<div class="osd-toolbar" style="position:absolute;right:18px;bottom:18px">'
            f'<button class="btn image-button">{ic("zoom-out")}</button><span class="label">{zoom}</span>'
            f'<button class="btn image-button">{ic("zoom-in")}</button></div>{area_extra}</div>')


SWATCHES = ["coral", "amber", "lime", "teal", "azure", "violet", "ink"]


def swatch_row(sel="coral", colors=None):
    colors = colors or [f"var(--annota-{c})" for c in SWATCHES]
    out = []
    for i, c in enumerate(colors):
        s = " selected" if (sel == i or (isinstance(sel, str) and sel in c)) else ""
        out.append(f'<span class="swatch{s}" style="--c:{c}"></span>')
    return '<div class="swatch-row">' + "".join(out) + "</div>"


def spin(v, w=120):
    return (f'<span class="spin" style="width:{w}px"><span class="value">{v}</span>'
            f'<button class="btn">{ic("minus")}</button><button class="btn">{ic("plus")}</button></span>')


def arrow_props(title="Arrow"):
    return f"""<div class="preferences-group">
  <div class="group-header"><div class="text"><span class="heading">{title}</span></div></div>
  {swatch_row()}
</div>
<div class="boxed-list">
  <div class="row"><div class="text"><span class="title">Width</span></div><div class="suffix">{spin("6")}</div></div>
  <div class="row"><div class="text"><span class="title">Shadow</span></div><div class="suffix"><span class="switch on"></span></div></div>
</div>"""


def sidebar(inner):
    return f'<div class="sidebar-pane end props"><div class="scroll">{inner}</div></div>'


TRAY_ITEMS = [("b-lumen-checkout", "Checkout flow"), ("a-harbor-backup", "Backup settings"),
              ("c-code-editor", "Build warning"), ("d-tidewater-analytics", "September dashboard"),
              ("e-relay-chat", "Chat with Jonas"), ("f-field-notes", "Field notes")]


def tray(selected=(), open_idx=0, bar=None):
    items = []
    for i, (img, name) in enumerate(TRAY_ITEMS):
        cls = "tray-item" + (" open" if i == open_idx and not selected else "") + (" sel" if i in selected else "")
        tick = ""
        if selected:
            c = "check checked" if i in selected else "check"
            tick = f'<span class="tick"><span class="{c}"></span></span>'
        items.append(f'<div class="{cls}"><div class="thumb"><img src="{SP}{img}.png" alt=""></div>{tick}<span class="name">{name}</span></div>')
    if bar is None:
        bar = (f'<span class="caption-heading dim-label">Recent Captures</span><span class="spacer"></span>'
               f'<button class="btn flat small text-button">Select</button>')
    return f'<div class="tray"><div class="tray-bar">{bar}</div><div class="tray-items">{"".join(items)}</div></div>'


def editor(*, title="Checkout flow", subtitle="1600 × 1000 · Edited", active="arrow", hover_tool=None,
           canvas_html=None, side=None, overlay="", tray_html="", hb_kwargs=None):
    hb = headerbar(title, subtitle, **(hb_kwargs or {}))
    canvas_html = canvas_html if canvas_html is not None else canvas()
    side = side if side is not None else sidebar(arrow_props())
    return f"""<div class="window" style="width:1280px;height:800px">
{hb}
<div class="editor-col">
<div class="split-view">
{tool_strip(active, hover_tool)}
<div class="content-pane">{canvas_html}</div>
{side}
</div>
{tray_html}
</div>
{overlay}
</div>"""


def page(title, body, size="1280x800", css="", body_cls="mock-frame flush"):
    return head(title, size, css) + f'<body class="{body_cls}">\n{body}\n' + TAIL


def write(change, name, html):
    d = ROOT / change / "src"
    d.mkdir(parents=True, exist_ok=True)
    (d / f"{name}.html").write_text(html, encoding="utf-8")


def popover_menu(items, style, cls="popover"):
    out = [f'<div class="{cls}" style="{style}">']
    for it in items:
        if it == "-":
            out.append('<div class="menu-sep"></div>')
        elif isinstance(it, tuple) and it[0] == "title":
            out.append(f'<div class="menu-section-title">{it[1]}</div>')
        else:
            label, accel, *rest = it
            extra = rest[0] if rest else ""
            a = f'<span class="accel">{accel}</span>' if accel else ""
            out.append(f'<div class="menu-item {extra}"><span class="label">{label}</span>{a}</div>')
    out.append("</div>")
    return "\n".join(out)


def toast(text, button=None, close=True, where="over-canvas"):
    b = f'<button class="btn text-button">{button}</button>' if button else ""
    c = f'<button class="btn flat circular">{ic("close")}</button>' if close else ""
    return f'<div class="toast-overlay {where}"><div class="toast"><span class="toast-title">{text}</span>{b}{c}</div></div>'


def alert(heading, body, actions, width=None):
    acts = "".join(f'<button class="btn {c}">{l}</button>' for l, c in actions)
    st = f' style="width:{width}px;max-width:{width}px"' if width else ""
    return (f'<div class="dialog-host"><div class="dialog alert"{st}><div class="alert-body">'
            f'<div class="alert-heading">{heading}</div><div>{body}</div></div>'
            f'<div class="alert-actions">{acts}</div></div></div>')
