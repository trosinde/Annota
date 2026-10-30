import sys
sys.path.insert(0, "/tmp/claude-0/-home-claude/a6fbb290-1e39-5b02-b596-1e25d7216ff2/scratchpad/asx")
from asx_common import *

C = "add-export-share"

CSS = """
  .exp { width: 560px; max-width: 560px; }
  .exp .dialog-body { display: flex; flex-direction: column; gap: 18px; padding: 6px 24px 24px; }
  .exp-preview { display: flex; gap: 18px; align-items: center; }
  .exp-preview .thumb { width: 160px; height: 100px; border-radius: var(--radius-s); overflow: hidden; box-shadow: 0 0 0 1px var(--borders); flex: none; position: relative; }
  .exp-preview .thumb img { width: 100%; height: 100%; display: block; }
  .exp-preview .thumb svg { position: absolute; inset: 0; width: 100%; height: 100%; }
  .exp-preview .meta { display: flex; flex-direction: column; gap: 4px; }
  .row .scale { width: 220px; min-height: 0; }
  .row .toggle-group .toggle { min-width: 52px; }
  .pattern { font-family: var(--font-mono); font-size: 14px; }
  .drag-ghost { position: absolute; z-index: 60; width: 160px; height: 100px; border-radius: var(--radius-s); overflow: hidden; box-shadow: 0 6px 20px rgb(0 0 6 / 35%), 0 0 0 1px rgb(0 0 6 / 15%); opacity: .92; }
  .drag-ghost img { width: 100%; height: 100%; display: block; }
  .drag-ghost .badge { position: absolute; right: 6px; bottom: 6px; }
  .cursor { position: absolute; z-index: 61; width: 20px; height: 24px; }
  .selection-bar { display: flex; align-items: center; gap: 6px; width: 100%; }
  .files-grid { display: grid; grid-template-columns: repeat(4, 96px); gap: 18px 12px; padding: 18px; }
  .file { display: flex; flex-direction: column; align-items: center; gap: 6px; font-size: var(--fs-caption); text-align: center; }
  .file .ft { width: 72px; height: 54px; border-radius: 4px; overflow: hidden; box-shadow: 0 0 0 1px var(--borders); }
  .file .ft img { width: 100%; height: 100%; object-fit: cover; object-position: top; display: block; }
  .file.new .ft { box-shadow: 0 0 0 2px var(--accent-bg); }
  .drop-hl { outline: 2px solid var(--accent-bg); outline-offset: -2px; background: var(--accent-a12) !important; }
  .batch-list .row .thumb { width: 64px; height: 40px; border-radius: 4px; overflow: hidden; box-shadow: 0 0 0 1px var(--borders); }
  .batch-list .row .thumb img { width: 100%; height: 100%; object-fit: cover; object-position: top; display: block; }
  .pixel-blk { fill: #8b86b8; }
"""

# annotated-checkout preview svg for thumbnails (viewBox of canvas 768x480)
PREV_SVG = f'<svg viewBox="0 0 768 480">{CHECKOUT_ARROW}{CHECKOUT_EXTRA}</svg>'


def exp_dialog(fmt="PNG", *, quality=None, webp=None, pdf=False, name="Checkout flow", folder="~/Pictures/Annota/Exports",
               size="100 % · 1600 × 1000 px", est="≈ 412 KB", extra_top="", name_cls="", export_disabled=False, size_open=False):
    ext = {"PNG": "png", "JPG": "jpg", "WebP": "webp", "PDF": "pdf"}[fmt]
    toggles = "".join(f'<span class="toggle {"active" if f == fmt else ""}">{f}</span>' for f in ["PNG", "JPG", "WebP", "PDF"])
    rows = [
        f'<div class="row entry-row {name_cls}"><div class="text"><span class="title">File name</span><span>{name}<span class="dim-label">.{ext}</span></span></div></div>',
        f'<div class="row"><div class="text"><span class="title">Folder</span><span class="subtitle">{folder}</span></div><div class="suffix"><button class="btn flat image-button" title="Choose Folder…">{ic("folder")}</button></div></div>',
    ]
    fmt_rows = [f'<div class="row"><div class="text"><span class="title">Format</span></div><div class="suffix"><div class="toggle-group">{toggles}</div></div></div>']
    if quality is not None:
        fmt_rows.append(f'<div class="row"><div class="text"><span class="title">Quality</span></div><div class="suffix"><div class="scale"><div class="trough"><div class="fill" style="width:{quality}%"></div><div class="knob" style="left:{quality}%"></div></div><span class="value">{quality}</span></div></div></div>')
    if webp:
        fmt_rows.append(f'<div class="row"><div class="text"><span class="title">Compression</span></div><div class="suffix"><div class="toggle-group"><span class="toggle {"active" if webp=="lossless" else ""}">Lossless</span><span class="toggle {"active" if webp=="lossy" else ""}">Lossy</span></div></div></div>')
        if webp == "lossy":
            fmt_rows.append(f'<div class="row"><div class="text"><span class="title">Quality</span></div><div class="suffix"><div class="scale"><div class="trough"><div class="fill" style="width:80%"></div><div class="knob" style="left:80%"></div></div><span class="value">80</span></div></div></div>')
    sz_cls = "dropdown flat" + (" hover" if size_open else "")
    fmt_rows.append(f'<div class="row"><div class="text"><span class="title">Size</span></div><div class="suffix"><span class="{sz_cls}">{size}</span></div></div>')
    if pdf:
        fmt_rows.append('<div class="row"><div class="text"><span class="title">Keep annotations as vectors</span><span class="subtitle">Text and shapes stay sharp at any zoom</span></div><div class="suffix"><span class="switch on"></span></div></div>')
        fmt_rows.append('<div class="row"><div class="text"><span class="title">Page size</span></div><div class="suffix"><span class="dropdown flat">Fit to image</span></div></div>')
    else:
        fmt_rows.append('<div class="row"><div class="text"><span class="title">Flatten annotations</span><span class="subtitle">Image formats always contain the flattened result. Save as .annota to keep them editable.</span></div><div class="suffix"><span class="switch on disabled"></span></div></div>')
    ed = " disabled" if export_disabled else ""
    return f"""<div class="dialog-host"><div class="dialog exp">
  <div class="headerbar"><div class="start"><button class="btn text-button">Cancel</button></div><div class="title-widget"><span class="title">Export Image</span></div><div class="end"><button class="btn text-button suggested-action"{ed}>Export</button></div></div>
  <div class="dialog-body">
    {extra_top}
    <div class="exp-preview"><div class="thumb"><img src="{SP}b-lumen-checkout.png" alt="">{PREV_SVG}</div>
      <div class="meta"><span class="heading">Checkout flow</span><span class="caption dim-label">{size.split("·")[-1].strip()} · {fmt} · {est}</span><span class="caption dim-label">Pixel-identical to the canvas at 100 %</span></div></div>
    <div class="boxed-list">{"".join(rows)}</div>
    <div class="boxed-list">{"".join(fmt_rows)}</div>
  </div>
</div></div>"""


HB = dict(drag=True, share=True)

# 01 — editor header with drag handle + share, Share menu open
share_menu = popover_menu([("Email…", None, "hover"), ("Open With…", None), "-", ("Copy Image", "Ctrl+C"), ("Save As…", "Shift+Ctrl+S")],
                          "right:114px;top:50px;width:240px;--arrow-x:192px")
ed = editor(hb_kwargs=dict(HB, share_state="active"), overlay=share_menu, tray_html=tray())
write(C, "01-share-menu", page("share menu", ed, css=CSS))

# 02 — export dialog PNG (default)
ed = editor(hb_kwargs=dict(HB, export_state="active"), tray_html=tray(), overlay=exp_dialog("PNG"))
write(C, "02-export-dialog-png", page("export png", ed, css=CSS))

# 03 — export dialog JPG with quality + size dropdown open
size_pop = """<div class="popover" style="left:678px;top:574px;width:220px;--arrow-x:112px">
  <div class="menu-item"><span class="label">200 % · 3200 × 2000</span></div>
  <div class="menu-item"><span class="label">100 % · 1600 × 1000</span></div>
  <div class="menu-item selected"><span class="label">50 % · 800 × 500</span></div>
  <div class="menu-item"><span class="label">25 % · 400 × 250</span></div>
  <div class="menu-sep"></div>
  <div class="menu-item"><span class="label">Custom Width…</span></div>
</div>"""
dlg = exp_dialog("JPG", quality=85, size="50 % · 800 × 500 px", est="≈ 96 KB", size_open=True)
dlg = dlg.replace("</div></div>\n  </div>\n</div></div>", "</div></div>\n  </div>\n</div>" + size_pop + "</div>") if False else dlg[:-len("</div></div>")] + "</div>" + size_pop + "</div>"
ed = editor(hb_kwargs=HB, tray_html=tray(), overlay=dlg)
write(C, "03-export-dialog-jpg-size", page("export jpg", ed, css=CSS))

# 04 — WebP lossy + error: file exists / folder not writable
err_banner = f'<div class="banner error" style="border-radius:var(--radius-m);margin-bottom:-6px"><span class="banner-title">No permission to write to this folder</span><button class="btn">Choose Folder…</button></div>'
dlg = exp_dialog("WebP", webp="lossy", folder="/mnt/share/exports", est="≈ 128 KB", extra_top=err_banner, export_disabled=True)
ed = editor(hb_kwargs=HB, tray_html=tray(), overlay=dlg)
write(C, "04-export-dialog-webp-error", page("export webp error", ed, css=CSS))

# 05 — PDF
ed = editor(hb_kwargs=HB, tray_html=tray(), overlay=exp_dialog("PDF", pdf=True, est="≈ 640 KB"))
write(C, "05-export-dialog-pdf", page("export pdf", ed, css=CSS))

# 06 — Ctrl+S quick save toast
ed = editor(title="Annota 2026-10-30 14-22-08", subtitle="~/Pictures/Annota · Saved", hb_kwargs=HB, tray_html=tray(),
            overlay=toast("Saved as “Annota 2026-10-30 14-22-08.annota”", "Show in Files", where="over-canvas with-tray"))
write(C, "06-quick-save-toast", page("quick save", ed, css=CSS))

# 07 — Ctrl+C clipboard toast (nothing selected)
ed = editor(active="select", hb_kwargs=dict(HB, copy_state="active"), tray_html=tray(),
            overlay=toast("Image copied to clipboard", None, where="over-canvas with-tray"))
write(C, "07-clipboard-toast", page("clipboard", ed, css=CSS))

# 08 — drag handle hover tooltip
tip = '<div class="tooltip" style="right:178px;top:48px">Drag image to another app</div>'
ed = editor(hb_kwargs=dict(HB, drag_state="hover"), tray_html=tray(), overlay=tip)
write(C, "08-drag-handle-hover", page("drag handle", ed, css=CSS))

# 09 — dragging into Files window (desktop scene)
DESK_CSS = CSS + """
  .scene { position: relative; width: 1280px; height: 800px; }
  .scene .window { position: absolute; }
  .mini-editor .tool-strip { width: 44px; }
  .mini-editor .tool-strip .btn { width: 34px; min-height: 34px; }
"""
files_win = f"""<div class="window" style="left:700px;top:250px;width:540px;height:470px;z-index:5">
  <div class="headerbar"><div class="start"><div class="linked"><button class="btn image-button">{ic("go-previous")}</button><button class="btn image-button">{ic("go-next")}</button></div></div>
    <div class="title-widget"><span class="title">Tickets</span></div>
    <div class="end"><button class="btn flat image-button">{ic("search")}</button><button class="btn flat image-button">{ic("open-menu")}</button><div class="window-controls"><span class="wc">{ic("close")}</span></div></div></div>
  <div class="view drop-hl" style="flex:1">
    <div class="files-grid">
      <div class="file"><div class="ft"><img src="{SP}d-tidewater-analytics.png" alt=""></div>dashboard-q3.png</div>
      <div class="file"><div class="ft"><img src="{SP}c-code-editor.png" alt=""></div>build-log.png</div>
      <div class="file"><div class="ft" style="display:grid;place-items:center;background:var(--fill-1)">{ic("text","icon large")}</div>notes.txt</div>
    </div>
  </div>
</div>"""
ed_small = editor(hb_kwargs=dict(HB, drag_state="active"), canvas_html=canvas(w=560, h=350, vb="0 0 768 480", zoom="35 %"))
ed_small = ed_small.replace('<div class="window" style="width:1280px;height:800px">', '<div class="window mini-editor" style="left:30px;top:30px;width:1000px;height:640px">', 1)
ghost = (f'<div class="drag-ghost" style="left:850px;top:430px;z-index:60"><img src="{SP}b-lumen-checkout.png" alt="">'
         f'<svg viewBox="0 0 768 480" style="position:absolute;inset:0;width:100%;height:100%">{CHECKOUT_ARROW}{CHECKOUT_EXTRA}</svg><span class="badge">PNG</span></div>'
         '<svg class="cursor" style="left:842px;top:420px" viewBox="0 0 20 24"><path d="M2 2 L2 19 L6.5 15 L9.5 22 L12.5 20.7 L9.6 14 L16 14 Z" fill="#000" stroke="#fff" stroke-width="1.5" stroke-linejoin="round"/></svg>')
scene = f'<div class="scene">{ed_small}{files_win}{ghost}</div>'
write(C, "09-drag-into-files", page("drag to files", scene, css=DESK_CSS, body_cls="mock-frame"))

# 10 — pixelated redaction warning
def pixel_block(x, y, w, h, cell=12):
    out = []
    shades = ["#9d97c9", "#8a84b6", "#b3aed8", "#7f79a8", "#a6a1cf", "#938dbf"]
    i = 0
    for yy in range(y, y + h, cell):
        for xx in range(x, x + w, cell):
            out.append(f'<rect x="{xx}" y="{yy}" width="{cell}" height="{cell}" fill="{shades[(i*7 + yy) % len(shades)]}"/>')
            i += 1
    return "".join(out)

# relay chat at 768x480: scale 0.48 from 1600x1000
redact = pixel_block(238, 76, 144, 12, 6) + pixel_block(238, 179, 60, 12, 6) + pixel_block(634, 94, 102, 12, 6) + pixel_block(634, 117, 64, 12, 6)
c10 = canvas("e-relay-chat", redact, zoom="48 %")
red_props = f"""<div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Redact</span></div></div>
<div class="toggle-group" style="width:100%"><span class="toggle" style="flex:1">Blur</span><span class="toggle active" style="flex:1">Pixelate</span><span class="toggle" style="flex:1">Solid</span></div></div>
<div class="boxed-list"><div class="row"><div class="text"><span class="title">Block size</span></div><div class="suffix">{spin("12")}</div></div></div>"""
warn = alert("Make Redactions Solid?",
             "4 redactions use pixelate. Pixelated text can sometimes be reconstructed. Solid boxes hide it for good.",
             [("Cancel", ""), ("Export Anyway", ""), ("Convert to Solid", "suggested-action")], width=460)
warn = warn.replace('<div class="alert-body">', f'<div class="alert-body"><div style="color:var(--warning)">{ic("warning","icon large")}</div>')
ed = editor(title="Chat with Jonas", subtitle="1600 × 1000 · Edited", active="blur", canvas_html=c10, side=sidebar(red_props),
            hb_kwargs=HB, tray_html=tray(open_idx=4), overlay=warn)
write(C, "10-pixelate-warning", page("pixelate warning", ed, css=CSS))

# 11 — batch: tray selection mode, 3 selected
bar = (f'<div class="selection-bar"><button class="btn small text-button flat">Cancel</button><span class="spacer"></span>'
       f'<span class="heading" style="font-size:var(--fs-small)">3 Selected</span><span class="spacer"></span>'
       f'<button class="btn small text-button">Select All</button><button class="btn small text-button suggested-action">Export 3…</button></div>')
ed = editor(hb_kwargs=HB, tray_html=tray(selected=(0, 3, 4), bar=bar))
write(C, "11-batch-select-tray", page("batch select", ed, css=CSS))

# 12 — batch export dialog
def batch_row(img, name, out):
    return (f'<div class="row"><div class="prefix"><span class="thumb"><img src="{SP}{img}.png" alt=""></span></div>'
            f'<div class="text"><span class="title">{name}</span><span class="subtitle">→ {out}</span></div></div>')

batch = f"""<div class="dialog-host"><div class="dialog exp" style="width:580px;max-width:580px">
  <div class="headerbar"><div class="start"><button class="btn text-button">Cancel</button></div><div class="title-widget"><span class="title">Export 3 Images</span></div><div class="end"><button class="btn text-button suggested-action">Export</button></div></div>
  <div class="dialog-body">
    <div class="boxed-list batch-list">
      {batch_row("b-lumen-checkout", "Checkout flow", "Checkout flow.jpg")}
      {batch_row("d-tidewater-analytics", "September dashboard", "September dashboard.jpg")}
      {batch_row("e-relay-chat", "Chat with Jonas", "Chat with Jonas.jpg")}
    </div>
    <div class="boxed-list">
      <div class="row"><div class="text"><span class="title">Format</span></div><div class="suffix"><div class="toggle-group"><span class="toggle">PNG</span><span class="toggle active">JPG</span><span class="toggle">WebP</span><span class="toggle">PDF</span></div></div></div>
      <div class="row"><div class="text"><span class="title">Quality</span></div><div class="suffix"><div class="scale"><div class="trough"><div class="fill" style="width:85%"></div><div class="knob" style="left:85%"></div></div><span class="value">85</span></div></div></div>
      <div class="row"><div class="text"><span class="title">Size</span></div><div class="suffix"><span class="dropdown flat">100 %</span></div></div>
    </div>
    <div class="boxed-list">
      <div class="row entry-row"><div class="text"><span class="title">File name pattern</span><span class="pattern">%name</span></div><div class="suffix"><button class="btn flat image-button" title="Pattern Help">{ic("info")}</button></div></div>
      <div class="row"><div class="text"><span class="title">Folder</span><span class="subtitle">~/Pictures/Annota/Exports</span></div><div class="suffix"><button class="btn flat image-button">{ic("folder")}</button></div></div>
      <div class="row"><div class="text"><span class="title">If a file exists</span></div><div class="suffix"><span class="dropdown flat">Add a number</span></div></div>
    </div>
  </div>
</div></div>"""
ed = editor(hb_kwargs=HB, tray_html=tray(selected=(0, 3, 4), bar=bar), overlay=batch)
write(C, "12-batch-export-dialog", page("batch export", ed, css=CSS))

# 13 — batch done toast
ed = editor(hb_kwargs=HB, tray_html=tray(),
            overlay=toast("3 images exported to Exports", "Show in Files", where="over-canvas with-tray"))
write(C, "13-batch-export-done", page("batch done", ed, css=CSS))

# 14 — Email via portal: toast while handing over, and Open With…
ed = editor(hb_kwargs=HB, tray_html=tray(),
            overlay=toast("Opening your mail app with “Checkout flow.png” attached…", None, close=False, where="over-canvas with-tray"))
write(C, "14-email-handoff", page("email handoff", ed, css=CSS))
print("ok")
