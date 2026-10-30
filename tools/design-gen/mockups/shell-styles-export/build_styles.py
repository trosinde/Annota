import sys
sys.path.insert(0, "/tmp/claude-0/-home-claude/a6fbb290-1e39-5b02-b596-1e25d7216ff2/scratchpad/asx")
from asx_common import *

C = "add-style-system"

LOCK = ('<svg class="icon lock" viewBox="0 0 16 16"><path fill="currentColor" d="M8 1a4 4 0 0 0-4 4v2H3.5A1.5 1.5 0 0 0 2 8.5v5A1.5 1.5 0 0 0 3.5 15h9a1.5 1.5 0 0 0 1.5-1.5v-5A1.5 1.5 0 0 0 12.5 7H12V5a4 4 0 0 0-4-4zm0 2a2 2 0 0 1 2 2v2H6V5a2 2 0 0 1 2-2z"/></svg>')

CSS = """
  .qs-head { display: flex; align-items: center; gap: 6px; }
  .qs-head .heading { flex: 1; }
  .qs-head.theme-row { justify-content: space-between; margin-top: -6px; }
  .qs-head.theme-row .dropdown { min-height: 30px; }
  .qs-grid { display: grid; grid-template-columns: repeat(4, 63px); gap: 8px; }
  .qs { width: 63px; height: 48px; border-radius: var(--radius-m); background: var(--annota-paper); box-shadow: 0 0 0 1px var(--borders); position: relative; display: grid; place-items: center; }
  .qs svg.prev { width: 63px; height: 48px; }
  .qs.selected { box-shadow: 0 0 0 2px var(--accent-bg); }
  .qs.hover { box-shadow: 0 0 0 1px var(--borders-strong); background-image: linear-gradient(var(--row-hover), var(--row-hover)); }
  .qs.focus { outline: 2px solid var(--accent-a50); outline-offset: 2px; }
  .qs.new { box-shadow: 0 0 0 2px var(--accent-bg), 0 0 0 6px var(--accent-a25); }
  .qs .star { position: absolute; top: 2px; right: 2px; width: 14px; height: 14px; color: var(--annota-amber); }
  .qs .star .icon { width: 12px; height: 12px; filter: drop-shadow(0 0 1px rgb(0 0 0 / 40%)); }
  .qs.add { background: transparent; box-shadow: inset 0 0 0 1px var(--borders); color: var(--window-fg); opacity: .75; }
  .qs-label { font-size: var(--fs-caption-heading); font-weight: var(--fw-bold); opacity: var(--dim-opacity); margin: 4px 0 -6px; }
  .theme-chip { display: inline-flex; gap: 2px; }
  .theme-chip i { width: 10px; height: 10px; border-radius: 3px; display: inline-block; }
  .save-btn { width: 100%; }
  .modified { display: inline-flex; align-items: center; gap: 6px; font-size: var(--fs-small); }
  .modified .badge.dot { background: var(--accent-bg); }
  .fav-tool { display: flex; align-items: center; gap: 8px; font-size: var(--fs-caption-heading); font-weight: var(--fw-bold); opacity: var(--dim-opacity); margin-bottom: -6px; }
  .fav-tool .icon { width: 14px; height: 14px; }
  /* theme dialog */
  .theme-dlg { width: 900px; max-width: 900px; height: 700px; flex-direction: row; }
  .theme-dlg .tlist { width: 260px; flex: none; background: var(--sidebar-bg); display: flex; flex-direction: column; box-shadow: inset -1px 0 var(--sidebar-border); }
  .theme-dlg .tlist .headerbar { background: transparent; box-shadow: none; }
  .theme-dlg .tmain { flex: 1; display: flex; flex-direction: column; min-width: 0; }
  .theme-dlg .tmain .headerbar { background: transparent; box-shadow: none; }
  .theme-dlg .tbody { flex: 1; padding: 6px 24px 18px; display: flex; flex-direction: column; gap: 14px; overflow: hidden; }
  .theme-dlg .nav-row .theme-chip i { width: 8px; height: 16px; border-radius: 2px; }
  .theme-dlg .nav-row .icon.lock { opacity: var(--dim-opacity); }
  .pal { display: flex; gap: 10px; align-items: flex-start; }
  .pal .pc { display: flex; flex-direction: column; align-items: center; gap: 4px; font-size: 11px; font-family: var(--font-mono); opacity: .9; }
  .pal .swatch { width: 40px; height: 40px; border-radius: var(--radius-m); }
  .pal .swatch.add { background: transparent; box-shadow: inset 0 0 0 1px var(--borders-strong); display: grid; place-items: center; color: var(--window-fg); }
  .preview { border-radius: var(--radius-l); background: var(--view-bg); box-shadow: 0 0 0 1px var(--borders); padding: 12px; display: flex; flex-direction: column; gap: 12px; }
  .preview .tiles { display: flex; gap: 8px; }
  .preview .fonts { display: flex; align-items: baseline; gap: 12px; padding-top: 10px; box-shadow: inset 0 1px var(--separator); }
  .preview .mini { flex: none; }
  .ro .pal .swatch { cursor: default; }
  .font-sample { font-size: 20px; font-weight: 700; }
"""

# ---------------------------------------------------------------- previews
def arrow_prev(color, w=4, dashed=False, shadow=False):
    d = ' stroke-dasharray="6 5"' if dashed else ""
    sh = (f'<path d="M14 36 L46 14" stroke="rgb(0 0 0 / 25%)" stroke-width="{w}" stroke-linecap="round" transform="translate(1.5 2)"/>' if shadow else "")
    return (f'<svg class="prev" viewBox="0 0 63 48">{sh}<path d="M14 36 L46 14" stroke="{color}" stroke-width="{w}" stroke-linecap="round"{d} fill="none"/>'
            f'<path d="M35 13 L47 13 L45 25" stroke="{color}" stroke-width="{w}" stroke-linecap="round" stroke-linejoin="round" fill="none"/></svg>')


def callout_prev(fill, fg="#fff", stroke=None):
    s = f' stroke="{stroke}" stroke-width="2"' if stroke else ""
    return (f'<svg class="prev" viewBox="0 0 63 48"><path d="M10 9 h43 a4 4 0 0 1 4 4 v14 a4 4 0 0 1 -4 4 h-26 l-8 8 v-8 h-9 a4 4 0 0 1 -4 -4 v-14 a4 4 0 0 1 4 -4z" fill="{fill}"{s}/>'
            f'<rect x="14" y="16" width="26" height="3" rx="1.5" fill="{fg}"/><rect x="14" y="22" width="18" height="3" rx="1.5" fill="{fg}" opacity=".8"/></svg>')


def step_prev(fill, fg="#fff", shape="circle"):
    sh = (f'<circle cx="31.5" cy="24" r="13" fill="{fill}"/>' if shape == "circle"
          else f'<rect x="18.5" y="11" width="26" height="26" rx="5" fill="{fill}"/>')
    return (f'<svg class="prev" viewBox="0 0 63 48">{sh}<text x="31.5" y="29.5" text-anchor="middle" font-size="15" font-weight="800" fill="{fg}" font-family="Inter">1</text></svg>')


def shape_prev(color, w=3, fill="none"):
    return f'<svg class="prev" viewBox="0 0 63 48"><rect x="12" y="11" width="39" height="26" rx="4" fill="{fill}" stroke="{color}" stroke-width="{w}"/></svg>'


def qs(prev, cls="", star=False):
    s = f'<span class="star">{ic("star")}</span>' if star else ""
    return f'<div class="qs {cls}">{prev}{s}</div>'


CO = "var(--annota-coral)"; AM = "var(--annota-amber)"; LI = "var(--annota-lime)"; TE = "var(--annota-teal)"
AZ = "var(--annota-azure)"; VI = "var(--annota-violet)"; INK = "var(--annota-ink)"

BRAND = ["#0b5cad", "#00a0b0", "#f2a900", "#e4572e", "#2d2d34"]
BRAND_NAMES = ["#0B5CAD", "#00A0B0", "#F2A900", "#E4572E", "#2D2D34"]

ANNOTA_THEME_ARROWS = [arrow_prev(CO, 4), arrow_prev(AM, 4), arrow_prev(TE, 4), arrow_prev(AZ, 4),
                       arrow_prev(INK, 4), arrow_prev(CO, 7), arrow_prev(CO, 4, dashed=True), arrow_prev(CO, 5, shadow=True)]


def theme_dropdown(name="Annota", colors=("var(--annota-coral)", "var(--annota-amber)", "var(--annota-teal)", "var(--annota-azure)")):
    chip = '<span class="theme-chip">' + "".join(f'<i style="background:{c}"></i>' for c in colors) + "</span>"
    return f'<span class="dropdown flat" style="font-size:var(--fs-small)">{chip}<span>{name}</span></span>'


def styles_sidebar(tool="Arrow", theme_tiles=None, my_tiles=None, view="tool", save_state="", modified=False,
                   theme_name="Annota", theme_colors=None, extra_props=True, selected_props=None):
    theme_tiles = theme_tiles or [qs(p, "selected" if i == 0 else "") for i, p in enumerate(ANNOTA_THEME_ARROWS)]
    my_tiles = my_tiles if my_tiles is not None else [qs(arrow_prev("#e4572e", 6), star=True), qs(arrow_prev(INK, 3, dashed=True))]
    tg = (f'<div class="toggle-group" style="width:100%"><span class="toggle {"active" if view=="tool" else ""}" style="flex:1">{tool}</span>'
          f'<span class="toggle {"active" if view=="fav" else ""}" style="flex:1">{ic("star")} Favourites</span></div>')
    dd = theme_dropdown(theme_name, theme_colors) if theme_colors else theme_dropdown(theme_name)
    head = (f'<div class="qs-head"><span class="heading">Quick Styles</span></div>'
            f'<div class="qs-head theme-row"><span class="caption dim-label">Theme</span>{dd}</div>')
    grid = (f'<div class="qs-grid">{"".join(theme_tiles)}</div>'
            f'<div class="qs-label">My Styles</div><div class="qs-grid">{"".join(my_tiles)}</div>')
    mod = (f'<span class="modified"><span class="badge dot"></span>Modified</span>' if modified else "")
    props = selected_props if selected_props is not None else f"""
<div class="vbox" style="--gap:12px">
  <div class="qs-head"><span class="heading">Properties</span>{mod}</div>
  {swatch_row("coral")}
  <div class="boxed-list">
    <div class="row"><div class="text"><span class="title">Width</span></div><div class="suffix">{spin("4")}</div></div>
    <div class="row"><div class="text"><span class="title">Shadow</span></div><div class="suffix"><span class="switch"></span></div></div>
  </div>
  <button class="btn save-btn {save_state}">{ic("plus")} Save as Quick Style</button>
</div>"""
    return sidebar(f'{tg}<div class="vbox" style="--gap:12px">{head}{grid}</div>{props}')


# 01 — quick style grid, hover tooltip on a tile
tip = (f'<div class="tooltip" style="left:1030px;top:294px">Coral · 7 px · shadow</div>')
theme_tiles = [qs(p, "selected" if i == 0 else ("hover" if i == 5 else "")) for i, p in enumerate(ANNOTA_THEME_ARROWS)]
ed = editor(side=styles_sidebar(theme_tiles=theme_tiles), overlay=tip)
write(C, "01-quick-style-grid", page("quick styles", ed, css=CSS))

# 02 — modified props, save-as popover
ARROW_TEAL_W8 = arrow_prev("#14b8a6", 8)
canvas_teal = canvas(annotations="""<path d="M372 388 L276 292" stroke="var(--annota-teal)" stroke-width="10" stroke-linecap="round" fill="none"/>
<path d="M268 322 L268 284 L306 284" stroke="var(--annota-teal)" stroke-width="10" stroke-linecap="round" stroke-linejoin="round" fill="none"/>""",
    extra='<div class="sel-box" style="left:262px;top:278px;width:116px;height:116px"><span class="h" style="left:0;top:0"></span><span class="h" style="left:100%;top:100%"></span></div>')
props_mod = f"""
<div class="vbox" style="--gap:12px">
  <div class="qs-head"><span class="heading">Properties</span><span class="modified"><span class="badge dot"></span>Modified</span></div>
  {swatch_row("teal")}
  <div class="boxed-list">
    <div class="row"><div class="text"><span class="title">Width</span></div><div class="suffix">{spin("8")}</div></div>
    <div class="row"><div class="text"><span class="title">Shadow</span></div><div class="suffix"><span class="switch"></span></div></div>
  </div>
  <button class="btn save-btn active">{ic("plus")} Save as Quick Style</button>
</div>"""
theme_tiles_none = [qs(p) for p in ANNOTA_THEME_ARROWS]
pop = f"""<div class="popover arrow-down" style="right:12px;top:400px;width:276px;--arrow-x:138px;padding:12px">
  <div class="vbox" style="--gap:12px">
    <div class="heading">Save as Quick Style</div>
    <div class="hbox" style="--gap:12px"><div class="qs" style="flex:none">{ARROW_TEAL_W8}</div><div class="caption dim-label">Teal · 8 px · no shadow<br>Added to the end of Arrow</div></div>
    <span class="entry focus" style="width:100%"><span>Teal bold</span><span class="caret" style="margin-left:-4px"></span></span>
    <label class="check-label"><span class="check checked"></span>Add to Favourites</label>
    <div class="hbox" style="justify-content:flex-end"><button class="btn">Cancel</button><button class="btn suggested-action">Save</button></div>
  </div>
</div>"""
ed = editor(canvas_html=canvas_teal, side=styles_sidebar(theme_tiles=theme_tiles_none, selected_props=props_mod), overlay=pop)
write(C, "02-save-as-quick-style", page("save quick style", ed, css=CSS))

# 03 — new style appended at end + context menu on user style (rename/reorder/delete) + toast
my = [qs(arrow_prev("#e4572e", 6), star=True), qs(arrow_prev(INK, 3, dashed=True)), qs(ARROW_TEAL_W8, "new selected", star=True)]
menu = popover_menu([("Apply", "Enter"), ("Rename…", "F2"), "-", ("Move Left", "Ctrl+←"), ("Move Right", "Ctrl+→", "disabled"), "-",
                     ("Add to Favourites", None), ("Update from Current Properties", None), "-",
                     ("Delete", "Delete")], "left:1000px;top:406px;width:268px;--arrow-x:165px")
menu = menu.replace('<span class="label">Delete</span>', '<span class="label error">Delete</span>')
props_saved = props_mod.replace('<span class="modified"><span class="badge dot"></span>Modified</span>', '<span class="caption dim-label">Teal bold</span>').replace("btn save-btn active", "btn save-btn\" disabled=\"")
ed = editor(canvas_html=canvas_teal, side=styles_sidebar(theme_tiles=theme_tiles_none, my_tiles=my, selected_props=props_saved),
            overlay=menu + toast("Quick style “Teal bold” saved", "Undo"))
write(C, "03-style-saved-context-menu", page("style saved", ed, css=CSS))

# 04 — favourites view across tools
def fav_group(icon_name, label, tiles):
    return f'<div class="fav-tool">{ic(icon_name)}{label}</div><div class="qs-grid">{"".join(tiles)}</div>'

favs = (fav_group("arrow", "Arrow", [qs(arrow_prev("#e4572e", 6), star=True), qs(ARROW_TEAL_W8, star=True)]) +
        fav_group("callout", "Callout", [qs(callout_prev(CO), "hover focus", star=True), qs(callout_prev("#ffffff", INK, INK), star=True), qs(callout_prev(AM, INK), star=True)]) +
        fav_group("step", "Step", [qs(step_prev(CO), star=True), qs(step_prev(INK, shape="square"), star=True)]) +
        fav_group("shape", "Shape", [qs(shape_prev(CO, 3), star=True)]))
fav_side = sidebar(f"""<div class="toggle-group" style="width:100%"><span class="toggle" style="flex:1">Arrow</span><span class="toggle active" style="flex:1">{ic("star")} Favourites</span></div>
<div class="vbox" style="--gap:12px">{favs}</div>
<div class="caption dim-label">Pick a favourite to switch to its tool. Star any quick style with right-click → Add to Favourites.</div>""")
tip = '<div class="tooltip" style="left:784px;top:262px">Callout · Coral, white text <span class="shortcut">Enter</span></div>'
ed = editor(side=fav_side, overlay=tip)
write(C, "04-favourites-view", page("favourites", ed, css=CSS))

# 05 — after picking a callout favourite: callout tool active with that style
callout_svg = """<path d="M282 300 h196 a8 8 0 0 1 8 8 v48 a8 8 0 0 1 -8 8 h-196 a8 8 0 0 1 -8 -8 v-48 a8 8 0 0 1 8 -8z" fill="var(--annota-coral)"/>
<path d="M290 302 L246 278 L326 302z" fill="var(--annota-coral)"/>
<text x="290" y="328" font-family="Inter" font-size="15" font-weight="700" fill="#fff">Postcode needs 5 digits</text>
<text x="290" y="349" font-family="Inter" font-size="13" fill="#fff">Shown only after submit</text>"""
c5 = canvas(annotations=callout_svg + CHECKOUT_EXTRA)
callout_tiles = [qs(callout_prev(CO), "selected", star=True), qs(callout_prev(AM, INK)), qs(callout_prev(TE)), qs(callout_prev(AZ)),
                 qs(callout_prev("#ffffff", INK, INK), star=True), qs(callout_prev(INK)), qs(callout_prev("#ffffff", CO, CO)), qs(callout_prev(VI))]
my_c = [qs(callout_prev(AM, INK), star=True)]
props_c = f"""
<div class="vbox" style="--gap:12px">
  <div class="qs-head"><span class="heading">Properties</span></div>
  {swatch_row("coral")}
  <div class="boxed-list">
    <div class="row"><div class="text"><span class="title">Text colour</span></div><div class="suffix"><span class="swatch small" style="--c:#ffffff"></span></div></div>
    <div class="row"><div class="text"><span class="title">Font size</span></div><div class="suffix">{spin("15")}</div></div>
  </div>
  <button class="btn save-btn" disabled>{ic("plus")} Save as Quick Style</button>
</div>"""
side5 = styles_sidebar(tool="Callout", theme_tiles=callout_tiles, my_tiles=my_c, selected_props=props_c)
ed = editor(active="callout", canvas_html=c5, side=side5)
write(C, "05-favourite-applied-callout", page("favourite applied", ed, css=CSS))

# ---------------------------------------------------------------- theme dialog
def theme_row(name, colors, sel=False, lock=False, sub=None):
    chip = '<span class="theme-chip">' + "".join(f'<i style="background:{c}"></i>' for c in colors[:5]) + "</span>"
    l = LOCK if lock else ""
    s = f'<span class="count">{sub}</span>' if sub else ""
    return f'<div class="nav-row {"selected" if sel else ""}">{chip}<span class="label">{name}</span>{s}{l}</div>'

ANNOTA_COLS = ["#ff5a4e", "#f5a524", "#8bc34a", "#14b8a6", "#2f80ed", "#8b5cf6", "#1f1f24", "#ffffff"]
HARBOR = ["#14324f", "#3fa7d6", "#f0c808", "#d1495b"]
MONO = ["#1f1f24", "#77767b", "#ff5a4e"]


def theme_list(sel):
    return (f'<div class="tlist"><div class="headerbar"><div class="start"><button class="btn flat image-button" title="New Theme">{ic("plus")}</button></div>'
            f'<div class="title-widget"><span class="title">Themes</span></div><div class="end"><button class="btn flat image-button" title="Import Theme…">{ic("folder")}</button></div></div>'
            f'<div class="sidebar"><div class="sidebar-list" style="background:transparent">'
            f'<div class="section-title">Built-in</div>'
            + theme_row("Annota", ANNOTA_COLS, sel == "annota", sub="Default")
            + theme_row("Monochrome", MONO, sel == "mono")
            + '<div class="section-title">Installed by administrator</div>'
            + theme_row("Harbor Corporate", HARBOR, sel == "harbor", lock=True)
            + '<div class="section-title">My Themes</div>'
            + theme_row("Northwind Ferries", BRAND, sel == "north")
            + "</div></div></div>")


def palette(colors, names=None, editable=True, focus=None):
    names = names or [c.upper() for c in colors]
    out = []
    for i, (c, n) in enumerate(zip(colors, names)):
        cls = "swatch square" + (" focus" if i == focus else "") + (" light-ink" if c.lower() in ("#ffffff", "#f2a900", "#f0c808") else "")
        out.append(f'<div class="pc"><span class="{cls}" style="--c:{c}"></span>{n}</div>')
    if editable and len(colors) < 8:
        out.append(f'<div class="pc"><span class="swatch square add">{ic("plus")}</span>Add</div>')
    return f'<div class="pal">{"".join(out)}</div>'


def preview(colors, heading_font="Inter", body_font="Inter"):
    c0, c1, c2 = colors[0], colors[1], colors[2 % len(colors)]
    hf = {"Inter Display": "'Inter Display', Inter", "DejaVu Serif": "'DejaVu Serif'", "DejaVu Sans": "'DejaVu Sans'"}.get(heading_font, heading_font)
    bf = {"DejaVu Sans": "'DejaVu Sans'"}.get(body_font, body_font)
    return f"""<div class="preview"><div class="tiles">
  <div class="qs mini">{arrow_prev(c0, 4)}</div><div class="qs mini">{arrow_prev(c1, 4)}</div>
  <div class="qs mini">{callout_prev(c0)}</div><div class="qs mini">{callout_prev(c2, "#1f1f24")}</div>
  <div class="qs mini">{step_prev(c0)}</div><div class="qs mini">{step_prev(c1)}</div>
  <div class="qs mini">{shape_prev(c0)}</div><div class="qs mini">{shape_prev(c1)}</div>
</div><div class="fonts"><span style="font-family:{hf};font-size:20px;font-weight:700;color:{c0}">Delivery address</span><span style="font-family:{bf};font-size:15px">Enter a 5-digit postcode</span><span class="caption dim-label" style="margin-left:auto">{heading_font} / {body_font}</span></div></div>"""


def theme_dialog(sel, name, colors, names=None, ro=False, extra_body="", heading_font="Inter Display", body_font="Inter",
                 focus=None, end_buttons=None, use=True):
    end = end_buttons if end_buttons is not None else (
        f'<button class="btn flat image-button" title="Export Theme…">{ic("export")}</button>'
        f'<button class="btn flat image-button" title="More">{ic("view-more")}</button>')
    banner = ""
    if ro:
        banner = (f'<div class="banner" style="border-radius:var(--radius-m)"><span class="banner-title">Managed by your administrator · read-only</span>'
                  f'<button class="btn">Duplicate</button></div>')
    name_row = (f'<div class="row entry-row"><div class="text"><span class="title">Name</span><span>{name}</span></div></div>' if not ro else
                f'<div class="row property"><div class="text"><span class="title">Name</span><span class="subtitle">{name}</span></div><div class="suffix">{LOCK}</div></div>')
    dd = "dropdown flat" + (" disabled" if ro else "")
    ro_style = ' style="opacity:var(--disabled-opacity)"' if ro else ""
    return f"""<div class="dialog-host"><div class="dialog theme-dlg {"ro" if ro else ""}">
{theme_list(sel)}
<div class="tmain">
  <div class="headerbar"><div class="start">{'<button class="btn text-button suggested-action">Use Theme</button>' if use else '<span class="caption dim-label" style="padding-left:6px">In use</span>'}</div><div class="title-widget"><span class="title">{name}</span></div><div class="end">{end}<div class="window-controls"><span class="wc">{ic("close")}</span></div></div></div>
  <div class="tbody">
    {banner}
    <div class="boxed-list">{name_row}</div>
    <div class="preferences-group">
      <div class="group-header"><div class="text"><span class="heading">Palette</span><span class="description">2–8 colours. The first colour is the default for every tool.</span></div></div>
      <div>{palette(colors, names, editable=not ro, focus=focus)}</div>
    </div>
    <div class="boxed-list">
      <div class="row"><div class="text"><span class="title">Heading font</span><span class="subtitle">Callout titles, text tool headings</span></div><div class="suffix"><span class="{dd}"{ro_style}>{heading_font}</span></div></div>
      <div class="row"><div class="text"><span class="title">Body font</span><span class="subtitle">Callout and text body, step numbers</span></div><div class="suffix"><span class="{dd}"{ro_style}>{body_font}</span></div></div>
    </div>
    <div class="preferences-group">
      <div class="group-header"><div class="text"><span class="heading">Preview</span><span class="description">Theme quick styles generated for every tool</span></div></div>
      {preview(colors, heading_font, body_font)}
    </div>
    {extra_body}
  </div>
</div>
</div></div>"""


THEME_CSS = CSS
# 06 — theme editor (user theme, editable)
ed = editor(side=styles_sidebar(), overlay=theme_dialog("north", "Northwind Ferries", BRAND, BRAND_NAMES, focus=3,
            heading_font="Inter Display", body_font="Inter"))
write(C, "06-theme-editor", page("theme editor", ed, css=CSS))

# 07 — theme applied: sidebar shows brand-coloured theme styles; theme dropdown open
brand_tiles = [qs(arrow_prev(BRAND[0], 4), "selected"), qs(arrow_prev(BRAND[1], 4)), qs(arrow_prev(BRAND[2], 4)), qs(arrow_prev(BRAND[3], 4)),
               qs(arrow_prev(BRAND[4], 4)), qs(arrow_prev(BRAND[0], 7)), qs(arrow_prev(BRAND[0], 4, dashed=True)), qs(arrow_prev(BRAND[0], 5, shadow=True))]
props_brand = f"""
<div class="vbox" style="--gap:12px">
  <div class="qs-head"><span class="heading">Properties</span></div>
  {swatch_row(0, BRAND)}
  <div class="boxed-list">
    <div class="row"><div class="text"><span class="title">Width</span></div><div class="suffix">{spin("4")}</div></div>
    <div class="row"><div class="text"><span class="title">Shadow</span></div><div class="suffix"><span class="switch"></span></div></div>
  </div>
  <button class="btn save-btn" disabled>{ic("plus")} Save as Quick Style</button>
</div>"""
side7 = styles_sidebar(theme_tiles=brand_tiles, theme_name="Northwind Ferries", theme_colors=BRAND[:4], selected_props=props_brand)
side7 = side7.replace('<span class="dropdown flat"', '<span class="dropdown flat active" style="background:var(--fill-flat-active);font-size:var(--fs-small)"', 1)
chip = lambda cols: '<span class="theme-chip">' + "".join(f'<i style="background:{c}"></i>' for c in cols[:4]) + "</span>"
tpop = f"""<div class="popover" style="right:12px;top:182px;width:250px;--arrow-x:180px">
  <div class="menu-item">{chip(ANNOTA_COLS)}<span class="label">Annota</span></div>
  <div class="menu-item">{chip(MONO)}<span class="label">Monochrome</span></div>
  <div class="menu-item">{chip(HARBOR)}<span class="label">Harbor Corporate</span>{LOCK}</div>
  <div class="menu-item selected">{chip(BRAND)}<span class="label">Northwind Ferries</span>{ic("check")}</div>
  <div class="menu-sep"></div>
  <div class="menu-item"><span class="label">Manage Themes…</span></div>
</div>"""
c7 = canvas(annotations=CHECKOUT_ARROW.replace("var(--annota-coral)", BRAND[0]) + CHECKOUT_EXTRA.replace("var(--annota-coral)", BRAND[0]))
ed = editor(canvas_html=c7, side=side7, overlay=tpop)
write(C, "07-theme-applied", page("theme applied", ed, css=CSS))

# 08 — managed theme read-only
ed = editor(side=styles_sidebar(), overlay=theme_dialog("harbor", "Harbor Corporate", HARBOR, ro=True,
            heading_font="DejaVu Serif", body_font="DejaVu Sans",
            end_buttons=f'<button class="btn flat image-button" title="Export Theme…">{ic("export")}</button>'))
write(C, "08-managed-theme-readonly", page("managed theme", ed, css=CSS))

# 09 — more menu (export / duplicate / delete) + import success toast
more = popover_menu([("Export Theme…", None), ("Duplicate", None), "-", ("Delete Theme", None)],
                    "right:58px;top:132px;width:220px;--arrow-x:170px")
more = more.replace('<span class="label">Delete Theme</span>', '<span class="label error">Delete Theme</span>')
dlg = theme_dialog("north", "Northwind Ferries", BRAND, BRAND_NAMES)
dlg = dlg.replace('title="More">', 'title="More" style="background:var(--fill-flat-active)">')
dlg = dlg.replace('<div class="tmain">', '<div class="tmain" style="position:relative">')
dlg = dlg.replace("</div>\n</div></div>", more + "</div>\n</div></div>")
ed = editor(side=styles_sidebar(), overlay=dlg + toast("Theme “Northwind Ferries” imported", "Apply"))
write(C, "09-theme-import-export", page("theme import export", ed, css=CSS))

# 10 — import error
err = alert("Can’t Import Theme", "“brand-2024.annota-theme” is not a valid Annota theme. It contains 11 colours; a theme needs 2–8.",
            [("OK", "suggested-action")])
ed = editor(side=styles_sidebar(), overlay=err)
write(C, "10-theme-import-error", page("theme import error", ed, css=CSS))
print("ok")
