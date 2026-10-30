import sys
sys.path.insert(0, __file__.rsplit("/", 1)[0])
from lib import *  # noqa

D = "/home/claude/annota/docs/design/add-advanced-annotation-tools"
A = "../assets"
CORAL = "var(--annota-coral)"

EXTRA = """
  .stamp-grid { display: grid; grid-template-columns: repeat(5, 48px); gap: 6px; justify-content: space-between; }
  .stamp-cell { width: 48px; height: 48px; border-radius: var(--radius-s); display: grid; place-items: center; background: var(--fill-flat-hover); position: relative; }
  .stamp-cell svg { width: 36px; height: 36px; }
  .stamp-cell.selected { box-shadow: inset 0 0 0 2px var(--accent-bg); background: var(--accent-a12); }
  .stamp-cell.focus { outline: 2px solid var(--accent-a50); outline-offset: 1px; }
  .stamp-cell.add { background: transparent; box-shadow: inset 0 0 0 1px var(--borders); }
  .stamp-cell.add .icon { opacity: .7; }
  .chips { display: flex; flex-wrap: wrap; gap: 6px; }
  .chip { height: 28px; padding: 0 12px; border-radius: var(--radius-pill); background: var(--fill-1); display: inline-flex; align-items: center; gap: 6px; font-size: var(--fs-small); }
  .chip.active { background: var(--accent-bg); color: var(--accent-fg); }
  .chip .badge { height: 16px; min-width: 16px; font-size: 10px; }
  .edge-ico { width: 40px; height: 16px; display: block; }
  .lens { position: absolute; overflow: hidden; }
  .lens img { position: absolute; max-width: none; }
  .dim-layer { position: absolute; inset: 0; }
  .brush-cursor { position: absolute; border: 1.5px solid #1f1f24; box-shadow: 0 0 0 1px #fff; border-radius: 50%; }
  .measure { position: absolute; font-size: 11px; font-weight: 700; color: #fff; background: var(--accent-bg); padding: 2px 6px; border-radius: 4px; font-variant-numeric: tabular-nums; white-space: nowrap; }
  .sel-chip { display: inline-flex; align-items: center; gap: 6px; height: 26px; padding: 0 8px; border-radius: var(--radius-s); background: var(--fill-1); font-size: var(--fs-small); }
  .sel-chip .dot { width: 14px; height: 14px; border-radius: 4px; box-shadow: inset 0 0 0 1px rgb(0 0 6 / 20%); }
"""


def ed(name, **kw):
    kw.setdefault("extra_css", EXTRA)
    write(D, name, editor(**kw))


def head(ic, title, extra=""):
    return f'<div class="side-head">{icon(ic)}<span class="heading">{title}</span>{extra}</div>'


def footer(primary, secondary="Cancel", primary_kbd="Enter"):
    return f'<div class="foot"><button class="btn text-button" title="Esc">{secondary}</button><button class="btn text-button suggested-action" title="{primary_kbd}">{primary}</button></div>'


def text_only_row(on=False, disabled=False, detecting=False):
    sub = "Restricts the effect to detected text lines"
    suf = switch(on)
    cls = ""
    if disabled:
        sub = "Text recognition is not installed"
        suf = '<span class="switch disabled"></span>'
        cls = "disabled"
    if detecting:
        sub = "Detecting text…"
        suf = '<span class="spinner"></span>' + switch(True)
    return f'<div class="boxed-list">{row("Apply to Text Only", suf, sub, cls)}</div>'


# ------------------------------------------------------------------ 01 magnifier
src_x, src_y, src_r = 289, 336, 36      # source in 800×500 canvas coords (warning squiggle)
zoom = 3
lx, ly, lr = 610, 236, 108              # lens centre/radius
img_w, img_h = 800 * zoom, 500 * zoom
lens = f"""<div class="lens" style="left:{lx-lr}px;top:{ly-lr}px;width:{2*lr}px;height:{2*lr}px;border-radius:50%">
  <img src="{SAMPLE}/c-code-editor.png" style="width:{img_w}px;height:{img_h}px;left:{lr - src_x*zoom}px;top:{lr - src_y*zoom}px"></div>"""
import math
ang = math.atan2(ly - src_y, lx - src_x)
p1 = (src_x + src_r * math.cos(ang), src_y + src_r * math.sin(ang))
p2 = (lx - lr * math.cos(ang), ly - lr * math.sin(ang))
mag_svg = f"""
  <circle cx="{src_x}" cy="{src_y}" r="{src_r}" fill="none" stroke="{CORAL}" stroke-width="2" stroke-dasharray="5 4"/>
  <line x1="{p1[0]:.1f}" y1="{p1[1]:.1f}" x2="{p2[0]:.1f}" y2="{p2[1]:.1f}" stroke="{CORAL}" stroke-width="2.5"/>
  <circle cx="{lx}" cy="{ly}" r="{lr}" fill="none" stroke="{CORAL}" stroke-width="4"/>
  <circle cx="{lx}" cy="{ly}" r="{lr+5}" fill="none" stroke="rgb(0 0 6 / 25%)" stroke-width="6" opacity=".35"/>
  {''.join(f'<circle cx="{lx+dx}" cy="{ly+dy}" r="5" fill="#fff" stroke="var(--accent-bg)" stroke-width="1.5"/>' for dx,dy in [(lr,0),(-lr,0),(0,lr),(0,-lr)])}
  <circle cx="{src_x+src_r}" cy="{src_y}" r="5" fill="#fff" stroke="var(--accent-bg)" stroke-width="1.5"/>
"""
canvas = image(f"{SAMPLE}/c-code-editor.png", svg=mag_svg, inner=lens)
side = f"""{head('magnify', 'Magnifier')}
{group('Lens', f'''{toggles(['Circle','Rounded'],'Circle')}
{scale('Zoom', 29, '300 %', marks=[0,15.4,38.5,100])}
<div class="hint">125 % to 800 %. Scroll over the lens to change the zoom.</div>''')}
{group('Outline', f'''{swatches('coral')}
<div class="boxed-list">{row('Width', spin('4'))}{row('Shadow', switch(True))}</div>''')}
{group('Connector', f'''<div class="boxed-list">{row('Show Connector', switch(True), 'Line from source area to lens')}{row('Source Outline', '<span class="dropdown flat">Dashed</span>')}</div>
<div class="hint">Drag the lens to move it away from its source. Drag the dashed circle to change what is magnified.</div>''')}"""
ed("01-magnifier-offset-lens", title="scene.rs warning", subtitle="1600 × 1000 · Edited", tool="magnify",
   canvas=canvas, sidebar=side, page_title="magnifier")

# ------------------------------------------------------------------ 02 spotlight two spots
spots = [(132, 42, 156, 56), (569, 108, 215, 167)]  # x,y,w,h in 800 coords (Visitors KPI, Top routes)
mask = "".join(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="10" fill="#000"/>' for x, y, w, h in spots)
sp_svg = f"""<defs><mask id="m"><rect width="800" height="500" fill="#fff"/>{mask}</mask>
  <filter id="b"><feGaussianBlur stdDeviation="2"/></filter></defs>
  <rect width="800" height="500" fill="rgb(0 0 6 / 60%)" mask="url(#m)"/>
  {''.join(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="10" fill="none" stroke="#fff" stroke-opacity=".9" stroke-width="1.5"/>' for x,y,w,h in spots)}
  {handles_svg(*spots[1])}"""
canvas = image(f"{SAMPLE}/d-tidewater-analytics.png", svg=sp_svg)
spot_list = '<div class="boxed-list">' + row('Area 1', f'<button class="btn flat image-button" title="Remove">{icon("trash")}</button>', 'Rounded rectangle', prefix=icon('shape')) + row('Area 2', f'<button class="btn flat image-button" title="Remove">{icon("trash")}</button>', 'Rounded rectangle · selected', 'selected', prefix=icon('shape')) + '</div>'
side = f"""{head('spotlight', 'Spotlight')}
{group('Shape', toggles(['Rectangle','Rounded','Ellipse','Polygon'],'Rounded'))}
{group('Outside the Areas', f'''{scale('Dim', 67, '60 %')}{scale('Blur', 0, '0 px')}<div class="hint">Dim 0–90 %, blur 0–30 px. Both can be combined.</div>''')}
{group('Clear Areas', spot_list, desc='Draw more areas to keep them clear. Shift-drag adds an area.')}
{text_only_row(False)}"""
ed("02-spotlight-two-spots", title="September overview", subtitle="1600 × 1000 · Edited", tool="spotlight",
   canvas=canvas, sidebar=side, page_title="spotlight")

# ------------------------------------------------------------------ stamp sidebar helpers
def stamp_grid(items, sel=None, focus=None, add=False):
    cells = []
    for i, (name, fn) in enumerate(items):
        c = "stamp-cell" + (" selected" if name == sel else "") + (" focus" if name == focus else "")
        cells.append(f'<span class="{c}" title="{name}">{fn(36)}</span>')
    if add:
        cells.append(f'<span class="stamp-cell add" title="Import Stamp…">{icon("plus")}</span>')
    return f'<div class="stamp-grid">{"".join(cells)}</div>'


def chips(active, counts=None):
    names = ["All", "Status", "Numbers", "Pointers", "Faces", "Keys", "My Stamps"]
    out = []
    for n in names:
        b = f' <span class="badge neutral">{counts[n]}</span>' if counts and n in counts else ""
        out.append(f'<span class="chip{" active" if n == active else ""}">{n}{b}</span>')
    return f'<div class="chips">{"".join(out)}</div>'


def search(text="", focus=False, ph="Search stamps"):
    if text:
        return f'<span class="search-entry{" focus" if focus else ""}">{icon("search")}<span class="grow">{text}</span>{icon("edit-clear")}</span>'
    return f'<span class="search-entry">{icon("search")}<span class="placeholder">{ph}</span></span>'


def stamp_on(fn, x, y, s, sel=False):
    g = f'<g transform="translate({x} {y}) scale({s/48})">{fn(48)[fn(48).index(">")+1:-6]}</g>'
    if sel:
        g += handles_svg(x - 3, y - 3, s + 6, s + 6)
    return g


# ------------------------------------------------------------------ 03 stamp picker
cat = CATALOG
st_svg = (stamp_on(st_cross, 238, 255, 34) + stamp_on(st_check, 238, 175, 34) + stamp_on(st_check, 466, 175, 34)
          + stamp_on(lambda size=48: st_hand(size, 0, True), 522, 396, 40, sel=True))
canvas = image(f"{SAMPLE}/b-lumen-checkout.png", svg=st_svg)
side = f"""{head('stamp', 'Stamps')}
{search()}
{chips('All')}
{group('Status', stamp_grid(cat['Status'][:5], sel='Done'))}
{group('Numbers', stamp_grid(cat['Numbers'][:5]))}
{group('Pointers', stamp_grid(cat['Pointers'][:5], sel='Point left'))}
{group('Faces', stamp_grid(cat['Faces'][:5]))}"""
ed("03-stamp-picker", title="Checkout flow", subtitle="1600 × 1000 · Edited", tool="stamp",
   canvas=canvas, sidebar=side, page_title="stamps")

# ------------------------------------------------------------------ 04 stamp search
keys = [k for k in cat['Keys'] if True]
canvas_svg = (stamp_on(lambda size=48: st_key('Ctrl', size), 520, 300, 44) + stamp_on(lambda size=48: st_key('Shift', size), 572, 300, 44)
              + stamp_on(lambda size=48: st_key('C', size), 624, 300, 44, sel=True))
canvas = image(f"{SAMPLE}/c-code-editor.png", svg=canvas_svg)
side = f"""{head('stamp', 'Stamps')}
{search('key', focus=True)}
{chips('Keys', {'Keys': 12})}
{group('Keys', stamp_grid(cat['Keys'], sel='C', focus='Ctrl'), desc='12 stamps match “key”')}
{group('Size', scale('Size', 30, '44 px'))}
<div class="hint">Press <kbd class="key">Enter</kbd> to place the focused stamp at the canvas centre. Arrow keys move the focus.</div>"""
ed("04-stamp-search-keys", title="scene.rs warning", subtitle="1600 × 1000 · Edited", tool="stamp",
   canvas=canvas, sidebar=side, page_title="stamp search")

# ------------------------------------------------------------------ 05 stamp search empty
side = f"""{head('stamp', 'Stamps')}
{search('trophy', focus=True)}
{chips('All')}
<div class="status-page compact" style="padding:24px 6px">
  {icon('search','icon large status-icon')}
  <div class="status-title" style="font-size:20px">No Matching Stamps</div>
  <div class="status-description">Try another word, or import your own stamp.</div>
  <button class="btn text-button pill">Import Stamp…</button>
</div>"""
canvas = image(f"{SAMPLE}/b-lumen-checkout.png")
ed("05-stamp-search-empty", title="Checkout flow", subtitle="1600 × 1000", tool="stamp",
   canvas=canvas, sidebar=side, page_title="stamp search empty", tooltip=False)

# ------------------------------------------------------------------ 06 import dialog (file chooser result) + My Stamps empty
side = f"""{head('stamp', 'Stamps')}
{search()}
{chips('My Stamps', {'My Stamps': 0})}
<div class="status-page compact" style="padding:24px 6px">
  {icon('stamp','icon large status-icon')}
  <div class="status-title" style="font-size:20px">No Stamps of Your Own</div>
  <div class="status-description">Import a PNG or SVG, or turn the current selection into a stamp.</div>
  <div class="vbox" style="--gap:6px;align-items:stretch;width:220px">
    <button class="btn text-button suggested-action pill">Import Stamp…</button>
    <button class="btn text-button pill" disabled title="Select an area with the Selection tool (Q) first">Save Selection as Stamp</button>
  </div>
</div>"""
dialog = f"""<div class="dialog-host" style="position:fixed">
  <div class="dialog" style="width:560px">
    <div class="headerbar"><div class="start"><button class="btn text-button">Cancel</button></div>
      <div class="title-widget"><span class="title">Import Stamp</span></div>
      <div class="end"><button class="btn text-button suggested-action">Import</button></div></div>
    <div class="dialog-body vbox" style="--gap:18px;padding-top:12px">
      <div class="hbox" style="--gap:18px">
        <div class="checker" style="width:120px;height:120px;border-radius:12px;display:grid;place-items:center;box-shadow:inset 0 0 0 1px var(--borders)">{st_logo(96)}</div>
        <div class="vbox grow" style="--gap:4px"><span class="heading">company-logo.png</span><span class="caption dim-label">~/Pictures/Brand · 512 × 512 · PNG with transparency</span></div>
      </div>
      <div class="boxed-list">
        <div class="row entry-row"><div class="text"><span class="title" style="font-size:12px;opacity:.55">Name</span><span>Company logo</span></div></div>
        {row('Category', '<span class="dropdown flat">My Stamps</span>')}
        {row('Remove White Background', switch(False), 'Only needed for images without transparency')}
      </div>
      <div class="hint">Stamps are copied to ~/.local/share/annota/stamps and stay on this computer.</div>
    </div>
  </div>
</div>"""
write(D, "06-stamp-import-dialog", editor(title="Checkout flow", subtitle="1600 × 1000", tool="stamp",
      canvas=image(f"{SAMPLE}/b-lumen-checkout.png"), sidebar=side, overlay=dialog, tooltip=False, extra_css=EXTRA,
      page_title="import stamp"))

# ------------------------------------------------------------------ 07 my stamps with imported logo placed
mine = [("Company logo", st_logo)]
side = f"""{head('stamp', 'Stamps')}
{search()}
{chips('My Stamps', {'My Stamps': 1})}
{group('My Stamps', stamp_grid(mine, sel='Company logo', add=True), suffix=f'<button class="btn flat image-button" title="Stamp Options">{icon("view-more")}</button>')}
<div class="vbox" style="--gap:6px"><button class="btn text-button">Import Stamp…</button><button class="btn text-button">Save Selection as Stamp</button></div>
{group('Size', scale('Size', 40, '96 px'))}
<div class="hint">Right-click a stamp to rename, move to another category or remove it.</div>"""
logo_svg = stamp_on(st_logo, 640, 380, 112, sel=True)
canvas = image(f"{SAMPLE}/b-lumen-checkout.png", svg=logo_svg)
ed("07-my-stamps-placed", title="Checkout flow", subtitle="1600 × 1000 · Edited", tool="stamp",
   canvas=canvas, sidebar=side, overlay=toast('“Company logo” added to My Stamps', None), tooltip=False,
   page_title="my stamps")

# ------------------------------------------------------------------ edge style icons
def edge_icon(kind):
    if kind == "Straight":
        p = "M2 8H38"
    elif kind == "Zigzag":
        p = "M2 8" + "".join(f"L{2+i*4} {4 if i%2 else 12}" for i in range(1, 10))
    elif kind == "Wave":
        p = "M2 8 Q6 2 10 8 T18 8 T26 8 T34 8 T38 8"
    else:
        p = "M2 8 L5 6 L7 10 L10 5 L13 9 L15 7 L18 11 L21 6 L24 9 L27 5 L30 10 L33 7 L36 9 L38 7"
    return f'<svg class="edge-ico" viewBox="0 0 40 16"><path d="{p}" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" stroke-linecap="round"/></svg>'


def edge_toggles(active):
    out = []
    for k in ["Straight", "Zigzag", "Wave", "Torn"]:
        out.append(f'<span class="toggle{" active" if k == active else ""}" style="flex-direction:column;gap:2px;height:auto;padding:6px 0" title="{k}">{edge_icon(k)}<span class="caption">{k}</span></span>')
    return f'<div class="toggle-group full" style="height:auto">{"".join(out)}</div>'


def zig_path(x0, x1, y, amp=4, per=12):
    pts = [f"M{x0} {y}"]
    x = x0; i = 0
    while x < x1:
        x = min(x + per / 2, x1); i += 1
        pts.append(f"L{x} {y + (amp if i % 2 else -amp)}")
    return " ".join(pts)


# ------------------------------------------------------------------ 08 cut-out preview
S = 0.28
W, H = int(1600 * S), int(2600 * S)
y0, y1 = int(1240 * S), int(1690 * S)
cut_svg = f"""<rect x="0" y="{y0}" width="{W}" height="{y1-y0}" fill="var(--annota-coral)" fill-opacity=".18"/>
  <path d="{zig_path(0, W, y0)}" fill="none" stroke="{CORAL}" stroke-width="2"/>
  <path d="{zig_path(0, W, y1)}" fill="none" stroke="{CORAL}" stroke-width="2"/>
  <line x1="0" x2="{W}" y1="{(y0+y1)/2}" y2="{(y0+y1)/2}" stroke="{CORAL}" stroke-width="1" stroke-dasharray="3 4" opacity=".6"/>
  <circle cx="{W/2}" cy="{y0}" r="5" fill="#fff" stroke="var(--accent-bg)" stroke-width="1.5"/>
  <circle cx="{W/2}" cy="{y1}" r="5" fill="#fff" stroke="var(--accent-bg)" stroke-width="1.5"/>"""
inner = f'<span class="measure" style="left:{W+10}px;top:{(y0+y1)/2-10}px">− 450 px</span>'
canvas = image(f"{SAMPLE}/f-field-notes.png", W, H, svg=cut_svg, inner=inner)
side = f"""{head('cut-out', 'Cut-Out')}
{group('Direction', toggles(['Horizontal','Vertical'],'Horizontal'))}
{group('Seam', edge_toggles('Zigzag'))}
{group('Seam Size', scale('Size', 30, '10 px'))}
<div class="boxed-list">{row('Strip', '<span class="numeric">y 1240 – 1690</span>', 'Height 450 px')}{row('Result', '<span class="numeric">1600 × 2150</span>')}</div>
<div class="hint">Drag across the image to mark the strip to remove. Drag the handles to adjust it.</div>"""
ed("08-cut-out-preview", title="The tide clock", subtitle="1600 × 2600", tool="cut-out", canvas=canvas, sidebar=side,
   foot=footer("Cut Out"), zoom="28 %", page_title="cut-out preview")

# ------------------------------------------------------------------ 09 cut-out applied
H2 = int(2158 * S)
canvas = image(f"{A}/fieldnotes-cut.png", W, H2)
side = f"""{head('cut-out', 'Cut-Out')}
{group('Direction', toggles(['Horizontal','Vertical'],'Horizontal'))}
{group('Seam', edge_toggles('Zigzag'))}
{group('Seam Size', scale('Size', 30, '10 px'))}
<div class="hint">Drag across the image to mark another strip.</div>"""
ed("09-cut-out-applied", title="The tide clock", subtitle="1600 × 2150 · Edited", tool="cut-out", canvas=canvas,
   sidebar=side, overlay=toast("Removed a 450 px strip"), zoom="28 %", tooltip=False, page_title="cut-out applied")

# ------------------------------------------------------------------ selection sidebar
def sel_side(fill="Auto-Fill", has_sel=True):
    acts = ""
    if has_sel:
        acts = group('Selection', f'''<div class="boxed-list">{row('Size', '<span class="numeric">132 × 40 px</span>')}</div>
<div class="hbox" style="--gap:6px"><button class="btn text-button grow" title="Delete">Delete</button><button class="btn text-button grow" title="Ctrl+Shift+X">Crop to Selection</button></div>
<button class="btn text-button" style="width:100%">Save as Stamp</button>''')
    return f"""{head('selection', 'Selection')}
{group('Mode', toggles(['Rectangle','Ellipse','Polygon','Lasso'],'Rectangle'))}
{group('Fill Vacated Area', toggles(['Auto-Fill','Transparent'], fill), desc='Used when the selection is moved or deleted')}
{acts}
<div class="hint">Drag the selection to move it. <kbd class="key">Shift</kbd> adds to it, <kbd class="key">Alt</kbd> subtracts.</div>"""


bx, by, bw, bh = 545, 399, 64, 17   # Start backup button in 800 coords (1090..1217, 798..831)
# ------------------------------------------------------------------ 10 selection before
canvas = image(f"{SAMPLE}/a-harbor-backup.png", svg=ants_rect(bx - 2, by - 2, bw + 4, bh + 4) +
               f'<g transform="translate({bx+bw-6} {by+bh-4})"><path d="M0 0l12 7-5 1.3 3 5.6-2.3 1.2-3-5.6L1 13z" fill="#fff" stroke="#1f1f24" stroke-width="1.2"/></g>')
ed("10-selection-before-delete", title="Harbor preferences", subtitle="1600 × 1000", tool="selection",
   canvas=canvas, sidebar=sel_side(), page_title="selection before")

# ------------------------------------------------------------------ 11 selection after auto-fill
canvas = image(f"{A}/harbor-autofill.png", svg=ants_rect(bx - 2, by - 2, bw + 4, bh + 4))
ed("11-selection-auto-filled", title="Harbor preferences", subtitle="1600 × 1000 · Edited", tool="selection",
   canvas=canvas, sidebar=sel_side(), overlay=toast("Selection deleted · area auto-filled"), tooltip=False,
   page_title="selection auto-fill")

# ------------------------------------------------------------------ 12 selection moved with transparency
mx, my = 420, 399
inner = f'<img src="{A}/harbor-button.png" style="position:absolute;left:{mx}px;top:{my}px;width:{bw+2}px;height:{bh+2}px">'
canvas = image(f"{A}/harbor-transparent.png", svg=ants_rect(mx - 1, my - 1, bw + 4, bh + 4) +
               f'<rect x="{bx-1}" y="{by-1}" width="{bw+2}" height="{bh+2}" fill="none" stroke="var(--accent-bg)" stroke-dasharray="2 3" opacity=".7"/>',
               inner=inner, style="background-color:var(--canvas-checker-a)")
ed("12-selection-moved-transparent", title="Harbor preferences", subtitle="1600 × 1000 · Edited", tool="selection",
   canvas=canvas, sidebar=sel_side("Transparent"), tooltip=False, page_title="selection transparent")

# ------------------------------------------------------------------ 13 fill tolerance
def fill_side(mode_hint):
    return f"""{head('fill', 'Fill')}
{group('Colour', swatches('amber'))}
{group('Tolerance', scale('Tolerance', 12, '12'), desc='0 fills only the exact colour, 100 fills everything')}
<div class="boxed-list">{row('Anti-Aliased Edges', switch(True))}</div>
{mode_hint}"""


canvas = image(f"{A}/tidewater-filled.png", svg='<g transform="translate(380 70)"><path d="M0 14l9-9 7 7-9 9z" fill="#fff" stroke="#1f1f24" stroke-width="1.3"/><path d="M16 13c1.5 2 2 3.3 2 4a2 2 0 0 1-4 0c0-.7.5-2 2-4z" fill="var(--annota-amber)" stroke="#1f1f24"/></g>')
hint = '<div class="hint">Click fills the connected area. <kbd class="key">Shift</kbd>+click replaces this colour everywhere in the image.</div>'
ed("13-fill-tolerance", title="September overview", subtitle="1600 × 1000 · Edited", tool="fill", canvas=canvas,
   sidebar=fill_side(hint), page_title="fill")

# ------------------------------------------------------------------ 14 global replace
canvas = image(f"{A}/tidewater-replaced.png")
gr = group('Last Replace', '<div class="hbox" style="--gap:6px"><span class="sel-chip"><span class="dot" style="background:#1b7fa8"></span>#1B7FA8</span>' + icon('go-next') + '<span class="sel-chip"><span class="dot" style="background:var(--annota-coral)"></span>#FF5A4E</span></div>', desc='Global, tolerance 12 · 21,904 pixels')
side = f"""{head('fill', 'Fill')}
{group('Colour', swatches('coral'))}
{group('Tolerance', scale('Tolerance', 12, '12'))}
<div class="boxed-list">{row('Anti-Aliased Edges', switch(True))}</div>
{gr}
{hint}"""
ed("14-fill-global-replace", title="September overview", subtitle="1600 × 1000 · Edited", tool="fill", canvas=canvas,
   sidebar=side, overlay=toast("Replaced #1B7FA8 everywhere"), tooltip=False, page_title="global replace")

# ------------------------------------------------------------------ 15 eraser
er_svg = '<path d="M131 156 L166 156" stroke="#ebebed" stroke-width="13" stroke-linecap="round" fill="none"/>'
inner = '<span class="brush-cursor" style="left:159px;top:145px;width:22px;height:22px"></span>'
canvas = image(f"{SAMPLE}/a-harbor-backup.png", svg=er_svg, inner=inner)
side = f"""{head('eraser', 'Eraser')}
{group('Brush', toggles(['Round','Square'],'Round'))}
{group('Size', scale('Size', 22, '44 px'))}
<div class="boxed-list">{row('Canvas Colour', '<span class="sel-chip"><span class="dot" style="background:#ebebed"></span>#EBEBED</span>', 'Erased pixels take this colour')}</div>
<div class="hint">The eraser changes image pixels only; annotations are removed with the Delete key.</div>
<div class="hint">Change the brush size with <kbd class="key">[</kbd> and <kbd class="key">]</kbd>.</div>"""
ed("15-eraser", title="Harbor preferences", subtitle="1600 × 1000 · Edited", tool="eraser", canvas=canvas,
   sidebar=side, page_title="eraser")

# ------------------------------------------------------------------ 16 magic wand
inner = f'<img src="{A}/harbor-wand-overlay.png" style="position:absolute;inset:0;width:100%;height:100%">'
canvas = image(f"{SAMPLE}/a-harbor-backup.png", inner=inner,
               svg='<g transform="translate(186 296)"><path d="M0 14L12 2" stroke="#1f1f24" stroke-width="3" stroke-linecap="round"/><path d="M0 14L12 2" stroke="#fff" stroke-width="1.2" stroke-linecap="round"/><path d="M14 -4v4M12 0h4M18 2l2-2" stroke="#1f1f24" stroke-width="1.4" stroke-linecap="round"/></g>')
side = f"""{head('magic-wand', 'Magic Wand')}
{group('Tolerance', scale('Tolerance', 8, '8'))}
<div class="boxed-list">{row('Contiguous', switch(True), 'Off selects the colour everywhere')}{row('Smooth Edges', switch(True))}</div>
{group('Selection', '''<div class="boxed-list">''' + row('Size', '<span class="numeric">520 × 1520 px</span>') + '''</div>
<div class="hbox" style="--gap:6px"><button class="btn text-button grow">Delete</button><button class="btn text-button grow">Invert</button></div>''')}
<div class="hint">The selection works like any other: move it, delete it or fill it. <kbd class="key">Shift</kbd>+click adds an area.</div>"""
ed("16-magic-wand", title="Harbor preferences", subtitle="1600 × 1000", tool="magic-wand", canvas=canvas,
   sidebar=side, page_title="magic wand")

# ------------------------------------------------------------------ 17 text-only highlight
lines = [(655, 249, 749, 261), (655, 271, 735, 282), (655, 309, 732, 320), (655, 329, 757, 342), (655, 367, 772, 381), (655, 389, 830, 401)]
hl = "".join(f'<rect x="{x0/2-2}" y="{y0/2-2}" width="{(x1-x0)/2+4}" height="{(y1-y0)/2+4}" rx="1.5" fill="var(--annota-highlight)"/>' for x0, y0, x1, y1 in lines)
stroke_area = '<rect x="322" y="118" width="290" height="88" fill="none" stroke="var(--accent-bg)" stroke-dasharray="4 3" opacity=".8"/>'
canvas = image(f"{SAMPLE}/a-harbor-backup.png", svg=hl + stroke_area)
side = f"""{head('highlighter', 'Highlighter')}
{group('Colour', swatches('amber', ''))}
{group('Width', scale('Width', 40, '24 px'))}
<div class="boxed-list">{row('Blend', '<span class="dropdown flat">Multiply</span>')}</div>
{text_only_row(True)}
<div class="hint">6 text lines found in the drawn area. Icons and backgrounds stay untouched. Text recognition runs on this computer.</div>"""
ed("17-text-only-highlight", title="Harbor preferences", subtitle="1600 × 1000 · Edited", tool="highlighter",
   canvas=canvas, sidebar=side, page_title="text-only highlight")

# ------------------------------------------------------------------ 18 text-only blur (detecting) + 19 unavailable
blur_area = '<rect x="322" y="118" width="290" height="88" fill="none" stroke="var(--accent-bg)" stroke-dasharray="4 3"/>'
canvas = image(f"{SAMPLE}/a-harbor-backup.png", svg=blur_area)
side = f"""{head('redact', 'Redact')}
{group('Style', toggles(['Blur','Pixelate','Solid'],'Blur'))}
{group('Strength', scale('Strength', 50, '12 px'))}
{text_only_row(True, detecting=True)}"""
ed("18-text-only-detecting", title="Harbor preferences", subtitle="1600 × 1000", tool="redact", canvas=canvas,
   sidebar=side, page_title="text-only detecting")

side = f"""{head('spotlight', 'Spotlight')}
{group('Shape', toggles(['Rectangle','Rounded','Ellipse','Polygon'],'Rectangle'))}
{group('Outside the Areas', scale('Dim', 67, '60 %') + scale('Blur', 0, '0 px'))}
{text_only_row(False, disabled=True)}
<div class="banner warning" style="border-radius:12px;flex-direction:column;gap:6px;padding:12px;text-align:center"><span class="banner-title">Apply to Text Only needs the tesseract-ocr package.</span><span class="caption">Install it with your software manager, then restart Annota.</span></div>"""
canvas = image(f"{SAMPLE}/a-harbor-backup.png")
ed("19-text-only-unavailable", title="Harbor preferences", subtitle="1600 × 1000", tool="spotlight", canvas=canvas,
   sidebar=side, tooltip=False, page_title="text-only unavailable")
print("ok")
