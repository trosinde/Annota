import sys, random
sys.path.insert(0, __file__.rsplit("/", 1)[0])
from lib import *  # noqa

D = "/home/claude/annota/docs/design/add-image-effects"

EXTRA = """
  .pane-switch { margin-bottom: -6px; }
  .fx-stage { position: relative; display: flex; align-items: center; justify-content: center; padding: 28px; }
  .fx-stage.checker { box-shadow: 0 0 0 1px rgb(0 0 6 / 10%); }
  .fx-img { position: relative; display: block; }
  .fx-img img { display: block; width: 100%; height: 100%; }
  .fx-size { position: absolute; left: 0; right: 0; bottom: -26px; text-align: center; font-size: 12px; opacity: .55; font-variant-numeric: tabular-nums; }
  .stack .row { min-height: 50px; }
  .stack .row .prefix .icon.drag { opacity: .45; }
  .stack .row.off .text { opacity: .55; }
  .info-footer { display: flex; gap: 18px; align-items: center; padding: 0 14px; height: 26px; background: #1f1f24; color: #f2f2f4; font: 600 10px/1 Inter, sans-serif; letter-spacing: .1px; }
  .info-footer .k { opacity: .6; font-weight: 500; margin-right: 4px; }
  .check-list { display: flex; flex-direction: column; }
  .check-list .row { min-height: 44px; }
  .wm { position: absolute; display: flex; align-items: center; gap: 6px; font: 800 13px/1 Inter, sans-serif; }
  .qfx-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; }
  .qfx { border-radius: var(--radius-m); background: var(--card-bg); box-shadow: 0 0 0 1px var(--card-shade), 0 1px 2px var(--card-shade); padding: 8px; display: flex; flex-direction: column; gap: 6px; font-size: var(--fs-small); }
  .qfx .thumb { height: 52px; border-radius: 6px; background: var(--fill-1); display: grid; place-items: center; }
  .qfx .thumb i { display: block; width: 62px; height: 36px; background: #fff url(../../kit/sample-content/png/a-harbor-backup.png) center/cover; }
  .qfx .name { font-weight: 700; display: flex; align-items: center; gap: 4px; }
  .qfx.selected { box-shadow: 0 0 0 2px var(--accent-bg); }
  .qfx .sub { opacity: .55; font-size: 12px; }
  .sel-chip { display: inline-flex; align-items: center; gap: 6px; height: 26px; padding: 0 8px; border-radius: var(--radius-s); background: var(--fill-1); font-size: var(--fs-small); }
  .sel-chip .dot { width: 14px; height: 14px; border-radius: 4px; box-shadow: inset 0 0 0 1px rgb(0 0 6 / 20%); }
  .before-after { position: absolute; top: 12px; left: 50%; transform: translateX(-50%); z-index: 4; }
"""


def ed(name, **kw):
    kw.setdefault("extra_css", EXTRA)
    write(D, name, editor(**kw))


def pane_switch(active="Effects", count=None):
    b = f' <span class="badge neutral">{count}</span>' if count else ""
    return f'<div class="toggle-group full pane-switch"><span class="toggle{" active" if active=="Tool" else ""}">Tool</span><span class="toggle{" active" if active=="Effects" else ""}" title="Effects (Ctrl+Shift+E)">Effects{b}</span></div>'


def stack(items, sel=None, target="Whole Image"):
    rows = []
    for name, sub, on in items:
        cls = ("selected " if name == sel else "") + ("" if on else "off")
        rows.append(row(name, switch(on), sub, cls, prefix=icon('drag-handle', 'icon drag')))
    add = f'<button class="btn flat image-button" title="Add Effect">{icon("plus")}</button>'
    return group('Effect Stack', f'<div class="boxed-list stack">{"".join(rows)}</div>',
                 desc=f'{target} · applied top to bottom', suffix=add)


def foot_quick():
    return f'<div class="foot"><button class="btn text-button">Save as Quick Effect…</button></div>'


def torn_clip(sides=("bottom",), amp=2.2, seed=3):
    """clip-path polygon (percent) with torn edge on given sides."""
    rnd = random.Random(seed)
    pts = []
    def edge(n): return [rnd.uniform(0, amp) for _ in range(n)]
    # top: left->right
    if "top" in sides:
        for i, d in enumerate(edge(60)): pts.append((i / 59 * 100, d))
    else:
        pts += [(0, 0), (100, 0)]
    if "right" in sides:
        for i, d in enumerate(edge(40)): pts.append((100 - d, i / 39 * 100))
    else:
        pts += [(100, 100)]
    if "bottom" in sides:
        for i, d in enumerate(edge(60)): pts.append((100 - i / 59 * 100, 100 - d * 1.6 - (amp * .4)))
    else:
        pts += [(0, 100)]
    if "left" in sides:
        for i, d in enumerate(edge(40)): pts.append((d, 100 - i / 39 * 100))
    return "polygon(" + ",".join(f"{x:.2f}% {y:.2f}%" for x, y in pts) + ")"


def stage(src, w, h, *, border=None, radius=0, shadow=True, clip=None, filt="", transform="", footer="", overlay="",
          pad=28, size_label="", checker=True, extra_style=""):
    bw = border[1] if border else 0
    bstyle = f"box-shadow: inset 0 0 0 {bw}px {border[0]};" if border else ""
    img_style = f"width:{w}px;height:{h}px;border-radius:{radius}px;overflow:hidden;"
    sh = "filter: drop-shadow(6px 8px 10px rgb(0 0 6 / 38%));" if shadow else ""
    clip_s = f"clip-path:{clip};" if clip else ""
    inner = f'<img src="{src}" style="filter:{filt or "none"}">'
    border_div = f'<span style="position:absolute;inset:0;border-radius:{radius}px;{bstyle}pointer-events:none"></span>' if border else ""
    body = f'''<div style="{sh}{transform}">
      <div class="fx-img" style="{clip_s}">
        <div style="{img_style}position:relative;background:#fff">{inner}{overlay}{border_div}</div>
        {footer}
      </div></div>'''
    lab = f'<span class="fx-size">{size_label}</span>' if size_label else ""
    return f'<div class="fx-stage{" checker" if checker else ""}" style="padding:{pad}px;{extra_style}">{body}{lab}</div>'


H = f"{SAMPLE}/a-harbor-backup.png"
TIDE = f"{SAMPLE}/d-tidewater-analytics.png"
CHK = f"{SAMPLE}/b-lumen-checkout.png"
STD = [("Border", "Ink · 6 px · radius 12", True), ("Torn Edge", "Bottom · 18 px", True), ("Shadow", "8, 10 · blur 20 · 40 %", True)]

# ------------------------------------------------------------- 01 effects panel / border editor
canvas = stage(H, 640, 400, border=("#1f1f24", 4), radius=8, clip=torn_clip(("bottom",)), size_label="1680 × 1080 after effects")
side = f"""{pane_switch('Effects', 3)}
{stack(STD, sel='Border')}
{group('Border', f'''{swatches('ink')}
{scale('Width', 20, '6 px')}
{scale('Radius', 25, '12 px')}''')}"""
ed("01-effects-panel-border", title="Harbor preferences", subtitle="1600 × 1000 · Edited", tool="select", canvas=canvas,
   sidebar=side, foot=foot_quick(), tooltip=False, page_title="effects panel")

# ------------------------------------------------------------- 02 add-effect menu
menu = f"""<div class="popover" style="position:fixed;left:auto;right:14px;top:176px;width:240px;--arrow-x:218px;z-index:40">
  <div style="padding:6px"><span class="search-entry focus" style="width:100%">{icon('search')}<span class="placeholder">Search effects</span></span></div>
  <div class="menu-section-title">Frame</div>
  <div class="menu-item"><span class="label">Border</span></div>
  <div class="menu-item hover"><span class="label">Edges</span></div>
  <div class="menu-item"><span class="label">Shadow</span></div>
  <div class="menu-item"><span class="label">Perspective</span></div>
  <div class="menu-item"><span class="label">Page Curl</span></div>
  <div class="menu-item"><span class="label">Reflection</span></div>
  <div class="menu-sep"></div>
  <div class="menu-section-title">Colour</div>
  <div class="menu-item"><span class="label">Colour Adjustment</span></div>
  <div class="menu-item"><span class="label">Filter</span></div>
  <div class="menu-item"><span class="label">Replace Colour</span></div>
  <div class="menu-sep"></div>
  <div class="menu-section-title">Overlay</div>
  <div class="menu-item"><span class="label">Watermark</span></div>
  <div class="menu-item"><span class="label">Capture Info</span></div>
</div>"""
side = f"""{pane_switch('Effects', 1)}
{stack([("Border", "Ink · 6 px · radius 12", True)], sel=None)}"""
canvas = stage(H, 640, 400, border=("#1f1f24", 4), radius=8, shadow=False, size_label="1612 × 1012")
ed("02-add-effect-menu", title="Harbor preferences", subtitle="1600 × 1000 · Edited", tool="select", canvas=canvas,
   sidebar=side.replace(f'<button class="btn flat image-button" title="Add Effect">', f'<button class="btn flat image-button active" title="Add Effect">'),
   overlay=menu, foot=foot_quick(), tooltip=False, page_title="add effect")

# ------------------------------------------------------------- 03 empty effects
side = f"""{pane_switch('Effects')}
<div class="status-page compact" style="padding:48px 6px">
  {icon('effects','icon large status-icon')}
  <div class="status-title" style="font-size:20px">No Effects</div>
  <div class="status-description">Add borders, edges, shadows or capture info. Effects never change the original pixels.</div>
  <button class="btn text-button pill suggested-action">Add Effect…</button>
</div>
{group('Quick Effects', '<div class="qfx-grid">' + ''.join(f'<div class="qfx"><span class="thumb"><i style="{st}"></i></span><span class="name">{n}</span></div>' for n, st in [('Docs Frame','box-shadow:0 0 0 2px #1f1f24,3px 4px 6px rgb(0 0 0/.4)'),('Torn Bottom','clip-path:polygon(0 0,100% 0,100% 88%,90% 95%,78% 87%,64% 96%,50% 88%,36% 95%,22% 87%,10% 95%,0 88%)'),]) + '</div>')}"""
canvas = image(H, 768, 480)
ed("03-effects-empty", title="Harbor preferences", subtitle="1600 × 1000", tool="select", canvas=canvas,
   sidebar=side, tooltip=False, page_title="effects empty")

# ------------------------------------------------------------- 04 torn bottom edge
def side_toggles(active):
    out = "".join(f'<span class="toggle{" active" if s in active else ""}">{s}</span>' for s in ["Top", "Right", "Bottom", "Left"])
    return f'<div class="toggle-group full four">{out}</div>'


def edge_toggles(active):
    return toggles(["Torn", "Wave", "Zigzag", "Fade"], active)


canvas = stage(TIDE, 640, 400, shadow=True, clip=torn_clip(("bottom",), amp=2.6, seed=7), size_label="1600 × 1000 · bottom edge torn")
side = f"""{pane_switch('Effects', 1)}
{stack([("Edges", "Torn · bottom · 18 px", True)], sel='Edges')}
{group('Edges', f'''{edge_toggles('Torn')}
<span class="field-label">Sides</span>
{side_toggles(['Bottom'])}
{scale('Size', 36, '18 px')}
{scale('Roughness', 55, '55')}''', desc='Several sides can be active at once')}"""
ed("04-edges-torn-bottom", title="September overview", subtitle="1600 × 1000 · Edited", tool="select", canvas=canvas,
   sidebar=side, foot=foot_quick(), tooltip=False, page_title="torn edge")

# ------------------------------------------------------------- 05 shadow + perspective + reflection
refl = f'''<div style="position:absolute;left:0;right:0;top:100%;height:120px;overflow:hidden;opacity:.28;-webkit-mask:linear-gradient(#000,transparent 80%);transform:scaleY(-1);transform-origin:top"><img src="{CHK}" style="width:560px;height:350px;display:block;transform:translateY(-230px)"></div>'''
canvas = f'''<div class="fx-stage checker" style="padding:36px 60px 100px;perspective:1400px">
  <div style="transform:rotateY(-16deg) rotateX(6deg) rotateZ(-1deg);transform-style:preserve-3d;position:relative">
    <div style="filter:drop-shadow(14px 18px 16px rgb(0 0 6 / 40%))"><div style="width:560px;height:350px;border-radius:10px;overflow:hidden;background:#fff"><img src="{CHK}" style="width:100%;height:100%;display:block"></div></div>
    <div style="position:absolute;left:0;top:362px;width:560px;height:110px;overflow:hidden;opacity:.25;-webkit-mask:linear-gradient(#000,transparent);mask:linear-gradient(#000,transparent);border-radius:10px"><img src="{CHK}" style="width:560px;height:350px;display:block;transform:scaleY(-1) translateY(0);transform-origin:center"></div>
  </div><span class="fx-size">1932 × 1310 after effects</span></div>'''
side = f"""{pane_switch('Effects', 3)}
{stack([("Perspective", "Tilt 6° · rotation −16°", True), ("Shadow", "14, 18 · blur 32 · 40 %", True), ("Reflection", "Size 30 % · opacity 25 %", True)], sel='Perspective')}
{group('Perspective', f'''{scale('Tilt', 53, '6°')}
{scale('Rotation', 32, '−16°')}
<div class="hbox" style="--gap:6px"><button class="btn text-button grow">Left</button><button class="btn text-button grow">Flat</button><button class="btn text-button grow">Right</button></div>''', desc='Presets or fine-tune with the sliders')}"""
ed("05-perspective-shadow-reflection", title="Checkout flow", subtitle="1600 × 1000 · Edited", tool="select", canvas=canvas,
   sidebar=side, foot=foot_quick(), tooltip=False, page_title="perspective")

# ------------------------------------------------------------- 06 shadow editor + page curl
curl = '''<svg style="position:absolute;right:0;bottom:0;width:90px;height:90px" viewBox="0 0 90 90"><defs><linearGradient id="cg" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#f4f4f6"/><stop offset=".55" stop-color="#d4d4da"/><stop offset="1" stop-color="#9c9ca6"/></linearGradient></defs>
<path d="M90 0 L90 90 L0 90 Z" fill="#deddda"/><path d="M90 0 Q58 52 0 90 Q30 40 90 0 Z" fill="none"/><path d="M90 2 C70 30 40 62 2 90 C40 74 62 70 70 64 C76 44 84 26 90 2 Z" fill="url(#cg)" stroke="rgb(0 0 6/.18)"/></svg>'''
canvas = stage(TIDE, 640, 400, shadow=True, overlay=curl, size_label="1648 × 1052 after effects")
side = f"""{pane_switch('Effects', 2)}
{stack([("Shadow", "8, 10 · blur 20 · 40 %", True), ("Page Curl", "Bottom right", True)], sel='Shadow')}
{group('Shadow', f'''{swatches('ink')}
<div class="boxed-list">{row('Offset X', spin('8'))}{row('Offset Y', spin('10'))}</div>
{scale('Blur', 40, '20 px')}
{scale('Opacity', 40, '40 %')}''')}
{group('Page Curl', toggles(['↖','↗','↙','↘'],'↘'))}"""
ed("06-shadow-page-curl", title="September overview", subtitle="1600 × 1000 · Edited", tool="select", canvas=canvas,
   sidebar=side, foot=foot_quick(), tooltip=False, page_title="shadow")

# ------------------------------------------------------------- 07 capture info footer
foot_html = '<div class="info-footer"><span><span class="k">Captured</span>30 Oct 2026, 14:22</span><span><span class="k">App</span>Harbor Backup</span><span style="margin-left:auto;opacity:.6">annota</span></div>'
canvas = stage(H, 640, 400, shadow=True, footer=foot_html, size_label="1600 × 1066 · footer adds 66 px")
fields = [("Date and Time", "30 Oct 2026, 14:22", True), ("Application Name", "Harbor Backup", True),
          ("Window Title", "Backup — Preferences", False), ("Operating System", "Ubuntu 26.04 LTS", False),
          ("Image Size", "1600 × 1000", False), ("Custom Text", "Not set", False)]
fl = "".join(row(n, '', v, prefix=f'<span class="check{" checked" if on else ""}"></span>') for n, v, on in fields)
side = f"""{pane_switch('Effects', 1)}
{stack([("Capture Info", "Date and time, application", True)], sel='Capture Info')}
{group('Fields', f'<div class="boxed-list check-list">{fl}</div>', desc='Values come from the capture metadata')}
{group('Style', f'''{toggles(['Dark','Light','Accent'],'Dark')}
<div class="boxed-list">{row('Position', '<span class="dropdown flat">Bottom</span>')}{row('Date Format', '<span class="dropdown flat">30 Oct 2026, 14:22</span>')}</div>''')}"""
ed("07-capture-info-footer", title="Harbor preferences", subtitle="Window capture · 1600 × 1000 · Edited", tool="select",
   canvas=canvas, sidebar=side, foot=foot_quick(), tooltip=False, page_title="capture info")

# ------------------------------------------------------------- 08 capture info — field unavailable (screen capture)
fields2 = [("Date and Time", "30 Oct 2026, 14:31", True), ("Application Name", "Not available for screen captures", False),
           ("Window Title", "Not available for screen captures", False), ("Operating System", "Ubuntu 26.04 LTS", True),
           ("Image Size", "1600 × 1000", True), ("Custom Text", "Release notes 0.4 · draft", True)]
fl2 = "".join(row(n, '', v, "disabled" if "Not available" in v else "", prefix=f'<span class="check{" checked" if on else ""}"></span>') for n, v, on in fields2)
foot2 = '<div class="info-footer"><span>30 Oct 2026, 14:31</span><span>Ubuntu 26.04 LTS</span><span>1600 × 1000</span><span style="margin-left:auto">Release notes 0.4 · draft</span></div>'
canvas = stage(TIDE, 640, 400, shadow=False, footer=foot2.replace('#1f1f24', ''), size_label="1600 × 1066")
side = f"""{pane_switch('Effects', 1)}
{stack([("Capture Info", "4 fields", True)], sel='Capture Info')}
{group('Fields', f'<div class="boxed-list check-list">{fl2}</div>', desc='Screen captures carry no application or window data')}"""
ed("08-capture-info-screen", title="September overview", subtitle="Screen capture · 1600 × 1000 · Edited", tool="select",
   canvas=canvas, sidebar=side, foot=foot_quick(), tooltip=False, page_title="capture info screen")

# ------------------------------------------------------------- 09 colour adjustments + filters
canvas = stage(CHK, 640, 400, shadow=False, filt="saturate(.35) contrast(1.08) brightness(1.04)", size_label="1600 × 1000")
side = f"""{pane_switch('Effects', 2)}
{stack([("Colour Adjustment", "Saturation −65 · contrast +8", True), ("Filter", "Sharpen · 30 %", True)], sel='Colour Adjustment')}
{group('Colour Adjustment', f'''{scale('Brightness', 52, '+4')}
{scale('Contrast', 54, '+8')}
{scale('Hue', 50, '0°')}
{scale('Saturation', 17, '−65')}
{scale('Gamma', 50, '1.00')}
<button class="btn text-button" style="align-self:flex-start">Reset</button>''')}
{group('Filter', toggles(['Grayscale','Sepia','Invert','Sharpen','Soften'],'Sharpen').replace('class="toggle-group full four"','class="toggle-group full four" style="font-size:11px"'))}"""
ed("09-colour-adjustment", title="Checkout flow", subtitle="1600 × 1000 · Edited", tool="select", canvas=canvas,
   sidebar=side, foot=foot_quick(), tooltip=False, page_title="colour adjustment")

# ------------------------------------------------------------- 10 replace colour + grayscale on selection
sel_overlay = (f'<div style="position:absolute;left:366px;top:30px;width:264px;height:52px;overflow:hidden"><img src="{TIDE}" style="position:absolute;left:-366px;top:-30px;width:640px;height:400px;max-width:none;filter:grayscale(1)"></div>'
               '<span style="position:absolute;left:366px;top:30px;width:264px;height:52px;outline:1px dashed #1f1f24;box-shadow:0 0 0 1px #fff"></span>')
canvas = stage(TIDE, 640, 400, shadow=False, overlay=sel_overlay, size_label="Effect limited to the selection · 660 × 130 px")
side = f"""{pane_switch('Effects', 1)}
{stack([("Filter", "Grayscale", True)], sel='Filter', target='Selection 660 × 130')}
{group('Filter', toggles(['Grayscale','Sepia','Invert','Sharpen','Soften'],'Grayscale'))}
{group('Replace Colour', f'''<div class="hbox" style="--gap:6px"><span class="sel-chip"><span class="dot" style="background:#1b7fa8"></span>From</span>{icon('go-next')}<span class="sel-chip"><span class="dot" style="background:var(--annota-coral)"></span>To</span><span class="spacer"></span><button class="btn flat image-button" title="Pick From Image">{icon('eyedropper')}</button></div>
{scale('Tolerance', 20, '20')}''', desc='Not active — add it from the + menu')}
<div class="hint">Draw a selection with the Selection tool (Q) first to apply effects to a part of the image only.</div>"""
ed("10-effect-on-selection", title="September overview", subtitle="1600 × 1000 · Edited", tool="selection", canvas=canvas,
   sidebar=side, foot=foot_quick(), tooltip=False, page_title="effect on selection")

# ------------------------------------------------------------- 11 watermark
wm = f'<div class="wm" style="right:14px;bottom:12px;opacity:.55">{st_logo(40)}<span style="color:#1f1f24">Nordvale Docs</span></div>'
wm_tile = '<div style="position:absolute;inset:0;background:repeating-linear-gradient(-30deg,transparent 0 60px,rgb(31 31 36/.0) 60px 61px)"></div>'
canvas = stage(CHK, 640, 400, shadow=True, overlay=wm, size_label="1600 × 1000")
side = f"""{pane_switch('Effects', 2)}
{stack([("Watermark", "company-logo.png · bottom right · 55 %", True), ("Shadow", "8, 10 · blur 20 · 40 %", True)], sel='Watermark')}
{group('Watermark', f'''<div class="boxed-list">{row('Image', '<button class="btn text-button">company-logo.png</button>')}{row('Text', '<span class="dim-label">Nordvale Docs</span>')}</div>
<span class="field-label">Position</span>
<div style="display:grid;grid-template-columns:repeat(3,1fr);gap:4px;width:120px">{''.join(f'<span class="btn small{" suggested-action" if i==8 else ""}" style="min-height:24px;padding:0"></span>' for i in range(9))}</div>
{scale('Scale', 30, '30 %')}
{scale('Opacity', 55, '55 %')}
{toggles(['Overlay','Emboss','Tiled'],'Overlay')}''')}"""
ed("11-watermark", title="Checkout flow", subtitle="1600 × 1000 · Edited", tool="select", canvas=canvas,
   sidebar=side, foot=foot_quick(), tooltip=False, page_title="watermark")

# ------------------------------------------------------------- 12 save quick effect dialog
dialog = f"""<div class="dialog-host" style="position:fixed">
  <div class="dialog" style="width:480px">
    <div class="headerbar"><div class="start"><button class="btn text-button">Cancel</button></div>
      <div class="title-widget"><span class="title">Save Quick Effect</span></div>
      <div class="end"><button class="btn text-button suggested-action">Save</button></div></div>
    <div class="dialog-body vbox" style="--gap:18px;padding-top:12px">
      <div class="boxed-list">
        <div class="row entry-row focus"><div class="text"><span class="title" style="font-size:12px;opacity:.55">Name</span><span>Docs frame<span style="border-left:1px solid currentColor;margin-left:1px"></span></span></div></div>
      </div>
      <div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Includes 3 Effects</span></div></div>
        <div class="boxed-list">{row('Border','', 'Ink · 6 px · radius 12', prefix=icon('check'))}{row('Torn Edge','', 'Bottom · 18 px', prefix=icon('check'))}{row('Shadow','', 'Offset 8, 10 · blur 20 · 40 %', prefix=icon('check'))}</div></div>
      <div class="boxed-list">{row('Apply After Every Capture', switch(False), 'New captures open with these effects')}</div>
    </div>
  </div>
</div>"""
canvas = stage(H, 640, 400, border=("#1f1f24", 4), radius=8, clip=torn_clip(("bottom",)))
side = f"""{pane_switch('Effects', 3)}
{stack(STD)}"""
write(D, "12-save-quick-effect", editor(title="Harbor preferences", subtitle="1600 × 1000 · Edited", tool="select",
      canvas=canvas, sidebar=side, overlay=dialog, foot=foot_quick(), tooltip=False, extra_css=EXTRA, page_title="save quick effect"))

# ------------------------------------------------------------- 13 quick effects gallery
def qcard(name, style, sel=False, auto=False):
    a = f'<span class="badge" title="Applied after every capture">Auto</span>' if auto else ""
    return f'<div class="qfx{" selected" if sel else ""}"><span class="thumb"><i style="{style}"></i></span><span class="name">{name}{a}</span></div>'


cards = (qcard("Docs Frame", "box-shadow:0 0 0 2px #1f1f24,3px 4px 6px rgb(0 0 0/.4);border-radius:3px", True, True)
         + qcard("Torn Bottom", "clip-path:polygon(0 0,100% 0,100% 88%,90% 95%,78% 87%,64% 96%,50% 88%,36% 95%,22% 87%,10% 95%,0 88%)")
         + qcard("Tilted Hero", "transform:perspective(120px) rotateY(-16deg);box-shadow:4px 5px 6px rgb(0 0 0/.35)")
         + qcard("Grayscale", "filter:grayscale(1)"))
qmenu = f"""<div class="popover" style="position:fixed;left:880px;top:524px;width:248px;--arrow-x:180px;z-index:40">
  <div class="menu-item"><span class="label">Apply</span></div>
  <div class="menu-item"><span class="label">Apply After Every Capture</span>{icon('check')}</div>
  <div class="menu-sep"></div>
  <div class="menu-item"><span class="label">Rename…</span></div>
  <div class="menu-item"><span class="label">Update From Current Stack</span></div>
  <div class="menu-item"><span class="label" style="color:var(--destructive)">Delete</span></div>
</div>"""
canvas = stage(H, 640, 400, border=("#1f1f24", 4), radius=8, clip=torn_clip(("bottom",)))
side = f"""{pane_switch('Effects', 3)}
{stack(STD)}
{group('Quick Effects', f'<div class="qfx-grid">{cards}</div>', desc='Click to apply. One quick effect can run after every capture.')}"""
ed("13-quick-effects", title="Harbor preferences", subtitle="1600 × 1000 · Edited", tool="select", canvas=canvas,
   sidebar=side, overlay=qmenu, foot=foot_quick(), tooltip=False, page_title="quick effects")

# ------------------------------------------------------------- 14 new capture opens with auto effect
banner = '<div class="banner" style="position:absolute;top:0;left:0;right:0;z-index:6"><span class="banner-title">Quick effect “Docs Frame” was applied after capture</span><button class="btn">Remove</button></div>'
canvas = stage(CHK, 640, 400, border=("#1f1f24", 4), radius=8, clip=torn_clip(("bottom",), seed=11), size_label="1680 × 1080 after effects")
side = f"""{pane_switch('Effects', 3)}
{stack(STD, sel=None)}
<div class="hint">These effects were added by the quick effect “Docs Frame”. Change them here like any other effect; the quick effect itself stays unchanged.</div>"""
ed("14-auto-apply-after-capture", title="Capture 2026-10-30 14-40", subtitle="1600 × 1000 · New", tool="select", canvas=canvas,
   sidebar=side, overlay=banner, foot=foot_quick(), tooltip=False, page_title="auto apply", undo=False)

# ------------------------------------------------------------- 15 edit later: reopened .annota
canvas = stage(H, 640, 400, border=("#1f1f24", 8), radius=8, clip=torn_clip(("bottom",)), size_label="1696 × 1096 after effects")
side = f"""{pane_switch('Effects', 3)}
{stack([("Border", "Ink · 12 px · radius 12", True), ("Torn Edge", "Bottom · 18 px", True), ("Shadow", "8, 10 · blur 20 · 40 %", False)], sel='Border')}
{group('Border', f'''{swatches('ink')}
<div class="boxed-list">{row('Width', spin('12').replace('class="spin"','class="spin focus"'))}</div>
{scale('Radius', 25, '12 px')}''')}"""
ed("15-edit-later-reopened", title="harbor-backup.annota", subtitle="~/Pictures/Annota · Edited", tool="select", canvas=canvas,
   sidebar=side, foot=foot_quick(), tooltip=False, page_title="edit later")
print("ok")
