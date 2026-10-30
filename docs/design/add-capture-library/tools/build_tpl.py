"""Build add-templates mockups:  cd docs/design/add-capture-library/tools && python3 build_tpl.py"""
from mock import *

CH = "add-templates"
IT = ITEMS()
B = by_key(IT)

TPL_CSS = EDITOR_CSS + r"""
  /* document page (template) — theme colours are template tokens, shown with Annota ink */
  .page { position: relative; background: #ffffff; color: #1f1f24; box-shadow: 0 0 0 1px rgb(0 0 6 / 10%), 0 2px 10px rgb(0 0 6 / 18%); font-family: var(--font-ui); }
  .page .p-title { font-weight: 800; letter-spacing: -.01em; }
  .page .p-sub { color: #6b6b73; }
  .page .rule { height: 3px; background: var(--t, var(--annota-coral)); border-radius: 2px; }
  .page .num { display: grid; place-items: center; border-radius: 50%; background: var(--t, var(--annota-coral)); color: #fff; font-weight: 800; flex: none; }
  .page .cap { color: #1f1f24; }
  .page .cap .dim { color: #6b6b73; }
  .slot { position: relative; display: grid; place-items: center; overflow: hidden; border-radius: 6px; background: #f3f3f5; }
  .slot.empty { background: repeating-linear-gradient(135deg, #f6f6f8 0 10px, #efeff2 10px 20px); box-shadow: inset 0 0 0 1.5px #c8c8d0; border-radius: 6px; color: #77777f; }
  .slot.empty .hint { display: flex; flex-direction: column; align-items: center; gap: 6px; font-size: 13px; font-weight: 700; }
  .slot.empty .hint .caption { font-weight: 400; color: #8a8a92; }
  .slot.drop { background: rgb(53 132 228 / 12%); box-shadow: inset 0 0 0 2px #3584e4; color: #1c71d8; }
  .slot .timg { box-shadow: none; border-radius: 0; }
  .slot-sel { position: absolute; outline: 1.5px solid var(--accent-bg); pointer-events: none; }
  .slot-sel .h { position: absolute; width: 10px; height: 10px; margin: -5px 0 0 -5px; background: #fff; border: 1.5px solid var(--accent-bg); border-radius: 50%; }
  .slot-badge { position: absolute; top: 6px; left: 6px; height: 20px; padding: 0 7px; border-radius: 10px; background: var(--accent-bg); color: #fff; font-size: 11px; font-weight: 700; display: inline-flex; align-items: center; gap: 4px; }
  .slot-badge .icon { width: 12px; height: 12px; }
  /* chooser */
  .tpl-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 18px 14px; }
  .tpl { display: flex; flex-direction: column; gap: 8px; }
  .tpl .thumb { height: 132px; border-radius: var(--radius-l); background: var(--card-bg); box-shadow: var(--shadow-card); display: grid; place-items: center; padding: 12px; position: relative; }
  .tpl.selected .thumb { box-shadow: 0 0 0 2px var(--accent-bg), var(--shadow-card); }
  .tpl.hover .thumb { background-image: linear-gradient(var(--row-hover), var(--row-hover)); }
  .tpl .t { font-weight: 700; font-size: 14px; }
  .tpl .d { font-size: var(--fs-caption); opacity: var(--dim-opacity); margin-top: -6px; }
  .mini { width: 100%; max-width: 160px; height: 106px; background: #ffffff; border-radius: 3px; box-shadow: 0 0 0 1px rgb(0 0 6 / 12%), 0 1px 3px rgb(0 0 6 / 12%); position: relative; padding: 8px; display: flex; flex-direction: column; gap: 5px; }
  .mini .bar { height: 5px; border-radius: 2px; background: #1f1f24; width: 55%; }
  .mini .ln { height: 3px; border-radius: 2px; background: #c9c9d0; }
  .mini .bx { background: #dcdce2; border-radius: 2px; }
  .mini .dot { width: 9px; height: 9px; border-radius: 50%; background: var(--t, var(--annota-coral)); flex: none; }
  .mini .row { display: flex; gap: 5px; align-items: center; min-height: 0; padding: 0; }
  .side-cat .nav-row { min-height: 36px; }
  .combine-list .row { min-height: 0; padding: 8px 10px; gap: 10px; }
  .combine-list .row .tbox { width: 64px; height: 44px; flex: none; display: grid; place-items: center; background: var(--fill-1); border-radius: 6px; }
  .combine-list .row .n { width: 22px; height: 22px; border-radius: 50%; background: var(--annota-coral); color: #fff; font-weight: 800; font-size: 12px; display: grid; place-items: center; flex: none; }
  .combine-list .row .entry { flex: 1; min-height: 32px; font-size: 14px; }
  .combine-list .row.dragging { background: var(--card-bg); box-shadow: 0 6px 18px rgb(0 0 6 / 25%); transform: translateX(8px) rotate(-.6deg); z-index: 2; }
  .combine-list .row.gap { min-height: 60px; background: var(--accent-a12); box-shadow: inset 0 0 0 2px var(--accent-a50); }
"""


def W(name, title, body, size=(1280, 800)):
    write(CH, name, page(title, body, size, True, TPL_CSS))


def esc_(s):
    return esc(s)


# ---------------------------------------------------------------- page renderers
def slot_html(it, w, h, empty_label="Drop a capture", fill=False, state=""):
    if it is None:
        cls = "slot empty" + (" drop" if state == "drop" else "")
        icon = ic("image", "icon large")
        sub = '<span class="caption">or click to choose from the library</span>' if state != "drop" else '<span class="caption" style="color:inherit">Release to place · fitted</span>'
        return f'<div class="{cls}" style="width:{w}px;height:{h}px"><div class="hint">{icon}{esc(empty_label)}{sub}</div></div>'
    sel = ""
    if state == "selected":
        sel = ('<span class="slot-sel" style="inset:0"><span class="h" style="left:0;top:0"></span><span class="h" style="left:100%;top:0"></span>'
               '<span class="h" style="left:0;top:100%"></span><span class="h" style="left:100%;top:100%"></span></span>'
               f'<span class="slot-badge">{ic("image")}Fit</span>')
    return f'<div class="slot" style="width:{w}px;height:{h}px;overflow:visible">{timg(it, w, h, fill=fill)}{sel}</div>'


def step_guide_page(title, sub, steps, width=640, numbered=True, theme="var(--annota-coral)", slot_h=170, show_sel=None, drop=None, selected=None):
    """steps: list of (item or None, caption, detail)"""
    sw = width - 48 - 44
    rows = []
    for n, (it, cap, det) in enumerate(steps, 1):
        num = f'<span class="num" style="width:30px;height:30px;font-size:15px">{n}</span>' if numbered else ""
        state = "drop" if drop == n else ("selected" if selected == n else "")
        rows.append(f'<div style="display:flex;gap:14px;align-items:flex-start">{num}<div style="flex:1;display:flex;flex-direction:column;gap:8px">'
                    f'<div class="cap" style="font-size:14px;font-weight:700;line-height:1.3">{esc(cap)}<div class="dim" style="font-weight:400;font-size:12px;margin-top:2px">{esc(det)}</div></div>'
                    f'{slot_html(it, sw if numbered else sw + 44, slot_h, state=state)}</div></div>')
    return (f'<div class="page" style="width:{width}px;padding:26px 24px 24px;--t:{theme};display:flex;flex-direction:column;gap:16px">'
            f'<div><div class="p-title" style="font-size:24px">{esc(title)}</div><div class="p-sub" style="font-size:13px;margin-top:4px">{esc(sub)}</div></div>'
            f'<div class="rule"></div>{"".join(rows)}</div>')


# ---------------------------------------------------------------- mini template thumbnails (original layouts)
def mini_step(n=3, t="var(--annota-coral)"):
    rows = "".join('<div class="row"><span class="dot"></span><div class="bx" style="flex:1;height:18px"></div></div>' for _ in range(n))
    return f'<div class="mini" style="--t:{t}"><div class="bar"></div><div class="ln" style="width:80%"></div>{rows}</div>'


def mini_compare(cols=2, t="var(--annota-coral)"):
    c = "".join('<div style="flex:1;display:flex;flex-direction:column;gap:4px"><div class="ln" style="width:60%;background:var(--t)"></div><div class="bx" style="height:46px"></div><div class="ln"></div><div class="ln" style="width:70%"></div></div>' for _ in range(cols))
    return f'<div class="mini" style="--t:{t}"><div class="bar"></div><div style="display:flex;gap:6px;flex:1">{c}</div></div>'


def mini_beforeafter(t="var(--annota-coral)"):
    return (f'<div class="mini" style="--t:{t}"><div class="bar"></div><div style="display:flex;gap:4px;align-items:center;flex:1">'
            f'<div style="flex:1;display:flex;flex-direction:column;gap:3px"><div class="bx" style="height:52px;opacity:.7"></div><div class="ln" style="width:50%"></div></div>'
            f'<svg width="14" height="14" viewBox="0 0 14 14"><path d="M2 7h9M8 3.5 11.5 7 8 10.5" stroke="{t}" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round"/></svg>'
            f'<div style="flex:1;display:flex;flex-direction:column;gap:3px"><div class="bx" style="height:52px;box-shadow:inset 0 0 0 1.5px {t}"></div><div class="ln" style="width:50%;background:{t}"></div></div></div></div>')


def mini_grid(c=2, r=2, t="var(--annota-coral)"):
    cells = "".join('<div class="bx"></div>' for _ in range(c * r))
    return f'<div class="mini" style="--t:{t}"><div class="bar"></div><div style="display:grid;grid-template-columns:repeat({c},1fr);grid-template-rows:repeat({r},1fr);gap:4px;flex:1">{cells}</div></div>'


def mini_timeline(t="var(--annota-coral)"):
    pts = ""
    for i, x in enumerate([14, 50, 86]):
        top = i % 2 == 0
        pts += (f'<div style="position:absolute;left:{x}%;top:50%;width:9px;height:9px;margin:-4.5px 0 0 -4.5px;border-radius:50%;background:{t}"></div>'
                f'<div class="bx" style="position:absolute;left:calc({x}% - 20px);{"top:4px" if top else "bottom:4px"};width:40px;height:26px"></div>')
    return (f'<div class="mini" style="--t:{t}"><div class="bar"></div><div style="position:relative;flex:1">'
            f'<div style="position:absolute;left:4px;right:4px;top:50%;height:2px;margin-top:-1px;background:{t};opacity:.5"></div>{pts}</div></div>')


def mini_custom():
    return ('<div class="mini" style="--t:var(--annota-azure)"><div class="bar" style="width:40%"></div><div style="display:grid;grid-template-columns:2fr 1fr;gap:4px;flex:1">'
            '<div class="bx" style="grid-row:span 2"></div><div class="bx"></div><div class="bx"></div></div><div class="ln" style="width:60%"></div></div>')


TEMPLATES = [
    ("step", "Step Guide", "2–8 numbered steps", mini_step()),
    ("cmp2", "Comparison", "2 or 3 columns", mini_compare(2)),
    ("ba", "Before / After", "Two captures, one change", mini_beforeafter()),
    ("g22", "Grid 2 × 2", "Four equal tiles", mini_grid(2, 2)),
    ("g32", "Grid 3 × 2", "Six equal tiles", mini_grid(3, 2)),
    ("tl", "Timeline", "Captures along a line", mini_timeline()),
]


def tpl_card(key, title, desc, mini, sel=False, hover=False):
    return (f'<div class="tpl{" selected" if sel else ""}{" hover" if hover else ""}"><div class="thumb">{mini}</div>'
            f'<span class="t">{esc(title)}</span><span class="d">{esc(desc)}</span></div>')


def chooser(selected="step", cat="builtin", mine=False, hover=None, steps=4):
    cats = (nav("template", "Built-in", "6", "selected" if cat == "builtin" else "")
            + nav("star", "My Templates", "1" if mine else "0", "selected" if cat == "mine" else ""))
    if cat == "builtin":
        cards = "".join(tpl_card(k, t, d, m, k == selected, k == hover) for k, t, d, m in TEMPLATES)
        side = (f'<div class="preferences-group" style="gap:10px"><div class="group-header"><div class="text"><span class="heading">Step Guide</span></div></div>'
                f'<div class="boxed-list">'
                f'<div class="row"><div class="text"><span class="title">Steps</span></div><div class="suffix"><span class="spin" style="width:110px"><span class="value">{steps}</span>{btn("minus")}{btn("plus")}</span></div></div>'
                f'<div class="row"><div class="text"><span class="title">Numbering</span></div><div class="suffix"><span class="switch on"></span></div></div>'
                f'<div class="row"><div class="text"><span class="title">Theme</span></div><div class="suffix"><div class="swatch-row" style="gap:4px">'
                + "".join(f'<span class="swatch small{" selected" if c == "coral" else ""}" style="--c:var(--annota-{c})"></span>' for c in ["coral", "teal", "azure", "violet", "ink"])
                + f'</div></div></div></div><div class="caption dim-label" style="padding:0 4px">Page 1240 × auto · A4 when exported to PDF</div></div>')
    else:
        cards = (tpl_card("mine", "Release note card", "3 slots · saved today", mini_custom(), True)
                 + '<div class="tpl"><div class="thumb" style="box-shadow:inset 0 0 0 1px var(--borders);background:transparent">'
                 + f'<div class="vbox" style="--gap:6px;align-items:center;opacity:.6">{ic("plus", "icon large")}<span class="caption">Save the current document as a template</span></div></div></div>')
        side = (f'<div class="preferences-group" style="gap:10px"><div class="group-header"><div class="text"><span class="heading">Release note card</span><span class="description">Saved from “Release 0.4 highlights” · ~/.local/share/annota/templates</span></div></div>'
                f'<div class="boxed-list"><div class="row"><div class="text"><span class="title">Slots</span></div><div class="suffix"><span class="value">3</span></div></div>'
                f'<div class="row"><div class="text"><span class="title">Theme</span></div><div class="suffix"><span class="swatch small selected" style="--c:var(--annota-azure)"></span></div></div></div>'
                f'<div class="hbox" style="--gap:6px"><button class="btn text-button">Rename…</button><button class="btn text-button" style="color:var(--destructive)">Delete…</button></div></div>')
    return (f'<div class="dialog-host"><div class="dialog" style="width:1140px;max-width:1140px;height:640px">'
            + headerbar('<button class="btn text-button">Cancel</button>', "New from Template", end='<button class="btn text-button suggested-action">Create</button>', cls="raised")
            + f'<div style="display:flex;flex:1;min-height:0"><div class="sidebar side-cat" style="width:190px;background:var(--sidebar-bg)"><div class="sidebar-list">{cats}</div></div>'
            f'<div style="flex:1;padding:18px 18px;overflow:hidden"><div class="tpl-grid">{cards}</div></div>'
            f'<div style="width:290px;flex:none;padding:18px 16px;box-shadow:inset 1px 0 var(--separator)">{side}</div></div></div></div>')


def editor_win(canvas_html, props_html, title="Untitled step guide", subtitle="Step guide · 4 slots · Not saved", overlays="", tray_html="", active="select", cw=None):
    canvas = (f'<div class="content-pane" style="position:relative"><div class="canvas-area" style="align-items:flex-start;padding-top:24px">{canvas_html}'
              f'<div class="osd-toolbar" style="position:absolute;right:18px;bottom:18px">{btn("zoom-out")}<span class="label">60 %</span>{btn("zoom-in")}</div></div>{tray_html}</div>')
    return (f'<div class="window" style="width:1280px;height:800px">{editor_header(title, subtitle)}'
            f'<div class="split-view">{tool_strip(active)}{canvas}{props_html}</div>{overlays}</div>')


def props_doc(numbered=True, theme="coral", steps=4):
    sw = "".join(f'<span class="swatch{" selected" if c == theme else ""}" style="--c:var(--annota-{c})"></span>' for c in ["coral", "amber", "teal", "azure", "violet", "ink"])
    return (f'<div class="sidebar-pane end props"><div class="scroll">'
            f'<div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Step Guide</span><span class="description">Template layout</span></div></div>'
            f'<div class="boxed-list"><div class="row"><div class="text"><span class="title">Steps</span></div><div class="suffix"><span class="spin" style="width:110px"><span class="value">{steps}</span>{btn("minus")}{btn("plus")}</span></div></div>'
            f'<div class="row"><div class="text"><span class="title">Numbering</span></div><div class="suffix"><span class="switch{" on" if numbered else ""}"></span></div></div>'
            f'<div class="row"><div class="text"><span class="title">Image Scaling</span></div><div class="suffix"><div class="toggle-group"><span class="toggle active">Fit</span><span class="toggle">Fill</span></div></div></div></div></div>'
            f'<div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Theme Colour</span></div></div><div class="swatch-row">{sw}</div></div>'
            f'</div></div>')


def props_slot(fill=False, has_image=True):
    rows = (f'<div class="row"><div class="text"><span class="title">Scale</span></div><div class="suffix"><div class="toggle-group"><span class="toggle{"" if fill else " active"}">Fit</span><span class="toggle{" active" if fill else ""}">Fill</span></div></div></div>'
            f'<div class="row"><div class="text"><span class="title">Number</span></div><div class="suffix"><span class="value">2</span></div></div>'
            f'<div class="row entry-row"><div class="text"><span class="title">Caption</span><span>Choose a destination</span></div></div>')
    btns = (f'<div class="hbox" style="--gap:6px"><button class="btn text-button grow">Replace…</button><button class="btn text-button grow">Edit Image</button></div>'
            f'<button class="btn text-button" style="color:var(--destructive)">Clear Slot</button>') if has_image else ""
    return (f'<div class="sidebar-pane end props"><div class="scroll">'
            f'<div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Image Slot 2</span><span class="description">{"Harbor backup destination · 2200 × 1520" if has_image else "Empty"}</span></div></div>'
            f'<div class="boxed-list">{rows}</div></div>{btns}</div></div>')


# ---------------------------------------------------------------- 01 New → From template (editor menu)
it = B["t1"]
base_canvas = f'<div class="canvas-image" style="width:700px;height:261px;margin-top:150px">{timg(it, 700, 261)}</div>'
emenu = (menu([("New Capture", "Ctrl+N"), ("New from Template…", "Shift+Ctrl+N", "hover"), ("Open…", "Ctrl+O"), ("Library", "Ctrl+L"), "sep",
               ("Save As…", "Shift+Ctrl+S"), ("Save as Template…", ""), "sep", ("Preferences", "Ctrl+,"), ("Keyboard Shortcuts", "Ctrl+?")], 960, 48, 290, arrow_x=262))
W("01-new-from-template-menu", "Editor — new from template",
  editor_win(base_canvas, props_arrow(), "Checkout postcode error", "1600 × 1000 · In Library", overlays=emenu, active="arrow"))

# ---------------------------------------------------------------- 02 template chooser
W("02-template-chooser", "New from template", editor_win(base_canvas, props_arrow(), "Checkout postcode error", "1600 × 1000 · In Library", overlays=chooser(hover="ba"), active="arrow"))

# ---------------------------------------------------------------- 03 empty step guide with placeholders
empty_steps = [(None, "Step 1", "Add a caption"), (None, "Step 2", "Add a caption"), (None, "Step 3", "Add a caption"), (None, "Step 4", "Add a caption")]
page_empty = f'<div style="transform:scale(.93);transform-origin:top center">{step_guide_page("Untitled step guide", "Click to add a subtitle", empty_steps, slot_h=150)}</div>'
W("03-empty-step-guide", "Editor — empty step guide", editor_win(page_empty, props_doc()))

# ---------------------------------------------------------------- 04 tray multi-select → Combine
def tray_sel(selected=True):
    keys = ["t2", "t1", "y3", "t7", "t4", "y1"]
    sel = {"t2": 1, "y3": 2, "t7": 3} if selected else {}
    parts = ['<div class="tray"><span class="tray-label">Recent</span>']
    for k in keys:
        c = "sel" if k in sel else ""
        ck = f'<span class="ck" style="width:20px;height:20px;font-size:11px;font-weight:800">{sel[k]}</span>' if k in sel else ""
        parts.append(f'<div class="ti {c}">{timg(B[k], 100, 68)}{ck}</div>')
    parts.append(f'<div class="more">{ic("library")}Open Library<span class="caption">Ctrl+L</span></div></div>')
    return "".join(parts)


combine_bar = (f'<div class="osd-toolbar" style="position:absolute;left:50%;transform:translateX(-50%);bottom:116px;z-index:5">'
               f'<span class="label">3 selected</span><span class="sep"></span>'
               f'<button class="btn text-button suggested-action" style="padding:5px 14px">{ic("template")}Combine…</button>'
               f'<button class="btn text-button">Export…</button><button class="btn circular">{ic("close")}</button></div>')
W("04-tray-select-combine", "Editor — combine from tray",
  editor_win(base_canvas, props_arrow(), "Checkout postcode error", "1600 × 1000 · In Library", tray_html=tray_sel(), overlays=combine_bar, active="arrow"))


# ---------------------------------------------------------------- 05 combine dialog
def combine(dragging=False):
    items = [(B["t2"], "Open Backup preferences", ""), (B["y3"], "Choose a destination", ""), (B["t7"], "Check the summary", "")]
    caps = ["Open Backup preferences", "Choose a destination", "Check the summary"]
    rows = []
    order = [0, 1, 2]
    for pos, idx in enumerate(order, 1):
        it_ = items[idx][0]
        cls = ""
        if dragging and idx == 2:
            cls = "dragging"
        rows.append(f'<div class="row {cls}"><span style="opacity:.5">{ic("drag-handle")}</span><span class="n">{pos}</span>'
                    f'<span class="tbox">{timg(it_, 58, 38)}</span>'
                    f'<span class="entry{" focus" if pos == 2 and not dragging else ""}">{esc(caps[idx])}{"<span class=caret style=margin-left:1px></span>" if pos == 2 and not dragging else ""}</span>'
                    f'{btn("close", cls="flat small")}</div>')
    if dragging:
        rows = [rows[0], rows[2].replace('<span class="n">3</span>', '<span class="n">2</span>'),
                '<div class="row gap"></div>', rows[1].replace('<span class="n">2</span>', '<span class="n">3</span>')]
    left = (f'<div style="width:420px;flex:none;padding:12px 18px 18px;display:flex;flex-direction:column;gap:16px;overflow:hidden">'
            f'<div class="boxed-list"><div class="row entry-row"><div class="text"><span class="title">Title</span><span>Back up to your NAS</span></div></div>'
            f'<div class="row entry-row"><div class="text"><span class="title">Subtitle</span><span class="dim-label">Optional</span></div></div></div>'
            f'<div class="preferences-group" style="gap:8px"><div class="group-header"><div class="text"><span class="heading">Order and Captions</span><span class="description">Drag to reorder</span></div><button class="btn flat image-button">{ic("plus")}</button></div>'
            f'<div class="boxed-list combine-list" style="overflow:visible">{"".join(rows)}</div></div>'
            f'<div class="boxed-list"><div class="row"><div class="text"><span class="title">Numbering</span></div><div class="suffix"><span class="switch on"></span></div></div>'
            f'<div class="row"><div class="text"><span class="title">Theme</span></div><div class="suffix"><div class="swatch-row" style="gap:4px">'
            + "".join(f'<span class="swatch small{" selected" if c == "coral" else ""}" style="--c:var(--annota-{c})"></span>' for c in ["coral", "teal", "azure", "violet", "ink"])
            + '</div></div></div></div></div>')
    tsel = "".join(f'<span class="toggle{" active" if k == "step" else ""}">{t}</span>' for k, t in [("step", "Step Guide"), ("cmp", "Comparison"), ("ba", "Before / After"), ("g", "Grid"), ("tl", "Timeline")])
    pv_items = [(B["t2"], caps[0], ""), (B["y3"], caps[1], ""), (B["t7"], caps[2], "")]
    if dragging:
        pv_items = [pv_items[0], pv_items[2], pv_items[1]]
    pg = step_guide_page("Back up to your NAS", "Harbor Backup · 3 steps", pv_items, width=560, slot_h=128)
    right = (f'<div style="flex:1;display:flex;flex-direction:column;background:var(--canvas-bg);min-width:0">'
             f'<div style="display:flex;justify-content:center;padding:12px"><div class="toggle-group">{tsel}</div></div>'
             f'<div style="flex:1;overflow:hidden;display:flex;justify-content:center"><div style="transform:scale(.72);transform-origin:top center">{pg}</div></div></div>')
    return (f'<div class="dialog-host"><div class="dialog" style="width:1060px;max-width:1060px;height:700px">'
            + headerbar('<button class="btn text-button">Cancel</button>', "Combine 3 Captures", end='<button class="btn text-button suggested-action">Combine</button>', cls="raised")
            + f'<div style="display:flex;flex:1;min-height:0">{left}{right}</div></div></div>')


W("05-combine-dialog", "Combine captures",
  editor_win(base_canvas, props_arrow(), "Checkout postcode error", "1600 × 1000 · In Library", tray_html=tray_sel(), overlays=combine(), active="arrow"))
W("06-combine-reorder", "Combine captures — reorder",
  editor_win(base_canvas, props_arrow(), "Checkout postcode error", "1600 × 1000 · In Library", tray_html=tray_sel(), overlays=combine(True), active="arrow"))

# ---------------------------------------------------------------- 07 filled step guide in editor
filled = [(B["t2"], "Open Backup preferences", "Harbor Backup → Preferences → Backup"),
          (B["y3"], "Choose a destination", "Pick the share mounted from your NAS"),
          (B["t7"], "Check the summary", "Confirm before the first run")]
page_f = f'<div style="transform:scale(.86);transform-origin:top center">{step_guide_page("Back up to your NAS", "Harbor Backup · 3 steps", filled, slot_h=170)}</div>'
W("07-step-guide-filled", "Editor — step guide",
  editor_win(page_f, props_doc(steps=3), "Back up to your NAS", "Step guide · 3 slots · Edited", tray_html=tray_sel(False)))

# ---------------------------------------------------------------- 08 drop onto empty slot
drop_steps = [(B["t2"], "Open Backup preferences", "Harbor Backup → Preferences → Backup"), (None, "Choose a destination", "Pick the share mounted from your NAS"),
              (None, "Check the summary", "Confirm before the first run")]
page_d = f'<div style="transform:scale(.86);transform-origin:top center">{step_guide_page("Back up to your NAS", "Harbor Backup · 3 steps", drop_steps, slot_h=170, drop=2)}</div>'
ghost = (f'<div class="ghost" style="left:560px;top:430px"><div class="stack" style="transform:rotate(-2deg);opacity:.92">{timg(B["y3"], 132, 46)}</div></div>'
         f'<svg class="cursor-arrow" style="left:640px;top:466px" viewBox="0 0 20 20"><path d="M3 2 V16 L6.6 12.8 L9 18 L11.4 17 L9 11.8 H14 Z" fill="#fff" stroke="#000" stroke-width="1.2" stroke-linejoin="round"/></svg>')
W("08-slot-drop-target", "Editor — drop into slot",
  editor_win(page_d, props_slot(has_image=False), "Back up to your NAS", "Step guide · 3 slots · Edited", overlays=ghost,
             tray_html=tray_sel(False).replace('<div class="ti ">' + timg(B["y3"], 100, 68), '<div class="ti" style="opacity:.4">' + timg(B["y3"], 100, 68))))

# ---------------------------------------------------------------- 09 slot after drop: fitted + selected
after = [(B["t2"], "Open Backup preferences", "Harbor Backup → Preferences → Backup"), (B["y3"], "Choose a destination", "Pick the share mounted from your NAS"),
         (None, "Check the summary", "Confirm before the first run")]
page_a = step_guide_page("Back up to your NAS", "Harbor Backup · 3 steps", after, slot_h=170, selected=2)
page_a = f'<div style="transform:scale(.86);transform-origin:top center">{page_a}</div>'
W("09-slot-filled-fit", "Editor — slot filled",
  editor_win(page_a, props_slot(), "Back up to your NAS", "Step guide · 3 slots · Edited",
             overlays=toast("Placed in slot 2 · fitted", "Undo")))

# ---------------------------------------------------------------- 10 save as template dialog
rn_page = (f'<div class="page" style="width:640px;padding:24px;--t:var(--annota-azure);display:flex;flex-direction:column;gap:14px">'
           f'<div class="p-title" style="font-size:22px">Release 0.4 highlights</div><div class="rule"></div>'
           f'<div style="display:grid;grid-template-columns:2fr 1fr;gap:10px;height:300px">'
           f'<div style="grid-row:span 2">{slot_html(B["y1"], 410, 300)}</div>{slot_html(B["y8"], 196, 145)}{slot_html(B["t4"], 196, 145, fill=True)}</div>'
           f'<div class="cap" style="font-size:13px">New dashboard export, faster routes table and the KPI strip.</div></div>')
save_dlg = (f'<div class="dialog-host"><div class="dialog" style="width:500px;max-width:500px">'
            + headerbar('<button class="btn text-button">Cancel</button>', "Save as Template", end='<button class="btn text-button suggested-action">Save</button>')
            + f'<div class="dialog-body vbox" style="--gap:18px">'
            f'<div class="boxed-list"><div class="row entry-row focus"><div class="text"><span class="title">Name</span><span>Release note card<span class="caret" style="margin-left:1px;vertical-align:-3px"></span></span></div></div></div>'
            f'<div class="preferences-group" style="gap:8px"><div class="group-header"><div class="text"><span class="heading">Slots</span><span class="description">These images become empty placeholders. Everything else is kept.</span></div></div>'
            f'<div class="boxed-list">'
            + "".join(f'<div class="row"><div class="prefix"><span class="check checked"></span></div><span style="width:56px;height:38px;display:grid;place-items:center;background:var(--fill-1);border-radius:6px">{timg(B[k], 50, 32)}</span><div class="text"><span class="title">{t}</span><span class="subtitle">{s}</span></div></div>'
                      for k, t, s in [("y1", "Large image", "Slot 1 · fit"), ("y8", "Top right", "Slot 2 · fit"), ("t4", "Bottom right", "Slot 3 · fill")])
            + f'</div></div><div class="boxed-list"><div class="row"><div class="text"><span class="title">Keep Text as Sample</span><span class="subtitle">Title and caption stay as editable sample text</span></div><div class="suffix"><span class="switch on"></span></div></div></div>'
            f'</div></div></div>')
W("10-save-as-template", "Editor — save as template",
  editor_win(f'<div style="transform:scale(.9);transform-origin:top center">{rn_page}</div>', props_doc(theme="azure").replace('<span class="heading">Step Guide</span><span class="description">Template layout</span>', '<span class="heading">Document</span><span class="description">Custom layout</span>').replace('<div class="row"><div class="text"><span class="title">Steps</span></div><div class="suffix"><span class="spin" style="width:110px"><span class="value">4</span>' + btn("minus") + btn("plus") + '</span></div></div>', '').replace('<div class="row"><div class="text"><span class="title">Numbering</span></div><div class="suffix"><span class="switch on"></span></div></div>', ''), "Release 0.4 highlights", "Custom layout · 3 images", overlays=save_dlg))

# ---------------------------------------------------------------- 11 chooser: My templates
W("11-chooser-my-templates", "New from template — mine",
  editor_win(base_canvas, props_arrow(), "Checkout postcode error", "1600 × 1000 · In Library", overlays=chooser(cat="mine", mine=True), active="arrow"))

# ---------------------------------------------------------------- 12 before/after + comparison filled examples
def ba_page():
    bef = Item("bA", "", "", "", "b", (80, 275, 912, 340), "", "", ann=[A_rect(18, 236, 524, 82)])
    aft = Item("bB", "", "", "", "b", (80, 275, 912, 340), "", "")
    return (f'<div class="page" style="width:860px;padding:24px;--t:var(--annota-coral);display:flex;flex-direction:column;gap:14px">'
            f'<div><div class="p-title" style="font-size:22px">Postcode validation</div><div class="p-sub" style="font-size:13px;margin-top:3px">Checkout · release 0.4</div></div><div class="rule"></div>'
            f'<div style="display:flex;gap:14px;align-items:center">'
            f'<div style="flex:1;display:flex;flex-direction:column;gap:8px"><span style="font-size:12px;font-weight:800;letter-spacing:.06em;color:#6b6b73">BEFORE</span>{slot_html(bef, 370, 160)}<span class="cap" style="font-size:13px">Error only after submit</span></div>'
            f'<svg width="28" height="28" viewBox="0 0 28 28"><circle cx="14" cy="14" r="14" fill="var(--annota-coral)"/><path d="M8 14h11M15 9.5 19.5 14 15 18.5" stroke="#fff" stroke-width="2.4" fill="none" stroke-linecap="round" stroke-linejoin="round"/></svg>'
            f'<div style="flex:1;display:flex;flex-direction:column;gap:8px"><span style="font-size:12px;font-weight:800;letter-spacing:.06em;color:var(--annota-coral)">AFTER</span><div style="box-shadow:0 0 0 2px var(--annota-coral);border-radius:6px">{slot_html(aft, 370, 160)}</div><span class="cap" style="font-size:13px">Checked while typing</span></div></div></div>')


W("12-before-after-filled", "Editor — before/after", editor_win(f'<div style="margin-top:90px;transform:scale(.86);transform-origin:top center">{ba_page()}</div>', props_doc(), "Postcode validation", "Before / after · 2 slots · Edited").replace('<span class="heading">Step Guide</span><span class="description">Template layout</span>', '<span class="heading">Before / After</span><span class="description">Template layout</span>').replace('<div class="row"><div class="text"><span class="title">Steps</span></div><div class="suffix"><span class="spin" style="width:110px"><span class="value">4</span>' + btn("minus") + btn("plus") + '</span></div></div>', ''))


def tl_page():
    pts = [(B["t7"], "09:20", "Order summary"), (B["t4"], "11:05", "KPI row"), (B["t2"], "13:58", "Backup settings"), (B["t1"], "14:22", "Postcode error")]
    cells = ""
    for i, (it_, tm, cap) in enumerate(pts):
        cells += (f'<div style="flex:1;display:flex;flex-direction:column;align-items:center;gap:8px">'
                  f'{slot_html(it_, 170, 110)}<span class="num" style="width:16px;height:16px;font-size:0;box-shadow:0 0 0 4px #fff"></span>'
                  f'<span style="font-size:12px;font-weight:800;color:var(--annota-teal)">{tm}</span><span class="cap" style="font-size:13px">{cap}</span></div>')
    return (f'<div class="page" style="width:860px;padding:24px;--t:var(--annota-teal);display:flex;flex-direction:column;gap:16px">'
            f'<div class="p-title" style="font-size:22px">Friday, 30 October</div><div class="rule"></div>'
            f'<div style="position:relative;display:flex;gap:14px"><div style="position:absolute;left:0;right:0;top:125px;height:2px;background:var(--annota-teal);opacity:.45"></div>{cells}</div></div>')


W("13-timeline-filled", "Editor — timeline", editor_win(f'<div style="margin-top:110px;transform:scale(.86);transform-origin:top center">{tl_page()}</div>', props_doc(theme="teal"), "Friday, 30 October", "Timeline · 4 slots · Edited").replace('<span class="heading">Step Guide</span>', '<span class="heading">Timeline</span>'))

# ---------------------------------------------------------------- 14 narrow chooser (700)
nar_cards = "".join(tpl_card(k, t, d, m, k == "step") for k, t, d, m in TEMPLATES)
nar = (f'<div class="window" style="width:700px;height:800px">'
       + headerbar('<button class="btn text-button">Cancel</button>', "New from Template", end='<button class="btn text-button suggested-action">Create</button>')
       + f'<div style="padding:12px 18px;display:flex;justify-content:center"><div class="toggle-group"><span class="toggle active">Built-in</span><span class="toggle">My Templates</span></div></div>'
       f'<div style="flex:1;overflow:hidden;padding:6px 18px"><div class="tpl-grid" style="grid-template-columns:repeat(3,1fr)">{nar_cards}</div></div>'
       f'<div class="bottom-sheet" style="position:relative;padding:22px 18px 18px"><div class="boxed-list">'
       f'<div class="row"><div class="text"><span class="title">Steps</span></div><div class="suffix"><span class="spin" style="width:110px"><span class="value">4</span>{btn("minus")}{btn("plus")}</span></div></div>'
       f'<div class="row"><div class="text"><span class="title">Numbering</span></div><div class="suffix"><span class="switch on"></span></div></div></div></div></div>')
W("14-narrow-chooser", "New from template — narrow", nar, size=(700, 800))

print("ok")
