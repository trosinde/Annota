import sys; sys.path.insert(0, __file__.rsplit("/", 1)[0])
from common import *

C = "add-screen-recording"

CSS = """
  .rec-panel { position: absolute; z-index: 62; background: var(--popover-bg); color: var(--popover-fg); border-radius: 12px; box-shadow: var(--shadow-popover); padding: 12px; width: 380px; display: flex; flex-direction: column; gap: 12px; }
  .rec-panel .boxed-list { box-shadow: 0 0 0 1px var(--borders); }
  .rec-panel .row { min-height: 44px; padding: 4px 12px; }
  .rec-panel .row .title { font-size: var(--fs-small); }
  .rec-panel .row .subtitle { font-size: var(--fs-caption); }
  .meter { display: inline-flex; gap: 2px; align-items: center; }
  .meter i { width: 4px; height: 10px; border-radius: 1px; background: var(--fill-2); }
  .meter i.on { background: var(--success-bg); }
  .meter i.hot { background: var(--warning-bg); }
  .osd-meter i { background: rgb(255 255 255 / 25%); }
  .sel-frame { position: absolute; outline: 2px solid #fff; box-shadow: 0 0 0 1px rgb(0 0 6 / 40%); }
  .sel-frame .h { position: absolute; width: 12px; height: 12px; margin: -6px 0 0 -6px; background: #fff; border-radius: 50%; box-shadow: 0 0 0 1px rgb(0 0 6 / 40%); }
  .rec-frame { position: absolute; outline: 2px dashed var(--destructive-bg); outline-offset: 2px; }
  .rec-ctl { position: absolute; z-index: 63; gap: 2px; }
  .rec-ctl .label.time { font-size: 15px; min-width: 62px; }
  .countdown { position: absolute; width: 180px; height: 180px; border-radius: 50%; background: var(--osd-bg); color: #fff; display: grid; place-items: center; font-size: 96px; font-weight: 800; font-variant-numeric: tabular-nums; box-shadow: var(--shadow-osd); }
  .shape-pick { display: flex; gap: 6px; }
  .shape-pick .opt { width: 34px; height: 34px; display: grid; place-items: center; border-radius: var(--radius-s); background: var(--fill-1); }
  .shape-pick .opt.active { background: var(--accent-bg); color: #fff; }
  .shape-pick .opt i { width: 16px; height: 16px; border: 2px solid currentColor; display: block; }
  .pos-grid { display: grid; grid-template-columns: repeat(3, 14px); gap: 3px; padding: 4px; border-radius: var(--radius-s); background: var(--fill-1); }
  .pos-grid i { width: 14px; height: 9px; border-radius: 2px; background: var(--fill-2); }
  .pos-grid i.on { background: var(--accent-bg); }
"""


def meter(n_on, total=8, cls=""):
    return f'<span class="meter {cls}">' + "".join(
        f'<i class="{"hot" if i >= 6 and i < n_on else ("on" if i < n_on else "")}"></i>' for i in range(total)) + '</span>'


def cursor(x, y, ring=True, ripple=False):
    s = ""
    if ring:
        s += f'<circle cx="{x}" cy="{y}" r="26" fill="rgb(245 165 36 / 28%)" stroke="var(--annota-amber)" stroke-width="2.5"/>'
    if ripple:
        s += (f'<circle cx="{x}" cy="{y}" r="40" fill="none" stroke="var(--annota-coral)" stroke-width="2" opacity=".35"/>'
              f'<circle cx="{x}" cy="{y}" r="30" fill="none" stroke="var(--annota-coral)" stroke-width="3" opacity=".7"/>')
    s += (f'<path d="M{x} {y} l0 20 l5 -4.5 l3.6 7.6 l3.2 -1.5 l-3.5 -7.4 l6.7 -0.5 z" fill="#fff" stroke="#000" '
          f'stroke-width="1.4" stroke-linejoin="round"/>')
    return s


def desktop(sample, inner, rec=None, svg=""):
    return (f'<div class="desktop">{shell_panel(rec)}'
            f'<img class="screen-img" src="{S}{sample}.png" style="height:800px" alt="">'
            f'{inner}<svg style="position:absolute;inset:0;z-index:59" width="1280" height="800">{svg}</svg></div>')


def scrims(x, y, w, h):
    return (f'<div class="scrim" style="left:0;top:32px;width:1280px;height:{y - 32}px"></div>'
            f'<div class="scrim" style="left:0;top:{y}px;width:{x}px;height:{h}px"></div>'
            f'<div class="scrim" style="left:{x + w}px;top:{y}px;width:{1280 - x - w}px;height:{h}px"></div>'
            f'<div class="scrim" style="left:0;top:{y + h}px;width:1280px;height:{800 - y - h}px"></div>')


def sel_frame(x, y, w, h):
    hs = "".join(f'<span class="h" style="left:{a}px;top:{b}px"></span>' for a, b in
                 [(0, 0), (w / 2, 0), (w, 0), (0, h / 2), (w, h / 2), (0, h), (w / 2, h), (w, h)])
    return f'<div class="sel-frame" style="left:{x}px;top:{y}px;width:{w}px;height:{h}px">{hs}</div>'


REG = (130, 92, 700, 470)  # region over the Tidewater dashboard


def webcam(x, y, size, shape="circle"):
    rad = {"circle": "50%", "rounded": "22%", "square": "4px"}[shape]
    return (f'<div style="position:absolute;left:{x}px;top:{y}px;width:{size}px;height:{size}px;border-radius:{rad};overflow:hidden;'
            f'box-shadow:0 0 0 3px #fff, 0 4px 14px rgb(0 0 6 / 35%);z-index:58">'
            f'<svg width="{size}" height="{size}" viewBox="0 0 {size} {size}">{avatar_svg(0, 0, size, "square", "#bcd0dc")}</svg></div>')


def setup_panel(x, y, webcam_open=False, arrow_x=190):
    area = ('<div class="toggle-group" style="display:flex">'
            f'<span class="toggle active" style="flex:1">{icon("capture-region")}Region</span>'
            f'<span class="toggle" style="flex:1">{icon("capture-window")}Window</span>'
            f'<span class="toggle" style="flex:1">{icon("capture-screen")}Screen</span></div>')
    audio = ('<div class="boxed-list">'
             f'<div class="row"><div class="prefix">{icon("microphone")}</div><div class="text"><span class="title">Microphone</span>'
             f'<span class="subtitle">Built-in Audio</span></div><div class="suffix">{meter(5)}<span class="switch on"></span></div></div>'
             f'<div class="row"><div class="prefix">{icon("speaker")}</div><div class="text"><span class="title">System audio</span></div>'
             f'<div class="suffix">{meter(0)}<span class="switch"></span></div></div></div>')
    if webcam_open:
        cam = ('<div class="boxed-list">'
               f'<div class="row"><div class="prefix">{icon("camera")}</div><div class="text"><span class="title">Webcam</span>'
               f'<span class="subtitle">Integrated Camera</span></div><div class="suffix"><span class="switch on"></span></div></div>'
               '<div class="row"><div class="text"><span class="title">Shape</span></div><div class="suffix"><div class="shape-pick">'
               '<span class="opt active"><i style="border-radius:50%"></i></span><span class="opt"><i style="border-radius:4px"></i></span>'
               '<span class="opt"><i></i></span></div></div></div>'
               '<div class="row"><div class="text"><span class="title">Position</span></div><div class="suffix"><span class="pos-grid">'
               + "".join(f'<i class="{"on" if k == 8 else ""}"></i>' for k in range(9)) + '</span></div></div>'
               '<div class="row"><div class="text"><span class="title">Size</span></div><div class="suffix" style="width:170px">'
               '<div class="scale" style="flex:1"><div class="trough"><div class="fill" style="width:35%"></div><div class="knob" style="left:35%"></div></div></div>'
               '<span class="caption" style="width:44px;text-align:right">160 px</span></div></div></div>')
    else:
        cam = ('<div class="boxed-list">'
               f'<div class="row"><div class="prefix">{icon("camera")}</div><div class="text"><span class="title">Webcam</span>'
               f'<span class="subtitle">Circle · bottom right</span></div><div class="suffix">{icon("go-next")}<span class="switch on"></span></div></div></div>')
    fx = ('<div class="boxed-list">'
          f'<div class="row"><div class="prefix">{icon("cursor")}</div><div class="text"><span class="title">Highlight cursor</span></div><div class="suffix"><span class="switch on"></span></div></div>'
          f'<div class="row"><div class="prefix">{icon("effects")}</div><div class="text"><span class="title">Animate clicks</span></div><div class="suffix"><span class="switch on"></span></div></div>'
          f'<div class="row"><div class="prefix">{icon("timer")}</div><div class="text"><span class="title">Countdown</span></div><div class="suffix">'
          f'<span class="spin" style="width:108px"><span class="value">3 s</span><button class="btn">{icon("minus")}</button><button class="btn">{icon("plus")}</button></span></div></div></div>')
    if webcam_open:
        fx = ""
    btn = (f'<button class="btn pill destructive-action" style="width:100%;min-height:40px;padding:8px">{icon("record")}Start Recording'
           f'<kbd class="key" style="margin-left:6px;background:rgb(255 255 255 / 20%);box-shadow:none">Enter</kbd></button>')
    return (f'<div class="rec-panel popover" style="left:{x}px;top:{y}px;position:absolute;--arrow-x:{arrow_x}px">'
            f'{area}{audio}{cam}{fx}{btn}</div>')


# 01 setup popover
d = desktop("d-tidewater-analytics", scrims(*REG) + sel_frame(*REG)
            + f'<div class="osd-toolbar" style="position:absolute;left:{REG[0]}px;top:{REG[1] - 46}px;z-index:61"><span class="label">700 × 470</span></div>'
            + setup_panel(REG[0] + REG[2] + 18, 92).replace("rec-panel popover", "rec-panel popover arrow-left").replace("--arrow-x:190px", "--arrow-y:40px")
            + '<div class="osd-toolbar pill" style="position:absolute;left:50%;transform:translateX(-50%);bottom:24px;z-index:61;gap:6px">'
              f'<div class="toggle-group" style="background:rgb(255 255 255 / 10%)"><span class="toggle">{icon("camera")}Screenshot</span>'
              f'<span class="toggle active" style="background:rgb(255 255 255 / 25%)">{icon("record")}Video</span></div>'
              '<span class="label" style="font-weight:400;opacity:.8">Drag to change the area · Esc to cancel</span></div>')
write(C, "01-recording-setup", f"<style>{CSS}</style>" + d, "recording setup")

# 02 webcam options open
d = desktop("d-tidewater-analytics", scrims(*REG) + sel_frame(*REG) + webcam(REG[0] + REG[2] - 178, REG[1] + REG[3] - 178, 160)
            + setup_panel(REG[0] + REG[2] + 18, 92, webcam_open=True).replace("rec-panel popover", "rec-panel popover arrow-left").replace("--arrow-x:190px", "--arrow-y:40px"))
write(C, "02-webcam-overlay-options", f"<style>{CSS}</style>" + d, "webcam options")

# 03 countdown
d = desktop("d-tidewater-analytics", scrims(*REG) + f'<div class="rec-frame" style="left:{REG[0]}px;top:{REG[1]}px;width:{REG[2]}px;height:{REG[3]}px"></div>'
            + f'<div class="countdown" style="left:{REG[0] + REG[2] / 2 - 90}px;top:{REG[1] + REG[3] / 2 - 110}px">3</div>'
            + f'<div class="osd-toolbar pill" style="position:absolute;left:{REG[0] + REG[2] / 2 - 150}px;top:{REG[1] + REG[3] / 2 + 90}px;z-index:61;width:300px;justify-content:center">'
              f'<span class="label" style="font-weight:400">Recording starts in 3 s</span><span class="sep"></span><button class="btn text-button" style="padding:0 12px">Cancel</button></div>')
write(C, "03-countdown", f"<style>{CSS}</style>" + d, "countdown")


def control(x, y, time="00:12", paused=False, mic_muted=False, draw=False, swapped=False):
    dot = '<span class="rec-dot"></span>' if not paused else f'<span style="color:var(--warning)">{icon("pause")}</span>'
    return (f'<div class="osd-toolbar pill rec-ctl" style="left:{x}px;top:{y}px">'
            f'<span style="padding-left:8px;display:flex">{dot}</span><span class="label time">{time}</span><span class="sep"></span>'
            f'<button class="btn image-button{" active" if paused else ""}" title="{"Resume" if paused else "Pause"}">{icon("play" if paused else "pause")}</button>'
            f'<button class="btn image-button destructive-action" title="Stop (Shift+Ctrl+Print)" style="border-radius:50%">{icon("stop")}</button>'
            f'<span class="sep"></span>'
            f'<button class="btn image-button{" active" if draw else ""}" title="Draw">{icon("pen")}</button>'
            f'<button class="btn{" active" if mic_muted else ""}" title="Mute Microphone" style="gap:6px;padding:0 8px">'
            f'<span class="{"mic-off" if mic_muted else ""}" style="display:flex">{icon("microphone")}</span>{meter(0 if mic_muted else 5, 5, "osd-meter")}</button>'
            f'<button class="btn{" active" if swapped else ""}" title="Swap Screen and Webcam" style="gap:6px;padding:0 10px">{icon("camera")}<span style="font-size:13px">{"Show Screen" if swapped else "Swap"}</span></button>'
            f'<button class="btn image-button">{icon("view-more")}</button></div>')


rec_frame = f'<div class="rec-frame" style="left:{REG[0]}px;top:{REG[1]}px;width:{REG[2]}px;height:{REG[3]}px"></div>'
CTL = (REG[0] + 140, REG[1] + REG[3] + 18)
cam = webcam(REG[0] + REG[2] - 178, REG[1] + REG[3] - 178, 160)

# 04 recording: control, cursor ring + click ripple
d = desktop("d-tidewater-analytics", rec_frame + cam + control(*CTL), rec="00:12", svg=cursor(560, 150, True, True))
write(C, "04-recording-control", f"<style>{CSS}</style>" + d, "recording")

# 05 paused + mic muted
d = desktop("d-tidewater-analytics", rec_frame + cam + control(*CTL, time="00:27", paused=True, mic_muted=True)
            + f'<div class="tooltip" style="left:{CTL[0] + 230}px;top:{CTL[1] + 54}px">Microphone muted<span class="shortcut">M</span></div>'
            + f'<div class="osd-toolbar pill" style="position:absolute;left:{REG[0] + REG[2] / 2 - 90}px;top:{REG[1] + 24}px;z-index:61"><span class="label">{icon("pause")} Paused</span></div>',
            rec="00:27", svg=cursor(420, 300, True))
write(C, "05-paused-mic-muted", f"<style>{CSS}</style>" + d, "paused muted")

# 06 draw while recording
drawbar = (f'<div class="osd-toolbar rec-ctl" style="left:{CTL[0] + 166}px;top:{CTL[1] + 58}px;gap:3px">'
           f'<button class="btn image-button active">{icon("arrow")}</button><button class="btn image-button">{icon("shape")}</button>'
           f'<button class="btn image-button">{icon("step")}</button><span class="sep"></span>'
           '<span class="swatch small selected" style="--c:var(--annota-coral);box-shadow:0 0 0 2px #2b2b30,0 0 0 4px #fff"></span>'
           '<span class="swatch small" style="--c:var(--annota-amber)"></span><span class="swatch small" style="--c:var(--annota-azure)"></span>'
           '<span class="sep"></span><span class="dropdown flat" style="color:#fff;min-height:30px">Fade after 3 s</span>'
           f'<button class="btn text-button" style="padding:0 10px">{icon("edit-clear")}Clear</button></div>')
arrow = ('<path d="M620 300 L548 196" stroke="var(--annota-coral)" stroke-width="6" stroke-linecap="round"/>'
         '<path d="M538 222 L544 190 L575 197" stroke="var(--annota-coral)" stroke-width="6" fill="none" stroke-linecap="round" stroke-linejoin="round"/>'
         '<g opacity=".5"><rect x="214" y="96" width="250" height="92" rx="6" fill="none" stroke="var(--annota-coral)" stroke-width="5"/>'
         '<circle cx="214" cy="96" r="16" fill="var(--annota-coral)"/><text x="214" y="102" text-anchor="middle" font-size="17" font-weight="800" fill="#fff" font-family="Inter">1</text></g>')
d = desktop("d-tidewater-analytics", rec_frame + cam + control(*CTL, time="00:34", draw=True) + drawbar,
            rec="00:34", svg=arrow + cursor(622, 302, True))
write(C, "06-draw-while-recording", f"<style>{CSS}</style>" + d, "draw while recording")

# 07 swapped: full webcam in region
full = (f'<div style="position:absolute;left:{REG[0]}px;top:{REG[1]}px;width:{REG[2]}px;height:{REG[3]}px;overflow:hidden;z-index:57">'
        f'<svg width="{REG[2]}" height="{REG[3]}" viewBox="0 0 {REG[2]} {REG[3]}"><rect width="{REG[2]}" height="{REG[3]}" fill="#bcd0dc"/>'
        f'<rect y="{REG[3] * .62}" width="{REG[2]}" height="{REG[3]}" fill="#a9bfcd"/>'
        f'<circle cx="{REG[2] / 2}" cy="{REG[3] * .4}" r="{REG[3] * .17}" fill="#e3b394"/>'
        f'<ellipse cx="{REG[2] / 2}" cy="{REG[3] * 1.02}" rx="{REG[3] * .4}" ry="{REG[3] * .38}" fill="#3d5a73"/></svg>'
        f'<div class="osd-toolbar" style="position:absolute;left:18px;top:18px"><span class="label">{icon("camera")} Webcam only</span></div></div>')
d = desktop("d-tidewater-analytics", rec_frame + full + control(*CTL, time="00:41", swapped=True), rec="00:41")
write(C, "07-swapped-full-webcam", f"<style>{CSS}</style>" + d, "swap webcam")


# ---------------------------------------------------------------- video editor
ED_CSS = CSS + """
  .ve-body { flex: 1; display: flex; flex-direction: column; min-height: 0; }
  .preview { flex: 1; min-height: 0; background: var(--canvas-bg); display: flex; align-items: center; justify-content: center; position: relative; }
  .player { position: relative; width: 704px; height: 453px; background: #000; box-shadow: 0 2px 10px rgb(0 0 6 / 25%); overflow: hidden; }
  .timeline { flex: none; height: 252px; background: var(--view-bg); box-shadow: inset 0 1px var(--separator); display: flex; flex-direction: column; }
  .tl-bar { display: flex; align-items: center; gap: 6px; padding: 6px 12px; box-shadow: inset 0 -1px var(--separator); }
  .tl-bar .time { font-variant-numeric: tabular-nums; font-weight: var(--fw-bold); padding: 0 6px; }
  .tracks { position: relative; flex: 1; display: grid; grid-template-columns: 120px 1fr; }
  .track-heads { box-shadow: inset -1px 0 var(--separator); background: var(--window-bg); padding-top: 24px; }
  .track-heads div { height: 56px; display: flex; align-items: center; gap: 8px; padding: 0 12px; font-size: var(--fs-small); }
  .track-heads div.a { height: 40px; }
  .lanes { position: relative; padding: 0 18px; overflow: hidden; }
  .ruler { height: 24px; position: relative; font-size: 11px; opacity: .6; font-variant-numeric: tabular-nums; }
  .ruler span { position: absolute; top: 5px; transform: translateX(-50%); }
  .ruler i { position: absolute; bottom: 0; width: 1px; height: 5px; background: currentColor; }
  .lane { position: relative; height: 56px; }
  .lane.a { height: 40px; }
  .clip { position: absolute; top: 6px; bottom: 6px; border-radius: 6px; overflow: hidden; display: flex; box-shadow: 0 0 0 1px var(--borders); }
  .clip .thumb { flex: 1; background-size: cover; background-position: center; box-shadow: inset -1px 0 rgb(0 0 6 / 30%); }
  .clip .name { position: absolute; left: 30px; top: 4px; font-size: 11px; font-weight: 700; color: #fff; background: rgb(0 0 6 / 55%); padding: 1px 6px; border-radius: 4px; }
  .wave { position: absolute; top: 5px; bottom: 5px; border-radius: 5px; background: var(--accent-a12); color: var(--accent); display: flex; align-items: center; overflow: hidden; }
  .wave.sys { background: rgb(139 92 246 / 14%); color: var(--annota-violet); }
  .trimmed { position: absolute; top: 24px; bottom: 0; background: repeating-linear-gradient(135deg, rgb(0 0 6 / 30%) 0 6px, rgb(0 0 6 / 18%) 6px 12px); z-index: 3; }
  .trim-h { position: absolute; top: 24px; bottom: 6px; width: 12px; background: var(--annota-amber); z-index: 4; border-radius: 4px; display: grid; place-items: center; }
  .trim-h::after { content: ""; width: 2px; height: 22px; background: rgb(0 0 6 / 45%); border-radius: 1px; box-shadow: 4px 0 0 -0px transparent; }
  .trim-band { position: absolute; top: 24px; height: 3px; background: var(--annota-amber); z-index: 4; }
  .cut { position: absolute; top: 24px; bottom: 0; background: rgb(224 27 36 / 22%); box-shadow: inset 2px 0 var(--destructive-bg), inset -2px 0 var(--destructive-bg); z-index: 3; }
  .cut .lbl { position: absolute; top: 2px; left: 50%; transform: translateX(-50%); background: var(--destructive-bg); color: #fff; font-size: 11px; font-weight: 700; padding: 1px 6px; border-radius: 4px; white-space: nowrap; }
  .playhead { position: absolute; top: 0; bottom: 0; width: 2px; background: var(--accent-bg); z-index: 5; }
  .playhead::before { content: ""; position: absolute; top: 0; left: -6px; width: 14px; height: 14px; background: var(--accent-bg); clip-path: polygon(0 0, 100% 0, 100% 55%, 50% 100%, 0 55%); }
  .props .row { min-height: 44px; }
"""

LANE_W = 1280 - 120 - 36  # 1124 px, 0:00–0:52
DUR = 52.0


def px(t):
    return t / DUR * LANE_W


def thumbs(sample, n):
    return "".join(f'<span class="thumb" style="background-image:url({S}{sample}.png)"></span>' for _ in range(n))


def timeline(cut=None, playhead=12.4, trim=(1.2, 49.5), sel_clip=None):
    c1 = (0, 42.0)
    c2 = (42.0, 52.0)
    ticks = "".join(f'<i style="left:{px(t)}px"></i><span style="left:{px(t)}px">0:{t:02d}</span>' for t in range(0, 53, 5) if t > 0)
    sel1 = "box-shadow:0 0 0 2px var(--accent-bg);" if sel_clip == 1 else ""
    lanes = (f'<div class="ruler">{ticks}</div>'
             f'<div class="lane"><div class="clip" style="left:{px(c1[0])}px;width:{px(c1[1] - c1[0]) - 3}px;{sel1}">{thumbs("d-tidewater-analytics", 14)}<span class="name">Recording 10:42 · 0:42</span></div>'
             f'<div class="clip" style="left:{px(c2[0])}px;width:{px(c2[1] - c2[0])}px">{thumbs("a-harbor-backup", 4)}<span class="name">Recording 10:51 · 0:10</span></div></div>'
             f'<div class="lane a"><div class="wave" style="left:0;width:{px(42) - 3}px">{waveform(int(px(42)), 28, 3, quiet=(px(24), px(30)))}</div>'
             f'<div class="wave" style="left:{px(42)}px;width:{px(10)}px">{waveform(int(px(10)), 28, 9)}</div></div>'
             f'<div class="lane a"><div class="wave sys" style="left:0;width:{px(42) - 3}px">{waveform(int(px(42)), 28, 5)}</div></div>')
    over = (f'<div class="trimmed" style="left:18px;width:{px(trim[0])}px"></div>'
            f'<div class="trimmed" style="left:{18 + px(trim[1])}px;right:0"></div>'
            f'<div class="trim-h" style="left:{18 + px(trim[0]) - 6}px"></div>'
            f'<div class="trim-h" style="left:{18 + px(trim[1]) - 6}px"></div>'
            f'<div class="trim-band" style="left:{18 + px(trim[0])}px;width:{px(trim[1] - trim[0])}px"></div>')
    if cut:
        over += (f'<div class="cut" style="left:{18 + px(cut[0])}px;width:{px(cut[1] - cut[0])}px">'
                 f'<span class="lbl">Cut 0:{int(cut[0]):02d}.0 – 0:{int(cut[1]):02d}.0</span></div>')
    over += f'<div class="playhead" style="left:{18 + px(playhead)}px"></div>'
    return lanes, over


def tl_bar(cut_selected=False):
    t = "00:12.4"
    return (f'<div class="tl-bar">'
            f'<button class="btn image-button flat">{icon("go-previous")}</button>'
            f'<button class="btn image-button circular suggested-action" title="Play (Space)">{icon("play")}</button>'
            f'<button class="btn image-button flat">{icon("go-next")}</button>'
            f'<span class="time">{t}<span class="dim-label" style="font-weight:400"> / 00:52.0</span></span>'
            f'<span class="separator-v" style="height:22px;align-self:center"></span>'
            f'<button class="btn flat text-button" style="padding:0 10px" title="Split at Playhead (S)">{icon("cut-out")}Split</button>'
            f'<button class="btn text-button{" destructive-action" if cut_selected else " flat"}" style="padding:0 10px" title="Cut Out Range (Delete)">{icon("trash")}Cut Out Range</button>'
            f'<button class="btn flat text-button" style="padding:0 10px" title="Save Frame as Image (F)">{icon("image")}Save Frame</button>'
            f'<button class="btn flat text-button" style="padding:0 10px" title="Add Clip">{icon("plus")}Add Clip</button>'
            f'<span class="spacer"></span>'
            f'<button class="btn image-button flat">{icon("zoom-out")}</button><button class="btn image-button flat">{icon("zoom-in")}</button></div>')


def ve_props(state):
    rows = lambda items: '<div class="boxed-list">' + "".join(
        f'<div class="row"><div class="text"><span class="title">{a}</span></div><div class="suffix">{b}</div></div>' for a, b in items) + '</div>'
    tm = lambda v: f'<span class="entry" style="width:96px;font-variant-numeric:tabular-nums">{v}</span>'
    trim = ('<div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Trim</span>'
            '<span class="description">Drag the yellow handles or type a time</span></div></div>'
            + rows([("Start", tm("00:01.2")), ("End", tm("00:49.5")), ("Length", '<span class="value">00:48.3</span>')]) + '</div>')
    if state == "cut":
        sel = ('<div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Selected Range</span>'
               '<span class="description">Drag on the ruler to select</span></div></div>'
               + rows([("From", tm("00:24.0")), ("To", tm("00:30.0"))])
               + f'<button class="btn text-button destructive-action" style="width:100%">{icon("trash")}Cut Out 6.0 s</button></div>')
    else:
        sel = ('<div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Clips</span>'
               '<span class="description">Played one after the other</span></div></div>'
               '<div class="boxed-list">'
               f'<div class="row selected"><div class="prefix">{icon("drag-handle")}</div><div class="text"><span class="title">Recording 10:42</span><span class="subtitle">0:42 · Mic, system audio</span></div></div>'
               f'<div class="row"><div class="prefix">{icon("drag-handle")}</div><div class="text"><span class="title">Recording 10:51</span><span class="subtitle">0:10 · Mic</span></div></div>'
               f'<div class="row button-row suggested">{icon("plus")}&nbsp;Add Clip…</div></div></div>')
    audio = ('<div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Audio</span></div></div>'
             + rows([(f'Microphone', '<span class="switch on"></span>'), ("System audio", '<span class="switch on"></span>')]) + '</div>')
    return trim + sel


def video_editor(state="default", overlay=""):
    lanes, over = timeline(cut=(24, 30) if state == "cut" else None, sel_clip=1 if state != "cut" else None)
    player = (f'<div class="player"><img src="{S}d-tidewater-analytics.png" style="width:704px;height:453px;object-fit:cover;object-position:0 0" alt="">'
              f'<svg style="position:absolute;inset:0" width="704" height="453">{cursor(586, 126, True, True)}</svg>'
              f'<div style="position:absolute;right:14px;bottom:14px;width:110px;height:110px;border-radius:50%;overflow:hidden;box-shadow:0 0 0 3px #fff">'
              f'<svg width="110" height="110">{avatar_svg(0, 0, 110, "square", "#bcd0dc")}</svg></div></div>')
    hb = headerbar("Recording 10:42", "0:52 · 2 clips · 1400 × 940 · Edited",
                   end=(f'<button class="btn image-button" title="Copy Frame (Ctrl+C)">{icon("copy")}</button>'
                        f'<button class="btn text-button suggested-action" title="Export (Ctrl+E)">Export</button>'
                        f'<button class="btn flat image-button">{icon("open-menu")}</button>'))
    return f"""<div class="window" style="width:1280px;height:800px">
{hb}
<div class="ve-body">
  <div class="split-view" style="flex:1;min-height:0">
    <div class="content-pane"><div class="preview">{player}</div></div>
    <div class="sidebar-pane end props"><div class="scroll">{ve_props(state)}</div></div>
  </div>
  <div class="timeline">
    {tl_bar(state == "cut")}
    <div class="tracks">
      <div class="track-heads"><div>{icon("image")}Video</div><div class="a">{icon("microphone")}<span class="grow">Mic</span>{icon("speaker")}</div><div class="a">{icon("speaker")}<span class="grow">System</span>{icon("speaker")}</div></div>
      <div class="lanes">{lanes}{over}</div>
    </div>
  </div>
</div>
{overlay}
</div>"""


write(C, "08-video-editor-trim", f"<style>{ED_CSS}</style>" + video_editor(), "video editor")
write(C, "09-video-editor-cut-range", f"<style>{ED_CSS}</style>" + video_editor("cut",
      overlay=f'<div class="toast-overlay" style="right:300px;bottom:270px"><div class="toast"><span class="toast-title">Frame saved to Library as image</span>'
              f'<button class="btn text-button">Open</button><button class="btn flat circular">{icon("close")}</button></div></div>'),
      "cut range")


def export_dialog(fmt="GIF", progress=None):
    tg = lambda a: '<div class="toggle-group">' + "".join(f'<span class="toggle{" active" if x == a else ""}">{x}</span>' for x in ("MP4", "WebM", "GIF")) + '</div>'
    spin = lambda v, w=120: f'<span class="spin" style="width:{w}px"><span class="value">{v}</span><button class="btn">{icon("minus")}</button><button class="btn">{icon("plus")}</button></span>'
    body = ('<div class="boxed-list">'
            '<div class="row entry-row"><div class="text"><span class="title">File name</span><span>Recording 10:42.gif</span></div></div>'
            f'<div class="row"><div class="text"><span class="title">Folder</span><span class="subtitle">~/Videos/Annota</span></div><div class="suffix"><button class="btn flat image-button">{icon("folder")}</button></div></div>'
            f'<div class="row"><div class="text"><span class="title">Format</span></div><div class="suffix">{tg(fmt)}</div></div></div>')
    if fmt == "GIF":
        body += ('<div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">GIF</span>'
                 '<span class="description">No sound. Best for short clips up to 30 s.</span></div></div><div class="boxed-list">'
                 f'<div class="row"><div class="text"><span class="title">Frame rate</span></div><div class="suffix">{spin("15 fps", 130)}</div></div>'
                 f'<div class="row"><div class="text"><span class="title">Width</span><span class="subtitle">Height follows: 537 px</span></div><div class="suffix">{spin("800 px", 130)}</div></div>'
                 '<div class="row"><div class="text"><span class="title">Loop forever</span></div><div class="suffix"><span class="switch on"></span></div></div></div></div>')
    est = '<div class="hbox"><span class="caption dim-label grow">Trimmed 00:01.2 – 00:49.5 · 48.3 s</span><span class="caption dim-label">About 11 MB</span></div>'
    if progress is not None:
        est = (f'<div class="vbox" style="--gap:6px"><div class="hbox"><span class="spinner"></span><span class="grow">Exporting… frame 435 of 725</span>'
               f'<span class="caption dim-label">About 20 s left</span></div><div class="progress"><div class="bar" style="width:{progress}%"></div></div></div>')
    start = '<button class="btn text-button">Cancel</button>'
    end = (f'<button class="btn text-button suggested-action"{" disabled" if progress is not None else ""}>Export</button>')
    return (f'<div class="dialog-host"><div class="dialog" style="width:560px;max-width:560px">'
            f'<div class="headerbar"><div class="start">{start}</div><div class="title-widget"><span class="title">Export Video</span></div><div class="end">{end}</div></div>'
            f'<div class="dialog-body vbox" style="--gap:18px">{body}{est}</div></div></div>')


write(C, "10-export-gif", f"<style>{ED_CSS}</style>" + video_editor(overlay=export_dialog()), "export gif")
write(C, "11-export-progress", f"<style>{ED_CSS}</style>" + video_editor(overlay=export_dialog(progress=60)), "export progress")

mp4 = export_dialog("MP4").replace("Recording 10:42.gif", "Recording 10:42.mp4")
mp4 = mp4.replace('<div class="hbox"><span class="caption dim-label grow">',
                  '<div class="boxed-list"><div class="row"><div class="text"><span class="title">Quality</span></div><div class="suffix"><span class="dropdown flat">High · H.264</span></div></div>'
                  '<div class="row"><div class="text"><span class="title">Include audio</span><span class="subtitle">Mic and system audio, mixed</span></div><div class="suffix"><span class="switch on"></span></div></div></div>'
                  '<div class="hbox"><span class="caption dim-label grow">', 1).replace("About 11 MB", "About 6.1 MB")
write(C, "12-export-mp4", f"<style>{ED_CSS}</style>" + video_editor(overlay=mp4), "export mp4")
print("ok")
