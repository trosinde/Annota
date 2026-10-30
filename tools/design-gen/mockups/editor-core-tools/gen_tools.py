import pathlib, sys
sys.path.insert(0, str(pathlib.Path(__file__).parent))
import common as C
from common import *

OUT = pathlib.Path("/home/claude/annota/docs/design/add-core-annotation-tools/src")
OUT.mkdir(parents=True, exist_ok=True)
files = {}
SUB = "1600 × 1000 · Edited"

EXTRA = """
  .endbtn { display:flex; align-items:center; gap:6px; min-height:34px; padding:0 8px 0 10px; border-radius:var(--radius-s); background:var(--fill-1); }
  .endbtn svg.eg { width:44px; height:16px; }
  .endbtn::after { content:""; width:16px; height:16px; background:currentColor; -webkit-mask:url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'%3E%3Cpath d='M3.3 5.8a1 1 0 0 1 1.4 0L8 9.1l3.3-3.3a1 1 0 1 1 1.4 1.4l-4 4a1 1 0 0 1-1.4 0l-4-4a1 1 0 0 1 0-1.4z'/%3E%3C/svg%3E") center/16px no-repeat; }
  .endbtn.open { background:var(--fill-3); }
  .ends { display:grid; grid-template-columns: 1fr 1fr; gap:6px 12px; }
  .ends .spin { width:100%; }
  .eg-grid { display:grid; grid-template-columns: repeat(3, 1fr); gap:6px; padding:6px; }
  .eg-grid .opt { height:44px; border-radius:var(--radius-s); display:grid; place-items:center; }
  .eg-grid .opt.sel { background:var(--accent-a25); box-shadow: inset 0 0 0 2px var(--accent-bg); }
  .eg-grid .opt.hover { background:var(--fill-flat-hover); }
  .eg-grid .opt svg { width:56px; height:20px; }
  .eg-grid .cap { font-size:var(--fs-caption); opacity:var(--dim-opacity); text-align:center; margin-top:-4px; }
  .fmt { display:flex; gap:6px; }
  .fmt .linked .btn { width:34px; }
  .caret-line { position:absolute; width:2px; background:var(--accent-bg); }
  .picker { width:300px; padding:12px; }
  .picker .sv { position:relative; height:150px; border-radius:var(--radius-s); background: linear-gradient(to top, #000, transparent), linear-gradient(to right, #fff, hsl(4 100% 50%)); }
  .picker .sv .k { position:absolute; width:16px; height:16px; margin:-8px 0 0 -8px; border-radius:50%; border:2px solid #fff; box-shadow:0 0 0 1px rgb(0 0 6 / 40%); }
  .picker .hue { position:relative; height:12px; border-radius:6px; background: linear-gradient(to right, #f00, #ff0, #0f0, #0ff, #00f, #f0f, #f00); }
  .picker .hue .k, .picker .alpha .k { position:absolute; top:50%; width:18px; height:18px; margin:-9px 0 0 -9px; border-radius:50%; background:#fff; box-shadow:var(--shadow-slider); }
  .picker .alpha { position:relative; height:12px; border-radius:6px; }
  .picker .alpha::before { content:""; position:absolute; inset:0; border-radius:6px; background:linear-gradient(to right, transparent, var(--annota-azure)); }
  .picker .vals { display:grid; grid-template-columns: 1.6fr 1fr 1fr 1fr; gap:6px; }
  .picker .vals .entry { min-width:0; font-variant-numeric: tabular-nums; padding:0 8px; }
  .picker .vals .cap { font-size:var(--fs-caption); opacity:var(--dim-opacity); text-align:center; }
  .loupe { position:absolute; width:132px; height:132px; border-radius:50%; overflow:hidden; box-shadow: 0 0 0 3px #ffffff, 0 0 0 4px rgb(0 0 6 / 40%), 0 6px 18px rgb(0 0 6 / 35%); z-index:40; }
  .loupe .px { position:absolute; image-rendering: pixelated; }
  .loupe .cell { position:absolute; left:59px; top:59px; width:14px; height:14px; box-shadow: 0 0 0 2px #fff, 0 0 0 3px #000; }
  .angle-dial { width:56px; height:56px; border-radius:50%; background:var(--fill-1); position:relative; flex:none; }
  .angle-dial::after { content:""; position:absolute; left:50%; top:50%; width:22px; height:2px; background:var(--accent-bg); transform-origin:0 50%; transform:rotate(var(--a)); border-radius:1px; }
  .angle-dial::before { content:""; position:absolute; left:calc(50% - 4px); top:calc(50% - 4px); width:8px; height:8px; border-radius:50%; background:var(--accent-bg); }
  .stack { display:flex; flex-direction:column; gap:12px; }
  .mini-label { font-size:var(--fs-small); opacity:var(--dim-opacity); }
"""


def P(title, body, size=(1280, 800)):
    return page(title, body, size=size, extra_css=EXTRA)


def end_glyph(kind, side="end", color="currentColor"):
    """44x16 glyph showing a line with an end style at the right (end) or left (start)."""
    sw = 2
    if side == "end":
        line = f'<path d="M3 8H{36 if kind in ("triangle","circle","square") else 40}" stroke="{color}" stroke-width="{sw}" stroke-linecap="round"/>'
        x = 41
        d = -1
    else:
        line = f'<path d="M{8 if kind in ("triangle","circle","square") else 4} 8H41" stroke="{color}" stroke-width="{sw}" stroke-linecap="round"/>'
        x = 3
        d = 1
    if kind == "none":
        head = ""
    elif kind == "triangle":
        head = f'<path d="M{x} 8L{x + d*9} 3.5V12.5z" fill="{color}"/>'
    elif kind == "open":
        head = f'<path d="M{x + d*8} 3L{x} 8L{x + d*8} 13" fill="none" stroke="{color}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>'
    elif kind == "circle":
        head = f'<circle cx="{x + d*4}" cy="8" r="4" fill="{color}"/>'
    elif kind == "square":
        head = f'<rect x="{x - 7 if d < 0 else x}" y="4.5" width="7" height="7" fill="{color}"/>'
    else:  # bar
        head = f'<path d="M{x} 3V13" stroke="{color}" stroke-width="2" stroke-linecap="round"/>'
    return f'<svg class="eg" viewBox="0 0 44 16">{line}{head}</svg>'


def endbtn(kind, side, open_=False):
    return f'<span class="endbtn{" open" if open_ else ""}">{end_glyph(kind, side)}</span>'


def line_style_toggles(active=0):
    return toggles([("x:solid", "Solid"), ("x:dashed", "Dashed"), ("x:dotted", "Dotted")], active, glyphs=True)


def heading(title, sub):
    return f'<div class="group-title"><span class="heading">{title}</span><span class="dim-label caption">{sub}</span></div>'


def arrow_sidebar(title="Arrow", sub="1 selected", start="none", end="triangle", start_size="100 %", end_size="100 %",
                  width="8 px", ls=0, open_=None, colour="coral", shadow_on=True, select_all="Arrows", extra=""):
    return f"""
        {heading(title, sub)}
        {field("Colour", swatches(selected=colour))}
        {row_line("Width", spin(width))}
        {field("Ends", f'''<div class="ends">
            <span class="mini-label">Start</span><span class="mini-label">End</span>
            {endbtn(start, "start", open_ == "start")}{endbtn(end, "end", open_ == "end")}
            {spin(start_size, 132)}{spin(end_size, 132)}
          </div>''')}
        {field("Line style", line_style_toggles(ls))}
        {opacity_field(100)}
        {shadow_row(shadow_on)}
        {extra}
        {select_all_btn(select_all)}"""


def end_popover(left, top, selected="triangle", hover=None, arrow_x="50%"):
    kinds = ["none", "triangle", "open", "circle", "square", "bar"]
    names = ["None", "Triangle", "Open", "Circle", "Square", "Bar"]
    cells = "".join(
        f'<div><div class="opt{" sel" if k == selected else ""}{" hover" if k == hover else ""}">{end_glyph(k, "end").replace("class=\"eg\"", "")}</div><div class="cap">{n}</div></div>'
        for k, n in zip(kinds, names))
    return f'<div class="popover" style="left:{left}px;top:{top}px;width:252px;--arrow-x:{arrow_x}"><div class="eg-grid">{cells}</div></div>'


# ---------------------------------------------------------------- 01 tool switching tooltip
annos01 = arrow(1490, 420, 1222, 536, w=8) + step(612, 492, "1")
tt01 = tooltip(60, 47 + 6 + 58 + 4, "Arrow", "A")
side01 = arrow_sidebar(sub="Tool defaults", select_all="Arrows")
s01 = editor("Backup settings", SUB, "arrow", canvas("a-harbor-backup", annos01), side01,
             overlays_win=tt01, strip_hover="arrow", tray_html=tray(current="a-harbor-backup"))
files["01-tool-switch-tooltip"] = P("Tool switch tooltip", s01)

# ---------------------------------------------------------------- 02 curved arrow + end-style popover
x1, y1, x2, y2, mid = 690, 820, 1088, 612, (850, 660)
annos02 = arrow(x1, y1, x2, y2, w=8, ctrl=mid) + step(612, 492, "1")
ov02 = (handle(x1, y1, "big") + handle(x2, y2, "big") + handle(*mid, "curve", "Curve")
        + chip(mid[0] * .5 + 12, mid[1] * .5 + 8, "Curve"))
side02 = arrow_sidebar(open_="end")
pop02 = end_popover(980 + 12 + 144 - 126 + 20, 47 + 18 + 18 + 210 + 6 + 40, selected="triangle", hover="open", arrow_x="182px")
s02 = editor("Backup settings", SUB, "arrow", canvas("a-harbor-backup", annos02, ov02), side02,
             overlays_win=pop02, tray_html=tray(current="a-harbor-backup"))
files["02-arrow-curve-end-styles"] = P("Curved arrow, end styles", s02)

# ---------------------------------------------------------------- 03 double-ended dashed arrow, Shift 0° constraint
annos03 = (arrow(644, 858, 1216, 858, w=6, start="triangle", end="triangle", dash="dashed")
           + arrow(1290, 240, 1290, 620, w=6, start="circle", end="open", color="var(--annota-azure)")
           + arrow(270, 420, 500, 420, w=8, start="bar", end="square", color="var(--annota-violet)", shadow=False))
ov03 = (handle(644, 858, "big") + handle(1216, 858, "big") + handle(930, 858, "curve")
        + chip(420, 448, "0° · Shift: 0/45/90°") + cursor(608, 429, "cross"))
side03 = arrow_sidebar(sub="1 selected", start="triangle", end="triangle", width="6 px", ls=1)
s03 = editor("Backup settings", SUB, "arrow", canvas("a-harbor-backup", annos03, ov03), side03, tray_html=tray(current="a-harbor-backup"))
files["03-arrow-double-dashed"] = P("Double-ended arrow", s03)

# ---------------------------------------------------------------- 04 line on code editor
annos04 = (arrow(570, 910, 1050, 910, w=6, end="none")
           + arrow(432, 562, 592, 562, w=4, end="none", color="var(--annota-amber)", shadow=False, dash="dotted"))
ov04 = handle(570, 910, "big") + handle(1050, 910, "big") + handle(810, 910, "curve") + cursor(1050 * .5 + 2, 910 * .5 - 2, "cross")
side04 = arrow_sidebar(title="Line", sub="1 selected", start="none", end="none", width="6 px", select_all="Lines")
s04 = editor("Parser warning", SUB, "line", canvas("c-code-editor", annos04, ov04), side04, tray_html=tray(current=None))
files["04-line"] = P("Line", s04)

# ---------------------------------------------------------------- 05 pen
pen1 = ("M560 740 C 600 700, 700 690, 780 705 S 920 760, 980 735 S 1060 690, 1110 700")
pen2 = ("M1180 250 c 30 -40 90 -50 130 -20 s 40 90 -10 120 s -120 20 -150 -20 s -10 -70 30 -80")
annos05 = (f'<path d="{pen1}" fill="none" stroke="var(--annota-coral)" stroke-width="10" stroke-linecap="round" stroke-linejoin="round" filter="url(#sh)"/>'
           f'<path d="{pen2}" fill="none" stroke="var(--annota-lime)" stroke-width="8" stroke-linecap="round" stroke-linejoin="round" filter="url(#sh)"/>')
ov05 = sel(548, 678, 574, 94)
side05 = f"""
        {heading("Pen", "1 selected")}
        {field("Colour", swatches(selected="coral"))}
        {row_line("Width", spin("10 px", focus=True))}
        {field("Smoothing", scale(60, "60"))}
        {opacity_field(100)}
        {shadow_row(True)}
        <div class="card-note">{icon("info")}<span>Strokes stay editable vector paths. Hold <b>Shift</b> while drawing for a straight segment.</span></div>
        {select_all_btn("Pen Strokes")}"""
s05 = editor("Backup settings", SUB, "pen", canvas("a-harbor-backup", annos05, ov05), side05, tray_html=tray(current="a-harbor-backup"))
files["05-pen"] = P("Pen", s05)

# ---------------------------------------------------------------- 06 shape rectangle frame (transparent fill)
annos06 = (rect(258, 168, 568, 182, sw=6, rx=16)
           + f'<ellipse cx="1430" cy="608" rx="120" ry="56" fill="none" stroke="var(--annota-teal)" stroke-width="6" filter="url(#sh)"/>'
           + rect(1150, 358, 400, 144, sw=4, rx=0, fill="rgb(47 128 237 / 18%)", color="var(--annota-azure)", shadow=False))
ov06 = sel(255, 165, 574, 188, rot=True) + handle(258 + 16, 168 + 16, "active")
side06 = f"""
        {heading("Shape", "1 selected")}
        {toggles([("x:rect", "Rectangle"), ("x:rounded", "Rounded Rectangle"), ("x:ellipse", "Ellipse"), ("x:polygon", "Polygon")], 1, glyphs=True)}
        {field("Outline", swatches(selected="coral", second_row=False))}
        {field("Fill", swatches(selected="none", none=True, second_row=True).split('</div><div class="swatch-row">')[1].join(['<div class="vbox" style="--gap:6px"><div class="swatch-row">','']) if False else swatches(selected="none", none=True))}
        <div class="grid2"><div class="cell" style="flex-direction:column;align-items:stretch"><span class="mini-label">Outline width</span>{spin("6 px", 132)}</div><div class="cell" style="flex-direction:column;align-items:stretch"><span class="mini-label">Corner radius</span>{spin("16 px", 132)}</div></div>
        {field("Line style", line_style_toggles(0))}
        {opacity_field(100)}
        {shadow_row(True)}"""
s06 = editor("September dashboard", SUB, "shape", canvas("d-tidewater-analytics", annos06, ov06), side06, tray_html=tray(current="d-tidewater-analytics"))
files["06-shape-frame"] = P("Shape: frame", s06)

# ---------------------------------------------------------------- 07 polygon in progress
pts = [(600, 520), (960, 470), (1130, 600), (1060, 860)]
cur = (720, 880)
poly = " L".join(f"{x} {y}" for x, y in pts)
annos07 = (f'<path d="M{poly}" fill="rgb(255 90 78 / 14%)" stroke="var(--annota-coral)" stroke-width="6" stroke-linejoin="round"/>'
           f'<path d="M{pts[-1][0]} {pts[-1][1]} L{cur[0]} {cur[1]}" stroke="var(--annota-coral)" stroke-width="3" stroke-dasharray="10 8"/>'
           f'<path d="M{cur[0]} {cur[1]} L{pts[0][0]} {pts[0][1]}" stroke="var(--annota-coral)" stroke-width="2" stroke-dasharray="4 8" opacity=".6"/>')
ov07 = "".join(handle(x, y, "big" if i == 0 else "") for i, (x, y) in enumerate(pts)) + cursor(cur[0] * .5, cur[1] * .5, "cross") \
    + chip(cur[0] * .5 + 16, cur[1] * .5 + 10, "4 points · Enter or double-click to finish")
side07 = side06.replace('glyphs=True', '').replace("1 selected", "Drawing polygon")
side07 = f"""
        {heading("Shape", "Drawing polygon")}
        {toggles([("x:rect", "Rectangle"), ("x:rounded", "Rounded Rectangle"), ("x:ellipse", "Ellipse"), ("x:polygon", "Polygon")], 3, glyphs=True)}
        {field("Outline", swatches(selected="coral", second_row=False))}
        {field("Fill", swatches(selected="coral", none=True, paper=True, custom=False, eyedrop=False, second_row=False))}
        {opacity_field(14).replace("Opacity", "Fill opacity")}
        {row_line("Outline width", spin("6 px"))}
        <div class="card-note">{icon("info")}<span>Click to add a point. <b>Enter</b> or double-click finishes, clicking the first point closes the shape, <b>Backspace</b> removes the last point, <b>Esc</b> cancels.</span></div>"""
s07 = editor("September dashboard", "1600 × 1000 · Drawing", "shape", canvas("d-tidewater-analytics", annos07, ov07), side07, tray_html=tray(current="d-tidewater-analytics"))
files["07-shape-polygon-drawing"] = P("Polygon in progress", s07)

# ---------------------------------------------------------------- 08 text edit in place
txt_lines = ["Runs nightly at 02:00", "on mains power only"]
annos08 = text_obj(600, 922, txt_lines, fs=34, color="var(--annota-coral)", outline="#ffffff", ow=6, weight=700)
# selection highlight on "02:00"
ov08 = (f'<div class="ov" style="left:{596*.5}px;top:{882*.5}px;width:{360*.5}px;height:{96*.5}px;outline:1px dashed var(--accent-bg);outline-offset:6px"></div>'
        f'<div class="ov" style="left:{851*.5}px;top:{890*.5}px;width:{102*.5}px;height:{44*.5}px;background:var(--accent-a50)"></div>'
        f'<span class="ov caret-line" style="left:{953*.5}px;top:{890*.5}px;height:{44*.5}px"></span>'
        + cursor(760 * .5, 970 * .5, "text") + chip(980 * .5, 880 * .5 - 2, "Esc or click outside to finish"))
side08 = f"""
        {heading("Text", "Editing")}
        <div class="hbox" style="--gap:6px"><span class="dropdown grow">Adwaita Sans</span>{spin("34", 110)}</div>
        <div class="fmt">
          <div class="linked"><button class="btn image-button active" title="Bold (Ctrl+B)"><b>B</b></button><button class="btn image-button" title="Italic (Ctrl+I)"><i style="font-family:serif;font-size:17px">I</i></button><button class="btn image-button" title="Underline (Ctrl+U)"><u>U</u></button><button class="btn image-button" title="Strikethrough"><s>S</s></button></div>
          <span class="spacer"></span>
          <div class="linked"><button class="btn image-button active" title="Align Left">{glyph("align-left")}</button><button class="btn image-button" title="Centre">{glyph("align-center")}</button><button class="btn image-button" title="Align Right">{glyph("align-right")}</button></div>
        </div>
        {field("Colour", swatches(selected="coral"))}
        {field("Outline", swatches(selected="paper", none=True, custom=False, eyedrop=False, second_row=False).replace('<span class="swatch selected" style="--c:var(--annota-coral)"', '<span class="swatch" style="--c:var(--annota-coral)"').replace('<div class="vbox" style="--gap:6px"><div class="swatch-row">', '<div class="vbox" style="--gap:6px"><div class="swatch-row"><span class="swatch none small" title="No Outline"></span><span class="swatch small light-ink selected" style="--c:var(--annota-paper)"></span>').replace('class="swatch"', 'class="swatch small"').replace('class="swatch selected"', 'class="swatch small"'))}
        {row_line("Outline width", spin("6 px"))}
        {opacity_field(100)}
        {shadow_row(False)}"""
s08 = editor("Backup settings", SUB, "text", canvas("a-harbor-backup", annos08, ov08), side08, tray_html=tray(current="a-harbor-backup"))
files["08-text-edit-in-place"] = P("Text edit in place", s08)

# ---------------------------------------------------------------- 09 callout with two tails
cb = (290, 420, 480, 130)
t1 = (880, 262, 600, 420)
t2 = (880, 330, 700, 420)
annos09 = callout(*cb, ["Both paths must be on", "the same backup disk"], [(*t1[:2], *t1[2:]), (*t2[:2], *t2[2:])],
                  fill="var(--annota-paper)", stroke="var(--annota-coral)", sw=5, fg="var(--annota-ink)", fs=30, rx=18, tail_w=40, weight=600)
annos09 = (f'<g filter="url(#sh)">'
           + callout(*cb, [], [(t1[0], t1[1], t1[2], t1[3]), (t2[0], t2[1], t2[2], t2[3])], fill="#ffffff", stroke="var(--annota-coral)", sw=5, rx=18, tail_w=40, shadow=False)
           + '</g>'
           + callout(*cb, ["Both paths must be on", "the same backup disk"], [], fill="none", fg="var(--annota-ink)", fs=30, rx=18, shadow=False, weight=600))
ov09 = (sel(cb[0], cb[1], cb[2], cb[3])
        + handle(t1[0], t1[1], "active", "Tail tip") + handle(t2[0], t2[1], "active", "Tail tip")
        + handle(cb[0] + cb[2] / 2, cb[1] + cb[3] + 46, "add", "Drag to add a tail")
        + chip(t2[0] * .5 + 12, t2[1] * .5 - 2, "Tail 2"))
side09 = f"""
        {heading("Callout", "1 selected · 2 tails")}
        {toggles([("x:rect", "Rectangle"), ("x:rounded", "Rounded"), ("x:ellipse", "Ellipse"), ("x:cloud", "Cloud")], 1, glyphs=True)}
        {field("Fill", swatches(selected="paper", none=True))}
        {field("Outline", swatches(selected="coral", none=True, paper=False, custom=False, eyedrop=False, second_row=False))}
        <div class="grid2"><div class="cell" style="flex-direction:column;align-items:stretch"><span class="mini-label">Outline width</span>{spin("5 px", 132)}</div><div class="cell" style="flex-direction:column;align-items:stretch"><span class="mini-label">Padding</span>{spin("18 px", 132)}</div></div>
        <div class="hbox" style="--gap:6px"><span class="dropdown grow">Adwaita Sans</span>{spin("30", 110)}</div>
        <span class="hint">Drag <b>+</b> to add a tail. Drag a tail tip onto the body to remove it. The body grows with the text.</span>
        {shadow_row(True)}"""
s09 = editor("Backup settings", SUB, "callout", canvas("a-harbor-backup", annos09, ov09), side09, tray_html=tray(current="a-harbor-backup"))
files["09-callout-two-tails"] = P("Callout with two tails", s09)

# ---------------------------------------------------------------- 10 callout shapes gallery (ellipse, cloud, rect) on checkout
def cloud(cx, cy, w, h, fill="var(--annota-amber)", tip=None):
    bumps = []
    import math as m
    n = 11
    for i in range(n):
        a = 2 * m.pi * i / n
        bumps.append(f'<ellipse cx="{cx + m.cos(a) * w * .42:.0f}" cy="{cy + m.sin(a) * h * .36:.0f}" rx="{w * .2:.0f}" ry="{h * .26:.0f}" fill="{fill}"/>')
    return f'<g filter="url(#sh)">{"".join(bumps)}<ellipse cx="{cx}" cy="{cy}" rx="{w*.42}" ry="{h*.36}" fill="{fill}"/><path d="M{cx+30} {cy-h*.3} L{tip[0]} {tip[1]} L{cx+90} {cy-h*.25}Z" fill="{fill}"/></g>'

annos10 = (callout(560, 140, 500, 96, ["Postcode needs 5 digits"], [(470, 520, 700, 236)], fs=32, shape="rect")
           + callout(1010, 530, 280, 120, ["Free", "shipping!"], [(1478, 668, 1270, 610)], fs=30, shape="ellipse", fill="var(--annota-teal)")
           + cloud(1200, 935, 300, 100, tip=(1320, 845))
           + f'<text x="1200" y="935" dy=".35em" text-anchor="middle" style="font-family:var(--font-ui);font-weight:700;font-size:28px" fill="var(--annota-ink)">Pay here</text>')
ov10 = sel(1040, 880, 320, 112) + handle(1320, 845, "active")
side10 = f"""
        {heading("Callout", "1 selected · 1 tail")}
        {toggles([("x:rect", "Rectangle"), ("x:rounded", "Rounded"), ("x:ellipse", "Ellipse"), ("x:cloud", "Cloud")], 3, glyphs=True)}
        {field("Fill", swatches(selected="amber", none=True))}
        {field("Outline", swatches(selected="none", none=True, paper=False, custom=False, eyedrop=False, second_row=False).replace('class="swatch selected"', 'class="swatch"'))}
        <div class="grid2"><div class="cell" style="flex-direction:column;align-items:stretch"><span class="mini-label">Outline width</span>{spin("0 px", 132)}</div><div class="cell" style="flex-direction:column;align-items:stretch"><span class="mini-label">Padding</span>{spin("24 px", 132)}</div></div>
        {shadow_row(True)}
        {select_all_btn("Callouts")}"""
s10 = editor("Checkout flow", SUB, "callout", canvas("b-lumen-checkout", annos10, ov10), side10)
files["10-callout-shapes"] = P("Callout shapes", s10)

# ---------------------------------------------------------------- 11 steps + type dropdown
steps11 = [(258, 196, "1"), (258, 286, "2"), (258, 372, "3"), (258, 492, "4")]
steps11 = [(320, 470, "1"), (860, 470, "2"), (860, 640, "3"), (500, 640, "4")]
steps11 = [(600, 256, "1"), (600, 492, "2"), (600, 604, "3"), (1270, 814, "4")]
annos11 = "".join(step(x, y, l, r=28) for x, y, l in steps11)
annos11 += f'<g opacity=".6">{step(1450, 680, "5", r=28, shadow=False)}</g>'
ov11 = cursor(1450 * .5 + 8, 680 * .5 + 8, "cross") + chip(1450 * .5 - 60, 680 * .5 - 46, "Click to place 5")
pop11 = f"""<div class="popover" style="left:{980+12}px;top:{47+18+28+70+38}px;width:276px">
      <div class="menu-item selected"><span class="label">1, 2, 3</span>{icon("check")}</div>
      <div class="menu-item hover"><span class="label">A, B, C</span></div>
      <div class="menu-item"><span class="label">a, b, c</span></div>
      <div class="menu-item"><span class="label">I, II, III</span></div>
    </div>"""
def step_sidebar(sub, seq="1, 2, 3", shape_active=0, nxt="5", value=None, colour="coral"):
    val = ""
    if value is not None:
        val = row_line("Value", f'<span class="entry focus" style="width:120px">{value}</span>', sub="This marker only")
    return f"""
        {heading("Step", sub)}
        {field("Sequence", f'<span class="dropdown full">{seq}</span>')}
        {field("Shape", toggles([("i:step", "Circle"), ("x:rect", "Square"), ("x:rounded", "Rounded Square"), ("i:pin", "Pin")], shape_active, glyphs=True))}
        {field("Colour", swatches(selected=colour))}
        {row_line("Size", spin("28 px"))}
        {val}
        {row_line("Next value", spin(nxt, 120))}
        <button class="btn full" style="justify-content:center">Restart Sequence</button>
        {shadow_row(True)}"""
side11 = step_sidebar("Tool defaults")
s11 = editor("Backup settings", SUB, "step", canvas("a-harbor-backup", annos11, ov11), side11, overlays_win=pop11, tray_html=tray(current="a-harbor-backup"))
files["11-step-sequence-type"] = P("Step markers, sequence type", s11)

# ---------------------------------------------------------------- 12 steps: edit value, restart, letters, shapes
annos12 = (step(600, 256, "1", r=28) + step(600, 492, "2", r=28) + step(600, 604, "3", r=28)
           + step(1250, 230, "A", r=26, shape="rounded", color="var(--annota-azure)")
           + step(1250, 330, "B", r=26, shape="rounded", color="var(--annota-azure)")
           + step(1250, 486, "7", r=24, shape="pin", color="var(--annota-violet)"))
ov12 = sel(1220, 434, 60, 104) + chip(1280 * .5 + 0, 450 * .5 - 30, "Manual value")
side12 = step_sidebar("1 selected", seq="1, 2, 3", shape_active=3, nxt="8", value="7", colour="violet")
s12 = editor("Backup settings", SUB, "select", canvas("a-harbor-backup", annos12, ov12), side12, tray_html=tray(current="a-harbor-backup"))
files["12-step-edit-value-shapes"] = P("Step markers, edit value", s12)

# ---------------------------------------------------------------- 13 renumber on delete
annos13 = (step(600, 256, "1", r=28) + step(600, 604, "2", r=28) + step(1270, 814, "3", r=28)
           + f'<g opacity=".25">{step(600, 492, "2", r=28, shadow=False)}</g>')
ov13 = chip(600 * .5 - 90, 492 * .5 - 12, "Deleted") + chip(600 * .5 - 76, 604 * .5 - 12, "3 → 2") + chip(1270 * .5 + 22, 814 * .5 - 12, "4 → 3")
side13 = step_sidebar("Tool defaults", nxt="4")
s13 = editor("Backup settings", SUB, "step", canvas("a-harbor-backup", annos13, ov13), side13,
             overlays_win=toast("Step 2 deleted · 2 steps renumbered", "Undo"), tray_html=tray(current="a-harbor-backup"))
files["13-step-renumber-on-delete"] = P("Step renumber on delete", s13)

# ---------------------------------------------------------------- 14 highlighter
annos14 = (highlight(1146, 280, 176, 32, opacity=.45) + highlight(102, 514, 434, 64, opacity=.45)
           + highlight(1062, 626, 432, 82, color="rgb(139 195 74)", opacity=.4))
ov14 = sel(1146, 280, 176, 32) + cursor(1322 * .5, 312 * .5, "cross")
side14 = f"""
        {heading("Highlighter", "1 selected")}
        {field("Colour", swatches(selected="amber").replace('style="--c:var(--annota-amber)"', 'style="--c:rgb(255 222 0)"'))}
        {opacity_field(45)}
        {row_line("Blend", '<span class="dropdown" style="width:150px">Multiply</span>')}
        <div class="card-note">{icon("info")}<span>Multiply keeps text under the highlight readable. Drag to mark a rectangle; hold <b>Shift</b> for a single text line height.</span></div>
        {select_all_btn("Highlights")}"""
s14 = editor("Checkout flow", SUB, "highlighter", canvas("b-lumen-checkout", annos14, ov14), side14)
files["14-highlighter"] = P("Highlighter", s14)

# ---------------------------------------------------------------- 15 redact: blur / pixelate / solid
def region(asset, x, y, w, h, radius="0"):
    return (f'<div class="ov" style="left:{x*.5}px;top:{y*.5}px;width:{w*.5}px;height:{h*.5}px;border-radius:{radius};'
            f'background:url(assets/{asset}) -{x*.5}px -{y*.5}px / 800px 500px no-repeat"></div>')

ov15 = (region("e-relay-chat-blur.png", 492, 158, 300, 30)
        + region("e-relay-chat-pixelate.png", 1138, 254, 78, 26)
        + region("e-relay-chat-pixelate.png", 698, 277, 86, 26)
        + region("e-relay-chat-pixelate.png", 1316, 196, 220, 26)
        + region("e-relay-chat-pixelate.png", 492, 370, 130, 30)
        + f'<div class="ov" style="left:{934*.5}px;top:{510*.5}px;width:{110*.5}px;height:{24*.5}px;background:var(--annota-ink)"></div>'
        + f'<div class="ov" style="left:{1318*.5}px;top:{244*.5}px;width:{136*.5}px;height:{24*.5}px;background:var(--annota-ink)"></div>'
        + f'<div class="ov" style="left:{576*.5}px;top:{606*.5}px;width:{106*.5}px;height:{24*.5}px;background:var(--annota-ink)"></div>'
        + region("e-relay-chat-blur.png", 1406, 22, 92, 92, "50%")
        + sel(1138, 254, 78, 26))
side15 = f"""
        {heading("Redact", "1 selected")}
        {field("Type", toggles(["Blur", "Pixelate", "Solid"], 1))}
        {field("Intensity", scale(70, "70"))}
        {field("Shape", toggles([("x:rect", "Rectangle"), ("x:ellipse", "Ellipse")], 0, glyphs=True))}
        {field("Solid colour", swatches(selected="ink", second_row=False).replace('selected"', '" ').replace('<span class="swatch "', '<span class="swatch"')).replace('class="field"', 'class="field" style="opacity:.5"')}
        <div class="card-note warn">{icon("warning")}<span>Blur and pixelate can sometimes be reversed. For passwords and personal data use <b>Solid</b>. Annota asks again before export.</span></div>
        {select_all_btn("Redactions")}"""
s15 = editor("Event chat", SUB, "blur", canvas("e-relay-chat", "", ov15), side15, tray_html=tray(current=None))
files["15-redact-blur-pixelate-solid"] = P("Redaction", s15)

# ---------------------------------------------------------------- 16 colour picker popover
annos16 = arrow(1240, 610, 660, 560, w=8, ctrl=(980, 790), color="#2f6fd0")
ov16 = handle(1240, 610, "big") + handle(660, 560, "big")
pick = f"""<div class="popover picker arrow-right" style="left:{980-312}px;top:{47+18+18+20}px;--arrow-y:58px">
      <div class="stack">
        <div class="sv"><span class="k" style="left:84%;top:18%;background:#2f6fd0"></span></div>
        <div class="hue"><span class="k" style="left:60%"></span></div>
        <div class="alpha" style="background-color:var(--canvas-checker-a)"><span class="k" style="left:100%"></span></div>
        <div class="vals">
          <span class="entry focus">#2F6FD0</span><span class="entry">47</span><span class="entry">111</span><span class="entry">208</span>
          <span class="cap">Hex</span><span class="cap">R</span><span class="cap">G</span><span class="cap">B</span>
        </div>
        <div class="hbox" style="--gap:6px">
          <button class="btn" title="Pick Colour From Screen">{icon("eyedropper")}<span>Pick From Screen</span></button>
          <span class="spacer"></span>
          <span class="swatch small" style="--c:#2f6fd0"></span><span class="swatch small" style="--c:#c64600"></span><span class="swatch small" style="--c:#0a7f5a"></span>
        </div>
      </div>
    </div>"""
side16 = arrow_sidebar(colour="none").replace('<span class="swatch custom"', '<span class="swatch custom selected"', 1)
side16 = side16.replace('<span class="swatch custom selected" title="Custom Colour…"></span>', '<span class="swatch selected" style="--c:#2f6fd0" title="#2F6FD0"></span><span class="swatch custom" title="Custom Colour…" style="box-shadow:0 0 0 2px var(--window-bg), 0 0 0 4px var(--fill-3)"></span>')
s16 = editor("Backup settings", SUB, "arrow", canvas("a-harbor-backup", annos16, ov16), side16, overlays_win=pick, tray_html=tray(current="a-harbor-backup"))
files["16-colour-picker"] = P("Colour picker", s16)

# ---------------------------------------------------------------- 17 eyedropper active (screen pick)
loupe = f"""<div class="loupe" style="left:712px;top:372px">
      <img class="px" src="{C.SAMPLES}/a-harbor-backup.png" alt="" style="width:{1600*4.5}px;height:{1000*4.5}px;max-width:none;left:{-(1150*4.5)+66}px;top:{-(814*4.5)+66}px;position:absolute">
      <span class="cell"></span>
    </div>
    <div class="chip" style="position:absolute;left:700px;top:512px;z-index:41">#3584E4 · Click to pick · Esc cancels</div>
    <svg class="cursor" style="position:absolute;left:691px;top:507px;width:22px;height:22px;z-index:42" viewBox="0 0 22 22"><path d="M2.5 19.5l1-4 9-9 3 3-9 9z" fill="#fff" stroke="#000" stroke-width="1.4" stroke-linejoin="round"/><path d="M12 4l2.5-2.5a2 2 0 0 1 3 0l3 3a2 2 0 0 1 0 3L18 10z" fill="#000" stroke="#fff" stroke-width="1"/></svg>"""
side17 = side16.replace('<span class="swatch eyedrop" title="Pick Colour From Screen">', '<span class="swatch eyedrop" style="background:var(--accent-bg);color:#fff" title="Pick Colour From Screen">')
s17 = editor("Backup settings", "Pick a colour anywhere on the screen", "arrow", canvas("a-harbor-backup", annos16, ov16), side17,
             overlays_win=loupe, tray_html=tray(current="a-harbor-backup"))
files["17-eyedropper-active"] = P("Eyedropper", s17)

# ---------------------------------------------------------------- 18 shadow editor
shadow_inner = f"""<div class="expander-content" style="padding:12px;display:flex;flex-direction:column;gap:12px">
              {field("Colour", swatches(selected="ink", second_row=False).replace('class="swatch"', 'class="swatch small"').replace('class="swatch selected"', 'class="swatch small selected"'))}
              <div class="hbox" style="--gap:12px"><span class="angle-dial" style="--a:45deg"></span><div class="vbox grow" style="--gap:6px"><span class="mini-label">Angle</span>{spin("45°", 150)}</div></div>
              {field("Distance", scale(30, "9 px"))}
              {field("Blur", scale(40, "12 px"))}
              {field("Opacity", scale(45, "45 %"))}
            </div>"""
annos18 = callout(840, 140, 600, 120, ["Keep 30 versions"], [(1180, 560, 1100, 260)], fs=40, fill="var(--annota-coral)").replace('filter="url(#sh)"', 'filter="url(#sh-big)"')
ov18 = sel(840, 140, 600, 120)
side18 = f"""
        {heading("Callout", "1 selected")}
        {field("Fill", swatches(selected="coral", second_row=False))}
        {shadow_row(True, expanded=True, inner=shadow_inner)}
        <button class="btn full" style="justify-content:center">Reset Shadow</button>"""
s18 = editor("Backup settings", SUB, "select", canvas("a-harbor-backup", annos18, ov18), side18, tray_html=tray(current="a-harbor-backup"))
files["18-shadow-editor"] = P("Shadow editor", s18)

# ---------------------------------------------------------------- 19 export warning for reversible redaction
dlg19 = f"""<div class="dialog-host" style="z-index:30">
    <div class="dialog alert" style="width:440px;max-width:440px">
      <div class="alert-body">
        <span class="alert-heading">Redactions May Be Reversible</span>
        <span>This capture has 6 blurred or pixelated areas. Someone could partly restore what is underneath. Solid boxes hide it completely.</span>
      </div>
      <div class="alert-actions" style="flex-direction:column;gap:12px">
        <button class="btn suggested-action" style="width:100%">Convert to Solid and Export</button>
        <button class="btn" style="width:100%">Export As Is</button>
        <button class="btn" style="width:100%">Cancel</button>
      </div>
    </div>
  </div>"""
s19 = editor("Event chat", SUB, "select", canvas("e-relay-chat", "", ov15.replace(sel(1138, 254, 78, 26), "")), side15.replace("1 selected", "8 in document"),
             overlays_win=dlg19, tray_html=tray(current=None))
files["19-export-redaction-warning"] = P("Export warning", s19)

# ---------------------------------------------------------------- 20 narrow tool properties (text) in bottom sheet + dark-content
C.ZOOM = 0.375
annos20 = text_obj(600, 922, txt_lines, fs=34, color="var(--annota-coral)", outline="#ffffff", ow=6) + step(600, 256, "1", r=28) + arrow(1490, 420, 1222, 536, w=8)
narrow_canvas = f"""<div class="canvas-area" style="align-items:flex-start;padding-top:40px">
        <div class="canvas-image" style="width:600px;height:375px">
          <img src="{C.SAMPLES}/a-harbor-backup.png" alt="">
          <svg class="annotations" viewBox="0 0 1600 1000">{SVG_DEFS}{annos20}</svg>
          {sel(590, 880, 370, 104)}
        </div>
      </div>"""
sheet20 = f"""<div class="bottom-sheet" style="padding:22px 18px 18px;left:52px">
      <div class="hbox" style="margin-bottom:12px"><span class="heading grow">Text</span><button class="btn flat">Select All Text</button></div>
      <div class="vbox" style="--gap:12px">
        <div class="hbox" style="--gap:6px"><span class="dropdown grow">Adwaita Sans</span>{spin("34", 110)}
          <div class="linked"><button class="btn image-button active"><b>B</b></button><button class="btn image-button"><i style="font-family:serif;font-size:17px">I</i></button><button class="btn image-button"><u>U</u></button><button class="btn image-button"><s>S</s></button></div></div>
        {swatches(second_row=False)}
        <div class="hbox" style="--gap:12px"><span class="grow">Outline</span><span class="swatch small light-ink selected" style="--c:var(--annota-paper)"></span>{spin("6 px")}</div>
      </div>
    </div>"""
s20 = f"""<div class="window" style="width:700px;height:800px">
  {headerbar("Backup settings", SUB, narrow=True)}
  <div class="split-view">{tool_strip("text")}<div class="editor-col">{narrow_canvas}</div></div>
  {sheet20}
</div>"""
files["20-narrow-text-properties"] = P("Narrow, text properties", s20, size=(700, 800))
C.ZOOM = 0.5

for k, v in files.items():
    (OUT / f"{k}.html").write_text(v, encoding="utf-8")
print("\n".join(files))
