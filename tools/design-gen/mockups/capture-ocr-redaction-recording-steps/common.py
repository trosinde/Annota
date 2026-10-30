"""Shared helpers for generating Annota mockup HTML (designer agent, phase-2/3 changes)."""
import pathlib, random

ROOT = pathlib.Path("/home/claude/annota/docs/design")
S = "../../kit/sample-content/png/"


def icon(name, cls="icon"):
    return f'<svg class="{cls}"><use href="#i-{name}"/></svg>'


def write(change, name, body, title, size="1280x800", css="", flush=True):
    d = ROOT / change / "src"
    d.mkdir(parents=True, exist_ok=True)
    html = f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport-size" content="{size}">
<title>Annota — {title}</title>
<link rel="stylesheet" href="../../kit/tokens.css">
<link rel="stylesheet" href="../../kit/adwaita.css">
<style>
{BASE_CSS}
{css}
</style>
</head>
<body class="mock-frame{' flush' if flush else ''}">
{body}
<script src="../../kit/icons.js"></script>
</body>
</html>
"""
    (d / f"{name}.html").write_text(html, encoding="utf-8")


BASE_CSS = """
  .tool-strip { width: 52px; flex: none; display: flex; flex-direction: column; align-items: center; gap: 4px; padding: 6px 0; background: var(--window-bg); box-shadow: inset -1px 0 var(--separator); }
  .tool-strip .btn { width: 40px; min-height: 40px; }
  .tool-strip .separator-h { width: 28px; margin: 4px 0; }
  .props .scroll { padding: 18px 12px; display: flex; flex-direction: column; gap: 18px; }
  .native { position: absolute; left: 0; top: 0; transform-origin: 0 0; }
  .native > img { display: block; }
  .native > svg.ov { position: absolute; left: 0; top: 0; overflow: visible; }
  .patch { position: absolute; display: flex; align-items: center; white-space: nowrap; }
  .sec-title { font-size: var(--fs-caption-heading); font-weight: var(--fw-bold); opacity: var(--dim-opacity); padding: 0 2px; }
  .hint { font-size: var(--fs-small); opacity: var(--dim-opacity); line-height: 1.4; }
  .local-note { display: flex; gap: 8px; align-items: flex-start; font-size: var(--fs-small); line-height: 1.4; }
  .local-note .icon { margin-top: 1px; opacity: var(--dim-opacity); }
  .local-note span { opacity: var(--dim-opacity); }
  /* desktop scene (GNOME Shell) — the panel is shell chrome, black in GNOME 46 */
  .desktop { position: relative; width: 1280px; height: 800px; overflow: hidden; background: var(--desktop-bg); }
  .shell-panel { position: absolute; left: 0; right: 0; top: 0; height: 32px; background: #000; color: #fff; display: flex; align-items: center; justify-content: space-between; padding: 0 12px; font-size: 14px; font-weight: var(--fw-bold); z-index: 60; }
  .shell-panel .ws { display: flex; gap: 6px; align-items: center; }
  .shell-panel .ws i { width: 8px; height: 8px; border-radius: 4px; background: rgb(255 255 255 / 45%); }
  .shell-panel .ws i.on { width: 30px; background: #fff; }
  .shell-panel .sys { display: flex; gap: 12px; align-items: center; }
  .shell-panel .rec-ind { display: inline-flex; align-items: center; gap: 6px; height: 22px; padding: 0 10px; border-radius: 11px; background: var(--destructive-bg); font-variant-numeric: tabular-nums; }
  .shell-notif { position: absolute; left: 50%; top: 44px; width: 420px; margin-left: -210px; z-index: 61; background: var(--osd-bg-solid); color: #fff; border-radius: 18px; padding: 12px; display: flex; gap: 12px; box-shadow: 0 4px 18px rgb(0 0 6 / 35%); }
  .shell-notif .t { font-weight: var(--fw-bold); }
  .shell-notif .b { font-size: var(--fs-small); opacity: .8; }
  .shell-notif .acts { display: flex; gap: 6px; margin-top: 10px; }
  .shell-notif .acts .btn { background: var(--osd-fill); color: #fff; flex: 1; }
  .screen-img { position: absolute; left: 0; top: 32px; width: 1280px; height: 800px; }
  .scrim { position: absolute; background: rgb(0 0 6 / 45%); }
  .mic-off { position: relative; }
  .mic-off::after { content: ""; position: absolute; left: 50%; top: 50%; width: 22px; height: 2px; margin: -1px 0 0 -11px; background: currentColor; transform: rotate(-45deg); box-shadow: 0 0 0 1.5px var(--osd-bg-solid); border-radius: 1px; }
  .kbd-row { display: inline-flex; gap: 4px; align-items: center; }
"""


def native(sample, scale, w, h, inner="", offset_y=0, img_h=1000, img_w=1600):
    """Sample capture in native pixel coordinates scaled into a w×h canvas-image box."""
    return (f'<div class="canvas-image" style="width:{w}px;height:{h}px;overflow:hidden">'
            f'<div class="native" style="width:{img_w}px;height:{img_h}px;transform:scale({scale}) translateY(-{offset_y}px)">'
            f'<img src="{S}{sample}.png" style="width:{img_w}px;height:{img_h}px" alt="">{inner}</div></div>')


def ov(content, w=1600, h=1000):
    return f'<svg class="ov" width="{w}" height="{h}" viewBox="0 0 {w} {h}">{content}</svg>'


TOOLS = [("select", "Select (V)"), ("arrow", "Arrow (A)"), ("shape", "Shape (R)"), ("text", "Text (T)"),
         ("callout", "Callout (C)"), ("step", "Step (S)"), ("highlighter", "Highlighter (H)"), "-",
         ("redact", "Redact (B)"), ("magnify", "Magnify (M)"), ("spotlight", "Spotlight (L)"),
         ("stamp", "Stamp (K)"), ("crop", "Crop (X)"), ("ocr-text", "Recognise Text (O)")]


def tool_strip(active=None):
    out = ['<div class="tool-strip">']
    for t in TOOLS:
        if t == "-":
            out.append('<span class="separator-h"></span>')
            continue
        n, tip = t
        out.append(f'<button class="btn flat{" active" if n == active else ""}" title="{tip}">{icon(n)}</button>')
    out.append('</div>')
    return "".join(out)


def headerbar(title, subtitle, start=None, end=None, export=True, copy=True):
    if start is None:
        start = (f'<button class="btn image-button" title="New Capture">{icon("capture-region")}</button>'
                 f'<div class="linked"><button class="btn image-button">{icon("undo")}</button>'
                 f'<button class="btn image-button">{icon("redo")}</button></div>')
    if end is None:
        end = ""
        if copy:
            end += f'<button class="btn image-button" title="Copy (Ctrl+C)">{icon("copy")}</button>'
        if export:
            end += '<button class="btn text-button suggested-action" title="Export (Ctrl+E)">Export</button>'
        end += f'<button class="btn flat image-button">{icon("open-menu")}</button>'
    sub = f'<span class="subtitle">{subtitle}</span>' if subtitle else ""
    return (f'<div class="headerbar"><div class="start">{start}</div>'
            f'<div class="title-widget"><span class="title">{title}</span>{sub}</div>'
            f'<div class="end">{end}<div class="window-controls"><span class="wc">{icon("close")}</span></div></div></div>')


def editor(title, subtitle, active, canvas, props, overlay="", zoom="50 %", banner=""):
    return f"""<div class="window" style="width:1280px;height:800px">
{headerbar(title, subtitle)}
{banner}
<div class="split-view">
{tool_strip(active)}
<div class="content-pane" style="position:relative">
  <div class="canvas-area">
    {canvas}
    <div class="osd-toolbar" style="position:absolute;right:18px;bottom:18px">
      <button class="btn image-button">{icon("zoom-out")}</button><span class="label">{zoom}</span><button class="btn image-button">{icon("zoom-in")}</button>
    </div>
  </div>
</div>
<div class="sidebar-pane end props"><div class="scroll">{props}</div></div>
</div>
{overlay}
</div>"""


def shell_panel(rec=None, clock="Fri 30 Oct  10:42"):
    r = f'<span class="rec-ind">{icon("record")}{rec}</span>' if rec else ""
    return (f'<div class="shell-panel"><span class="ws"><i class="on"></i><i></i><i></i></span>'
            f'<span>{clock}</span><span class="sys">{r}{icon("speaker")}{icon("settings")}</span></div>')


def avatar_svg(x, y, size, shape="circle", bg="#c9d6df"):
    """Simple geometric avatar (no real faces)."""
    r = size / 2
    clip_id = f"av{int(x)}{int(y)}{shape}"
    if shape == "circle":
        clip = f'<circle cx="{x + r}" cy="{y + r}" r="{r}"/>'
    elif shape == "rounded":
        clip = f'<rect x="{x}" y="{y}" width="{size}" height="{size}" rx="{size * .22}"/>'
    else:
        clip = f'<rect x="{x}" y="{y}" width="{size}" height="{size}"/>'
    return (f'<defs><clipPath id="{clip_id}">{clip}</clipPath></defs>'
            f'<g clip-path="url(#{clip_id})"><rect x="{x}" y="{y}" width="{size}" height="{size}" fill="{bg}"/>'
            f'<circle cx="{x + r}" cy="{y + size * .42}" r="{size * .2}" fill="#e3b394"/>'
            f'<ellipse cx="{x + r}" cy="{y + size * 1.02}" rx="{size * .38}" ry="{size * .36}" fill="#3d5a73"/></g>')


def waveform(w, h, seed, n=None, color="currentColor", quiet=None):
    rnd = random.Random(seed)
    n = n or w // 3
    bars = []
    for i in range(n):
        x = i * w / n
        a = (0.25 + 0.75 * rnd.random()) * (0.6 + 0.4 * abs(((i * 7) % 23) / 23 - .5) * 2)
        if quiet and quiet[0] <= x <= quiet[1]:
            a = 0.03
        bh = max(1.5, a * h * .9)
        bars.append(f'<rect x="{x:.1f}" y="{(h - bh) / 2:.1f}" width="{max(1.2, w / n - 1):.1f}" height="{bh:.1f}" rx=".8"/>')
    return f'<svg width="{w}" height="{h}" viewBox="0 0 {w} {h}" fill="{color}">{"".join(bars)}</svg>'
