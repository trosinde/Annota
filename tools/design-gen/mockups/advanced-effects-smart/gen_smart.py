import sys
sys.path.insert(0, __file__.rsplit("/", 1)[0])
from lib import *  # noqa

D = "/home/claude/annota/docs/design/add-smart-editing"
A = "../assets"
H = f"{SAMPLE}/a-harbor-backup.png"

EXTRA = """
  .det { fill: none; stroke: var(--annota-azure); stroke-width: 2; stroke-dasharray: 6 4; }
  .det.text { stroke: var(--annota-violet); }
  .det.hover { stroke-dasharray: none; stroke-width: 3; fill: rgb(47 128 237 / 10%); }
  .legend { display: flex; flex-direction: column; gap: 6px; font-size: var(--fs-small); }
  .legend span { display: inline-flex; align-items: center; gap: 8px; }
  .legend i { width: 18px; height: 12px; border-radius: 2px; border: 2px dashed var(--annota-azure); }
  .legend i.t { border-color: var(--annota-violet); }
  .pal { display: flex; flex-direction: column; gap: 6px; }
  .pal .row { min-height: 50px; }
  .pal-dots { display: inline-flex; gap: 3px; }
  .pal-dots b { width: 14px; height: 14px; border-radius: 4px; box-shadow: inset 0 0 0 1px rgb(0 0 6 / 15%); }
  .ba-label { position: absolute; top: 10px; z-index: 3; font-size: 12px; font-weight: 700; padding: 3px 10px; border-radius: 999px; background: var(--osd-bg); color: #fff; }
  .split-canvas { display: flex; gap: 18px; align-items: center; }
  .split-canvas .canvas-image { position: relative; }
  .busy { position: absolute; inset: 0; display: grid; place-items: center; z-index: 3; }
  .busy .osd-toolbar { gap: 10px; padding: 10px 16px; }
  .drag-ghost { filter: drop-shadow(0 6px 10px rgb(0 0 6 / 35%)); }
"""


def ed(name, **kw):
    kw.setdefault("extra_css", EXTRA)
    write(D, name, editor(**kw))


# ---- detected elements in 1600×1000 image coordinates (measured from a-harbor-backup.png)
UI = [  # (x0,y0,x1,y1)
    (1314, 133, 1337, 154), (479, 137, 492, 150),                   # close, search
    (1181, 259, 1196, 271), (1181, 318, 1196, 330), (1195, 376, 1201, 387),  # folder, folder, chevron
    (1043, 486, 1194, 499), (1158, 534, 1205, 559), (1100, 588, 1205, 621),  # dropdown, switch, spin
    (658, 730, 1201, 739),                                          # usage bar
    (992, 798, 1077, 831), (1090, 798, 1217, 831),                  # cancel, start backup
    (256, 213, 503, 250),                                           # selected nav row
]
TEXT = [
    (336, 137, 423, 149), (903, 138, 956, 151),                     # titles
    (645, 188, 715, 200), (645, 210, 1031, 222),                    # locations head + desc
    (655, 249, 749, 282), (654, 309, 757, 341), (655, 367, 831, 400),
    (645, 439, 712, 450), (655, 487, 719, 498), (655, 531, 902, 564), (655, 590, 898, 622),
    (645, 661, 775, 676), (659, 707, 811, 719), (933, 707, 1200, 721), (659, 747, 993, 756),
    (294, 184, 350, 198), (294, 264, 360, 278), (294, 304, 349, 320), (294, 344, 336, 358),
]
NAV_ICONS = [(266, 184, 281, 199), (266, 264, 281, 279), (266, 304, 281, 319), (266, 344, 281, 359)]
CARDS = [(639, 234, 1220, 414), (639, 465, 1220, 637), (639, 688, 1220, 777)]
COARSE = CARDS + [(250, 120, 509, 880), (980, 790, 1225, 840), (640, 180, 1040, 225), (640, 432, 720, 455), (640, 655, 780, 680)]

SCALE = 0.5


def boxes(lst, cls="det", pad=4, skip=None):
    out = []
    for b in lst:
        if skip and b == skip:
            continue
        x0, y0, x1, y1 = [v * SCALE for v in b]
        p = pad * SCALE
        out.append(f'<rect class="{cls}" x="{x0-p:.1f}" y="{y0-p:.1f}" width="{x1-x0+2*p:.1f}" height="{y1-y0+2*p:.1f}" rx="3" vector-effect="non-scaling-stroke"/>')
    return "".join(out)


BTN = (1090, 798, 1217, 831)


def smart_side(detail=55, count="31 elements · 19 text blocks", busy=False, sel=None):
    stat = f'<div class="boxed-list">{row("Found", "", count)}</div>' if not busy else \
        f'<div class="boxed-list">{row("Analysing image…", "<span class=\"spinner\"></span>", "Runs on this computer")}</div>'
    seltxt = ""
    if sel:
        seltxt = group("Selected Element", f'''<div class="boxed-list">{row(sel[0], '', sel[1])}</div>
<div class="hbox" style="--gap:6px"><button class="btn text-button grow">Delete</button><button class="btn text-button grow">Duplicate</button></div>''')
    return f"""<div class="side-head">{icon('select')}<span class="heading">Smart Move</span>{switch(True)}</div>
{group('Detail', scale('Detail', detail, f'{detail}', marks=[0,50,100]) + '<div class="hbox" style="justify-content:space-between;padding:0 2px 0 96px"><span class="caption dim-label">Large blocks</span><span class="caption dim-label">Single controls</span></div>', desc='How finely the image is split into objects')}
{stat}
<div class="legend"><span><i></i>UI element</span><span><i class="t"></i>Text block</span></div>
{seltxt}
<div class="hint">Drag an element to move it. Its old place is filled from the surrounding background. <kbd class="key">Delete</kbd> removes it.</div>"""


def smart_toggle_header():
    return ""


# ------------------------------------------------------------ 01 smart move analysing
busy = f'<div class="busy"><div class="osd-toolbar"><span class="spinner"></span><span class="label" style="font-weight:600">Detecting elements…</span><button class="btn text-button" style="min-height:28px">Cancel</button></div></div>'
canvas = image(H, inner=f'<div style="position:absolute;inset:0;background:rgb(255 255 255 / 35%)"></div>{busy}')
ed("01-smart-move-analysing", title="Harbor preferences", subtitle="1600 × 1000", tool="select", canvas=canvas,
   sidebar=smart_side(busy=True), tooltip=False, page_title="smart move analysing")

# ------------------------------------------------------------ 02 detected elements, default detail
svg = boxes(UI) + boxes(TEXT, "det text", 3) + boxes([BTN], "det hover")
tip = '<span class="tooltip" style="left:520px;top:432px">Button · drag to move</span>'
canvas = image(H, svg=svg, inner=tip)
ed("02-smart-move-detected", title="Harbor preferences", subtitle="1600 × 1000", tool="select", canvas=canvas,
   sidebar=smart_side(), tooltip=False, page_title="smart move detected")

# ------------------------------------------------------------ 03 low detail
svg = boxes(COARSE, pad=6)
canvas = image(H, svg=svg)
ed("03-smart-move-low-detail", title="Harbor preferences", subtitle="1600 × 1000", tool="select", canvas=canvas,
   sidebar=smart_side(detail=15, count="9 elements · 0 text blocks"), tooltip=False, page_title="smart move low detail")

# ------------------------------------------------------------ 04 dragging the button (auto-filled hole)
nx, ny = 700, 798   # new position in image coords
bx0, by0, bx1, by1 = BTN
ghost = f'<img class="drag-ghost" src="{A}/harbor-button.png" style="position:absolute;left:{(nx-2)*SCALE}px;top:{(ny-2)*SCALE}px;width:{(bx1-bx0+5)*SCALE}px;height:{(by1-by0+5)*SCALE}px">'
svg = (boxes([u for u in UI if u != BTN]) + boxes(TEXT, "det text", 3)
       + f'<rect x="{bx0*SCALE-2}" y="{by0*SCALE-2}" width="{(bx1-bx0)*SCALE+4}" height="{(by1-by0)*SCALE+4}" rx="3" fill="none" stroke="var(--annota-azure)" stroke-width="1.5" stroke-dasharray="2 3"/>'
       + f'<path d="M{bx0*SCALE-6} {by0*SCALE+8} L{(nx+bx1-bx0)*SCALE+10} {ny*SCALE+8}" stroke="var(--annota-azure)" stroke-width="1" stroke-dasharray="3 3" opacity=".7"/>'
       + f'<line x1="{(nx+(bx1-bx0)/2)*SCALE}" y1="{(234)*SCALE}" x2="{(nx+(bx1-bx0)/2)*SCALE}" y2="{(by1+6)*SCALE}" stroke="var(--annota-coral)" stroke-width="1" opacity="0"/>')
inner = ghost + f'<span class="measure" style="position:absolute;left:{nx*SCALE}px;top:{(ny+44)*SCALE}px;font-size:11px;font-weight:700;color:#fff;background:var(--accent-bg);padding:2px 6px;border-radius:4px">x 700 · y 798</span>'
canvas = image(f"{A}/harbor-autofill.png", svg=svg, inner=inner)
ed("04-smart-move-dragging", title="Harbor preferences", subtitle="1600 × 1000 · Edited", tool="select", canvas=canvas,
   sidebar=smart_side(sel=("Button", "127 × 33 px · from x 1090, y 798")), tooltip=False, page_title="smart move dragging")

# ------------------------------------------------------------ 05 moved result (smart move off, plain view)
inner = f'<img src="{A}/harbor-button.png" style="position:absolute;left:{(nx-2)*SCALE}px;top:{(ny-2)*SCALE}px;width:{(bx1-bx0+5)*SCALE}px;height:{(by1-by0+5)*SCALE}px">'
svg = handles_svg((nx - 2) * SCALE, (ny - 2) * SCALE, (bx1 - bx0 + 5) * SCALE, (by1 - by0 + 5) * SCALE)
canvas = image(f"{A}/harbor-autofill.png", svg=svg, inner=inner)
ed("05-smart-move-result", title="Harbor preferences", subtitle="1600 × 1000 · Edited", tool="select", canvas=canvas,
   sidebar=smart_side(sel=("Button", "Moved 390 px left · background filled")),
   overlay=toast("Moved “Start backup” · background filled"), tooltip=False, page_title="smart move result")


# ------------------------------------------------------------ simplify drawing
PALETTES = {
    "Annota Neutral": dict(bg="#e4e4e8", win="#ffffff", side="#f0f0f3", card="#ffffff", line="#d4d4da", text="#8a8a94", head="#4a4a52", accent="#ff5a4e", ctrl="#c4c4cc", sel="#dcdce2", desk="#cfcfd6"),
    "Coral": dict(bg="#ffe9e6", win="#ffffff", side="#fff4f2", card="#ffffff", line="#ffd2cc", text="#ffaaa1", head="#e0463b", accent="#ff5a4e", ctrl="#ffc1b9", sel="#ffdcd7", desk="#ffd9d4"),
    "Azure": dict(bg="#e3eefc", win="#ffffff", side="#eff5fe", card="#ffffff", line="#c9dcf7", text="#98bbec", head="#2f6fc9", accent="#2f80ed", ctrl="#b4cff4", sel="#d4e4fa", desk="#cfe0f8"),
    "Ink on Paper": dict(bg="#f4f4f4", win="#ffffff", side="#fafafa", card="#ffffff", line="#1f1f24", text="#1f1f24", head="#1f1f24", accent="#1f1f24", ctrl="#ffffff", sel="#e8e8e8", desk="#ffffff", outline=True),
}


def bar(x0, y0, x1, y1, c, r=None):
    h = max(6, (y1 - y0) * .7)
    cy = (y0 + y1) / 2
    r = h / 2 if r is None else r
    return f'<rect x="{x0}" y="{cy-h/2:.1f}" width="{x1-x0}" height="{h:.1f}" rx="{r:.1f}" fill="{c}"/>'


def simplify_svg(p, detail=60):
    o = p.get("outline")
    st = f' stroke="{p["line"]}" stroke-width="2"' if o else ""
    s = [f'<rect width="1600" height="1000" fill="{p["desk"]}"/>',
         f'<rect x="250" y="120" width="1100" height="760" rx="12" fill="{p["win"]}"{st}/>',
         f'<path d="M262 120h247v760H262a12 12 0 0 1-12-12V132a12 12 0 0 1 12-12z" fill="{p["side"]}"{st}/>',
         f'<line x1="509" y1="167" x2="1350" y2="167" stroke="{p["line"]}" stroke-width="2"/>',
         bar(336, 137, 423, 150, p["head"]), bar(903, 138, 956, 151, p["head"]),
         f'<circle cx="1325" cy="143" r="12" fill="{p["ctrl"]}"{st}/>',
         f'<circle cx="485" cy="143" r="8" fill="{p["ctrl"]}"/>',
         f'<rect x="256" y="213" width="247" height="37" rx="8" fill="{p["sel"]}"/>']
    for (x0, y0, x1, y1), t in zip(NAV_ICONS, [(294, 350), (294, 360), (294, 349), (294, 336)]):
        s.append(f'<rect x="{x0}" y="{y0}" width="15" height="15" rx="4" fill="{p["ctrl"]}"/>')
        s.append(bar(t[0], y0, t[1], y1, p["text"]))
    s.append(bar(294, 224, 346, 239, p["head"]))
    s.append(f'<rect x="266" y="224" width="15" height="15" rx="4" fill="{p["head"]}"/>')
    for x0, y0, x1, y1 in CARDS:
        s.append(f'<rect x="{x0}" y="{y0}" width="{x1-x0}" height="{y1-y0}" rx="12" fill="{p["card"]}" stroke="{p["line"]}" stroke-width="2"/>')
    for y in (295, 353, 517, 576):
        s.append(f'<line x1="641" y1="{y}" x2="1218" y2="{y}" stroke="{p["line"]}" stroke-width="2"/>')
    s += [bar(645, 188, 715, 201, p["head"]), bar(645, 210, 1031, 222, p["text"]),
          bar(645, 439, 712, 451, p["head"]), bar(645, 661, 775, 676, p["head"])]
    for (x0, y0, x1, y1) in [(655, 249, 749, 261), (655, 309, 732, 320), (655, 367, 772, 381), (655, 487, 719, 498),
                             (655, 531, 787, 543), (655, 595, 754, 606), (659, 707, 811, 719)]:
        s.append(bar(x0, y0, x1, y1, p["head"]))
    if detail >= 40:
        for (x0, y0, x1, y1) in [(655, 271, 735, 282), (655, 329, 757, 342), (655, 389, 830, 401), (655, 553, 902, 564), (655, 612, 898, 622), (933, 707, 1200, 721)]:
            s.append(bar(x0, y0, x1, y1, p["text"]))
        s.append(f'<rect x="1181" y="259" width="16" height="13" rx="3" fill="{p["ctrl"]}"/><rect x="1181" y="318" width="16" height="13" rx="3" fill="{p["ctrl"]}"/>')
        s.append(f'<path d="M1195 376l6 5.5-6 5.5" fill="none" stroke="{p["head"]}" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>')
    s.append(bar(1043, 486, 1176, 499, p["text"]))
    s.append(f'<rect x="1158" y="534" width="47" height="26" rx="13" fill="{p["ctrl"]}"/><circle cx="1171" cy="547" r="10" fill="#fff"/>')
    s.append(f'<rect x="1100" y="588" width="105" height="34" rx="8" fill="{p["ctrl"]}"/>')
    s.append(f'<rect x="658" y="730" width="543" height="10" rx="5" fill="{p["sel"]}"/><rect x="658" y="730" width="125" height="10" rx="5" fill="{p["accent"]}"/>')
    if detail >= 40:
        s.append(bar(659, 747, 993, 757, p["text"]))
    s.append(f'<rect x="992" y="798" width="86" height="34" rx="8" fill="{p["ctrl"]}"/>')
    s.append(f'<rect x="1090" y="798" width="128" height="34" rx="8" fill="{p["accent"]}"/>')
    return "".join(s)


def simplify_side(pal="Annota Neutral", detail=60, applied=False, sel=None):
    rows = []
    for name, p in PALETTES.items():
        dots = "".join(f'<b style="background:{p[k]}"></b>' for k in ("head", "text", "ctrl", "accent"))
        rows.append(row(name, f'<span class="pal-dots">{dots}</span>', "", "selected" if name == pal else "",
                        prefix=f'<span class="radio{" checked" if name == pal else ""}"></span>'))
    rows.append(row("New Palette…", "", "", "button-row"))
    body = f"""<div class="side-head">{icon('shape')}<span class="heading">Simplify</span></div>
{group('Detail', scale('Detail', detail, str(detail), marks=[0,50,100]) + '<div class="hint">Low keeps only headings and controls. High keeps every text line and icon.</div>')}
{group('Palette', f'<div class="boxed-list pal">{"".join(rows)}</div>', desc='Colours used for bars and shapes')}
"""
    if not applied:
        body += f"""<div class="boxed-list">{row('Show Original', switch(False), 'Compare with the capture while you adjust')}</div>"""
    if sel:
        body += group("Selected Shape", f'<div class="boxed-list">{row(sel[0], "", sel[1])}</div>')
    return body


def sfoot(applied):
    if applied:
        return f'<div class="foot"><button class="btn text-button">Revert to Original</button></div>'
    return f'<div class="foot"><button class="btn text-button">Cancel</button><button class="btn text-button suggested-action" title="Enter">Apply</button></div>'


# ------------------------------------------------------------ 06 simplify before/after preview (split)
p = PALETTES["Annota Neutral"]
before = f'<div class="canvas-image" style="width:400px;height:250px"><img src="{H}" style="display:block;width:100%;height:100%"><span class="ba-label" style="left:10px">Before</span></div>'
after = f'<div class="canvas-image" style="width:400px;height:250px"><span class="ba-label" style="left:10px">After · preview</span><svg class="annotations" viewBox="0 0 1600 1000" style="position:absolute;inset:0">{simplify_svg(p)}</svg></div>'
canvas = f'<div class="split-canvas" style="display:flex;gap:18px;align-items:center;position:relative">{before}{after}</div>'
ed("06-simplify-before-after", title="Harbor preferences", subtitle="1600 × 1000", tool="select", canvas=canvas,
   sidebar=simplify_side(), foot=sfoot(False), tooltip=False, page_title="simplify preview", zoom="24 %")

# ------------------------------------------------------------ 07 simplify applied, palette chooser open on Coral, one bar selected
p = PALETTES["Coral"]
x0, y0, x1, y1 = 655, 367, 772, 381
bx, by, bw, bh = x0 * SCALE - 4, y0 * SCALE - 4, (x1 - x0) * SCALE + 8, (y1 - y0) * SCALE + 8
sel = f'<rect x="{bx}" y="{by}" width="{bw}" height="{bh}" fill="none" stroke="var(--accent-bg)"/>' + ''.join(f'<circle cx="{cx}" cy="{cy}" r="4.5" fill="#fff" stroke="var(--accent-bg)" stroke-width="1.5"/>' for cx, cy in [(bx, by), (bx + bw, by), (bx, by + bh), (bx + bw, by + bh)])
canvas = f'''<div class="canvas-image" style="width:800px;height:500px"><svg class="annotations" viewBox="0 0 1600 1000" style="position:absolute;inset:0">{simplify_svg(p)}</svg>
  <svg class="annotations" viewBox="0 0 800 500">{sel}</svg></div>'''
ed("07-simplify-applied-coral", title="Harbor preferences", subtitle="1600 × 1000 · Simplified", tool="select", canvas=canvas,
   sidebar=simplify_side("Coral", applied=True, sel=("Text bar", "Rounded bar · 117 × 14 px · editable")), foot=sfoot(True),
   overlay=toast("Simplified · 46 shapes"), tooltip=False, page_title="simplify applied")

# ------------------------------------------------------------ 08 low detail ink palette
p = PALETTES["Ink on Paper"]
canvas = f'<div class="canvas-image" style="width:800px;height:500px"><svg class="annotations" viewBox="0 0 1600 1000" style="position:absolute;inset:0">{simplify_svg(p, detail=20)}</svg></div>'
ed("08-simplify-low-detail-ink", title="Harbor preferences", subtitle="1600 × 1000", tool="select", canvas=canvas,
   sidebar=simplify_side("Ink on Paper", detail=20), foot=sfoot(False), tooltip=False, page_title="simplify low detail")

# ------------------------------------------------------------ 09 new palette dialog
dialog = f"""<div class="dialog-host" style="position:fixed">
  <div class="dialog" style="width:460px">
    <div class="headerbar"><div class="start"><button class="btn text-button">Cancel</button></div>
      <div class="title-widget"><span class="title">New Palette</span></div>
      <div class="end"><button class="btn text-button suggested-action">Save</button></div></div>
    <div class="dialog-body vbox" style="--gap:18px;padding-top:12px">
      <div class="boxed-list"><div class="row entry-row focus"><div class="text"><span class="title" style="font-size:12px;opacity:.55">Name</span><span>Release notes</span></div></div></div>
      <div class="boxed-list">
        {''.join(row(n, f'<span class="sel-chip"><span class="dot" style="background:{c}"></span>{c.upper()}</span>') for n, c in [('Headings', '#14866d'), ('Text Lines', '#9ccfc2'), ('Controls', '#cfe7e1'), ('Accent', '#e5a50a'), ('Background', '#ffffff')])}
      </div>
      <div class="hint">Palettes are stored on this computer and appear in the Simplify sidebar.</div>
    </div>
  </div>
</div>"""
p = PALETTES["Annota Neutral"]
canvas = f'<div class="canvas-image" style="width:800px;height:500px"><svg class="annotations" viewBox="0 0 1600 1000" style="position:absolute;inset:0">{simplify_svg(p)}</svg></div>'
write(D, "09-simplify-new-palette", editor(title="Harbor preferences", subtitle="1600 × 1000", tool="select", canvas=canvas,
      sidebar=simplify_side(), foot=sfoot(False), overlay=dialog, tooltip=False,
      extra_css=EXTRA + " .sel-chip{display:inline-flex;align-items:center;gap:6px;height:26px;padding:0 8px;border-radius:6px;background:var(--fill-1);font-size:13px;font-variant-numeric:tabular-nums}.sel-chip .dot{width:14px;height:14px;border-radius:4px;box-shadow:inset 0 0 0 1px rgb(0 0 6/20%)}",
      page_title="new palette"))

# ------------------------------------------------------------ 10 Smart menu entry points (image menu)
menu = f"""<div class="popover" style="position:fixed;right:58px;top:46px;width:280px;--arrow-x:250px;z-index:40">
  <div class="menu-item"><span class="label">Effects…</span><span class="accel">Ctrl+Shift+E</span></div>
  <div class="menu-sep"></div>
  <div class="menu-section-title">Smart Editing</div>
  <div class="menu-item hover"><span class="label">Smart Move</span><span class="accel">Ctrl+Shift+M</span></div>
  <div class="menu-item"><span class="label">Simplify…</span><span class="accel">Ctrl+Shift+Y</span></div>
  <div class="menu-sep"></div>
  <div class="menu-item"><span class="label">Library</span><span class="accel">Ctrl+L</span></div>
  <div class="menu-item"><span class="label">Save</span><span class="accel">Ctrl+S</span></div>
  <div class="menu-item"><span class="label">Keyboard Shortcuts</span><span class="accel">Ctrl+?</span></div>
</div>"""
canvas = image(H)
side = f"""<div class="side-head">{icon('select')}<span class="heading">Select</span></div>
<div class="hint">Click an annotation to edit it. Smart Move turns the parts of the screenshot itself into movable objects.</div>"""
html = editor(title="Harbor preferences", subtitle="1600 × 1000", tool="select", canvas=canvas, sidebar=side,
              overlay=menu, tooltip=False, extra_css=EXTRA, page_title="smart menu")
html = html.replace('<button class="btn flat image-button" title="Main Menu">', '<button class="btn flat image-button active" title="Main Menu">')
write(D, "10-main-menu-entry", html)
print("ok")
