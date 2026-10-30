import pathlib
from PIL import Image
D = pathlib.Path('/home/claude/annota/docs/design')
SP = '../../kit/sample-content/png/'

def px(change, asset, x, y):
    im = Image.open(D / change / 'src/assets' / f'{asset}-light@1x.png').convert('RGB')
    return '#%02X%02X%02X' % im.getpixel((x, y))

def I(name, cls='icon'):
    return f'<svg class="{cls}"><use href="#i-{name}"/></svg>'

def keys(*ks, cls='key'):
    if cls == 'key':
        return '<span class="keys">' + ''.join(f'<kbd class="key">{k}</kbd>' for k in ks) + '</span>'
    return '<span class="keys">' + ''.join(f'<kbd class="{cls}">{k}</kbd>' for k in ks) + '</span>'

BASE_CSS = r'''
.keys{display:inline-flex;gap:4px;align-items:center}
kbd.okey{display:inline-flex;align-items:center;justify-content:center;min-width:22px;height:22px;padding:0 6px;border-radius:5px;background:var(--osd-fill-hover);color:var(--osd-fg);font-family:var(--font-ui);font-size:var(--fs-caption);font-weight:var(--fw-bold)}
/* ---------- overlay ---------- */
.ov{position:relative;width:1920px;height:1080px;overflow:hidden;background:#000}
.desk{position:absolute;inset:0;background-size:1920px 1080px;background-repeat:no-repeat}
.dim-all{position:absolute;inset:0;background:rgb(0 0 6 / 48%)}
.selr{position:absolute;border:1px solid #ffffff;box-shadow:0 0 0 1px rgb(0 0 6 / 40%),0 0 0 9999px rgb(0 0 6 / 48%)}
.selr.live{box-shadow:0 0 0 1px rgb(0 0 6 / 40%),0 0 0 9999px rgb(0 0 6 / 32%)}
.selr.plain{box-shadow:0 0 0 1px rgb(0 0 6 / 40%)}
.selr .h{position:absolute;width:12px;height:12px;margin:-6px 0 0 -6px;border-radius:50%;background:#ffffff;border:2px solid var(--accent-bg);box-shadow:0 1px 3px rgb(0 0 6 / 35%)}
.ochip{position:absolute;display:inline-flex;align-items:center;gap:6px;padding:4px 9px;border-radius:var(--radius-s);background:var(--osd-bg-solid);color:var(--osd-fg);font-size:var(--fs-small);font-weight:var(--fw-bold);font-variant-numeric:tabular-nums;white-space:nowrap;box-shadow:0 1px 4px rgb(0 0 6 / 35%)}
.ochip .icon{width:14px;height:14px}
.ochip.violet{background:var(--annota-violet)}
.ochip.accent{background:var(--accent-bg);color:var(--accent-fg)}
.ochip .sw{width:14px;height:14px;border-radius:3px;box-shadow:inset 0 0 0 1px rgb(255 255 255 / 45%)}
.ochip .dimt{opacity:.7;font-weight:var(--fw-regular)}
.hint{position:absolute;left:50%;transform:translateX(-50%);display:inline-flex;align-items:center;gap:8px;padding:7px 14px;border-radius:var(--radius-pill);background:var(--osd-bg);color:var(--osd-fg);font-size:var(--fs-small);white-space:nowrap;-webkit-backdrop-filter:blur(12px);backdrop-filter:blur(12px);box-shadow:var(--shadow-osd)}
.hint .dot{opacity:.5}
.otb{position:absolute;left:50%;transform:translateX(-50%);display:inline-flex;align-items:center;gap:6px;padding:7px;border-radius:var(--radius-xl);background:var(--osd-bg);color:var(--osd-fg);box-shadow:var(--shadow-osd);-webkit-backdrop-filter:blur(14px);backdrop-filter:blur(14px)}
.otb .btn{background:transparent;color:inherit}
.otb .btn.active{background:var(--osd-fill-active)}
.otb .btn.suggested-action{background:var(--accent-bg);color:var(--accent-fg)}
.otb .sep{width:1px;height:24px;background:rgb(255 255 255 / 20%);margin:0 4px}
.otb .tg{display:inline-flex;gap:3px;padding:3px;border-radius:var(--radius-m);background:var(--osd-fill)}
.otb .tg .btn{min-height:30px;border-radius:var(--radius-s);padding:0 10px;font-size:var(--fs-small)}
.otb .tg .btn.image-button{width:34px;padding:0}
.otb .tg .btn.active{background:var(--osd-fill-active)}
.otb .oentry{display:inline-flex;align-items:center;gap:6px;height:34px;padding:0 8px;border-radius:var(--radius-s);background:var(--osd-fill);font-variant-numeric:tabular-nums;min-width:74px;outline:2px solid transparent;outline-offset:-2px}
.otb .oentry .k{opacity:.6;font-size:var(--fs-small)}
.otb .oentry.focus{outline-color:rgb(120 174 237 / 80%)}
.otb .oentry .caret{display:inline-block;width:1px;height:18px;background:currentColor;margin-left:-4px}
.otb .times{opacity:.6}
.otb .lbl{font-size:var(--fs-small);padding:0 4px;font-variant-numeric:tabular-nums;white-space:nowrap}
.otb .odrop{display:inline-flex;align-items:center;gap:6px;height:34px;padding:0 8px 0 10px;border-radius:var(--radius-s);font-weight:var(--fw-bold);font-size:var(--fs-small)}
.otb .odrop:after{content:"";width:16px;height:16px;background:currentColor;-webkit-mask:url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'%3E%3Cpath d='M3.3 5.8a1 1 0 0 1 1.4 0L8 9.1l3.3-3.3a1 1 0 1 1 1.4 1.4l-4 4a1 1 0 0 1-1.4 0l-4-4a1 1 0 0 1 0-1.4z'/%3E%3C/svg%3E") center/16px no-repeat}
.loupe{position:absolute;width:136px;height:136px;border-radius:50%;overflow:hidden;image-rendering:pixelated;background-repeat:no-repeat;box-shadow:0 0 0 2px #ffffff,0 0 0 3px rgb(0 0 6 / 35%),0 8px 22px rgb(0 0 6 / 45%)}
.loupe:before{content:"";position:absolute;inset:0;background:repeating-linear-gradient(90deg,rgb(0 0 6 / 9%) 0 1px,transparent 1px 8px),repeating-linear-gradient(0deg,rgb(0 0 6 / 9%) 0 1px,transparent 1px 8px)}
.loupe .cx{position:absolute;left:64px;top:64px;width:8px;height:8px;outline:2px solid #ffffff;box-shadow:0 0 0 3px rgb(0 0 6 / 55%)}
.loupe .lv{position:absolute;left:67px;width:2px;background:rgb(255 90 78 / 85%)}
.loupe .lh{position:absolute;top:67px;height:2px;background:rgb(255 90 78 / 85%)}
.ptr{position:absolute;width:24px;height:24px;margin:-12px 0 0 -12px}
.note{position:absolute;max-width:360px;padding:10px 14px;border-radius:var(--radius-m);border:2px dashed var(--annota-violet);background:rgb(139 92 246 / 14%);color:#ffffff;font-size:var(--fs-small);line-height:1.4;text-shadow:0 1px 2px rgb(0 0 6 / 60%)}
.note b{display:block;margin-bottom:2px}
/* ---------- editor ---------- */
.tool-strip{width:52px;flex:none;display:flex;flex-direction:column;align-items:center;gap:4px;padding:6px 0;background:var(--window-bg);box-shadow:inset -1px 0 var(--separator)}
.tool-strip .btn{width:40px;min-height:40px}
.tool-strip .separator-h{width:28px;margin:2px 0}
.props .scroll{padding:18px 12px;display:flex;flex-direction:column;gap:18px}
.props .lbl{font-size:var(--fs-small);opacity:var(--dim-opacity);padding:0 2px}
.props .field{display:flex;flex-direction:column;gap:6px}
.props .boxed-list .row{min-height:46px;padding:6px 12px}
.props .hint2{font-size:var(--fs-small);opacity:var(--dim-opacity);line-height:1.35;padding:0 2px}
.editor-col{flex:1;min-width:0;display:flex;flex-direction:column;position:relative}
.editor-col>.canvas-area{flex:1}
.tray{flex:none;height:107px;display:flex;align-items:flex-start;gap:12px;padding:9px 12px 12px;background:var(--window-bg);box-shadow:inset 0 1px var(--separator);position:relative}
.tray .tgroup{display:flex;flex-direction:column;gap:6px;min-width:0}
.tray .tgroup.grow{flex:1;overflow:hidden}
.tray .tlabel{font-size:var(--fs-caption);font-weight:var(--fw-bold);opacity:var(--dim-opacity);height:15px;line-height:15px}
.tray .thumbs{display:flex;gap:6px}
.tray .thumb{width:100px;height:62px;flex:none;border-radius:var(--radius-s);overflow:hidden;position:relative;box-shadow:0 0 0 1px var(--borders);background:var(--view-bg)}
.tray .thumb>img,.tray .thumb>div{width:100%;height:100%;object-fit:cover;display:block}
.tray .thumb.current{box-shadow:0 0 0 2px var(--window-bg),0 0 0 4px var(--accent-bg)}
.tray .thumb .unsaved{position:absolute;top:4px;right:4px;width:8px;height:8px;border-radius:50%;background:var(--accent-bg);box-shadow:0 0 0 2px var(--view-bg)}
.tray .fade{position:absolute;right:0;top:0;bottom:0;width:60px;background:linear-gradient(to right,transparent,var(--window-bg))}
.crop{background-repeat:no-repeat}
'''

def doc(title, body, css='', vp='1280x800', flush=True, extra_head=''):
    return f'''<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport-size" content="{vp}">
<title>Annota — {title}</title>
<link rel="stylesheet" href="../../kit/tokens.css">
<link rel="stylesheet" href="../../kit/adwaita.css">
<style>{BASE_CSS}
{css}</style>{extra_head}
</head>
<body class="mock-frame{' flush' if flush else ''}">
{body}
<script src="../../kit/icons.js"></script>
</body>
</html>
'''

def desk_css(asset):
    return f'''.desk,.loupe{{background-image:url(assets/{asset}-light@1x.png)}}
:root[data-theme="dark"] .desk,:root[data-theme="dark"] .loupe{{background-image:url(assets/{asset}-dark@1x.png)}}
.shot{{background-image:url(assets/{asset}-light@1x.png)}}'''

def handles(w, h):
    pts = [(0,0),(w/2,0),(w,0),(w,h/2),(w,h),(w/2,h),(0,h),(0,h/2)]
    return ''.join(f'<span class="h" style="left:{x-1}px;top:{y-1}px"></span>' for x,y in pts)

def sel(x, y, w, h, cls='', inner='', hd=True):
    return f'<div class="selr {cls}" style="left:{x}px;top:{y}px;width:{w}px;height:{h}px">{handles(w,h) if hd else ""}{inner}</div>'

def loupe(lx, ly, x, y):
    # 17x17 pixels at 8x, centre pixel (x,y) at 64..72
    return (f'<div class="loupe" style="left:{lx}px;top:{ly}px;background-size:15360px 8640px;'
            f'background-position:{-(x-8)*8}px {-(y-8)*8}px"><span class="lv" style="top:0;height:62px"></span>'
            f'<span class="lv" style="top:74px;height:62px"></span><span class="lh" style="left:0;width:62px"></span>'
            f'<span class="lh" style="left:74px;width:62px"></span><span class="cx"></span></div>')

CROSS = '''<svg class="ptr" viewBox="0 0 24 24" style="left:{x}px;top:{y}px"><path d="M12 1v9M12 14v9M1 12h9M14 12h9" stroke="#000006" stroke-width="3.2" stroke-linecap="round"/><path d="M12 1.8v8M12 14.2v8M1.8 12h8M14.2 12h8" stroke="#fff" stroke-width="1.3" stroke-linecap="round"/></svg>'''
ARROWPTR = '''<svg style="position:absolute;left:{x}px;top:{y}px;width:22px;height:30px" viewBox="0 0 22 30"><path d="M2 2v22l6-5.5 4 9 4-1.8-4-8.7h8z" fill="#000006" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/></svg>'''
HANDPTR = '''<svg style="position:absolute;left:{x}px;top:{y}px;width:24px;height:26px" viewBox="0 0 24 26"><path d="M8 13V3.5a1.8 1.8 0 0 1 3.6 0V11l.2-2.2a1.8 1.8 0 0 1 3.6.3l-.1 2.3.4-1.5a1.8 1.8 0 0 1 3.5.8v.4a1.8 1.8 0 0 1 3.3 1V17c0 4.5-3 7.8-7.4 7.8h-1.6c-2.6 0-4.4-1.2-6-3.5L3 16.4a1.9 1.9 0 0 1 2.9-2.4z" fill="#fff" stroke="#000006" stroke-width="1.4" stroke-linejoin="round"/></svg>'''

TOOLS = [('select','Select','V'),None,('arrow','Arrow','A'),('line','Line','L'),('pen','Pen','P'),('shape','Shape','U'),
         ('text','Text','T'),('callout','Callout','K'),('step','Step','S'),None,('highlighter','Highlighter','H'),
         ('blur','Redact','B'),('magnify','Magnify','M'),('spotlight','Spotlight','G'),('stamp','Stamp','I'),None,
         ('crop','Crop','C'),('view-more','More Tools','')]

def toolstrip(active='arrow'):
    out = []
    for t in TOOLS:
        if t is None: out.append('<span class="separator-h"></span>'); continue
        n, l, k = t
        out.append(f'<button class="btn flat{" active" if n==active else ""}" title="{l}{f" ({k})" if k else ""}">{I(n)}</button>')
    return '<div class="tool-strip">' + ''.join(out) + '</div>'

def headerbar(title, subtitle, capture_cls='', copy=True, undo_disabled=True):
    return f'''<div class="headerbar">
    <div class="start">
      <button class="btn image-button {capture_cls}" title="New Capture (Shift+Print)">{I('capture-region')}</button>
      <div class="linked"><button class="btn image-button{' disabled' if undo_disabled else ''}" title="Undo (Ctrl+Z)">{I('undo')}</button><button class="btn image-button disabled" title="Redo (Ctrl+Shift+Z)">{I('redo')}</button></div>
    </div>
    <div class="title-widget"><span class="title">{title}</span><span class="subtitle">{subtitle}</span></div>
    <div class="end">
      <button class="btn image-button" title="Copy (Ctrl+C)">{I('copy')}</button>
      <button class="btn text-button suggested-action" title="Export (Ctrl+E)">Export</button>
      <button class="btn flat image-button" title="Main Menu">{I('open-menu')}</button>
      <div class="window-controls"><span class="wc">{I('close')}</span></div>
    </div>
  </div>'''

def arrow_props(extra=''):
    sw = ''.join(f'<span class="swatch{" selected" if c=="coral" else ""}" style="--c:var(--annota-{c})"></span>' for c in ['coral','amber','lime','teal','azure','violet','ink'])
    return f'''<div class="sidebar-pane end props"><div class="scroll">
      {extra}
      <div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Arrow</span></div></div>
        <div class="field"><span class="lbl">Colour</span><div class="swatch-row">{sw}</div></div></div>
      <div class="boxed-list">
        <div class="row"><div class="text"><span class="title">Width</span></div><div class="suffix"><span class="spin" style="width:120px"><span class="value">6 px</span><button class="btn">{I('minus')}</button><button class="btn">{I('plus')}</button></span></div></div>
        <div class="row"><div class="text"><span class="title">Shadow</span></div><div class="suffix"><span class="switch on"></span></div></div>
      </div>
    </div></div>'''

def thumb(inner, cls=''):
    return f'<div class="thumb {cls}">{inner}</div>'

def img_thumb(name, pos='0% 0%'):
    return f'<img src="{SP}{name}.png" style="object-position:{pos}" alt="">'

def tray(open_thumbs, recent=None):
    recent = recent or [img_thumb('d-tidewater-analytics'), img_thumb('a-harbor-backup','50% 50%'), img_thumb('e-relay-chat'), img_thumb('c-code-editor'), img_thumb('b-lumen-checkout','100% 0%'), img_thumb('f-field-notes')]
    return f'''<div class="tray">
      <div class="tgroup"><span class="tlabel">Open</span><div class="thumbs">{''.join(open_thumbs)}</div></div>
      <span class="separator-v" style="margin:21px 0 0"></span>
      <div class="tgroup grow"><span class="tlabel">Recent</span><div class="thumbs">{''.join(thumb(r) for r in recent)}</div></div>
      <div class="fade"></div>
    </div>'''

def editor(title, subtitle, canvas, props, tray_html, overlay='', capture_cls='', active='arrow', zoom='100 %', canvas_cls=''):
    return f'''<div class="window" style="width:1280px;height:800px">
  {headerbar(title, subtitle, capture_cls)}
  <div class="split-view">
    {toolstrip(active)}
    <div class="editor-col">
      <div class="canvas-area {canvas_cls}">
        {canvas}
        <div class="osd-toolbar" style="position:absolute;right:18px;bottom:18px">
          <button class="btn image-button" title="Zoom Out (Ctrl+−)">{I('zoom-out')}</button><span class="label">{zoom}</span><button class="btn image-button" title="Zoom In (Ctrl++)">{I('zoom-in')}</button>
        </div>
      </div>
      {tray_html}
    </div>
    {props}
  </div>
  {overlay}
</div>'''

def write(change, name, html):
    p = D / change / 'src' / f'{name}.html'
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(html)
