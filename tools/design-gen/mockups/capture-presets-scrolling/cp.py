from lib import *
import sc  # reuse helpers (re-writes sc files harmlessly)
from sc import std_canvas, std_tray, hint, prefs, pcss
C = 'add-capture-presets'
DESK = desk_css('desk-a')

def editor_with(overlay, css=''):
    return doc('x', editor('Checkout flow', '1600 × 1000 · Edited', std_canvas(), arrow_props(), std_tray(), overlay=overlay, zoom='48 %'), pcss + css)

PRESETS = [
 ('capture-region', '800×600 to clipboard', 'Region · 800 × 600 · Copy to clipboard', ['Ctrl','Print']),
 ('capture-window', 'Window with shadow', 'Window · Quick effect “Soft shadow” · Open in editor', ['Super','Shift','E']),
 ('capture-region', 'Bug report', 'Region · 3 s delay · pointer · Save to ~/Pictures/Bugs', None),
 ('timer', 'Build monitor time-lapse', 'Time-lapse · every 10 s · Save to ~/Pictures/Annota/Time-lapse', None),
]
def preset_rows(sel=None):
    out = []
    for i,(ic,t,s,acc) in enumerate(PRESETS):
        a = ''.join(f'<kbd class="key">{k}</kbd>' for k in acc) if acc else '<span class="sct">No shortcut</span>'
        out.append(f'<div class="row{" focus" if i==sel else ""}"><div class="prefix">{I(ic)}</div><div class="text"><span class="title">{t}</span><span class="subtitle">{s}</span></div><div class="suffix"><span class="accel">{a}</span><button class="btn flat image-button" title="Edit Preset">{I("go-next")}</button></div></div>')
    out.append(f'<div class="row button-row"><span class="hbox">{I("plus")}Add Preset</span></div>')
    return ''.join(out)

def prefs_presets(extra='', sel=None, toast=''):
    return f'''<div class="dialog-host"><div class="dialog pref-dlg">
<div class="headerbar"><div class="start"></div><div class="title-widget" style="flex:0 0 auto"><div class="toggle-group flat vs"><span class="toggle">{I('settings')}General</span><span class="toggle active">{I('camera')}Capture</span><span class="toggle">{I('library')}Library</span></div></div><div class="end"><button class="btn flat circular">{I('close')}</button></div></div>
<div class="pbody"><div class="preferences-page">
 <div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Global Shortcuts</span></div></div>
  <div class="boxed-list">
   <div class="row"><div class="text"><span class="title">Region capture</span></div><div class="suffix"><span class="accel"><kbd class="key">Shift</kbd><kbd class="key">Print</kbd></span><span style="opacity:.55">{I("go-next")}</span></div></div>
   <div class="row"><div class="text"><span class="title">Window capture</span></div><div class="suffix"><span class="accel"><kbd class="key">Super</kbd><kbd class="key">Shift</kbd><kbd class="key">W</kbd></span><span style="opacity:.55">{I("go-next")}</span></div></div>
  </div></div>
 <div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Presets</span><span class="description">Saved capture settings. Run them from the capture menu or with their own shortcut.</span></div></div>
  <div class="boxed-list">{preset_rows(sel)}</div></div>
</div></div>{toast}
</div></div>{extra}'''

css = '''.boxed-list .row .prefix .icon{opacity:.8}'''
write(C, '01-presets-list', editor_with(prefs_presets(sel=0), css).replace('Annota — x', 'Annota — Presets list'))

# 02 editor dialog
ecss = css + '''.pe{width:560px;max-width:560px;height:740px}
.pe .pb{flex:1;overflow:hidden;padding:6px 24px 24px;display:flex;flex-direction:column;gap:18px}
.pe .tgw .toggle{flex:1}
.pe .boxed-list .row.entry-row .text>span:last-child{font-size:var(--fs-body)}
.pe .scrow .sct{font-size:var(--fs-small);opacity:var(--dim-opacity)}
.pe .size{display:flex;gap:6px;align-items:center}
.pe .size .entry{width:84px;font-variant-numeric:tabular-nums}
'''
def preset_editor(shortcut_html, title='Edit Preset', conflict=False):
    return f'''<div class="dialog-host" style="background:transparent"><div class="dialog pe">
<div class="headerbar"><div class="start"><button class="btn text-button">Cancel</button></div><div class="title-widget"><span class="title">{title}</span></div><div class="end"><button class="btn text-button suggested-action{' disabled' if conflict else ''}" {'disabled' if conflict else ''}>Save</button></div></div>
<div class="pb">
 <div class="boxed-list"><div class="row entry-row"><div class="text"><span class="title">Name</span><span>800×600 to clipboard</span></div></div></div>
 <div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Capture</span></div></div>
 <div class="boxed-list">
  <div class="row"><div class="text"><span class="title">Mode</span></div><div class="suffix"><div class="toggle-group"><span class="toggle active">{I('capture-region')}Region</span><span class="toggle">{I('capture-window')}Window</span><span class="toggle">{I('capture-screen')}Screen</span><span class="toggle">{I('timer')}Time-lapse</span></div></div></div>
  <div class="row"><div class="text"><span class="title">Fixed size</span><span class="subtitle">Place the frame, then press Enter</span></div><div class="suffix size"><span class="switch on"></span><span class="entry">800</span><span class="dim-label">×</span><span class="entry">600</span></div></div>
  <div class="row"><div class="text"><span class="title">Delay</span></div><div class="suffix"><span class="spin" style="width:118px"><span class="value">0 s</span><button class="btn">{I('minus')}</button><button class="btn">{I('plus')}</button></span></div></div>
  <div class="row"><div class="text"><span class="title">Include pointer</span></div><div class="suffix"><span class="switch"></span></div></div>
 </div></div>
 <div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">After Capture</span></div></div>
 <div class="boxed-list">
  <div class="row"><div class="text"><span class="title">Quick effect</span></div><div class="suffix"><span class="dropdown flat">None</span></div></div>
  <div class="row"><div class="text"><span class="title">Output</span></div><div class="suffix"><div class="toggle-group"><span class="toggle">Editor</span><span class="toggle active">Clipboard</span><span class="toggle">File</span></div></div></div>
 </div></div>
 <div class="boxed-list">{shortcut_html}</div>
 <div class="row button-row destructive" style="min-height:34px;padding:0">Delete Preset</div>
</div></div></div>'''

sh_set = f'<div class="row scrow"><div class="text"><span class="title">Shortcut</span><span class="subtitle">Works in every app</span></div><div class="suffix"><span class="accel"><kbd class="key">Ctrl</kbd><kbd class="key">Print</kbd></span><button class="btn flat image-button" title="Remove Shortcut">{I("edit-clear")}</button></div></div>'
write(C, '02-preset-editor', editor_with(prefs_presets() + preset_editor(sh_set), ecss).replace('Annota — x','Annota — Preset editor'))

# 03 shortcut recording + conflict
sh_rec = f'''<div class="row scrow focus" style="background:var(--accent-a12)"><div class="text"><span class="title">Shortcut</span><span class="subtitle accent" style="opacity:1">Press keys… Esc cancels, Backspace removes</span></div><div class="suffix"><span class="accel"><kbd class="key">Ctrl</kbd><kbd class="key" style="opacity:.5">…</kbd></span></div></div>'''
sh_conf = f'''<div class="row scrow"><div class="text"><span class="title">Shortcut</span><span class="subtitle error" style="opacity:1">Shift+Print is already used by “Region capture”</span></div><div class="suffix"><span class="accel"><kbd class="key" style="box-shadow:inset 0 0 0 1px var(--destructive)">Shift</kbd><kbd class="key" style="box-shadow:inset 0 0 0 1px var(--destructive)">Print</kbd></span><button class="btn">Replace</button></div></div>'''
write(C, '03-preset-shortcut-recording', editor_with(prefs_presets() + preset_editor(sh_rec, 'New Preset'), ecss).replace('Annota — x','Annota — Preset shortcut recording'))
write(C, '04-preset-shortcut-conflict', editor_with(prefs_presets() + preset_editor(sh_conf, 'New Preset', conflict=True), ecss).replace('Annota — x','Annota — Preset shortcut conflict'))

# 05 preset fixed-frame overlay (Ctrl+Print)
def ov(name, title, inner, css=''):
    write(C, name, doc(title, f'<div class="ov"><div class="desk"></div>{inner}</div>', DESK + css, vp='1920x1080'))
sx, sy = 1000, 380
inner = (sel(sx, sy, 800, 600, '', hd=False)
  + f'<div class="ochip accent" style="left:{sx}px;top:{sy-34}px">{I("capture-region")}800×600 to clipboard <span class="dimt" style="opacity:.85">800 × 600 · fixed</span></div>'
  + HANDPTR.format(x=sx+380, y=sy+280)
  + hint([f'Move the frame', f'{keys("Enter",cls="okey")} Copy to clipboard', f'{keys("←","↑","→","↓",cls="okey")} Nudge', f'{keys("Esc",cls="okey")} Cancel']))
ov('05-preset-fixed-frame', 'Preset – fixed 800×600 frame', inner)

# 06 copied toast (no editor) — shown as GNOME notification-free OSD toast on desktop
tcss = '''.osdtoast{position:absolute;left:50%;bottom:64px;transform:translateX(-50%);display:flex;align-items:center;gap:12px;padding:8px 8px 8px 8px;border-radius:var(--radius-xl);background:var(--toast-bg);color:#fff;box-shadow:0 4px 18px rgb(0 0 6 / 40%)}
.osdtoast .th{width:96px;height:72px;border-radius:8px;background-size:240px 135px;overflow:hidden;box-shadow:0 0 0 1px rgb(255 255 255 / 15%)}
.osdtoast .tx{display:flex;flex-direction:column;gap:2px;padding-right:8px}.osdtoast .tx .s{font-size:var(--fs-small);opacity:.7}
.osdtoast .btn{background:var(--osd-fill);color:#fff}'''
inner = f'''<div class="osdtoast"><div class="th shot crop" style="background-size:{1920*96/800:.0f}px {1080*96/800:.0f}px;background-position:-{1000*96/800:.0f}px -{380*72/600:.0f}px"></div>
<div class="tx"><b>Copied to clipboard</b><span class="s">800 × 600 · preset “800×600 to clipboard” · saved to library</span></div>
<button class="btn text-button">Open in Editor</button><button class="btn circular" style="background:transparent">{I('close')}</button></div>'''
ov('06-preset-copied-toast', 'Preset result – copied without editor', inner, tcss)

# 07 multi-area selection
mcss = '''.num{position:absolute;width:28px;height:28px;margin:-14px 0 0 -14px;border-radius:50%;background:var(--accent-bg);color:#fff;font-weight:800;font-size:14px;display:grid;place-items:center;box-shadow:0 0 0 2px #fff,0 2px 6px rgb(0 0 6 / 40%)}
.hole{position:absolute;border:1px solid #fff;box-shadow:0 0 0 1px rgb(0 0 6 / 40%)}
.hole.dash{border:1px dashed #fff}
'''
areas = [(132, 286, 684, 256), (852, 220, 360, 240), (1334, 550, 404, 120)]
mask = '<svg style="position:absolute;inset:0" width="1920" height="1080"><defs><mask id="m"><rect width="1920" height="1080" fill="#fff"/>' + ''.join(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="#000"/>' for x,y,w,h in areas) + '</mask></defs><rect width="1920" height="1080" fill="rgb(0 0 6 / 48%)" mask="url(#m)"/></svg>'
inner = mask
for i,(x,y,w,h) in enumerate(areas):
    last = i == len(areas)-1
    inner += f'<div class="hole{" dash" if last else ""}" style="left:{x}px;top:{y}px;width:{w}px;height:{h}px"></div><div class="num" style="left:{x}px;top:{y}px">{i+1}</div>'
    inner += f'<div class="ochip" style="left:{x+20}px;top:{y+h+8}px">{w} × {h}</div>'
x,y,w,h = areas[-1]
inner += CROSS.format(x=x+w, y=y+h)
inner += f'<div class="ochip accent" style="left:50%;transform:translateX(-50%);top:56px">{I("selection")}3 areas · result 1606 × 450 on transparent canvas</div>'
inner += hint([f'{keys("Ctrl",cls="okey")} + drag Add area', f'{keys("Ctrl",cls="okey")} + click Remove area', f'{keys("Backspace",cls="okey")} Remove last', f'{keys("Enter",cls="okey")} Capture all', f'{keys("Esc",cls="okey")} Cancel'])
ov('07-multi-area-selection', 'Multi-area selection', inner, mcss)

# 08 multi-area result in editor
bx0, by0 = 132, 220
res = '<div class="canvas-image" style="width:803px;height:225px">' + ''.join(
  f'<div class="shot crop" style="position:absolute;left:{(x-bx0)/2}px;top:{(y-by0)/2}px;width:{w/2}px;height:{h/2}px;background-size:960px 540px;background-position:-{x/2}px -{y/2}px"></div>' for x,y,w,h in areas) + '</div>'
props8 = f'''<div class="sidebar-pane end props"><div class="scroll">
 <div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Capture</span></div></div>
 <div class="boxed-list">
  <div class="row property"><div class="text"><span class="title">Size</span><span class="subtitle numeric">1606 × 450 px</span></div></div>
  <div class="row property"><div class="text"><span class="title">Source</span><span class="subtitle">Region · 3 areas</span></div></div>
  <div class="row property"><div class="text"><span class="title">Background</span><span class="subtitle">Transparent</span></div></div>
 </div></div>
 <span class="hint2">Areas keep their screen positions. Gaps stay transparent; set a canvas colour in Image properties.</span>
</div></div>'''
nt = thumb('<div class="checker" style="position:relative">' + ''.join(f'<div class="shot crop" style="position:absolute;left:{(x-bx0)/16}px;top:{(y-by0)/16 + 17}px;width:{w/16}px;height:{h/16}px;background-size:120px 67.5px;background-position:-{x/16}px -{y/16}px"></div>' for x,y,w,h in areas) + '</div>', 'current')
html = doc('Multi-area result', editor('Capture 2026-10-30 14-31-52', '1606 × 450 · Not saved', res, props8, std_tray(extra_first=nt), active='select', zoom='50 %'), DESK)
write(C, '08-multi-area-result', html)

# 09 time-lapse setup popover (launcher, time-lapse mode)
pop_css = sc.pop_css + '.cap-pop{width:380px}'
pop = f'''<div class="popover cap-pop">
  <div class="toggle-group modes">
    <span class="toggle active">{I('capture-region')}Region<small>Shift+Print</small></span>
    <span class="toggle">{I('capture-window')}Window<small>Super+Shift+W</small></span>
    <span class="toggle">{I('capture-screen')}Screen<small>Super+Shift+S</small></span>
  </div>
  <div class="menu-sep"></div>
  <div class="menu-section-title">Presets</div>
  <div class="menu-item">{I('capture-region')}<span class="label">800×600 to clipboard</span><span class="accel">Ctrl+Print</span></div>
  <div class="menu-item">{I('capture-window')}<span class="label">Window with shadow</span><span class="accel">Super+Shift+E</span></div>
  <div class="menu-item">{I('capture-region')}<span class="label">Bug report</span></div>
  <div class="menu-item hover">{I('timer')}<span class="label">Build monitor time-lapse</span></div>
  <div class="menu-sep"></div>
  <div class="menu-item">{I('timer')}<span class="label">Time-Lapse…</span></div>
  <div class="menu-item">{I('settings')}<span class="label">Manage Presets…</span></div>
</div>'''
write(C, '09-launcher-presets-menu', doc('Launcher with presets', editor('Checkout flow', '1600 × 1000 · Edited', std_canvas(), arrow_props(), std_tray(), overlay=pop, capture_cls='active', zoom='48 %'), pop_css))

# 10 time-lapse dialog
tl = f'''<div class="dialog-host"><div class="dialog" style="width:520px">
<div class="headerbar"><div class="start"><button class="btn text-button">Cancel</button></div><div class="title-widget"><span class="title">Time-Lapse</span></div><div class="end"><button class="btn text-button suggested-action">Select Region…</button></div></div>
<div class="dialog-body vbox" style="--gap:18px">
 <div class="boxed-list">
  <div class="row"><div class="text"><span class="title">Interval</span><span class="subtitle">1 s – 60 min</span></div><div class="suffix"><span class="spin" style="width:130px"><span class="value">10 s</span><button class="btn">{I('minus')}</button><button class="btn">{I('plus')}</button></span></div></div>
  <div class="row"><div class="text"><span class="title">Stop after</span></div><div class="suffix"><span class="dropdown flat">1 min</span></div></div>
  <div class="row"><div class="text"><span class="title">Folder</span><span class="subtitle">~/Pictures/Annota/Time-lapse/2026-10-30 14-40</span></div><div class="suffix"><button class="btn flat image-button">{I('folder')}</button></div></div>
  <div class="row"><div class="text"><span class="title">Include pointer</span></div><div class="suffix"><span class="switch"></span></div></div>
 </div>
 <div class="hbox dim-label" style="font-size:var(--fs-small);--gap:8px">{I('info')}6 images will be saved. Stop any time with Shift+Ctrl+Print.</div>
</div></div></div>'''
write(C, '10-time-lapse-setup', doc('Time-lapse setup', editor('Checkout flow', '1600 × 1000 · Edited', std_canvas(), arrow_props(), std_tray(), overlay=tl, zoom='48 %')))

# 11 time-lapse running
rcss = '''.tlr{position:absolute;border:2px dashed rgb(255 255 255 / 85%);box-shadow:0 0 0 1px rgb(0 0 6 / 40%)}
.runbar{position:absolute;display:flex;align-items:center;gap:6px;padding:6px 6px 6px 14px;border-radius:var(--radius-pill);background:var(--osd-bg);color:#fff;box-shadow:var(--shadow-osd);-webkit-backdrop-filter:blur(14px);backdrop-filter:blur(14px);font-variant-numeric:tabular-nums}
.runbar .lbl{font-weight:700;padding:0 6px}.runbar .s{opacity:.7;font-size:var(--fs-small);padding-right:6px}
.runbar .btn{background:var(--osd-fill);color:#fff}
.runbar .pr{width:120px;height:4px;border-radius:2px;background:rgb(255 255 255 / 20%);overflow:hidden}.runbar .pr i{display:block;height:100%;width:50%;background:var(--annota-coral)}
.flash{position:absolute;border:3px solid #fff;opacity:.0}
.topind{position:absolute;right:136px;top:6px;display:flex;align-items:center;gap:6px;padding:0 10px;height:20px;border-radius:10px;background:var(--annota-coral);color:#fff;font-size:12px;font-weight:700}'''
inner = f'''<div class="tlr" style="left:1060px;top:470px;width:770px;height:532px;border-radius:9px"></div>
<div class="ochip" style="left:1060px;top:434px">{I('timer')}Time-lapse region · 770 × 532</div>
<div class="runbar" style="left:1060px;top:1016px"><span class="rec-dot" style="width:10px;height:10px;border-radius:50%;background:var(--annota-coral);box-shadow:0 0 0 3px rgb(255 90 78 / 35%)"></span><span class="lbl">3 of 6</span><span class="pr"><i></i></span><span class="s">next in 7 s</span><button class="btn text-button" title="Pause">{I('pause')}</button><button class="btn text-button" title="Stop (Shift+Ctrl+Print)">{I('stop')}Stop</button></div>
<div class="topind">{I('timer')}3/6</div>'''
ov('11-time-lapse-running', 'Time-lapse running', inner, rcss)

# 12 finished toast in editor
toast = f'''<div class="toast-overlay" style="bottom:130px;left:52px;right:300px"><div class="toast"><span class="toast-title">Time-lapse finished · 6 images saved</span><button class="btn flat text-button">Open Folder</button><button class="btn flat circular">{I('close')}</button></div></div>'''
tl_thumbs = [thumb('<div class="shot crop" style="background-size:249px 140px;background-position:-138px -61px"></div>' + (f'<span class="badge neutral" style="position:absolute;right:4px;bottom:4px;background:var(--osd-bg);color:#fff">{n}</span>'), 'current' if n==6 else '') for n in (6,5,4)]
write(C, '12-time-lapse-finished', doc('Time-lapse finished', editor('Checkout flow', '1600 × 1000 · Edited', std_canvas(), arrow_props(), tray([thumb(img_thumb('b-lumen-checkout'),'current')], [t.replace('<div class="thumb current">','').replace('<div class="thumb ">','')[:-6] for t in tl_thumbs] + [img_thumb('d-tidewater-analytics'), img_thumb('e-relay-chat'), img_thumb('c-code-editor')]), overlay=toast, zoom='48 %'), DESK))
print('ok')
