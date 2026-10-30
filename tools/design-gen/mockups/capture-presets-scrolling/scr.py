from lib import *
from sc import std_tray, hint
C = 'add-scrolling-capture'
F = SP + 'f-field-notes.png'

def dcss(a):
    return f'''.desk{{background-image:url(assets/{a}-light@1x.png)}}:root[data-theme="dark"] .desk{{background-image:url(assets/{a}-dark@1x.png)}}'''

CSS = '''
.strip{position:absolute;right:14px;top:56px;bottom:24px;width:242px;display:flex;flex-direction:column;gap:10px;padding:14px;border-radius:var(--radius-xl);background:var(--osd-bg);color:#fff;box-shadow:var(--shadow-osd);-webkit-backdrop-filter:blur(16px);backdrop-filter:blur(16px)}
.strip .hd{display:flex;align-items:center;gap:8px;font-weight:700}
.strip .hd .sp{flex:1}
.strip .meta{display:flex;justify-content:space-between;font-size:var(--fs-small);font-variant-numeric:tabular-nums;opacity:.85}
.strip .bar{height:6px;border-radius:3px;background:rgb(255 255 255 / 18%);overflow:hidden}.strip .bar i{display:block;height:100%;background:var(--annota-coral);border-radius:3px}
.strip .pv{flex:1;position:relative;overflow:hidden;border-radius:var(--radius-m);background:rgb(0 0 6 / 35%)}
.strip .pv .img{position:absolute;left:12px;right:12px;top:12px;background-image:url(''' + F + ''');background-size:190px auto;background-repeat:no-repeat;box-shadow:0 0 0 1px rgb(255 255 255 / 25%)}
.strip .pv .vp{position:absolute;left:8px;right:8px;border:2px solid var(--annota-coral);border-radius:3px;background:rgb(255 90 78 / 12%)}
.strip .pv .seam{position:absolute;left:12px;right:12px;height:0;border-top:1px dashed rgb(255 255 255 / 55%)}
.strip .pv .fadeb{position:absolute;left:0;right:0;bottom:0;height:40px;background:linear-gradient(transparent,rgb(0 0 6 / 50%))}
.strip .note2{display:flex;gap:8px;align-items:flex-start;padding:8px 10px;border-radius:var(--radius-m);background:rgb(255 255 255 / 10%);font-size:var(--fs-small);line-height:1.35}
.strip .note2.warn{background:rgb(229 165 10 / 30%)}
.strip .note2 .icon{flex:none;margin-top:1px}
.strip .btns{display:flex;gap:6px}.strip .btns .btn{flex:1;background:var(--osd-fill);color:#fff}.strip .btns .btn.suggested-action{background:var(--accent-bg)}
.strip .tg{display:flex;gap:3px;padding:3px;border-radius:var(--radius-m);background:var(--osd-fill)}
.strip .tg span{flex:1;display:flex;align-items:center;justify-content:center;gap:6px;height:30px;border-radius:var(--radius-s);font-size:var(--fs-small);font-weight:700}
.strip .tg span.on{background:var(--osd-fill-active)}
.strip .tg span.off{opacity:.45}
.capfr{position:absolute;border:2px solid var(--annota-coral);box-shadow:0 0 0 9999px rgb(0 0 6 / 45%)}
.capfr .tag{position:absolute;left:-2px;top:-30px;display:flex;align-items:center;gap:6px;padding:3px 9px;border-radius:var(--radius-s) var(--radius-s) 0 0;background:var(--annota-coral);color:#fff;font-size:var(--fs-small);font-weight:700}
.capfr .sticky{position:absolute;left:0;right:0;top:0;background:repeating-linear-gradient(-45deg,rgb(139 92 246 / 22%) 0 8px,rgb(139 92 246 / 8%) 8px 16px);border-bottom:2px solid var(--annota-violet)}
.capfr .sticky .lab{position:absolute;right:24px;top:50%;transform:translateY(-50%);display:flex;gap:6px;align-items:center;padding:3px 9px;border-radius:var(--radius-s);background:var(--annota-violet);color:#fff;font-size:var(--fs-small);font-weight:700}
.scrollcue{position:absolute;display:flex;flex-direction:column;align-items:center;gap:6px;padding:12px 16px;border-radius:var(--radius-xl);background:var(--osd-bg);color:#fff;font-size:var(--fs-small);font-weight:700;-webkit-backdrop-filter:blur(12px);backdrop-filter:blur(12px)}
'''
# frame = browser content area: x 260..1660, y 117..1030
FX, FY, FW, FH = 260, 117, 1400, 913
SCALE = 190/1600  # preview px per source px

def strip(state):
    img_h = int(2600*SCALE)
    if state == 'start':
        cap = 0; total = 913; vp_top = 0
    elif state == 'mid':
        cap = 1100; total = 1100+913; vp_top = 1100
    elif state == 'end':
        cap = 1600; total = 2600; vp_top = 1600
    s_px = int(total/0.875)
    shown_h = int(min(s_px, 2600)*SCALE)
    vp = f'<div class="vp" style="top:{12+int(vp_top/0.875*SCALE)}px;height:{int(913/0.875*SCALE)}px"></div>'
    seams = ''.join(f'<div class="seam" style="top:{12+int(y*SCALE)}px"></div>' for y in (1043, 1600) if y < s_px)
    head = {'start':'Ready','mid':'Capturing…','end':'End of page'}[state]
    icon = {'start':I('pause'),'mid':'<span style="width:10px;height:10px;border-radius:50%;background:var(--annota-coral);box-shadow:0 0 0 3px rgb(255 90 78/35%)"></span>','end':I('check')}[state]
    note = ''
    if state in ('mid','end'):
        note = f'<div class="note2">{I("info")}<span><b>Sticky header found</b><br>Kept once at the top of the result.</span></div>'
    pct = min(s_px, 20000)/20000*100
    auto = '<div class="tg"><span class="on">Manual</span><span>Auto-scroll</span></div>' if state!='end' else '<div class="tg"><span>Manual</span><span class="on">Auto-scroll</span></div>'
    btns = f'<div class="btns"><button class="btn text-button">Cancel</button><button class="btn text-button suggested-action">{"Finish" if state!="start" else "Finish"}</button></div>'
    return f'''<div class="strip"><div class="hd">{icon}<span class="sp">{head}</span><span style="opacity:.7;font-size:var(--fs-small)">Vertical</span></div>
{auto}
<div class="meta"><span>{s_px:,} px captured</span><span>limit 20,000</span></div>
<div class="bar"><i style="width:{max(pct,1.5):.1f}%"></i></div>
<div class="pv"><div class="img" style="height:{shown_h}px;background-size:190px {img_h}px"></div>{seams}{vp}</div>
{note}
{btns}
<div style="font-size:var(--fs-small);opacity:.8;text-align:center">{keys("Enter",cls="okey")} Finish <span style="opacity:.5">·</span> {keys("Esc",cls="okey")} Cancel</div>
</div>'''

def frame(sticky=False):
    st = f'<div class="sticky" style="height:60px"><span class="lab">{I("pin")}Sticky header · 68 px</span></div>' if sticky else ''
    return f'<div class="capfr" style="left:{FX-2}px;top:{FY-2}px;width:{FW+4}px;height:{FH+4}px"><span class="tag">{I("capture-region")}Scrolling region · 1400 px wide</span>{st}</div>'

def ov(name, title, asset, inner):
    write(C, name, doc(title, f'<div class="ov"><div class="desk"></div>{inner}</div>', dcss(asset) + CSS, vp='1920x1080'))

ov('01-scroll-start', 'Scrolling capture – ready', 'desk-s0',
   frame() + f'<div class="scrollcue" style="left:{FX+FW/2-110}px;top:{FY+FH-150}px;width:220px">{I("pan-down","icon large")}Scroll the page to capture<span style="font-weight:400;opacity:.8">or switch to Auto-scroll</span></div>' + strip('start'))
ov('02-scroll-capturing-sticky', 'Scrolling capture – stitching, sticky header', 'desk-s1100', frame(True) + strip('mid'))

# 03 auto scroll finishing at end
ov('03-auto-scroll-end', 'Auto-scroll – end of page', 'desk-s1500', frame(True) + strip('end') +
   f'<div class="hint" style="bottom:36px;left:{FX+FW/2}px">{I("check")} End of page reached · opening editor…</div>')

# 04 limit reached / auto unavailable
lim = strip('mid').replace('1,257 px captured','20,000 px captured').replace('<div class="note2">', '<div class="note2 warn">'+I('warning')+'<span><b>Limit reached (20,000 px)</b><br>Capture stopped. Finish to open what was captured.</span></div><div class="note2" style="display:none">',1)
lim = lim.replace('Capturing…','Limit reached')
import re
lim = re.sub(r'<div class="img" style="height:\d+px;[^"]*"></div>', '<div class="img" style="top:auto;bottom:12px;height:308px;background-size:190px 308px"></div><div style="position:absolute;left:0;right:0;top:0;height:120px;display:flex;align-items:flex-start;justify-content:center;padding-top:14px;font-size:12px;opacity:.8">↑ 18,900 px above</div>', lim)
lim = re.sub(r'<div class="seam"[^>]*></div>', '', lim)
lim = re.sub(r'<div class="vp" style="top:\d+px;', '<div class="vp" style="bottom:8px;', lim)
import re
lim = re.sub(r'<i style="width:[0-9.]+%"></i>','<i style="width:100%"></i>',lim)
lim = re.sub(r'[0-9,]+ px captured','20,000 px captured',lim)
ov('04-limit-reached', 'Scrolling capture – limit reached', 'desk-s1500', frame(True) + lim)

au = strip('start').replace('<div class="tg"><span class="on">Manual</span><span>Auto-scroll</span></div>',
  f'<div class="tg"><span class="on">Manual</span><span class="off">Auto-scroll</span></div><div class="note2">{I("info")}<span>Auto-scroll needs permission to control input. Allow it when GNOME asks, or keep scrolling manually.</span></div>')
ov('05-auto-scroll-unavailable', 'Auto-scroll not allowed', 'desk-s0', frame() + f'<div class="scrollcue" style="left:{FX+FW/2-110}px;top:{FY+FH-150}px;width:220px">{I("pan-down","icon large")}Scroll the page to capture</div>' + au)

# 06 stitch error (match lost)
er = strip('mid').replace('<div class="note2">', f'<div class="note2 warn">{I("warning")}<span><b>Scrolled too fast</b><br>Scroll back up a little so the frames overlap.</span></div><div class="note2" style="display:none">',1).replace('Capturing…','Paused')
ov('06-stitch-lost-overlap', 'Scrolling capture – lost overlap', 'desk-s1100', frame(True) + er)

# 07 result in editor
H = 2600; W = 1600
z = 0.25
can = f'''<div class="canvas-image" style="width:{W*z:.0f}px;height:{H*z:.0f}px;margin-top:18px;flex:none"><img src="{F}" alt=""></div>'''
props = f'''<div class="sidebar-pane end props"><div class="scroll">
 <div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Capture</span></div></div>
 <div class="boxed-list">
  <div class="row property"><div class="text"><span class="title">Size</span><span class="subtitle numeric">1600 × 2600 px</span></div></div>
  <div class="row property"><div class="text"><span class="title">Source</span><span class="subtitle">Scrolling · 4 frames stitched</span></div></div>
  <div class="row property"><div class="text"><span class="title">Sticky header</span><span class="subtitle">Kept once · 68 px</span></div></div>
 </div></div>
 <span class="hint2">Zoomed out to fit the height. Use Crop (C) to trim, Ctrl+0 for 100 %.</span>
</div></div>'''
nt = thumb(f'<img src="{F}" style="object-position:50% 0" alt="">', 'current')
css7 = '.canvas-area.tall{align-items:flex-start;overflow:hidden}.vsb{position:absolute;right:4px;top:8px;width:6px;height:120px;border-radius:3px;background:rgb(0 0 6 / 30%)}'
ed = editor('Scrolling capture 2026-10-30 14-52-17', '1600 × 2600 · Not saved', can + '<span class="vsb"></span>', props, std_tray(extra_first=nt), active='select', zoom='25 %', canvas_cls='tall',
  overlay='<div class="toast-overlay" style="bottom:130px;left:52px;right:300px"><div class="toast"><span class="toast-title">Stitched 4 frames · 1600 × 2600</span><button class="btn flat text-button">Fit Width</button></div></div>')
write(C, '07-result-in-editor', doc('Scrolling capture result', ed, css7))

# 08 launcher entry
import sc
pop = f'''<div class="popover cap-pop">
  <div class="toggle-group modes">
    <span class="toggle">{I('capture-region')}Region<small>Shift+Print</small></span>
    <span class="toggle">{I('capture-window')}Window<small>Super+Shift+W</small></span>
    <span class="toggle">{I('capture-screen')}Screen<small>Super+Shift+S</small></span>
    <span class="toggle active">{I('pan-down')}Scrolling<small>Not set</small></span>
  </div>
  <div style="height:6px"></div>
  <div class="rowl"><span class="t"><span>Direction</span></span><div class="toggle-group"><span class="toggle active">Vertical</span><span class="toggle">Horizontal</span></div></div>
  <div class="rowl"><span class="t"><span>Auto-scroll</span><span class="s">Scrolls for you where GNOME allows it</span></span><span class="switch on"></span></div>
  <div style="padding:6px 0 4px"><button class="btn suggested-action" style="width:100%;min-height:38px">Select Scrolling Region…</button></div>
</div>'''
from sc import std_canvas
write(C, '08-launcher-scrolling-mode', doc('Launcher – scrolling mode', editor('Checkout flow', '1600 × 1000 · Edited', std_canvas(), arrow_props(), std_tray(), overlay=pop, capture_cls='active', zoom='48 %'), sc.pop_css + '.cap-pop{width:440px}'))
print('ok')
