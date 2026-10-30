"""Shared mockup shell for Annota designer (advanced tools, image effects, smart editing). Private copy."""
import pathlib

KIT = "../../kit"
SAMPLE = f"{KIT}/sample-content/png"

TOOLS = [
    ("select", "Select", "V"), ("selection", "Selection", "Q"), ("magic-wand", "Magic Wand", "W"), None,
    ("arrow", "Arrow", "A"), ("line", "Line", "L"), ("pen", "Pen", "P"), ("shape", "Shape", "U"),
    ("text", "Text", "T"), ("callout", "Callout", "K"), ("step", "Step", "S"), ("highlighter", "Highlighter", "H"), None,
    ("redact", "Redact", "B"), ("magnify", "Magnify", "M"), ("spotlight", "Spotlight", "G"), ("stamp", "Stamp", "I"), None,
    ("crop", "Crop", "C"), ("cut-out", "Cut-Out", "X"), ("fill", "Fill", "F"), ("eraser", "Eraser", "E"),
]

CSS = """
  .tool-strip { width: 52px; flex: none; display: flex; flex-direction: column; align-items: center; gap: 2px; padding: 6px 0; background: var(--window-bg); box-shadow: inset -1px 0 var(--separator); position: relative; z-index: 5; }
  .tool-strip .btn { width: 36px; min-height: 34px; padding: 0; }
  .tool-strip .separator-h { width: 28px; margin: 4px 0; }
  .props { display: flex; flex-direction: column; }
  .props .scroll { padding: 18px 12px; display: flex; flex-direction: column; gap: 18px; }
  .props .side-head { display: flex; align-items: center; gap: 8px; padding: 0 2px; }
  .props .side-head .heading { font-weight: var(--fw-bold); flex: 1; }
  .props .foot { margin-top: auto; padding: 12px; display: flex; gap: 6px; box-shadow: inset 0 1px var(--separator); }
  .props .foot .btn { flex: 1; }
  .lbl { font-size: var(--fs-small); }
  .field-label { font-size: var(--fs-small); font-weight: var(--fw-bold); opacity: var(--dim-opacity); padding: 0 2px; }
  .row-stack { display: flex; flex-direction: column; gap: 6px; }
  .full { width: 100%; }
  .toggle-group.full { display: flex; }
  .toggle-group.full > .toggle { flex: 1; justify-content: center; padding: 0 4px; font-size: var(--fs-small); white-space: nowrap; }
  .toggle-group.full.four > .toggle { font-size: var(--fs-caption); padding: 0 2px; }
  .canvas-wrap { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; }
  svg.annotations text { font-family: var(--font-ui); }
  .ants { fill: none; stroke: #ffffff; stroke-width: 1; }
  .ants2 { fill: none; stroke: #1f1f24; stroke-width: 1; stroke-dasharray: 4 4; }
  .hint { font-size: var(--fs-caption); opacity: var(--dim-opacity); padding: 0 2px; line-height: 1.4; }
  .scale .trough { flex: 1; }
  .scale .lbl { width: 84px; flex: none; }
"""


def icon(name, cls="icon"):
    return f'<svg class="{cls}"><use href="#i-{name}"/></svg>'


def tool_strip(active, tooltip=True):
    out = ['<div class="tool-strip">']
    y = 6
    tip = ""
    for t in TOOLS:
        if t is None:
            out.append('<span class="separator-h"></span>')
            y += 11
            continue
        tid, name, key = t
        a = " active" if tid == active else ""
        out.append(f'<button class="btn flat{a}" title="{name} ({key})">{icon(tid)}</button>')
        if tid == active and tooltip:
            tip = f'<span class="tooltip" style="left:56px;top:{y+5}px">{name} <span class="shortcut">{key}</span></span>'
        y += 36
    out.append(tip)
    out.append("</div>")
    return "\n".join(out)


def headerbar(title, subtitle, undo_enabled=True):
    dis = "" if undo_enabled else " disabled"
    return f"""
  <div class="headerbar">
    <div class="start">
      <button class="btn image-button" title="New Capture">{icon('capture-region')}</button>
      <div class="linked">
        <button class="btn image-button" title="Undo (Ctrl+Z)"{dis}>{icon('undo')}</button>
        <button class="btn image-button" title="Redo (Ctrl+Shift+Z)" disabled>{icon('redo')}</button>
      </div>
    </div>
    <div class="title-widget"><span class="title">{title}</span><span class="subtitle">{subtitle}</span></div>
    <div class="end">
      <button class="btn image-button" title="Copy (Ctrl+C)">{icon('copy')}</button>
      <button class="btn text-button suggested-action" title="Export (Ctrl+E)">Export</button>
      <button class="btn flat image-button" title="Main Menu">{icon('open-menu')}</button>
      <div class="window-controls"><span class="wc">{icon('close')}</span></div>
    </div>
  </div>"""


def zoom_osd(z="50 %"):
    return f"""<div class="osd-toolbar" style="position:absolute;right:18px;bottom:18px;z-index:4">
          <button class="btn image-button">{icon('zoom-out')}</button>
          <span class="label">{z}</span>
          <button class="btn image-button">{icon('zoom-in')}</button>
        </div>"""


def page(title, body, extra_css="", viewport="1280x800"):
    w, h = viewport.split("x")
    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport-size" content="{viewport}">
<title>Annota — {title}</title>
<link rel="stylesheet" href="{KIT}/tokens.css">
<link rel="stylesheet" href="{KIT}/adwaita.css">
<style>{CSS}{extra_css}</style>
</head>
<body class="mock-frame flush">
<div class="window" style="width:{w}px;height:{h}px">
{body}
</div>
<script src="{KIT}/icons.js"></script>
</body>
</html>
"""


def editor(*, title, subtitle, tool, canvas, sidebar, overlay="", zoom="50 %", tooltip=True,
           extra_css="", page_title=None, undo=True, foot=""):
    body = f"""{headerbar(title, subtitle, undo)}
  <div class="split-view">
    {tool_strip(tool, tooltip)}
    <div class="content-pane" style="position:relative">
      <div class="canvas-area">
        {canvas}
        {zoom_osd(zoom) if zoom else ''}
      </div>
      {overlay}
    </div>
    <div class="sidebar-pane end props">
      <div class="scroll">
{sidebar}
      </div>
      {foot}
    </div>
  </div>"""
    return page(page_title or title, body, extra_css)


def image(src, w=800, h=500, svg="", style="", inner=""):
    return f"""<div class="canvas-image" style="width:{w}px;height:{h}px;{style}">
          <img src="{src}" alt="">
          {inner}
          <svg class="annotations" viewBox="0 0 {w} {h}">{svg}</svg>
        </div>"""


def swatches(sel="coral", extra=""):
    names = ["coral", "amber", "lime", "teal", "azure", "violet", "ink"]
    s = "".join(f'<span class="swatch{" selected" if n == sel else ""}" style="--c:var(--annota-{n})"></span>' for n in names)
    return f'<div class="swatch-row">{s}{extra}<span class="swatch custom"></span></div>'


def group(heading, inner, desc="", suffix=""):
    d = f'<span class="description">{desc}</span>' if desc else ""
    return f"""<div class="preferences-group">
  <div class="group-header"><div class="text"><span class="heading">{heading}</span>{d}</div>{suffix}</div>
  {inner}
</div>"""


def scale(label, pct, value, disabled=False, marks=None):
    m = ""
    if marks:
        m = "".join(f'<span class="mark" style="left:{p}%"></span>' for p in marks)
    d = " disabled" if disabled else ""
    return f'<div class="scale{d}"><span class="lbl">{label}</span><div class="trough"><div class="fill" style="width:{pct}%"></div><div class="knob" style="left:{pct}%"></div>{m}</div><span class="value">{value}</span></div>'


def spin(v, w=110):
    return f'<span class="spin" style="width:{w}px"><span class="value">{v}</span><button class="btn">{icon("minus")}</button><button class="btn">{icon("plus")}</button></span>'


def row(title, suffix="", subtitle="", cls="", prefix=""):
    sub = f'<span class="subtitle">{subtitle}</span>' if subtitle else ""
    pre = f'<div class="prefix">{prefix}</div>' if prefix else ""
    return f'<div class="row {cls}">{pre}<div class="text"><span class="title">{title}</span>{sub}</div><div class="suffix">{suffix}</div></div>'


def switch(on=False, extra=""):
    return f'<span class="switch{" on" if on else ""}{extra}"></span>'


def toggles(items, active, full=True, icon_only=False):
    out = []
    for it in items:
        lab = icon(it) if icon_only else it
        cls = "toggle" + (" active" if it == active else "") + (" icon-only" if icon_only else "")
        out.append(f'<span class="{cls}">{lab}</span>')
    four = " four" if len(items) >= 4 and not icon_only else ""
    return f'<div class="toggle-group{" full" if full else ""}{four}">{"".join(out)}</div>'


def toast(text, action="Undo"):
    btn = f'<button class="btn">{action}</button>' if action else ""
    return f'<div class="toast-overlay"><div class="toast"><span class="toast-title">{text}</span>{btn}<button class="btn circular flat">{icon("close")}</button></div></div>'


def ants_rect(x, y, w, h, rx=0):
    return (f'<rect class="ants" x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}"/>'
            f'<rect class="ants2" x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}"/>')


def handles_svg(x, y, w, h, color="var(--accent-bg)"):
    pts = [(x, y), (x + w / 2, y), (x + w, y), (x, y + h / 2), (x + w, y + h / 2), (x, y + h), (x + w / 2, y + h), (x + w, y + h)]
    s = f'<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="none" stroke="{color}" stroke-width="1"/>'
    s += "".join(f'<circle cx="{px}" cy="{py}" r="5" fill="#fff" stroke="{color}" stroke-width="1.5"/>' for px, py in pts)
    return s


def write(dirpath, name, html):
    p = pathlib.Path(dirpath) / "src" / f"{name}.html"
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(html, encoding="utf-8")


# --------------------------------------------------------------- stamps
# Original Annota stamp art, 48×48 viewBox. Drawn for Annota; CC-BY-SA-4.0.
C = {"coral": "#ff5a4e", "amber": "#f5a524", "lime": "#5fa83a", "teal": "#14b8a6",
     "azure": "#2f80ed", "violet": "#8b5cf6", "ink": "#1f1f24", "green": "#2ec27e", "red": "#e01b24"}
FONT = "font-family:Inter,Cantarell,sans-serif"


def _svg(inner, size=48):
    return f'<svg viewBox="0 0 48 48" width="{size}" height="{size}" xmlns="http://www.w3.org/2000/svg">{inner}</svg>'


def st_check(size=48):
    return _svg(f'<circle cx="24" cy="24" r="20" fill="{C["green"]}"/><circle cx="24" cy="24" r="20" fill="none" stroke="#fff" stroke-opacity=".35" stroke-width="2" transform="scale(.9) translate(2.7 2.7)"/><path d="M15 24.5l6 6 12-13" fill="none" stroke="#fff" stroke-width="4.5" stroke-linecap="round" stroke-linejoin="round"/>', size)


def st_cross(size=48):
    return _svg(f'<circle cx="24" cy="24" r="20" fill="{C["red"]}"/><path d="M17 17l14 14M31 17L17 31" stroke="#fff" stroke-width="4.5" stroke-linecap="round"/>', size)


def st_question(size=48):
    return _svg(f'<circle cx="24" cy="24" r="20" fill="{C["azure"]}"/><path d="M18.5 19a5.5 5.5 0 1 1 8 4.9c-1.6.8-2.5 1.9-2.5 3.6v1" fill="none" stroke="#fff" stroke-width="4" stroke-linecap="round"/><circle cx="24" cy="34.5" r="2.6" fill="#fff"/>', size)


def st_warn(size=48):
    return _svg(f'<path d="M24 5.5c1.4 0 2.6.7 3.3 1.9l16 27.8c1.5 2.6-.4 5.8-3.3 5.8H8c-3 0-4.8-3.2-3.3-5.8l16-27.8c.7-1.2 1.9-1.9 3.3-1.9z" fill="{C["amber"]}"/><path d="M24 16v11" stroke="#1f1f24" stroke-width="4.5" stroke-linecap="round"/><circle cx="24" cy="33.5" r="2.7" fill="#1f1f24"/>', size)


def st_info(size=48):
    return _svg(f'<rect x="4" y="4" width="40" height="40" rx="12" fill="{C["teal"]}"/><circle cx="24" cy="15" r="3" fill="#fff"/><path d="M24 22v13" stroke="#fff" stroke-width="4.5" stroke-linecap="round"/>', size)


def st_star(size=48):
    return _svg(f'<path d="M24 5l5.6 12 13 1.5-9.7 8.9 2.7 12.9L24 33.8l-11.6 6.5 2.7-12.9-9.7-8.9 13-1.5z" fill="{C["amber"]}" stroke="#c97f00" stroke-width="1.5" stroke-linejoin="round"/>', size)


def st_num(n, size=48, color="coral", shape="circle"):
    c = C[color]
    if shape == "circle":
        base = f'<circle cx="24" cy="24" r="20" fill="{c}"/><circle cx="24" cy="24" r="17" fill="none" stroke="#fff" stroke-opacity=".45" stroke-width="1.5"/>'
    else:
        base = f'<rect x="5" y="5" width="38" height="38" rx="9" fill="{c}"/>'
    return _svg(f'{base}<text x="24" y="31.5" text-anchor="middle" style="{FONT};font-weight:800;font-size:21px" fill="#fff">{n}</text>', size)


def st_hand(size=48, rot=0, mirror=False):
    # Annota pointing hand (points right at rot=0): rounded fist + long index finger + coral sleeve.
    outline = ('<path d="M10 16h11c2 0 3.5-.6 5-1.6l1.5-1h13.5a4.3 4.3 0 0 1 0 8.6H31v.2a4 4 0 0 1-.4 7.8 4 4 0 0 1-1 7.7 3.7 3.7 0 0 1-2.6 6.3H16c-3.3 0-6-2.7-6-6z"/>')
    tf = f'rotate({rot} 24 24)' + (' translate(48 0) scale(-1 1)' if mirror else '')
    g = (f'<g transform="{tf}">'
         '<rect x="3" y="16" width="8" height="26" rx="2.5" fill="#ff5a4e" stroke="#1f1f24" stroke-width="2.4"/>'
         f'<g fill="#fff" stroke="#1f1f24" stroke-width="2.4" stroke-linejoin="round">{outline}</g>'
         '<path d="M31 21.8h-6M30.6 29.6h-5M29.6 37.3h-4" stroke="#1f1f24" stroke-width="2.2" stroke-linecap="round"/>'
         '</g>')
    return _svg(g, size)


def st_pointer(size=48):
    return _svg('<path d="M12 6l26 15.5-11 2.8 6.5 12.3-5 2.6-6.5-12.3L14 34z" fill="#fff" stroke="#1f1f24" stroke-width="2.6" stroke-linejoin="round"/>', size)


def st_click(size=48):
    return _svg(f'<circle cx="20" cy="20" r="6" fill="{C["coral"]}"/><circle cx="20" cy="20" r="12" fill="none" stroke="{C["coral"]}" stroke-width="2.5" opacity=".6"/><circle cx="20" cy="20" r="18" fill="none" stroke="{C["coral"]}" stroke-width="2" opacity=".3"/><path d="M24 24l18 10.5-7.5 2 4.5 8.5-3.5 1.8-4.5-8.5L25.5 44z" fill="#fff" stroke="#1f1f24" stroke-width="2.2" stroke-linejoin="round"/>', size)


def _face(mouth, eyes="dots", size=48, fill=None):
    fill = fill or "#ffc93c"
    e = '<circle cx="17.5" cy="20" r="2.8" fill="#1f1f24"/><circle cx="30.5" cy="20" r="2.8" fill="#1f1f24"/>'
    if eyes == "wide":
        e = '<circle cx="17.5" cy="19.5" r="4" fill="#fff" stroke="#1f1f24" stroke-width="2"/><circle cx="30.5" cy="19.5" r="4" fill="#fff" stroke="#1f1f24" stroke-width="2"/><circle cx="17.5" cy="20" r="1.6" fill="#1f1f24"/><circle cx="30.5" cy="20" r="1.6" fill="#1f1f24"/>'
    if eyes == "closed":
        e = '<path d="M14 20q3.5-3 7 0M27 20q3.5-3 7 0" fill="none" stroke="#1f1f24" stroke-width="2.4" stroke-linecap="round"/>'
    return _svg(f'<circle cx="24" cy="24" r="20" fill="{fill}" stroke="#d99a00" stroke-width="1.5"/>{e}{mouth}', size)


def st_smile(size=48):
    return _face('<path d="M15.5 28q8.5 9 17 0" fill="none" stroke="#1f1f24" stroke-width="2.8" stroke-linecap="round"/>', size=size)


def st_happy(size=48):
    return _face('<path d="M14.5 27h19q-1.5 9-9.5 9t-9.5-9z" fill="#1f1f24"/>', eyes="closed", size=size)


def st_neutral(size=48):
    return _face('<path d="M17 31h14" stroke="#1f1f24" stroke-width="2.8" stroke-linecap="round"/>', size=size)


def st_sad(size=48):
    return _face('<path d="M16 33q8-7 16 0" fill="none" stroke="#1f1f24" stroke-width="2.8" stroke-linecap="round"/>', size=size)


def st_wow(size=48):
    return _face('<ellipse cx="24" cy="31.5" rx="4" ry="5" fill="#1f1f24"/>', eyes="wide", size=size)


def st_key(label, size=48, wide=False):
    fs = 13 if len(label) <= 3 else 10.5
    return _svg(f'<rect x="4" y="9" width="40" height="31" rx="6" fill="#c9c9cf"/><rect x="4" y="7" width="40" height="29" rx="6" fill="#fbfbfc" stroke="#9a9aa2" stroke-width="1.3"/>'
                f'<text x="24" y="26" text-anchor="middle" style="{FONT};font-weight:700;font-size:{fs}px" fill="#2e2e33">{label}</text>', size)


def st_logo(size=48):
    # invented placeholder "company logo" (fictional Nordvale): two rings + wordmark
    return _svg('<rect x="2" y="8" width="44" height="32" rx="6" fill="#ffffff" stroke="#d0d0d6"/><circle cx="16" cy="21" r="6.5" fill="none" stroke="#14866d" stroke-width="3"/><circle cx="23" cy="21" r="6.5" fill="none" stroke="#e5a50a" stroke-width="3"/>'
                f'<text x="34" y="24.5" text-anchor="middle" style="{FONT};font-weight:800;font-size:8px" fill="#1f1f24">NV</text><text x="24" y="35.5" text-anchor="middle" style="{FONT};font-weight:600;font-size:6px;letter-spacing:.8px" fill="#55555c">NORDVALE</text>', size)


def st_approved(size=48):
    return _svg(f'<rect x="3" y="13" width="42" height="22" rx="5" fill="none" stroke="{C["green"]}" stroke-width="3"/><text x="24" y="28.5" text-anchor="middle" style="{FONT};font-weight:800;font-size:10px;letter-spacing:.5px" fill="{C["green"]}">OK</text>', size)


def st_new(size=48):
    return _svg(f'<path d="M24 3l4.6 5.6 7-1.8 1.2 7.1 6.8 2.6-3.1 6.5 3.1 6.5-6.8 2.6-1.2 7.1-7-1.8L24 45l-4.6-5.6-7 1.8-1.2-7.1-6.8-2.6 3.1-6.5-3.1-6.5 6.8-2.6 1.2-7.1 7 1.8z" fill="{C["violet"]}"/><text x="24" y="28" text-anchor="middle" style="{FONT};font-weight:800;font-size:11px" fill="#fff">NEW</text>', size)


CATALOG = {
    "Status": [("Done", st_check), ("Wrong", st_cross), ("Question", st_question), ("Warning", st_warn), ("Info", st_info), ("Star", st_star), ("OK", st_approved), ("New", st_new)],
    "Numbers": [(str(i), (lambda i=i: lambda size=48: st_num(i, size))()) for i in range(1, 9)] + [("1 square", lambda size=48: st_num(1, size, "azure", "square")), ("2 square", lambda size=48: st_num(2, size, "azure", "square"))],
    "Pointers": [("Point right", st_hand), ("Point up", lambda size=48: st_hand(size, -90)), ("Point left", lambda size=48: st_hand(size, 0, True)), ("Point down", lambda size=48: st_hand(size, 90)), ("Pointer", st_pointer), ("Click", st_click)],
    "Faces": [("Smile", st_smile), ("Happy", st_happy), ("Neutral", st_neutral), ("Sad", st_sad), ("Surprised", st_wow)],
    "Keys": [(k, (lambda k=k: lambda size=48: st_key(k, size))()) for k in ["Ctrl", "Shift", "Alt", "Super", "Enter", "Esc", "Tab", "Del", "F2", "A", "C", "V"]],
}
