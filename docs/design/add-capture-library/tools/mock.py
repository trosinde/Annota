"""Shared building blocks for the Annota library / organisation / template mockups.

Generates static HTML that uses ONLY ../../kit/tokens.css, adwaita.css and icons.js.
Run the per-change build scripts next to this file; they write docs/design/<id>/src/*.html.
"""
import html as _html
import pathlib

K = "../../kit/"
SRC_DIMS = {"a": (1600, 1000), "b": (1600, 1000), "c": (1600, 1000), "d": (1600, 1000),
            "e": (1600, 1000), "f": (1600, 2600)}
SRC_FILE = {"a": "a-harbor-backup", "b": "b-lumen-checkout", "c": "c-code-editor",
            "d": "d-tidewater-analytics", "e": "e-relay-chat", "f": "f-field-notes"}

ROOT = pathlib.Path(__file__).resolve().parents[3]  # docs/


def esc(s):
    return _html.escape(s, quote=False)


def ic(name, cls="icon", style=""):
    st = f' style="{style}"' if style else ""
    return f'<svg class="{cls}"{st}><use href="#i-{name}"/></svg>'


# --------------------------------------------------------------------------- css
LIB_CSS = r"""
  /* library window — layout only, colours from tokens */
  .lib-side { width: 280px; }
  .lib-side .sidebar-list { flex: 1; }
  .lib-side .nav-row .icon { flex: none; }
  .nav-row.drop { background: var(--accent-a25); outline: 2px solid var(--accent-bg); outline-offset: -2px; }
  .nav-row .spinner { margin-left: auto; }
  .side-foot { flex: none; display: flex; align-items: center; gap: 12px; padding: 10px 16px 12px; box-shadow: inset 0 1px var(--separator); font-size: var(--fs-small); }
  .side-foot .vbox { min-width: 0; flex: 1; }
  .side-foot .path { font-family: var(--font-mono); font-size: 12px; opacity: var(--dim-opacity); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .state-dot { width: 8px; height: 8px; border-radius: 50%; background: var(--success-bg); flex: none; }
  .state-dot.off { background: var(--warning-bg); }
  .grid-scroll { flex: 1; min-height: 0; overflow: hidden; position: relative; padding: 0 18px 18px; }
  .day-head { display: flex; align-items: baseline; gap: 10px; padding: 18px 2px 12px; }
  .day-head .caption { opacity: var(--dim-opacity); }
  .day-head .spacer { flex: 1; }
  .tiles { display: grid; grid-template-columns: repeat(auto-fill, var(--tw, 150px)); gap: 18px 12px; }
  .tile { width: var(--tw, 150px); display: flex; flex-direction: column; gap: 6px; position: relative; }
  .tile .frame { height: var(--th, 112px); border-radius: 9px; background: var(--row-hover); box-shadow: inset 0 0 0 1px var(--separator);
                 display: grid; place-items: center; position: relative; }
  .timg { position: relative; display: block; background-repeat: no-repeat;
                 box-shadow: 0 0 0 1px var(--shade), 0 1px 3px rgb(0 0 6 / 14%); border-radius: 2px; overflow: hidden; }
  .timg > svg { position: absolute; inset: 0; width: 100%; height: 100%; }
  .tile .name { font-size: var(--fs-small); padding: 0 2px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .tile .meta { font-size: var(--fs-caption); padding: 0 2px; opacity: var(--dim-opacity); display: flex; gap: 4px; align-items: center; margin-top: -4px; white-space: nowrap; overflow: hidden; }
  .tile .meta .icon { width: 12px; height: 12px; }
  .tile.hover .frame { background: var(--fill-flat-hover); }
  .tile.selected .frame { background: var(--accent-a12); box-shadow: inset 0 0 0 2px var(--accent-bg); }
  .tile.focus .frame { outline: 2px solid var(--accent-a50); outline-offset: 2px; }
  .tile.dim { opacity: .35; }
  .tile.gone { opacity: .25; transform: scale(.94); }
  .tile .sel { position: absolute; top: 6px; left: 6px; width: 22px; height: 22px; border-radius: 50%; display: grid; place-items: center;
               background: var(--view-bg); box-shadow: inset 0 0 0 2px var(--borders-strong), 0 1px 2px rgb(0 0 6 / 20%); }
  .tile .sel.on { background: var(--accent-bg); color: var(--accent-fg); box-shadow: 0 1px 2px rgb(0 0 6 / 20%); }
  .tile .sel .icon { width: 14px; height: 14px; }
  .tile .fav { position: absolute; top: 6px; right: 6px; width: 24px; height: 24px; border-radius: 50%; display: grid; place-items: center; background: var(--osd-bg); color: #ffffff; }
  .tile .fav .icon { width: 14px; height: 14px; }
  .tile .fav.on .icon { fill: var(--annota-amber); }
  .dur { position: absolute; right: 6px; bottom: 6px; display: inline-flex; align-items: center; gap: 3px; height: 20px; padding: 0 7px 0 5px; border-radius: 10px;
         background: var(--osd-bg); color: var(--osd-fg); font-size: 11px; font-weight: var(--fw-bold); font-variant-numeric: tabular-nums; }
  .dur .icon { width: 12px; height: 12px; }
  .kind-badge { position: absolute; left: 6px; bottom: 6px; height: 18px; padding: 0 6px; border-radius: 9px; background: var(--osd-bg); color: var(--osd-fg);
                font-size: 11px; font-weight: var(--fw-bold); display: inline-flex; align-items: center; }
  .match { font-size: var(--fs-caption); padding: 0 2px; margin-top: -2px; line-height: 1.35; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .match .src { display: block; }
  .match .src { opacity: var(--dim-opacity); }
  mark { background: var(--accent-a25); color: inherit; border-radius: 3px; padding: 0 1px; box-shadow: 0 0 0 1px var(--accent-a50); }
  .scrollbar { position: absolute; right: 3px; width: 6px; border-radius: 3px; background: var(--fill-3); opacity: .6; }
  .hdr-scale { width: 104px; min-height: 0; gap: 6px; }
  .hdr-scale .trough .knob { width: 16px; height: 16px; margin: -8px 0 0 -8px; }
  .hdr-scale .trough .mark { top: 8px; height: 4px; }
  /* preview pane */
  .pv { width: 320px; flex: none; display: flex; flex-direction: column; background: var(--window-bg); box-shadow: inset 1px 0 var(--separator); min-height: 0; }
  .pv .headerbar { background: var(--window-bg); box-shadow: none; }
  .pv-body { flex: 1; min-height: 0; overflow: hidden; padding: 0 12px 12px; display: flex; flex-direction: column; gap: 12px; }
  .pv-body > * { flex: none; }
  .pv-img { height: 196px; border-radius: var(--radius-l); background: var(--canvas-bg); display: grid; place-items: center; position: relative; flex: none; }
  .pv-title { display: flex; align-items: center; gap: 6px; padding: 0 2px; }
  .pv-title .title-4 { flex: 1; min-width: 0; font-size: 17px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .pv .boxed-list .row { min-height: 0; padding: 7px 12px; }
  .pv .row.property .subtitle { font-size: var(--fs-small); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .pv .row.property .title { font-size: var(--fs-caption); }
  .pv .actions { display: flex; gap: 6px; }
  .pv .actions .btn.grow { flex: 1; }
  .mono-path { font-family: var(--font-mono); font-size: 12px !important; }
  /* list view */
  .lv { flex: 1; min-height: 0; overflow: hidden; position: relative; }
  .lv table { width: 100%; border-collapse: collapse; font-size: var(--fs-small); table-layout: fixed; }
  .lv th { text-align: left; font-weight: var(--fw-bold); font-size: var(--fs-small); height: 34px; padding: 0 10px; box-shadow: inset 0 -1px var(--separator); white-space: nowrap; }
  .lv th .icon { width: 12px; height: 12px; vertical-align: -1px; margin-left: 2px; }
  .lv th.sorted { color: var(--accent); }
  .lv th + th { box-shadow: inset 0 -1px var(--separator), inset 1px 0 var(--separator); }
  .lv td { height: 44px; padding: 0 10px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .lv td.dim { opacity: var(--dim-opacity); }
  .lv td.num { font-variant-numeric: tabular-nums; }
  .lv tr.sel td { background: var(--accent-a12); }
  .lv tr.grp td { height: 38px; padding-top: 10px; font-weight: var(--fw-bold); }
  .lv tr.grp td .caption { font-weight: var(--fw-regular); opacity: var(--dim-opacity); margin-left: 8px; }
  .lv .namecell { display: flex; align-items: center; gap: 10px; }
  .lv .namecell .tbox { width: 44px; height: 32px; flex: none; display: grid; place-items: center; border-radius: 4px; background: var(--row-hover); }
  .lv .namecell .timg { position: relative; display: block; background-repeat: no-repeat; box-shadow: 0 0 0 1px var(--shade); }
  /* action bar (GtkActionBar) */
  .action-bar { flex: none; display: flex; align-items: center; gap: 6px; padding: 7px 12px; background: var(--headerbar-bg); box-shadow: inset 0 1px var(--headerbar-shade); }
  .action-bar .count { font-weight: var(--fw-bold); padding: 0 6px; }
  /* quick look */
  .ql { position: absolute; inset: 0; z-index: 35; background: rgb(0 0 6 / 93%); -webkit-backdrop-filter: blur(6px); backdrop-filter: blur(6px); color: #ffffff; display: flex; flex-direction: column; }
  .ql-top { display: flex; align-items: center; gap: 12px; padding: 12px 14px; }
  .ql-top .grow .t { font-weight: var(--fw-bold); }
  .ql-top .grow .s { font-size: var(--fs-caption); opacity: .7; }
  .ql-img { flex: 1; display: grid; place-items: center; position: relative; }
  .ql .btn.osd { background: rgb(255 255 255 / 14%); }
  .ql .btn.osd:hover { background: rgb(255 255 255 / 22%); }
  .ql-hint { display: flex; justify-content: center; gap: 18px; padding: 14px; font-size: var(--fs-small); opacity: .8; }
  .ql-hint kbd.key { background: rgb(255 255 255 / 15%); box-shadow: none; color: #ffffff; }
  .ql .nav { position: absolute; top: 50%; margin-top: -21px; width: 42px; min-height: 42px; }
  /* chips / filter bar */
  .filter-bar { flex: none; display: flex; align-items: center; gap: 6px; flex-wrap: wrap; padding: 8px 18px; background: var(--view-bg); box-shadow: inset 0 -1px var(--separator); }
  .chip { display: inline-flex; align-items: center; gap: 6px; height: 30px; padding: 0 4px 0 10px; border-radius: var(--radius-pill);
          background: var(--accent-a12); color: var(--accent); font-size: var(--fs-small); font-weight: var(--fw-bold); white-space: nowrap; }
  .chip .k { font-weight: var(--fw-regular); opacity: .85; }
  .chip .x { width: 22px; height: 22px; border-radius: 50%; display: grid; place-items: center; }
  .chip .x .icon { width: 12px; height: 12px; }
  .chip.plain { background: var(--fill-1); color: inherit; padding-right: 10px; }
  .chip.add { background: transparent; color: inherit; box-shadow: inset 0 0 0 1px var(--borders); padding-right: 8px; font-weight: var(--fw-bold); }
  .tagc { display: inline-flex; align-items: center; gap: 4px; height: 22px; padding: 0 8px; border-radius: var(--radius-pill); background: var(--fill-1); font-size: var(--fs-caption); white-space: nowrap; }
  .tagc .icon { width: 12px; height: 12px; opacity: .7; }
  .tagc .x { width: 14px; height: 14px; opacity: .7; }
  .tag-dot { width: 10px; height: 10px; border-radius: 50%; flex: none; background: var(--c); box-shadow: 0 0 0 1px var(--borders); }
  /* drag ghost */
  .ghost { position: absolute; z-index: 45; pointer-events: none; }
  .ghost .stack { position: relative; border-radius: 6px; box-shadow: 0 6px 18px rgb(0 0 6 / 35%); }
  .ghost .badge { position: absolute; top: -8px; right: -8px; }
  .cursor-arrow { position: absolute; z-index: 46; width: 20px; height: 20px; }
  /* misc */
  .kv { display: grid; grid-template-columns: auto 1fr; gap: 4px 12px; font-size: var(--fs-small); }
  .kv .k { opacity: var(--dim-opacity); }
  .skeleton { background: var(--fill-1); border-radius: 4px; }
  .pill-status { display: inline-flex; gap: 6px; align-items: center; font-size: var(--fs-caption); }
"""


def page(title, body, size=(1280, 800), flush=True, css=""):
    w, h = size
    frame = "mock-frame flush" if flush else "mock-frame"
    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport-size" content="{w}x{h}">
<title>Annota — {esc(title)}</title>
<link rel="stylesheet" href="{K}tokens.css">
<link rel="stylesheet" href="{K}adwaita.css">
<style>{LIB_CSS}{css}</style>
</head>
<body class="{frame}">
{body}
<script src="{K}icons.js"></script>
</body>
</html>
"""


# --------------------------------------------------------------------------- items
class Item:
    def __init__(self, key, day, time, name, src, crop, kind, app, title="", url="", fsize="",
                 ann=None, dur=None, px=None, scale=1, tags=(), fav=False, plain=False):
        self.key, self.day, self.time, self.name, self.src, self.crop = key, day, time, name, src, crop
        self.kind, self.app, self.title, self.url, self.fsize = kind, app, title, url, fsize
        self.ann = ann or []
        self.dur, self.scale, self.tags, self.fav, self.plain = dur, scale, list(tags), fav, plain
        cw, ch = crop[2], crop[3]
        self.px = px or f"{cw * scale} × {ch * scale}"


def _sw(cw):
    return max(4.0, cw / 170)


def A_rect(x, y, w, h, c="var(--annota-coral)"):
    return ("rect", x, y, w, h, c)


def A_arrow(x1, y1, x2, y2):
    return ("arrow", x1, y1, x2, y2)


def A_step(x, y, n):
    return ("step", x, y, n)


def A_hl(x, y, w, h):
    return ("hl", x, y, w, h)


def A_mark(x, y, w, h):
    return ("mark", x, y, w, h)


def A_blur(x, y, w, h):
    return ("blur", x, y, w, h)


def A_callout(x, y, w, h, text, tx, ty):
    return ("callout", x, y, w, h, text, tx, ty)


def ann_svg(it):
    if not it.ann:
        return ""
    cw, ch = it.crop[2], it.crop[3]
    sw = _sw(cw)
    out = [f'<svg viewBox="0 0 {cw} {ch}" preserveAspectRatio="none">']
    for a in it.ann:
        t = a[0]
        if t == "rect":
            _, x, y, w, h, c = a
            out.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{sw}" fill="none" stroke="{c}" stroke-width="{sw}"/>')
        elif t == "arrow":
            _, x1, y1, x2, y2 = a
            import math
            ang = math.atan2(y2 - y1, x2 - x1)
            L = sw * 4.5
            p1 = (x2 - L * math.cos(ang - 0.5), y2 - L * math.sin(ang - 0.5))
            p2 = (x2 - L * math.cos(ang + 0.5), y2 - L * math.sin(ang + 0.5))
            out.append(f'<path d="M{x1} {y1} L{x2} {y2}" stroke="var(--annota-coral)" stroke-width="{sw*1.3}" stroke-linecap="round" fill="none"/>')
            out.append(f'<path d="M{p1[0]:.1f} {p1[1]:.1f} L{x2} {y2} L{p2[0]:.1f} {p2[1]:.1f}" stroke="var(--annota-coral)" stroke-width="{sw*1.3}" stroke-linecap="round" stroke-linejoin="round" fill="none"/>')
        elif t == "step":
            _, x, y, n = a
            r = max(14, cw / 40)
            out.append(f'<circle cx="{x}" cy="{y}" r="{r}" fill="var(--annota-coral)"/><text x="{x}" y="{y + r*0.36:.1f}" font-size="{r*1.05:.1f}" font-weight="700" fill="#fff" text-anchor="middle" font-family="Inter">{n}</text>')
        elif t == "callout":
            _, x, y, w, h, text, tx, ty = a
            fs = h * 0.46
            out.append(f'<path d="M{x + w*0.18} {y + h} L{tx} {ty} L{x + w*0.34} {y + h} Z" fill="var(--annota-coral)"/>'
                       f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{h*0.22}" fill="var(--annota-coral)"/>'
                       f'<text x="{x + w/2}" y="{y + h*0.64}" font-size="{fs:.1f}" font-weight="700" fill="#fff" text-anchor="middle" font-family="Inter">{text}</text>')
        elif t == "mark":
            _, x, y, w, h = a
            out.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="3" fill="rgb(53 132 228 / 28%)" stroke="#3584e4" stroke-width="{sw*0.6:.1f}"/>')
        elif t == "hl":
            _, x, y, w, h = a
            out.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="var(--annota-highlight)"/>')
        elif t == "blur":
            _, x, y, w, h = a
            out.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="4" fill="#8a8a96"/>'
                       f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="4" fill="url(#px)" opacity=".55"/>')
    out.append("</svg>")
    s = "".join(out)
    if "url(#px)" in s:
        s = s.replace('preserveAspectRatio="none">', 'preserveAspectRatio="none"><defs><pattern id="px" width="14" height="14" patternUnits="userSpaceOnUse"><rect width="7" height="7" fill="#ffffff"/><rect x="7" y="7" width="7" height="7" fill="#ffffff"/></pattern></defs>', 1)
    return s


def timg(it, bw, bh, extra_style="", fill=False, svg=True):
    """Image of a crop of a sample PNG fitted (contain) into bw×bh. Returns html."""
    W, H = SRC_DIMS[it.src]
    cx, cy, cw, ch = it.crop
    s = (max if fill else min)(bw / cw, bh / ch)
    dw, dh = (bw, bh) if fill else (cw * s, ch * s)
    ox = -cx * s - ((cw * s - bw) / 2 if fill else 0)
    oy = -cy * s - ((ch * s - bh) / 2 if fill else 0)
    url = f"{K}sample-content/png/{SRC_FILE[it.src]}.png"
    st = (f"width:{dw:.1f}px;height:{dh:.1f}px;background-image:url({url});"
          f"background-size:{W*s:.1f}px {H*s:.1f}px;background-position:{ox:.1f}px {oy:.1f}px;{extra_style}")
    inner = ann_svg(it) if svg and not fill else ""
    return f'<span class="timg" style="{st}">{inner}</span>'


KIND_ICON = {"Region": "capture-region", "Window": "capture-window", "Screen": "capture-screen",
             "Clipboard": "copy", "File": "image", "Video": "record", "PNG image": "image",
             "Step guide": "step", "Template": "template"}


def tile(it, st=(), tw=150, th=112, meta=None, match="", extra=""):
    st = set(st)
    cls = "tile " + " ".join(st)
    inner = timg(it, tw - 16, th - 16)
    sel = ""
    if "selected" in st and "multi" in st:
        sel = f'<span class="sel on">{ic("check")}</span>'
    elif "hover" in st or "multi" in st:
        sel = '<span class="sel"></span>'
    fav = ""
    if it.fav and "showfav" in st:
        fav = f'<span class="fav on">{ic("star")}</span>'
    dur = f'<span class="dur">{ic("play")}{it.dur}</span>' if it.dur else ""
    kb = '<span class="kind-badge">PNG</span>' if it.plain else ""
    m = meta if meta is not None else f'{ic(KIND_ICON.get(it.kind, "image"))}<span>{it.time}</span>'
    name = esc(it.name) if "rename" not in st else extra
    return (f'<div class="{cls}" style="--tw:{tw}px;--th:{th}px"><div class="frame">{inner}{sel}{fav}{dur}{kb}</div>'
            f'<div class="name">{name}</div><div class="meta">{m}</div>{match}</div>')


def ITEMS():
    I = Item
    return [
        # ---------------- Today, Friday 30 October 2026
        I("t1", "today", "14:22", "Checkout postcode error", "b", (80, 275, 912, 340), "Region", "Firefox",
          "Checkout — Lumen Outfitters", "lumen-outfitters.example/checkout", "184 KB",
          [A_rect(18, 236, 524, 82), A_arrow(700, 180, 560, 262)], tags=["bug", "checkout"], fav=True),
        I("t2", "today", "13:58", "Harbor backup destination", "a", (250, 120, 1100, 760), "Window", "Harbor Backup",
          "Preferences — Backup", "", "312 KB", [A_rect(392, 175, 576, 58), A_step(392, 175, 1)], scale=2,
          tags=["docs"]),
        I("t3", "today", "13:41", "2026-10-30 13-41-07", "c", (322, 330, 1000, 400), "Region", "Text Editor",
          "scene.rs — quayside", "", "156 KB", tags=["dev"]),
        I("t4", "today", "11:05", "KPI row September", "d", (264, 83, 1302, 112), "Region", "Firefox",
          "Overview — Tidewater", "tidewater.example/overview", "96 KB", [A_hl(670, 50, 120, 40)], tags=["report"]),
        I("t5", "today", "10:47", "Relay — venue booking", "e", (0, 0, 1600, 1000), "Window", "Relay",
          "Jonas Okafor — Relay", "", "402 KB", [A_blur(497, 162, 292, 22), A_blur(700, 278, 510, 22)], scale=2,
          tags=["support"]),
        I("t6", "today", "10:12", "Screen recording 10-12", "d", (0, 0, 1600, 1000), "Video", "Screen",
          "", "", "8.4 MB", dur="0:42", px="2560 × 1600"),
        I("t7", "today", "09:20", "Order summary", "b", (1040, 188, 480, 708), "Region", "Firefox",
          "Checkout — Lumen Outfitters", "lumen-outfitters.example/checkout", "142 KB", tags=["checkout"]),
        # ---------------- Yesterday, Thursday 29 October
        I("y1", "yesterday", "17:46", "Daily visitors chart", "d", (264, 217, 856, 757), "Region", "Firefox",
          "Overview — Tidewater", "tidewater.example/overview", "220 KB", [A_rect(720, 200, 120, 330), A_step(720, 200, 1)],
          tags=["report"], fav=True),
        I("y2", "yesterday", "16:30", "The tide clock — article", "f", (0, 0, 1600, 1000), "Window", "Firefox",
          "The tide clock — Field Notes", "fieldnotes.example/tide-clock", "520 KB"),
        I("y3", "yesterday", "15:12", "Shipping options", "b", (80, 634, 912, 320), "Region", "Firefox",
          "Checkout — Lumen Outfitters", "lumen-outfitters.example/checkout", "98 KB",
          [A_step(40, 98, 1), A_step(40, 176, 2), A_step(40, 252, 3)], tags=["checkout", "release-notes"]),
        I("y4", "yesterday", "14:05", "2026-10-29 14-05-51", "c", (322, 792, 1278, 180), "Region", "Text Editor",
          "scene.rs — quayside", "", "64 KB", tags=["bug", "dev"]),
        I("y5", "yesterday", "11:48", "Harbor sidebar", "a", (250, 120, 260, 760), "Region", "Harbor Backup",
          "Preferences — Backup", "", "58 KB"),
        I("y6", "yesterday", "10:33", "Contact card", "e", (1300, 0, 300, 400), "Region", "Relay",
          "Jonas Okafor — Relay", "", "41 KB", [A_blur(20, 200, 260, 24), A_blur(20, 246, 180, 24)]),
        I("y7", "yesterday", "09:58", "contains() tooltip", "c", (560, 400, 560, 140), "Clipboard", "Clipboard",
          "", "", "37 KB"),
        I("y8", "yesterday", "09:12", "Top routes", "d", (1138, 217, 428, 330), "Region", "Firefox",
          "Overview — Tidewater", "tidewater.example/overview", "72 KB", tags=["report"]),
        # ---------------- Wednesday 28 October
        I("w1", "wed", "16:40", "2026-10-28 16-40-02", "d", (0, 0, 1600, 1000), "Screen", "Screen", "", "", "1.1 MB",
          px="2560 × 1600"),
        I("w2", "wed", "15:15", "Checkout page", "b", (0, 0, 1600, 1000), "Window", "Firefox",
          "Checkout — Lumen Outfitters", "lumen-outfitters.example/checkout", "488 KB",
          [A_rect(100, 515, 440, 90), A_arrow(760, 650, 560, 570)], tags=["bug"]),
        I("w3", "wed", "14:02", "Recording — checkout flow", "b", (0, 0, 1600, 1000), "Video", "Firefox",
          "Checkout — Lumen Outfitters", "lumen-outfitters.example/checkout", "12.6 MB", dur="1:18", px="1920 × 1200"),
        I("w4", "wed", "11:30", "Backup schedule group", "a", (642, 436, 576, 200), "Region", "Harbor Backup",
          "Preferences — Backup", "", "46 KB", tags=["docs"]),
        I("w5", "wed", "10:05", "Tide table", "f", (380, 1830, 840, 280), "Region", "Firefox",
          "The tide clock — Field Notes", "fieldnotes.example/tide-clock", "51 KB"),
        I("w6", "wed", "09:31", "Printer address", "e", (430, 330, 600, 150), "Region", "Relay",
          "Jonas Okafor — Relay", "", "38 KB", [A_rect(64, 38, 210, 34)], tags=["support"]),
        # ---------------- Tuesday 27 October
        I("u1", "tue", "17:02", "Quayside editor", "c", (0, 0, 1600, 1000), "Window", "Text Editor",
          "scene.rs — quayside", "", "610 KB", scale=2, tags=["dev"]),
        I("u2", "tue", "15:20", "Devices split", "d", (1138, 540, 428, 130), "Region", "Firefox",
          "Overview — Tidewater", "tidewater.example/overview", "22 KB"),
        I("u3", "tue", "12:47", "Discount code field", "b", (1040, 540, 480, 80), "Region", "Firefox",
          "Checkout — Lumen Outfitters", "lumen-outfitters.example/checkout", "18 KB"),
        I("u4", "tue", "10:02", "2026-10-27 10-02-44", "a", (0, 0, 1600, 1000), "Screen", "Screen", "", "", "902 KB",
          px="2560 × 1600"),
        # ---------------- Monday 26 October
        I("m1", "mon", "16:15", "Field notes header", "f", (0, 0, 1600, 600), "Region", "Firefox",
          "The tide clock — Field Notes", "fieldnotes.example/tide-clock", "188 KB"),
        I("m2", "mon", "14:40", "Chat list", "e", (72, 0, 330, 500), "Region", "Relay", "Chats — Relay", "", "44 KB"),
        I("m3", "mon", "09:05", "Problems panel warning", "c", (322, 792, 700, 120), "Region", "Text Editor",
          "scene.rs — quayside", "", "29 KB", tags=["bug"]),
        # ---------------- 18 October 2026
        I("o1", "oct18", "15:44", "Backup storage usage", "a", (642, 665, 576, 110), "Region", "Harbor Backup",
          "Preferences — Backup", "", "31 KB"),
        I("o2", "oct18", "11:20", "Ridgeline pack", "b", (1068, 265, 424, 72), "Region", "Firefox",
          "Checkout — Lumen Outfitters", "lumen-outfitters.example/checkout", "16 KB"),
        I("o3", "oct18", "10:02", "Visitors KPI", "d", (264, 83, 312, 112), "Region", "Firefox",
          "Overview — Tidewater", "tidewater.example/overview", "12 KB"),
    ]


DAYS = [("today", "Today", "Friday, 30 October"),
        ("yesterday", "Yesterday", "Thursday, 29 October"),
        ("wed", "Wednesday", "28 October"),
        ("tue", "Tuesday", "27 October"),
        ("mon", "Monday", "26 October"),
        ("oct18", "18 October 2026", "Sunday")]


def by_key(items):
    return {i.key: i for i in items}


# --------------------------------------------------------------------------- building blocks
def headerbar(start="", title="", subtitle="", end="", close=False, cls="", title_html=None):
    tw = title_html if title_html is not None else (
        f'<div class="title-widget"><span class="title">{esc(title)}</span>'
        + (f'<span class="subtitle">{esc(subtitle)}</span>' if subtitle else "") + "</div>")
    wc = ('<div class="window-controls"><span class="wc">' + ic("close") + "</span></div>") if close else ""
    return f'<div class="headerbar {cls}"><div class="start">{start}</div>{tw}<div class="end">{end}{wc}</div></div>'


def btn(icon=None, label=None, cls="", tip=None, style=""):
    inner = (ic(icon) if icon else "") + (esc(label) if label else "")
    kind = "image-button" if icon and not label else ("text-button" if label and not icon else "")
    st = f' style="{style}"' if style else ""
    return f'<button class="btn {kind} {cls}"{st}>{inner}</button>'


def nav(icon, label, count="", cls="", extra=""):
    c = f'<span class="count">{count}</span>' if count != "" else ""
    return f'<div class="nav-row {cls}">{ic(icon)}<span class="label">{esc(label)}</span>{extra}{c}</div>'


def zoom_scale(step=1):
    pos = [0, 50, 100][step]
    return (f'<div class="scale hdr-scale" title="Thumbnail size">{ic("zoom-out", style="opacity:.55")}'
            f'<div class="trough"><div class="fill" style="width:{pos}%"></div>'
            f'<span class="mark" style="left:0"></span><span class="mark" style="left:50%"></span><span class="mark" style="left:100%"></span>'
            f'<div class="knob" style="left:{pos}%"></div></div></div>')


def view_toggle(mode="grid"):
    g = "active" if mode == "grid" else ""
    l = "active" if mode == "list" else ""
    return (f'<div class="toggle-group"><span class="toggle icon-only {g}">{ic("grid-view")}</span>'
            f'<span class="toggle icon-only {l}">{ic("list-view")}</span></div>')


def sort_btn(label="Capture Date", cls=""):
    return f'<button class="btn flat {cls}" style="gap:4px;padding:5px 8px 5px 10px">{esc(label)}{ic("pan-down")}</button>'


def content_start(mode="grid", zoom=1):
    return view_toggle(mode) + (zoom_scale(zoom) if mode == "grid" else "")


def content_end(mode="grid", zoom=1, sort="Capture Date", preview_on=False, search=False, sort_cls="", extra=""):
    s = btn("search", cls="flat" + (" active" if search == "on" else "")) if search else ""
    return (extra + s + sort_btn(sort, sort_cls)
            + btn("sidebar-show-right", cls="flat " + ("active" if preview_on else "")))


def side_foot(path="/mnt/nas/annota", state="Connected · 31 captures · 1.8 GB", off=False):
    dot = '<span class="state-dot off"></span>' if off else '<span class="state-dot"></span>'
    return (f'<div class="side-foot">{ic("folder")}<div class="vbox" style="--gap:1px">'
            f'<span class="path">{esc(path)}</span><span class="caption" style="opacity:.75">{esc(state)}</span></div>{dot}</div>')


def sidebar_mvp(selected="all", counts=None, foot=None, header_end=None):
    c = counts or {"all": "31", "recent": "", "trash": "2"}
    rows = [nav("library", "All Captures", c.get("all", ""), "selected" if selected == "all" else ""),
            nav("timer", "Recent", c.get("recent", ""), "selected" if selected == "recent" else ""),
            nav("trash", "Trash", c.get("trash", ""), "selected" if selected == "trash" else "")]
    he = header_end if header_end is not None else btn("open-menu", cls="flat")
    return (f'<div class="sidebar-pane sidebar lib-side">'
            + headerbar(btn("capture-region", cls="flat"), "Annota", end=he)
            + f'<div class="sidebar-list">{"".join(rows)}</div>'
            + (foot if foot is not None else side_foot()) + "</div>")


TAG_COLORS = {"bug": "var(--annota-coral)", "checkout": "var(--annota-amber)", "docs": "var(--annota-azure)",
              "release-notes": "var(--annota-teal)", "report": "var(--annota-violet)", "support": "var(--annota-lime)",
              "dev": "var(--annota-ink)", "invoices": "var(--annota-azure)"}


def sidebar_org(selected="all", tag_counts=None, extra_rows=None, drop=None, foot=None, header_end=None,
                collections=None, cut=False, focus=None, menu_on=None):
    tc = tag_counts or [("bug", 6), ("checkout", 4), ("docs", 2), ("dev", 4), ("report", 4), ("support", 2)]

    def cls(k):
        r = []
        if selected == k:
            r.append("selected")
        if drop == k:
            r.append("drop")
        if focus == k:
            r.append("focus")
        if menu_on == k:
            r.append("hover")
        return " ".join(r)
    rows = [nav("library", "All Captures", "31", cls("all")),
            nav("star", "Favourites", "4", cls("fav")),
            nav("timer", "Recent", "", cls("recent")),
            nav("trash", "Trash", "2", cls("trash")),
            '<div class="section-title" style="display:flex;align-items:center">Collections<span class="spacer"></span></div>']
    cols = collections or [("folder", "Release notes 0.4", "12", "rn"), ("folder", "Onboarding guide", "12", "onb"),
                           ("search", "Bugs, last 30 days", "6", "smart")]
    for icn, lab, cnt, k in cols:
        rows.append(nav(icn, lab, cnt, cls(k)))
    rows.append('<div class="section-title">Tags</div>')
    for t, n in tc:
        rows.append(f'<div class="nav-row {cls("tag-" + t)}"><span class="tag-dot" style="--c:{TAG_COLORS.get(t, "var(--annota-azure)")};margin:0 3px"></span>'
                    f'<span class="label">{esc(t)}</span><span class="count">{n}</span></div>')
    if not cut:
        rows.append('<div class="section-title">Folders</div>')
        rows.append(nav("folder", "Annota", "31", cls("f-annota")))
        rows.append(nav("folder", "Screenshots", "86", cls("f-shots")))
    if extra_rows:
        rows.extend(extra_rows)
    he = header_end if header_end is not None else btn("open-menu", cls="flat")
    return (f'<div class="sidebar-pane sidebar lib-side">'
            + headerbar(btn("capture-region", cls="flat"), "Annota", end=he)
            + f'<div class="sidebar-list" style="gap:1px">{"".join(rows)}</div>'
            + (foot if foot is not None else "") + "</div>")


def grid(items, groups=None, tw=150, th=112, states=None, scroll=(0.0, 0.32), pad_top=0, day_extra=None,
         per_tile=None):
    """groups: list of (key,label,sub); states: dict key->set."""
    states = states or {}
    per_tile = per_tile or {}
    groups = groups if groups is not None else DAYS
    out = [f'<div class="grid-scroll" style="padding-top:{pad_top}px">']
    for k, label, sub in groups:
        its = [i for i in items if i.day == k]
        if not its:
            continue
        n = len(its)
        extra = (day_extra or {}).get(k, "")
        out.append(f'<div class="day-head"><span class="heading">{esc(label)}</span><span class="caption">{esc(sub)} · {n} capture{"s" if n != 1 else ""}</span><span class="spacer"></span>{extra}</div>')
        out.append('<div class="tiles" style="--tw:%dpx">' % tw)
        for i in its:
            kw = per_tile.get(i.key, {})
            out.append(tile(i, states.get(i.key, ()), tw, th, **kw))
        out.append("</div>")
    if scroll:
        out.append(f'<span class="scrollbar" style="top:{scroll[0]*100:.0f}%;height:{scroll[1]*100:.0f}%"></span>')
    out.append("</div>")
    return "".join(out)


def prop(title, value, mono=False, suffix=""):
    v = f'<span class="subtitle{" mono-path" if mono else ""}">{esc(value)}</span>'
    sfx = f'<div class="suffix">{suffix}</div>' if suffix else ""
    return f'<div class="row property"><div class="text"><span class="title">{esc(title)}</span>{v}</div>{sfx}</div>'


def preview_pane(it, folder="/mnt/nas/annota", extra_top="", extra_props="", fav_btn=False, fav_on=False,
                 img_html=None, close=True, title_html=None, actions=None, hide_props=False, video=False):
    star = ""
    if fav_btn:
        star = btn("star", cls="flat" + (" active" if fav_on else ""),
                   style=("color:var(--annota-amber)" if fav_on else ""))
    end = star + btn("copy", cls="flat") + btn("view-more", cls="flat")
    hdr = headerbar("", "", end=end, close=close, title_html="<div></div>")
    img = img_html if img_html is not None else timg(it, 280, 180)
    vid = ""
    if video or it.dur:
        vid = (f'<div class="osd-toolbar" style="position:absolute;left:10px;right:10px;bottom:10px;padding:4px 6px">'
               f'<button class="btn circular">{ic("play")}</button><div class="progress grow" style="background:rgb(255 255 255 / 25%)"><div class="bar" style="width:0%"></div></div>'
               f'<span class="label" style="font-size:12px;padding:0 4px">0:00 / {it.dur}</span><button class="btn circular">{ic("speaker")}</button></div>')
    t = title_html if title_html is not None else f'<span class="title-4">{esc(it.name)}</span>{btn("pen", cls="flat small")}'
    acts = actions if actions is not None else (
        f'<div class="actions"><button class="btn suggested-action grow">Open in Editor</button>'
        f'{btn("folder", cls="")}{btn("export")}</div>')
    rows = [prop("Captured", f'{dayname(it.day)}, {it.time}' + (" · " + it.dur if it.dur else "")),
            prop("Type", it.kind if not it.plain else "PNG image"),
            prop("Application", it.app)]
    if it.title:
        rows.append(prop("Window title", it.title))
    if it.url:
        rows.append(prop("Source", "https://" + it.url, suffix=""))
    rows.append(prop("Dimensions", it.px + (f" · scale {it.scale}×" if it.scale != 1 else " · scale 1×")))
    rows.append(prop("File size", it.fsize + (" · MP4" if it.dur else (" · PNG" if it.plain else " · .annota"))))
    rows.append(prop("Location", folder + "/" + filename(it), mono=True))
    props = "" if hide_props else f'<div class="boxed-list">{"".join(rows)}{extra_props}</div>'
    return (f'<div class="pv">{hdr}<div class="pv-body">{extra_top}<div class="pv-img">{img}{vid}</div>'
            f'<div class="pv-title">{t}</div>{acts}{props}</div></div>')


def dayname(k):
    return {"today": "Today", "yesterday": "Yesterday", "wed": "Wed 28 Oct", "tue": "Tue 27 Oct",
            "mon": "Mon 26 Oct", "oct18": "18 Oct 2026", "oct12": "Mon 12 Oct", "oct09": "Fri 9 Oct"}.get(k, k)


def filename(it):
    if it.dur:
        return f"{it.name}.mp4"
    if it.plain:
        return f"{it.name}.png"
    return f"{it.name}.annota"


def content(header, body, cls="view", style=""):
    return f'<div class="content-pane {cls}" style="{style}">{header}{body}</div>'


def window(inner, w=1280, h=800, overlays="", style=""):
    return f'<div class="window" style="width:{w}px;height:{h}px;{style}"><div class="split-view">{inner}</div>{overlays}</div>'


def toast(text, button=None, close=True):
    b = f'<button class="btn flat text-button">{esc(button)}</button>' if button else ""
    c = f'<button class="btn flat circular">{ic("close")}</button>' if close else ""
    return f'<div class="toast-overlay"><div class="toast"><span class="toast-title">{text}</span>{b}{c}</div></div>'


def menu(items, x, y, w=260, arrow_x=None, cls=""):
    """items: list of tuples (label, accel) | 'sep' | ('#title', text) | ('!radio', label, on) ..."""
    out = []
    for it in items:
        if it == "sep":
            out.append('<div class="menu-sep"></div>')
            continue
        kind = it[0]
        if kind == "#":
            out.append(f'<div class="menu-section-title">{esc(it[1])}</div>')
        elif kind in ("radio", "check"):
            _, lab, on, *rest = it
            hov = " hover" if rest and rest[0] == "hover" else ""
            mark = (f'<span class="radio{" checked" if on else ""}" style="width:16px;height:16px"></span>' if kind == "radio"
                    else f'<span class="check{" checked" if on else ""}" style="width:16px;height:16px"></span>')
            out.append(f'<div class="menu-item{hov}">{mark}<span class="label">{esc(lab)}</span></div>')
        else:
            lab, acc, *rest = it
            st = rest[0] if rest else ""
            a = f'<span class="accel">{esc(acc)}</span>' if acc else ""
            out.append(f'<div class="menu-item {st}"><span class="label">{esc(lab)}</span>{a}</div>')
    ax = f"--arrow-x:{arrow_x}px;" if arrow_x is not None else ""
    return f'<div class="popover {cls}" style="left:{x}px;top:{y}px;width:{w}px;{ax}">{"".join(out)}</div>'


def write(change, name, html):
    p = ROOT / "design" / change / "src" / f"{name}.html"
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(html, encoding="utf-8")
    return p


# --------------------------------------------------------------------------- editor shell (kit/template.html)
EDITOR_CSS = r"""
  .tool-strip { width: 52px; flex: none; display: flex; flex-direction: column; align-items: center; gap: 4px; padding: 6px 0; background: var(--window-bg); box-shadow: inset -1px 0 var(--separator); }
  .tool-strip .btn { width: 40px; min-height: 40px; }
  .tool-strip .separator-h { width: 28px; margin: 4px 0; }
  .props .scroll { padding: 18px 12px; display: flex; flex-direction: column; gap: 18px; }
  .tray { flex: none; height: 104px; display: flex; align-items: center; gap: 10px; padding: 0 12px; background: var(--window-bg); box-shadow: inset 0 1px var(--separator); position: relative; }
  .tray .tray-label { writing-mode: vertical-rl; transform: rotate(180deg); font-size: var(--fs-caption-heading); font-weight: var(--fw-bold); opacity: var(--dim-opacity); }
  .tray .ti { width: 112px; height: 80px; flex: none; border-radius: 8px; display: grid; place-items: center; position: relative; background: var(--row-hover); box-shadow: inset 0 0 0 1px var(--separator); }
  .tray .ti.cur { box-shadow: inset 0 0 0 2px var(--accent-bg); background: var(--accent-a12); }
  .tray .ti.hover { background: var(--fill-flat-hover); }
  .tray .ti.sel { box-shadow: inset 0 0 0 2px var(--accent-bg); background: var(--accent-a12); }
  .tray .ti .dot { position: absolute; top: 5px; left: 5px; width: 8px; height: 8px; border-radius: 50%; background: var(--accent-bg); box-shadow: 0 0 0 2px var(--view-bg); }
  .tray .ti .ck { position: absolute; top: 4px; left: 4px; width: 18px; height: 18px; border-radius: 50%; background: var(--accent-bg); color: #fff; display: grid; place-items: center; }
  .tray .ti .ck .icon { width: 12px; height: 12px; }
  .tray .sepv { width: 1px; height: 64px; background: var(--separator); flex: none; }
  .tray .more { width: 112px; height: 80px; flex: none; border-radius: 8px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 4px; box-shadow: inset 0 0 0 1px var(--borders); font-size: var(--fs-small); font-weight: var(--fw-bold); }
  .tray .more .caption { font-weight: var(--fw-regular); opacity: var(--dim-opacity); }
"""

TOOLS = ["select", "arrow", "shape", "text", "callout", "step", "highlighter", "|", "blur", "magnify", "spotlight", "stamp", "crop"]


def tool_strip(active="arrow"):
    out = ['<div class="tool-strip">']
    for t in TOOLS:
        if t == "|":
            out.append('<span class="separator-h"></span>')
        else:
            out.append(f'<button class="btn flat{" active" if t == active else ""}">{ic(t)}</button>')
    out.append("</div>")
    return "".join(out)


def editor_header(title="Checkout flow", subtitle="1600 × 1000 · Edited", end_extra="", primary="Export"):
    start = (btn("capture-region") + '<div class="linked">' + btn("undo") + btn("redo") + "</div>")
    end = end_extra + btn("copy") + f'<button class="btn text-button suggested-action">{esc(primary)}</button>' + btn("open-menu", cls="flat")
    return headerbar(start, title, subtitle, end=end, close=True)


def props_arrow():
    sw = "".join(f'<span class="swatch{" selected" if i == 0 else ""}" style="--c:var(--annota-{c})"></span>'
                 for i, c in enumerate(["coral", "amber", "lime", "teal", "azure", "violet", "ink"]))
    return (f'<div class="sidebar-pane end props"><div class="scroll"><div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Arrow</span></div></div>'
            f'<div class="swatch-row">{sw}</div></div><div class="boxed-list">'
            f'<div class="row"><div class="text"><span class="title">Width</span></div><div class="suffix"><span class="spin" style="width:120px"><span class="value">6</span>{btn("minus")}{btn("plus")}</span></div></div>'
            f'<div class="row"><div class="text"><span class="title">Shadow</span></div><div class="suffix"><span class="switch on"></span></div></div></div></div></div>')
