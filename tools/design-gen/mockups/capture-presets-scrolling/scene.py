"""Render desktop scene PNGs (content behind capture overlays) from kit sample PNGs."""
import pathlib, sys
from playwright.sync_api import sync_playwright
KIT = pathlib.Path('/home/claude/annota/docs/design/kit').as_uri()
SP = KIT + '/sample-content/png/'

def shell():
    return f'''<div class="shell"><div class="ws"><i class="on"></i><i></i><i></i></div>
<span>Fri 30 Oct  14:22</span>
<div class="st"><svg class="icon"><use href="#i-speaker"/></svg><svg class="icon"><use href="#i-settings"/></svg><svg class="icon"><use href="#i-pan-down"/></svg></div></div>'''

CSS = '''
body{margin:0;width:1920px;height:1080px;overflow:hidden;position:relative;
 background: radial-gradient(1400px 900px at 25% 15%, rgb(255 255 255 / 22%), transparent 60%), radial-gradient(900px 700px at 90% 100%, rgb(53 132 228 / 18%), transparent 70%), var(--desktop-bg);}
.shell{position:absolute;left:0;right:0;top:0;height:32px;background:#000006;color:#fff;display:flex;align-items:center;justify-content:center;font-size:14px;font-weight:700}
.shell .ws{position:absolute;left:16px;display:flex;gap:5px;align-items:center}.shell .ws i{width:7px;height:7px;border-radius:4px;background:rgb(255 255 255/45%)}.shell .ws i.on{width:30px;background:#fff}
.shell .st{position:absolute;right:16px;display:flex;gap:10px}
.w{position:absolute;border-radius:12px;overflow:hidden;box-shadow:0 1px 3px 1px rgb(0 0 6/18%),0 8px 28px rgb(0 0 6/22%),0 0 0 1px rgb(0 0 6/12%)}
.w img{display:block;width:100%;height:100%}
.crop{background-repeat:no-repeat}
'''

def scene_a():
    return f'''{shell()}
<div class="w" style="left:1150px;top:64px;width:720px;height:450px"><img src="{SP}d-tidewater-analytics.png"></div>
<div class="w" style="left:72px;top:80px;width:1200px;height:750px"><img src="{SP}b-lumen-checkout.png"></div>
<div class="w crop" style="left:1060px;top:470px;width:770px;height:532px;border-radius:9px;background-image:url({SP}a-harbor-backup.png);background-size:1120px 700px;background-position:-175px -84px"></div>'''

def scene_s(off, gap=False):
    # browser window 260..1660 x 70..1030 ; content 1400 wide, scale .875
    s = .875
    return f'''{shell()}
<div class="w crop" style="left:1180px;top:120px;width:640px;height:400px;background-image:url({SP}e-relay-chat.png);background-size:1024px 640px;background-position:-384px 0"></div>
<div class="w window" style="left:260px;top:70px;width:1400px;height:960px;border-radius:12px">
 <div class="headerbar">
  <div class="start"><div class="linked"><button class="btn image-button"><svg class="icon"><use href="#i-go-previous"/></svg></button><button class="btn image-button"><svg class="icon"><use href="#i-go-next"/></svg></button></div></div>
  <div class="title-widget" style="flex:0 0 560px"><span class="entry" style="width:560px;justify-content:center;font-size:14px">fieldnotes.example/coast/the-tide-clock</span></div>
  <div class="end"><button class="btn flat image-button"><svg class="icon"><use href="#i-open-menu"/></svg></button><div class="window-controls"><span class="wc"><svg class="icon"><use href="#i-close"/></svg></span></div></div>
 </div>
 <div style="position:relative;flex:1;overflow:hidden;background:#fbfaf7">
  <div class="crop" style="position:absolute;left:0;top:{60}px;width:1400px;height:900px;background-image:url({SP}f-field-notes.png);background-size:1400px {2600*s}px;background-position:0 {-(68+off)*s}px"></div>
  <div class="crop" style="position:absolute;left:0;top:0;width:1400px;height:60px;background-image:url({SP}f-field-notes.png);background-size:1400px {2600*s}px;background-position:0 0;box-shadow:0 1px 0 rgb(0 0 6/10%),0 2px 6px rgb(0 0 6/8%)"></div>
  <div style="position:absolute;right:3px;top:{66+off/2600*840}px;width:6px;height:{913/(2600*s)*840}px;border-radius:3px;background:rgb(0 0 6/35%)"></div>
 </div>
</div>'''

def page(body, theme):
    return f'''<!doctype html><html data-theme="{theme}"><head><meta charset="utf-8">
<link rel="stylesheet" href="{KIT}/tokens.css"><link rel="stylesheet" href="{KIT}/adwaita.css"><style>{CSS}</style></head>
<body>{body}<script src="{KIT}/icons.js"></script></body></html>'''

def render(jobs):
    with sync_playwright() as pw:
        b = pw.chromium.launch()
        for out, body in jobs:
            out = pathlib.Path(out); out.parent.mkdir(parents=True, exist_ok=True)
            for theme in ('light', 'dark'):
                for sc in (1, 2):
                    p = b.new_page(viewport={'width':1920,'height':1080}, device_scale_factor=sc, color_scheme=theme)
                    # windows/browser content stay light (they are captured content); only wallpaper follows theme
                    html = page(body, 'light').replace('var(--desktop-bg)', '#c7c9d1' if theme=='light' else '#3d3f47')
                    f = out.parent / '_tmp.html'; f.write_text(html)
                    p.goto(f.as_uri()); p.wait_for_function("document.documentElement.dataset.iconsReady==='1'")
                    p.evaluate("document.fonts.ready"); p.wait_for_timeout(150)
                    p.screenshot(path=str(out.parent / f"{out.name}-{theme}@{sc}x.png")); p.close(); f.unlink()
                    print(out.name, theme, sc)
        b.close()

D = '/home/claude/annota/docs/design/'
render([
  (D+'add-screen-capture/src/assets/desk-a', scene_a()),
  (D+'add-capture-presets/src/assets/desk-a', scene_a()),
  (D+'add-scrolling-capture/src/assets/desk-s0', scene_s(0)),
  (D+'add-scrolling-capture/src/assets/desk-s600', scene_s(600)),
  (D+'add-scrolling-capture/src/assets/desk-s1100', scene_s(1100)),
  (D+'add-scrolling-capture/src/assets/desk-s1500', scene_s(1500)),
])
