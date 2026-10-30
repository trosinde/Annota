import sys; sys.path.insert(0, __file__.rsplit("/", 1)[0])
from common import *

C = "add-step-capture"

CSS = """
  .step-ctl { position: absolute; z-index: 63; gap: 3px; }
  .step-count { display: inline-flex; align-items: center; gap: 8px; padding: 0 10px 0 6px; font-weight: 700; font-variant-numeric: tabular-nums; }
  .step-count .n { min-width: 26px; height: 26px; border-radius: 13px; background: var(--annota-coral); color: #fff; display: grid; place-items: center; font-size: 14px; }
  .flash { position: absolute; z-index: 58; outline: 3px solid var(--annota-coral); outline-offset: 2px; border-radius: 4px; }
  .step-list { display: flex; flex-direction: column; gap: 12px; padding: 18px 24px; }
  .step-card { display: grid; grid-template-columns: 24px 36px 240px 1fr auto; gap: 12px; align-items: center; padding: 12px; }
  .step-card.selected { box-shadow: 0 0 0 2px var(--accent-bg), var(--shadow-card); }
  .step-card.dragging { transform: translate(14px, -6px) rotate(-.6deg); box-shadow: 0 10px 28px rgb(0 0 6 / 28%), 0 0 0 1px var(--borders); z-index: 3; position: relative; }
  .drop-gap { height: 4px; border-radius: 2px; background: var(--accent-bg); margin: -8px 0; }
  .step-num { width: 32px; height: 32px; border-radius: 16px; background: var(--annota-coral); color: #fff; display: grid; place-items: center; font-weight: 800; }
  .thumb { width: 240px; height: 134px; border-radius: var(--radius-s); overflow: hidden; position: relative; box-shadow: 0 0 0 1px var(--borders); }
  .thumb .native { transform-origin: 0 0; }
  .step-text { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .step-text .auto { font-size: var(--fs-caption); opacity: var(--dim-opacity); display: flex; gap: 6px; align-items: center; }
  .step-text .desc { font-size: var(--fs-title-4); font-weight: var(--fw-bold); }
  .step-text .note { font-size: var(--fs-small); opacity: var(--dim-opacity); }
  .step-actions { display: flex; flex-direction: column; gap: 6px; }
  .guide-sidebar .row { min-height: 44px; }
"""


def marker(x, y, n, r=22):
    return (f'<circle cx="{x}" cy="{y}" r="{r + 14}" fill="var(--annota-coral-a20)"/>'
            f'<circle cx="{x}" cy="{y}" r="{r}" fill="var(--annota-coral)" stroke="#fff" stroke-width="4"/>'
            f'<text x="{x}" y="{y + 8}" text-anchor="middle" font-size="{r}" font-weight="800" fill="#fff" font-family="Inter">{n}</text>')


# steps in harbor-backup native coords (1600×1000): click position + crop window origin
STEPS = [
    (1, 320, 231, 'Click "Backup"', "Sidebar row · Preferences window", (250, 120)),
    (2, 1188, 265, 'Click "Choose source folder"', "Button · Source folder row", (493, 120)),
    (3, 1181, 547, 'Turn on "Back up on battery"', "Switch · Schedule group", (493, 307)),
    (4, 1153, 814, 'Click "Start backup"', "Button · Backup page", (493, 400)),
]


def thumb(n, x, y, ox, oy, w=240, h=134, scale=0.28):
    return (f'<div class="thumb" style="width:{w}px;height:{h}px"><div class="native" style="width:1600px;height:1000px;'
            f'transform:scale({scale}) translate(-{ox}px,-{oy}px)"><img src="{S}a-harbor-backup.png" style="width:1600px;height:1000px" alt="">'
            f'{ov(marker(x, y, n, 26))}</div></div>')


def control(x, y, n, paused=False):
    return (f'<div class="osd-toolbar pill step-ctl" style="left:{x}px;top:{y}px">'
            f'<span class="step-count"><span class="n">{n}</span>{"Paused" if paused else ("Step" if n == 1 else "Steps")}</span>'
            f'<span class="sep"></span>'
            f'<button class="btn image-button{" active" if paused else ""}" title="{"Resume" if paused else "Pause"} (Shift+Ctrl+P)">{icon("play" if paused else "pause")}</button>'
            f'<button class="btn text-button destructive-action" style="border-radius:999px;padding:0 14px" title="Stop and Open Guide">{icon("stop")}Stop</button>'
            f'<button class="btn image-button" title="Undo Last Step">{icon("undo")}</button></div>')


def desktop(inner, svg="", rec=None, scale=0.8):
    return (f'<style>{CSS}</style><div class="desktop">{shell_panel(rec)}'
            f'<img class="screen-img" src="{S}a-harbor-backup.png" style="height:800px" alt="">'
            f'<svg style="position:absolute;left:0;top:32px;z-index:59" width="1280" height="800" viewBox="0 0 1600 1000">{svg}</svg>{inner}</div>')


# 01 start: hint before first click
hint = (f'<div class="osd-toolbar pill" style="position:absolute;left:50%;transform:translateX(-50%);bottom:84px;z-index:61;gap:8px">'
        f'{icon("info")}<span class="label" style="font-weight:400">Click through the task. Each click becomes a step.</span></div>')
write(C, "01-step-capture-started", desktop(control(470, 732, 0).replace('<span class="n">0</span>Steps', '<span class="n">0</span>Steps') + hint, rec="Steps"), "step capture started")

# 02 three clicks done (scenario Three clicks) — flash on 3rd target
svg = "".join(f'<g opacity=".35">{marker(x, y, n, 18)}</g>' for n, x, y, *_ in STEPS[:2]) + marker(1181, 547, 3, 18)
toastish = (f'<div class="osd-toolbar" style="position:absolute;left:870px;top:478px;z-index:61;gap:8px">'
            f'<span class="label" style="font-weight:400">Step 3 · Turn on “Back up on battery”</span></div>')
write(C, "02-step-capture-three-clicks", desktop(control(470, 732, 3) + toastish, svg, rec="3"), "three clicks")

# 03 paused
paused_hint = (f'<div class="osd-toolbar pill" style="position:absolute;left:50%;transform:translateX(-50%);bottom:84px;z-index:61">'
               f'<span class="label" style="font-weight:400">Paused · clicks are not recorded</span></div>')
write(C, "03-step-capture-paused", desktop(control(470, 732, 3, paused=True) + paused_hint, rec="3"), "paused")


# ---------- step list editor
def step_card(n, x, y, desc, sub, crop, selected=False, editing=False, dragging=False, manual=False):
    auto = (f'<span class="auto">{icon("magic-wand")}From accessibility · {sub}</span>' if not manual
            else f'<span class="auto">{icon("text")}Edited</span>')
    if editing:
        d = (f'<span class="entry focus" style="font-size:var(--fs-title-4);font-weight:700;min-height:40px">{desc}'
             f'<span class="caret" style="margin-left:1px"></span></span>')
        note = '<span class="entry" style="min-height:58px;align-items:flex-start;padding-top:8px"><span class="placeholder">Add a note for this step (optional)</span></span>'
    else:
        d = f'<span class="desc">{desc}</span>'
        note = '<span class="note">Add a note…</span>' if 'source folder' not in desc else '<span class="note" style="opacity:.8">The folder picker opens. Pick “Documents”.</span>'
    cls = "card step-card" + (" selected" if selected else "") + (" dragging" if dragging else "")
    return (f'<div class="{cls}"><span style="opacity:.55;display:flex;justify-content:center">{icon("drag-handle")}</span>'
            f'<span class="step-num">{n}</span>{thumb(n, x, y, *crop)}'
            f'<div class="step-text">{auto}{d}{note}</div>'
            f'<div class="step-actions"><button class="btn flat image-button" title="Edit Image in Editor">{icon("pen")}</button>'
            f'<button class="btn flat image-button" title="Delete Step (Delete)">{icon("trash")}</button></div></div>')


def guide_props(selected=3, desc='4 steps · captured 10:42', title='Set up a daily backup'):
    return (f'<div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Guide</span>'
            f'<span class="description">{desc}</span></div></div>'
            '<div class="boxed-list guide-sidebar">'
            f'<div class="row entry-row"><div class="text"><span class="title">Title</span><span>{title}</span></div></div>'
            '<div class="row"><div class="text"><span class="title">Template</span></div><div class="suffix"><span class="dropdown flat">Step Guide</span></div></div>'
            '<div class="row"><div class="text"><span class="title">Image crop</span></div><div class="suffix"><span class="dropdown flat">Around click</span></div></div>'
            '<div class="row"><div class="text"><span class="title">Click markers</span></div><div class="suffix"><span class="switch on"></span></div></div>'
            '</div></div>'
            f'<div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Marker</span></div></div>'
            '<div class="swatch-row"><span class="swatch selected" style="--c:var(--annota-coral)"></span><span class="swatch" style="--c:var(--annota-amber)"></span>'
            '<span class="swatch" style="--c:var(--annota-teal)"></span><span class="swatch" style="--c:var(--annota-azure)"></span><span class="swatch" style="--c:var(--annota-ink)"></span></div></div>'
            f'<button class="btn text-button" style="width:100%">{icon("plus")}Add Steps…</button>'
            f'<div class="local-note">{icon("info")}<span>Step text comes from the accessibility names on this computer.</span></div>')


def guide_window(cards_html, overlay="", title="Set up a daily backup", sub="4 steps · Edited", props=None):
    hb = headerbar(title, sub, start=(f'<button class="btn image-button" title="New Step Capture">{icon("record")}</button>'
                                      f'<div class="linked"><button class="btn image-button">{icon("undo")}</button><button class="btn image-button">{icon("redo")}</button></div>'),
                   end=(f'<button class="btn text-button suggested-action" title="Export (Ctrl+E)">Export</button>'
                        f'<button class="btn flat image-button">{icon("open-menu")}</button>'))
    return (f'<style>{CSS}</style><div class="window" style="width:1280px;height:800px">{hb}'
            f'<div class="split-view"><div class="content-pane view" style="overflow:hidden"><div class="step-list">{cards_html}</div></div>'
            f'<div class="sidebar-pane end props"><div class="scroll">{props or guide_props()}</div></div></div>{overlay}</div>')


cards = lambda kw={}: "".join(step_card(n, x, y, d, s, c, **kw.get(n, {})) for n, x, y, d, s, c in STEPS)
write(C, "04-step-list-editor", guide_window(cards({3: {"selected": True}})), "step list")

write(C, "05-edit-step-description",
      guide_window(step_card(*STEPS[0][:6]) + step_card(*STEPS[1][:6])
                   + step_card(3, 1181, 547, 'Turn on "Back up on battery" if you work on a laptop', "", STEPS[2][5], selected=True, editing=True, manual=True)
                   + step_card(*STEPS[3][:6])).replace(
          '<span class="auto">' + icon("text") + 'Edited</span>',
          '<span class="auto">' + icon("text") + 'Edited · was Turn on “Back up on battery” · <span class="link">Restore</span></span>'),
      "edit description")

# 06 reorder (drag)
order = (step_card(*STEPS[0][:6]) + '<div class="drop-gap"></div>'
         + step_card(*STEPS[2][:6], dragging=True) + step_card(*STEPS[1][:6]) + step_card(*STEPS[3][:6]))
write(C, "06-reorder-steps", guide_window(order, sub="4 steps · Moving step 3 to position 2"), "reorder")

# 07 delete with undo toast
after = (step_card(1, *STEPS[0][1:6]) + step_card(2, *STEPS[2][1:6]) + step_card(3, *STEPS[3][1:6]))
write(C, "07-step-deleted-undo",
      guide_window(after, sub="3 steps · Edited", props=guide_props(desc="3 steps · captured 10:42"),
                   overlay=f'<div class="toast-overlay" style="right:300px"><div class="toast"><span class="toast-title">Step deleted</span>'
                           f'<button class="btn flat text-button">Undo</button><button class="btn flat circular">{icon("close")}</button></div></div>'),
      "step deleted")

# 08 empty (stopped without clicks)
empty = (f'<div class="status-page" style="height:700px">{icon("step", "icon status-icon")}<div class="status-title">No Steps Yet</div>'
         '<div class="status-description">Start a step capture and click through the task. Every click adds a step with a picture and a description.</div>'
         f'<button class="btn pill suggested-action">{icon("record")}Start Step Capture</button></div>')
write(C, "08-guide-empty", guide_window(empty, title="New guide", sub="No steps", props=guide_props(desc="No steps yet", title="New guide")).replace('Export</button>', 'Export</button>', 1)
      .replace('class="btn text-button suggested-action" title="Export (Ctrl+E)"', 'class="btn text-button" disabled title="Export (Ctrl+E)"'),
      "empty")


# 09 export dialog: choices
def export(sel="md", progress=None):
    opt = lambda k, ic, t, s: (f'<div class="row{" selected" if k == sel else ""}"><div class="prefix"><span class="radio{" checked" if k == sel else ""}"></span></div>'
                               f'<div class="prefix">{icon(ic)}</div><div class="text"><span class="title">{t}</span><span class="subtitle">{s}</span></div></div>')
    rows = (opt("tpl", "template", "Step Guide document", "Editable .annota guide from the Step Guide template")
            + opt("pdf", "image", "PDF", "One page per 2 steps, A4 · ready to print or send")
            + opt("md", "text", "Markdown", "set-up-a-daily-backup.md and an images folder"))
    extra = ""
    if sel == "md":
        extra = ('<div class="card" style="padding:12px 14px;font-family:var(--font-mono);font-size:12.5px;line-height:1.7;box-shadow:inset 0 0 0 1px var(--borders)">'
                 f'<div class="hbox" style="font-family:var(--font-ui);font-size:var(--fs-small);margin-bottom:4px">{icon("folder")}<b>~/Documents/Guides/</b></div>'
                 '├── set-up-a-daily-backup.md<br>└── set-up-a-daily-backup-images/<br>&nbsp;&nbsp;&nbsp;&nbsp;├── step-01.png<br>&nbsp;&nbsp;&nbsp;&nbsp;├── step-02.png<br>&nbsp;&nbsp;&nbsp;&nbsp;└── … (4 images)</div>')
    if progress:
        extra = (f'<div class="vbox" style="--gap:6px"><div class="hbox"><span class="spinner"></span><span class="grow">Writing step 3 of 4…</span></div>'
                 f'<div class="progress"><div class="bar" style="width:{progress}%"></div></div></div>')
    return (f'<div class="dialog-host"><div class="dialog" style="width:580px;max-width:580px">'
            f'<div class="headerbar"><div class="start"><button class="btn text-button">Cancel</button></div><div class="title-widget"><span class="title">Export Guide</span></div>'
            f'<div class="end"><button class="btn text-button suggested-action"{" disabled" if progress else ""}>Export</button></div></div>'
            f'<div class="dialog-body vbox" style="--gap:18px"><div class="boxed-list">{rows}</div>'
            '<div class="boxed-list"><div class="row"><div class="text"><span class="title">Folder</span><span class="subtitle">~/Documents/Guides</span></div>'
            f'<div class="suffix"><button class="btn flat image-button">{icon("folder")}</button></div></div>'
            '<div class="row"><div class="text"><span class="title">Image width</span></div><div class="suffix"><span class="dropdown flat">1200 px</span></div></div></div>'
            f'{extra}</div></div></div>')


write(C, "09-export-markdown", guide_window(cards(), overlay=export("md")), "export markdown")
write(C, "10-export-pdf-progress", guide_window(cards(), overlay=export("pdf", progress=70)), "export pdf")
write(C, "11-export-done",
      guide_window(cards(), overlay=f'<div class="toast-overlay" style="right:300px"><div class="toast"><span class="toast-title">Exported “set-up-a-daily-backup.md” and 4 images</span>'
                                    f'<button class="btn text-button">Open Folder</button><button class="btn flat circular">{icon("close")}</button></div></div>'),
      "export done")

# 12 template document preview (step guide template)
page = f"""<style>{CSS}
  .page {{ width: 595px; background: #fff; color: #1f1f24; box-shadow: 0 2px 10px rgb(0 0 6 / 18%); padding: 40px 48px; font-family: var(--font-ui); }}
  .page h1 {{ font-size: 26px; margin: 0 0 4px; }}
  .page .meta {{ font-size: 12px; color: #77767b; margin-bottom: 24px; }}
  .page .st {{ display: grid; grid-template-columns: 34px 1fr; gap: 12px; margin-bottom: 20px; }}
  .page .st b {{ font-size: 15px; display: block; margin: 4px 0 8px; }}
</style>"""
pg = ('<div class="page"><h1>Set up a daily backup</h1><div class="meta">4 steps · Harbor Backup · 30 Oct 2026</div>'
      + "".join(f'<div class="st"><span class="step-num">{n}</span><div><b>{d}</b>{thumb(n, x, y, *c, w=450, h=252, scale=0.525)}</div></div>'
                for n, x, y, d, s, c in STEPS[:2]) + '</div>')
doc = (f'<div class="window" style="width:1280px;height:800px">'
       + headerbar("Set up a daily backup", "Step Guide template · page 1 of 2")
       + f'<div class="split-view">{tool_strip("select")}<div class="content-pane"><div class="canvas-area" style="align-items:flex-start;padding-top:24px">{pg}'
       f'<div class="osd-toolbar" style="position:absolute;right:18px;bottom:18px"><button class="btn image-button">{icon("zoom-out")}</button><span class="label">100 %</span><button class="btn image-button">{icon("zoom-in")}</button></div></div></div>'
       f'<div class="sidebar-pane end props"><div class="scroll"><div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Step Guide</span>'
       f'<span class="description">Template document · every block stays editable</span></div></div>'
       '<div class="boxed-list"><div class="row"><div class="text"><span class="title">Page size</span></div><div class="suffix"><span class="dropdown flat">A4</span></div></div>'
       '<div class="row"><div class="text"><span class="title">Steps per page</span></div><div class="suffix"><span class="dropdown flat">2</span></div></div>'
       '<div class="row"><div class="text"><span class="title">Show date</span></div><div class="suffix"><span class="switch on"></span></div></div></div></div></div></div></div></div>')
write(C, "12-template-document", page + doc, "template document")
print("ok")
