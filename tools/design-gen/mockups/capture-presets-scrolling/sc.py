from lib import *
C = 'add-screen-capture'
DESK = desk_css('desk-a')

def std_canvas():
    return f'''<div class="canvas-image" style="width:768px;height:480px"><img src="{SP}b-lumen-checkout.png" alt="">
<svg class="annotations" viewBox="0 0 1600 1000"><path d="M1000 985 L1114 873" stroke="var(--annota-coral)" stroke-width="8" stroke-linecap="round"/><path d="M1130 858 L1119 883 L1104 868Z" fill="var(--annota-coral)" stroke="var(--annota-coral)" stroke-width="3" stroke-linejoin="round"/><rect x="100" y="512" width="438" height="66" rx="10" fill="none" stroke="var(--annota-coral)" stroke-width="6"/></svg></div>'''

def std_tray(cur=None, extra_first=None):
    o = []
    if extra_first: o.append(extra_first)
    o.append(thumb(img_thumb('b-lumen-checkout'), '' if extra_first else 'current'))
    o.append(thumb(img_thumb('d-tidewater-analytics') + '<span class="unsaved"></span>'))
    return tray(o)

# ---------------------------------------------------------------- 01 launcher
pop_css = '''
.cap-pop{left:7px;top:52px;width:356px;--arrow-x:24px;padding:12px}
.cap-pop .modes{display:flex;width:100%}
.cap-pop .modes .toggle{flex:1;flex-direction:column;gap:3px;min-height:76px;padding:8px 4px;font-size:var(--fs-small)}
.cap-pop .modes .toggle small{font-size:11px;font-weight:400;opacity:.6}
.cap-pop .modes .toggle .icon{width:24px;height:24px}
.cap-pop .rowl{display:flex;align-items:center;gap:12px;min-height:44px;padding:0 6px}
.cap-pop .rowl .t{flex:1;display:flex;flex-direction:column}
.cap-pop .rowl .t .s{font-size:var(--fs-small);opacity:var(--dim-opacity)}
.cap-pop .menu-item{padding:0 8px}
'''
pop = f'''<div class="popover cap-pop">
  <div class="toggle-group modes">
    <span class="toggle active">{I('capture-region')}Region<small>Shift+Print</small></span>
    <span class="toggle hover">{I('capture-window')}Window<small>Super+Shift+W</small></span>
    <span class="toggle">{I('capture-screen')}Screen<small>Super+Shift+S</small></span>
  </div>
  <div style="height:6px"></div>
  <div class="rowl"><span class="t"><span>Delay</span><span class="s">Countdown before capturing</span></span><span class="spin" style="width:118px"><span class="value">0 s</span><button class="btn">{I('minus')}</button><button class="btn">{I('plus')}</button></span></div>
  <div class="rowl"><span class="t"><span>Include pointer</span></span><span class="switch"></span></div>
  <div style="padding:6px 0 4px"><button class="btn suggested-action" style="width:100%;min-height:38px">Capture Region<span style="opacity:.75;font-weight:400;margin-left:6px">Shift+Print</span></button></div>
  <div class="menu-sep"></div>
  <div class="menu-item">{I('copy')}<span class="label">Paste Image From Clipboard</span><span class="accel">Shift+Ctrl+V</span></div>
  <div class="menu-item">{I('image')}<span class="label">Open Image…</span><span class="accel">Ctrl+O</span></div>
</div>
'''
write(C, '01-capture-launcher', doc('Capture launcher', editor('Checkout flow', '1600 × 1000 · Edited', std_canvas(), arrow_props(), std_tray(), overlay=pop, capture_cls='active', zoom='48 %'), pop_css))

# ---------------------------------------------------------------- overlays
def hint(items, bottom=36):
    sep = '<span class="dot">·</span>'
    return f'<div class="hint" style="bottom:{bottom}px">' + sep.join(items) + '</div>'

H_STD = hint([f'{keys("Enter",cls="okey")} Capture', f'{keys("Esc",cls="okey")} Cancel', f'{keys("←","↑","→","↓",cls="okey")} Nudge 1 px', f'{keys("Shift",cls="okey")} ×10', f'{keys("Space",cls="okey")} Move selection'])

def ov(name, title, inner, css=''):
    write(C, name, doc(title, f'<div class="ov"><div class="desk"></div>{inner}</div>', DESK + css, vp='1920x1080'))

# 02 drag
sx, sy, sw, sh = 140, 296, 400, 300
cx, cy = sx + sw, sy + sh
col = px(C, 'desk-a', cx, cy)
inner = (sel(sx, sy, sw, sh, 'live', hd=False)
  + f'<div class="ochip" style="left:{sx}px;top:{sy-34}px">400 × 300</div>'
  + CROSS.format(x=cx, y=cy)
  + loupe(cx+26, cy+26, cx, cy)
  + f'<div class="ochip" style="left:{cx+26}px;top:{cy+172}px"><span class="sw" style="background:{col}"></span>{col}<span class="dimt">{cx}, {cy}</span></div>'
  + hint([f'Drag to select', f'{keys("Shift",cls="okey")} Square', f'{keys("Alt",cls="okey")} No snapping', f'{keys("Esc",cls="okey")} Cancel']))
ov('02-region-drag', 'Region overlay – dragging', inner)

# 03 adjust + fixed size typing
sx, sy, sw, sh = 140, 236, 800, 600
tb = f'''<div class="otb" style="left:{sx}px;top:{sy+sh+14}px;transform:none">
  <div class="tg"><button class="btn image-button active" title="Region (Shift+Print)">{I('capture-region')}</button><button class="btn image-button" title="Window (Super+Shift+W)">{I('capture-window')}</button><button class="btn image-button" title="Screen (Super+Shift+S)">{I('capture-screen')}</button></div>
  <span class="sep"></span>
  <span class="oentry" title="Width"><span class="k">W</span>800</span><span class="times">×</span><span class="oentry focus" title="Height"><span class="k">H</span>600<span class="caret"></span></span>
  <span class="lbl" style="opacity:.7">px</span>
  <span class="sep"></span>
  <button class="btn image-button" title="Include Pointer">{I('cursor')}</button>
  <span class="odrop" title="Delay">{I('timer')}Off</span>
  <span class="sep"></span>
  <button class="btn text-button" title="Cancel (Esc)">Cancel</button>
  <button class="btn text-button suggested-action" title="Capture (Enter)">{I('camera')}Capture</button>
</div>'''
inner = (sel(sx, sy, sw, sh)
  + f'<div class="ochip accent" style="left:{sx}px;top:{sy-34}px">800 × 600 <span class="dimt">at {sx}, {sy}</span></div>'
  + tb + H_STD)
ov('03-region-adjust-fixed-size', 'Region overlay – adjust and fixed size', inner)

# 04 snap
sx, sy, sw, sh = 1060, 530, 470, 330
ptr = (1066, 530)
col = px(C, 'desk-a', 1060, 530)
inner = (sel(sx, sy, sw, sh, 'live', hd=False)
  + f'<div style="position:absolute;left:1059px;top:470px;width:2px;height:532px;background:var(--annota-violet);box-shadow:0 0 0 1px rgb(255 255 255 / 60%)"></div>'
  + f'<div style="position:absolute;left:1060px;top:470px;width:770px;height:532px;outline:1px dashed rgb(139 92 246 / 90%);border-radius:9px"></div>'
  + f'<div class="ochip violet" style="left:{1076}px;top:{482}px">{I("check")}Snapped to window edge</div>'
  + f'<div class="ochip" style="left:{sx+sw-90}px;top:{sy+sh+10}px">470 × 330</div>'
  + CROSS.format(x=sx+sw, y=sy+sh)
  + loupe(sx+sw+26, sy+sh+26 - 260, sx+sw, sy+sh)
  + hint([f'Edges snap within 8 px', f'{keys("Alt",cls="okey")} Hold to place freely', f'{keys("Esc",cls="okey")} Cancel']))
ov('04-region-snap-window-edge', 'Region overlay – snap to window edge', inner)

# 05 Wayland portal window picker (system UI)
pcss = '''
.shellui{position:absolute;inset:0;background:rgb(0 0 6 / 82%)}
.pw{position:absolute;border-radius:10px;overflow:hidden;box-shadow:0 6px 20px rgb(0 0 6 / 45%)}
.pw img{width:100%;height:100%;display:block}
.pw.hov{box-shadow:0 0 0 4px #ffffff,0 6px 20px rgb(0 0 6 / 45%)}
.spanel{position:absolute;left:50%;bottom:48px;transform:translateX(-50%);width:440px;padding:14px 16px 18px;border-radius:24px;background:#2e2e32;color:#fff;display:flex;flex-direction:column;align-items:center;gap:14px;box-shadow:0 8px 30px rgb(0 0 6 / 50%)}
.spanel .ttl{font-weight:700}
.spanel .r{display:flex;align-items:center;gap:18px}
.spanel .tgg{display:flex;gap:4px;padding:4px;border-radius:14px;background:rgb(255 255 255 / 8%)}
.spanel .tgg span{width:64px;height:44px;border-radius:10px;display:grid;place-items:center}
.spanel .tgg span.on{background:rgb(255 255 255 / 22%)}
.spanel .shut{width:58px;height:58px;border-radius:50%;border:4px solid #fff;display:grid;place-items:center}
.spanel .shut i{width:44px;height:44px;border-radius:50%;background:#fff;display:block}
.spanel .x{position:absolute;right:12px;top:12px;width:28px;height:28px;border-radius:50%;background:rgb(255 255 255 / 12%);display:grid;place-items:center}
.ann{position:absolute;left:36px;top:60px;width:430px;padding:14px 16px;border-radius:var(--radius-l);background:var(--view-bg);color:var(--view-fg);box-shadow:0 0 0 2px var(--annota-violet),var(--shadow-popover);font-size:var(--fs-small);line-height:1.45}
.ann b{display:block;font-size:var(--fs-body);margin-bottom:4px;color:var(--annota-violet)}
'''
inner = f'''<div class="shellui"></div>
<div class="pw" style="left:220px;top:190px;width:760px;height:475px"><img src="{SP}b-lumen-checkout.png"></div>
<div class="pw hov" style="left:1040px;top:170px;width:660px;height:412px"><img src="{SP}a-harbor-backup.png" style="object-fit:cover;object-position:50% 45%;transform:scale(1.42);transform-origin:50% 45%"></div>
<div class="pw" style="left:1100px;top:640px;width:520px;height:325px"><img src="{SP}d-tidewater-analytics.png"></div>
<div class="spanel"><span class="x">{I('close')}</span><span class="ttl">Screenshot</span>
 <div class="r"><div class="tgg"><span>{I('capture-region')}</span><span>{I('capture-screen')}</span><span class="on">{I('capture-window')}</span></div><span class="shut"><i></i></span><div class="tgg"><span style="width:44px">{I('cursor')}</span></div></div></div>
{ARROWPTR.format(x=1360, y=380)}
<div class="ann"><b>System UI — not drawn by Annota</b>On GNOME Wayland, Window mode calls the Screenshot portal with <span class="monospace">interactive=true</span>. GNOME Shell shows its own picker (representation, actual look depends on the GNOME version). The picked window opens in the Annota editor; Esc / close returns without a capture and without an error.</div>'''
ov('05-window-wayland-portal-picker', 'Window mode – portal picker (Wayland)', inner, pcss)

# 06 X11 window hover
inner = f'''<div class="dim-all" style="background:rgb(0 0 6 / 40%)"></div>
<div style="position:absolute;left:1060px;top:470px;width:770px;height:532px;border-radius:9px;background-image:url(assets/desk-a-light@1x.png);background-position:-1060px -470px;box-shadow:0 0 0 3px var(--accent-bg),0 0 0 6px rgb(53 132 228 / 35%)" class="shot"></div>
<div class="ochip accent" style="left:1060px;top:434px">{I('capture-window')}Harbor Preferences <span class="dimt" style="opacity:.85">770 × 532</span></div>
{ARROWPTR.format(x=1420, y=700)}
''' + hint([f'Click a window to capture it', f'{keys("Tab",cls="okey")} Next window', f'{keys("Enter",cls="okey")} Capture', f'{keys("Esc",cls="okey")} Cancel'])
ov('06-window-x11-hover', 'Window mode – X11 click to pick', inner)

# 07 countdown
ccss = '''.cd{position:absolute;left:50%;top:50%;transform:translate(-50%,-50%);display:flex;flex-direction:column;align-items:center;gap:14px;padding:28px 36px 22px;border-radius:30px;background:var(--osd-bg);color:#fff;-webkit-backdrop-filter:blur(16px);backdrop-filter:blur(16px);box-shadow:var(--shadow-osd)}
.cd .ring{position:relative;width:148px;height:148px}.cd .ring svg{position:absolute;inset:0}
.cd .num{position:absolute;inset:0;display:grid;place-items:center;font-size:72px;font-weight:800;font-variant-numeric:tabular-nums}
.cd .t{font-size:var(--fs-body);font-weight:700}.cd .s{display:flex;gap:10px;align-items:center;font-size:var(--fs-small);opacity:.85}'''
import math
frac = 3/5; r = 66; L = 2*math.pi*r
inner = f'''<div class="cd"><div class="ring"><svg viewBox="0 0 148 148"><circle cx="74" cy="74" r="{r}" fill="none" stroke="rgb(255 255 255 / 18%)" stroke-width="8"/>
<circle cx="74" cy="74" r="{r}" fill="none" stroke="var(--annota-coral)" stroke-width="8" stroke-linecap="round" stroke-dasharray="{L*frac:.1f} {L:.1f}" transform="rotate(-90 74 74)"/></svg><span class="num">3</span></div>
<span class="t">Capturing region in 3 s</span>
<span class="s">{keys("Esc",cls="okey")} Cancel <span style="opacity:.5">·</span> {keys("Enter",cls="okey")} Capture now</span></div>
{ARROWPTR.format(x=600, y=660)}'''
ov('07-delay-countdown', 'Delayed capture – countdown', inner, ccss)

# 08 permission denied
alert = f'''<div class="dialog-host"><div class="dialog alert" style="width:420px;max-width:420px">
<div class="alert-body"><div style="display:flex;justify-content:center;margin-bottom:6px;color:var(--warning)">{I('warning','icon large')}</div>
<div class="alert-heading">Screen Access Denied</div>
<div>GNOME did not allow Annota to take a screenshot. To allow it, open <b>Settings</b>, choose <b>Apps → Annota</b> and turn on <b>Screenshots</b>. Then try again.</div>
<div class="dim-label" style="font-size:var(--fs-small)">Nothing was captured.</div></div>
<div class="alert-actions"><button class="btn">Close</button><button class="btn suggested-action focus">Open Settings</button></div></div></div>'''
write(C, '08-permission-denied', doc('Permission denied', editor('Checkout flow', '1600 × 1000 · Edited', std_canvas(), arrow_props(), std_tray(), overlay=alert, zoom='48 %')))

# 09 / 10 preferences
pcss = '''.pref-dlg{width:660px;max-width:660px;height:720px}
.pref-dlg .vs{display:flex;justify-content:center}
.pref-dlg .pbody{flex:1;overflow:hidden;background:var(--dialog-bg)}
.pref-dlg .preferences-page{padding:18px 30px;max-width:none;gap:24px}
.pref-dlg .banner{margin:0;border-radius:0}
.accel{display:inline-flex;gap:4px}
.accel kbd.key{font-size:var(--fs-caption)}
.pref-dlg .row .suffix .lbl{font-size:var(--fs-small);opacity:var(--dim-opacity)}
.pref-dlg .sct{font-size:var(--fs-small);opacity:var(--dim-opacity)}
'''
def sc_row(title, sub, accel, src=None, focus=False):
    a = ''.join(f'<kbd class="key">{k}</kbd>' for k in accel) if accel else '<span class="sct">Disabled</span>'
    s = f'<span class="subtitle">{sub}</span>' if sub else ''
    return f'<div class="row{" focus" if focus else ""}"><div class="text"><span class="title">{title}</span>{s}</div><div class="suffix"><span class="accel">{a}</span><span class="icon" style="opacity:.55">{I("go-next")}</span></div></div>'

def prefs(state):
    if state == 'fallback':
        banner = f'<div class="banner warning"><span class="banner-title">Global shortcuts are not active yet</span><button class="btn">Create GNOME Shortcuts</button></div>'
        sub = 'Not active yet'
        rows = (sc_row('Region capture', sub, ['Shift','Print'], focus=False)
              + sc_row('Window capture', sub, ['Super','Shift','W'])
              + sc_row('Screen capture', sub, ['Super','Shift','S']))
        desc = 'This system has no global-shortcuts portal. Annota adds GNOME keyboard shortcuts instead.'
        toast = ''
    else:
        banner = ''
        sub = 'GNOME shortcut · runs <span class="monospace">annota --capture {m}</span>'
        rows = (sc_row('Region capture', sub.format(m='region'), ['Shift','Print'])
              + sc_row('Window capture', sub.format(m='window'), ['Super','Shift','W'])
              + sc_row('Screen capture', sub.format(m='screen'), ['Super','Shift','S']))
        desc = 'Work in every app, even when Annota is closed'
        toast = '<div class="toast-overlay" style="bottom:30px"><div class="toast"><span class="toast-title">3 GNOME shortcuts created</span><button class="btn flat text-button">Open Keyboard Settings</button></div></div>'
    body = f'''<div class="dialog-host"><div class="dialog pref-dlg">
<div class="headerbar"><div class="start"></div><div class="title-widget" style="flex:0 0 auto"><div class="toggle-group flat vs"><span class="toggle">{I('settings')}General</span><span class="toggle active">{I('camera')}Capture</span><span class="toggle">{I('library')}Library</span></div></div><div class="end"><button class="btn flat circular">{I('close')}</button></div></div>
{banner}
<div class="pbody"><div class="preferences-page">
 <div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Global Shortcuts</span><span class="description">{desc}</span></div></div>
  <div class="boxed-list">{rows}</div></div>
 <div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Capture</span></div></div>
  <div class="boxed-list">
   <div class="row"><div class="text"><span class="title">Include pointer</span><span class="subtitle">Show the mouse pointer in captures</span></div><div class="suffix"><span class="switch"></span></div></div>
   <div class="row"><div class="text"><span class="title">Default delay</span></div><div class="suffix"><span class="spin" style="width:118px"><span class="value">0 s</span><button class="btn">{I('minus')}</button><button class="btn">{I('plus')}</button></span></div></div>
   <div class="row"><div class="text"><span class="title">Capture all monitors</span><span class="subtitle">Screen mode captures every monitor instead of the one under the pointer</span></div><div class="suffix"><span class="switch"></span></div></div>
   <div class="row"><div class="text"><span class="title">After capture</span></div><div class="suffix"><span class="dropdown flat">Open in Editor</span></div></div>
  </div></div>
</div></div>{toast}
</div></div>'''
    return body

write(C, '09-prefs-shortcuts-fallback', doc('Preferences – shortcuts fallback', editor('Checkout flow', '1600 × 1000 · Edited', std_canvas(), arrow_props(), std_tray(), overlay=prefs('fallback'), zoom='48 %'), pcss))
write(C, '10-prefs-shortcuts-created', doc('Preferences – GNOME shortcuts created', editor('Checkout flow', '1600 × 1000 · Edited', std_canvas(), arrow_props(), std_tray(), overlay=prefs('created'), zoom='48 %'), pcss))

# 11 shortcut capture dialog
kcss = pcss + '''.kdlg{width:440px;max-width:440px}
.kdlg .kb{display:flex;flex-direction:column;align-items:center;gap:14px;padding:12px 24px 24px;text-align:center}
.kdlg .kart{width:160px;height:96px;border-radius:var(--radius-l);background:var(--fill-1);display:grid;place-items:center}
.kdlg .live{display:flex;gap:6px}.kdlg .live kbd.key{min-width:34px;height:34px;font-size:var(--fs-body);padding:0 10px}
'''
kd = f'''<div class="dialog-host" style="background:transparent"><div class="dialog kdlg">
<div class="headerbar"><div class="start"><button class="btn text-button">Cancel</button></div><div class="title-widget"><span class="title">Set Shortcut</span></div><div class="end"><button class="btn text-button suggested-action">Set</button></div></div>
<div class="kb"><div>Press the new shortcut for <b>Window capture</b></div>
<div class="kart">{I('keyboard','icon large')}</div>
<div class="live"><kbd class="key">Super</kbd><kbd class="key">Shift</kbd><kbd class="key">W</kbd></div>
<div class="dim-label" style="font-size:var(--fs-small)">Esc cancels · Backspace disables the shortcut</div>
<div class="hbox success" style="font-size:var(--fs-small);--gap:6px">{I('check')}Not used by GNOME or another app</div></div></div></div>'''
ov_body = prefs('fallback').replace('</div></div>\n</div></div>', '</div></div>\n</div></div>')
write(C, '11-shortcut-capture-dialog', doc('Set shortcut dialog', editor('Checkout flow', '1600 × 1000 · Edited', std_canvas(), arrow_props(), std_tray(), overlay=prefs('fallback') + kd, zoom='48 %'), kcss))

# 12 result in editor (400 x 300 region)
res_canvas = '<div class="canvas-image shot" style="width:400px;height:300px;background-image:url(assets/desk-a-light@1x.png);background-size:1920px 1080px;background-position:-140px -296px"></div>'
new_thumb = thumb('<div class="shot crop" style="background-size:480px 270px;background-position:-35px -74px"></div>', 'current')
props12 = f'''<div class="sidebar-pane end props"><div class="scroll">
 <div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Capture</span></div></div>
 <div class="boxed-list">
  <div class="row property"><div class="text"><span class="title">Size</span><span class="subtitle numeric">400 × 300 px</span></div></div>
  <div class="row property"><div class="text"><span class="title">Source</span><span class="subtitle">Region · scale 1×</span></div></div>
  <div class="row property"><div class="text"><span class="title">Captured</span><span class="subtitle">30 Oct 2026, 14:22:08</span></div></div>
 </div></div>
 <span class="hint2">Saved to the library automatically. Pick a tool to start annotating.</span>
</div></div>'''
toast = '<div class="toast-overlay" style="bottom:130px;left:52px;right:300px"><div class="toast"><span class="toast-title">Region captured · 400 × 300</span><button class="btn flat text-button">Copy</button></div></div>'
write(C, '12-region-result-in-editor', doc('Region capture result', editor('Capture 2026-10-30 14-22-08', '400 × 300 · Not saved', res_canvas, props12, std_tray(extra_first=new_thumb), overlay=toast, active='select', zoom='100 %'), desk_css('desk-a')))

# 13 paste from clipboard
pc = f'''<div class="canvas-image" style="width:768px;height:480px"><img src="{SP}d-tidewater-analytics.png" alt=""></div>'''
nt = thumb(img_thumb('d-tidewater-analytics','0% 0%'), 'current')
props13 = props12.replace('400 × 300 px','1600 × 1000 px').replace('Region · scale 1×','Clipboard').replace('30 Oct 2026, 14:22:08','30 Oct 2026, 14:25:41')
toast = '<div class="toast-overlay" style="bottom:130px;left:52px;right:300px"><div class="toast"><span class="toast-title">New capture from clipboard</span><button class="btn flat text-button">Undo</button></div></div>'
write(C, '13-paste-from-clipboard', doc('Paste from clipboard', editor('Pasted Image', '1600 × 1000 · Not saved', pc, props13, std_tray(extra_first=nt), overlay=toast, active='select', zoom='48 %')))

# 14 drop file
drop = f'''<div style="position:absolute;left:52px;top:47px;right:300px;bottom:107px;background:var(--canvas-bg);opacity:.9;z-index:19"></div><div style="position:absolute;left:58px;top:53px;right:306px;bottom:113px;border-radius:var(--radius-l);border:2px dashed var(--accent-bg);background:var(--accent-a12);display:flex;flex-direction:column;align-items:center;justify-content:center;gap:10px;z-index:20">
<div style="width:72px;height:72px;border-radius:50%;background:var(--accent-bg);color:var(--accent-fg);display:grid;place-items:center">{I('image','icon large')}</div>
<div class="title-4">Drop to Open as New Capture</div><div class="dim-label">PNG, JPG or WebP · drop onto an annotation tool to insert instead</div></div>
<div style="position:absolute;left:640px;top:500px;z-index:21;width:120px;height:80px;border-radius:6px;overflow:hidden;box-shadow:0 8px 20px rgb(0 0 6/35%);transform:rotate(-4deg);opacity:.92"><img src="{SP}e-relay-chat.png" style="width:100%;height:100%;object-fit:cover"></div>
<div style="position:absolute;left:740px;top:568px;z-index:22;padding:2px 8px;border-radius:9px;background:var(--accent-bg);color:#fff;font-size:12px;font-weight:700">booking-chat.png</div>'''
write(C, '14-drop-image-file', doc('Drop image file', editor('Checkout flow', '1600 × 1000 · Edited', std_canvas(), arrow_props(), std_tray(), overlay=drop, zoom='48 %')))
print('done')
