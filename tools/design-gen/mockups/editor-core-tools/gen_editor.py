import pathlib, sys
sys.path.insert(0, str(pathlib.Path(__file__).parent))
import common as C
from common import *

OUT = pathlib.Path("/home/claude/annota/docs/design/add-annotation-editor/src")
OUT.mkdir(parents=True, exist_ok=True)
files = {}

# ---------------------------------------------------------------- shared content on the checkout capture
def checkout_annos(sel_ids=(), arrow_color=CORAL):
    a = []
    a.append(rect(100, 512, 438, 66, sw=6, rx=10))                                     # frame postcode
    a.append(callout(560, 168, 520, 92, ["Postcode needs 5 digits"], [(470, 508, 700, 260)], fs=32))
    a.append(highlight(1060, 728, 444, 46))
    a.append(arrow(1000, 985, 1130, 858, w=8, color=arrow_color))
    a.append(step(44, 384, "1"))
    a.append(step(44, 544, "2"))
    a.append(step(44, 733, "3"))
    return "".join(a)


def document_sidebar(objects=7, canvas_sel="window", snapping=True, extra_top=""):
    return f"""{extra_top}
        {group("Image", f'''<div class="boxed-list">
            <div class="row property"><div class="text"><span class="title">Size</span><span class="subtitle numeric">1600 × 1000 px</span></div><div class="suffix"><button class="btn">Resize…</button></div></div>
            <div class="row"><div class="text"><span class="title">Rotate &amp; Flip</span></div><div class="suffix"><div class="linked">
              <button class="btn image-button" title="Rotate Left (Ctrl+Shift+R)">{glyph("rotate-left")}</button>
              <button class="btn image-button" title="Rotate Right (Ctrl+R)">{glyph("rotate-right")}</button>
              <button class="btn image-button" title="Flip Horizontally">{glyph("flip-h")}</button>
              <button class="btn image-button" title="Flip Vertically">{glyph("flip-v")}</button></div></div></div>
            <div class="row"><div class="text"><span class="title">Corner radius</span></div><div class="suffix">{spin("0 px", 128)}</div></div>
          </div>''')}
        {group("Canvas", field("Background", swatches(selected=canvas_sel, extra_first=f'<span class="swatch checker{" selected light-ink" if canvas_sel=="transparent" else ""}" title="Transparent"></span>', paper=True)) + '<span class="hint">Drag a canvas edge to add space around the image.</span>')}
        {group("Objects", f'''<div class="boxed-list">
            <div class="row"><div class="text"><span class="title">Snap to guides</span><span class="subtitle">Ctrl+; · hold Alt to suspend</span></div><div class="suffix"><span class="switch{' on' if snapping else ''}"></span></div></div>
            <div class="row"><div class="text"><span class="title">{objects} editable objects</span></div><div class="suffix"><button class="btn">Flatten All</button></div></div>
          </div>''')}"""


def doc_header_sidebar(title="Document"):
    return ""


SUB = "1600 × 1000 · Edited"

# ================================================================ 01 overview
s01 = editor("Checkout flow", SUB, "select",
             canvas("b-lumen-checkout", checkout_annos()),
             document_sidebar(), undo_dis=False)
files["01-editor-overview"] = page("Editor overview", s01)

# ================================================================ 02 move + centre snap
# rectangle frame moved near canvas centre; guide lines
centre_ov = (f'<span class="ov guide v" style="left:400px;top:-30px;height:560px"></span>'
             f'<span class="ov guide h" style="left:-30px;top:250px;width:860px"></span>'
             + chip(406, -26, "Centre", "violet")
             + sel(580, 444, 440, 112, rot=True)
             + chip(520 * 0.5 + 0, 572 * 0.5 + 10, "X 580  Y 444")
             + cursor(400, 250, "move"))
annos02 = (rect(100, 512, 438, 66, sw=6, rx=10, opacity=.25, shadow=False)  # ghost of original position
           + rect(580, 444 + 23, 440, 66, sw=6, rx=10)
           + callout(560, 168, 520, 92, ["Postcode needs 5 digits"], [(470, 508, 700, 260)], fs=32)
           + highlight(1060, 728, 444, 46) + arrow(1000, 985, 1130, 858, w=8)
           + step(44, 384, "1") + step(44, 544, "2") + step(44, 733, "3"))
# selection box should match rect 580,467 440x66 -> adjust
centre_ov = (f'<span class="ov guide v" style="left:400px;top:-30px;height:560px"></span>'
             f'<span class="ov guide h" style="left:-30px;top:250px;width:860px"></span>'
             + chip(406, -26, "Canvas centre", "violet")
             + sel(577, 464, 446, 72, rot=True)
             + chip(290, 274, "X 580 · Y 467")
             + cursor(400, 250, "move"))
side02 = f"""
        <div class="group-title"><span class="heading">Rectangle</span><span class="dim-label caption">1 selected</span></div>
        {field("Outline", swatches(second_row=False))}
        {row_line("Width", spin("6 px"))}
        {arrange_group("580", "467", "440", "66")}
        {select_all_btn("Rectangles")}"""
s02 = editor("Checkout flow", SUB, "select", canvas("b-lumen-checkout", annos02, centre_ov), side02)
files["02-move-snap-centre"] = page("Move with centre snap", s02)

# ================================================================ 03 rotate with Shift + 8 handles + nudge info
annos03 = (checkout_annos().replace(rect(100, 512, 438, 66, sw=6, rx=10), "")
           + f'<g transform="rotate(-15 1300 300)">{callout(1110, 250, 380, 100, ["Free returns"], [], fill="var(--annota-teal)", fs=34)}</g>')
ov03 = (sel(1104, 244, 392, 112, rot=True, angle=-15)
        + chip(588, 60, "−15° · Shift: 15° steps")
        + cursor(658, 98, "rotate"))
annos03 = annos03 + rect(100, 512, 438, 66, sw=6, rx=10)
side03 = f"""
        <div class="group-title"><span class="heading">Callout</span><span class="dim-label caption">1 selected</span></div>
        {field("Fill", swatches(selected="teal", second_row=False))}
        {arrange_group("1110", "250", "380", "100", angle="−15°")}
        <div class="card-note">{icon("keyboard")}<span>Arrow keys move 1 px, <b>Shift</b>+arrow keys 10 px. Hold <b>Shift</b> while rotating or resizing to snap to 15° or keep the aspect ratio.</span></div>
        {select_all_btn("Callouts")}"""
s03 = editor("Checkout flow", SUB, "select", canvas("b-lumen-checkout", annos03, ov03), side03)
files["03-rotate-shift-snap"] = page("Rotate with Shift", s03)

# ================================================================ 04 select all arrows (dashboard, 5 arrows)
arrows5 = [(560, 300, 470, 470), (900, 330, 1040, 450), (1700 - 300, 700, 1240, 600), (500, 960, 600, 860), (1450, 920, 1460, 650)]
arrows5 = [(380, 330, 470, 520), (980, 360, 1040, 455), (1360, 740, 1300, 612), (760, 960, 860, 870), (1560, 900, 1500, 660)]
annos04 = "".join(arrow(*a, w=8, color=CORAL) for a in arrows5) + rect(1150, 290, 400, 255, sw=6, rx=12, color="var(--annota-teal)") + step(1150, 290, "A", color="var(--annota-teal)")
ov04 = ""
for x1, y1, x2, y2 in arrows5:
    x, y = min(x1, x2) - 14, min(y1, y2) - 14
    ov04 += sel(x, y, abs(x2 - x1) + 28, abs(y2 - y1) + 28, handles=0, thin=True)
ov04 += sel(356, 316, 1218, 658, rot=True)
ov04 += chip(184, 142, "5 arrows")
side04 = f"""
        <div class="group-title"><span class="heading">5 Arrows</span><span class="dim-label caption">Shared properties</span></div>
        {field("Colour", swatches(selected="coral"))}
        {row_line("Width", spin("8 px"))}
        {field("Line style", toggles([("x:solid", "Solid"), ("x:dashed", "Dashed"), ("x:dotted", "Dotted")], 0, glyphs=True))}
        {opacity_field(100)}
        {select_all_btn("Arrows", pressed=True)}
        <span class="hint">Changes apply to all 5 selected arrows. Properties that differ show “Mixed”.</span>"""
toast04 = toast("Colour changed on 5 arrows", "Undo")
s04 = editor("September dashboard", SUB, "select",
             canvas("d-tidewater-analytics", annos04, ov04), side04, overlays_win=toast04,
             tray_html=tray(current="d-tidewater-analytics"))
files["04-select-all-arrows"] = page("Select all arrows", s04)

# ================================================================ 05 context menu
annos05 = checkout_annos()
ov05 = sel(20, 360, 50, 400, handles=8) + chip(-6, 512, "2 steps · 1 frame")
ov05 = sel(10, 350, 540, 418) + cursor(220, 150)
menu05 = f"""<div class="popover no-arrow" style="left:{52+116+220+4}px;top:{120+150+4}px;width:300px">
      <div class="menu-item"><span class="label">Copy</span><span class="accel">Ctrl+C</span></div>
      <div class="menu-item"><span class="label">Duplicate</span><span class="accel">Ctrl+D</span></div>
      <div class="menu-item"><span class="label">Delete</span><span class="accel">Del</span></div>
      <div class="menu-sep"></div>
      <div class="menu-item"><span class="label">Bring to Front</span><span class="accel">Ctrl+Shift+]</span></div>
      <div class="menu-item"><span class="label">Bring Forward</span><span class="accel">Ctrl+]</span></div>
      <div class="menu-item"><span class="label">Send Backward</span><span class="accel">Ctrl+[</span></div>
      <div class="menu-item"><span class="label">Send to Back</span><span class="accel">Ctrl+Shift+[</span></div>
      <div class="menu-sep"></div>
      <div class="menu-item hover"><span class="label">Group</span><span class="accel">Ctrl+G</span></div>
      <div class="menu-item disabled"><span class="label">Ungroup</span><span class="accel">Ctrl+Shift+G</span></div>
      <div class="menu-sep"></div>
      <div class="menu-item"><span class="label">Flatten Selection</span><span class="accel">Ctrl+T</span></div>
      <div class="menu-item"><span class="label">Flatten All</span><span class="accel">Ctrl+Shift+T</span></div>
    </div>"""
side05 = f"""
        <div class="group-title"><span class="heading">3 Objects</span><span class="dim-label caption">2 steps, 1 rectangle</span></div>
        {field("Colour", swatches(selected="coral"))}
        {opacity_field(100)}
        {arrange_group("24", "350", "514", "418")}"""
s05 = editor("Checkout flow", SUB, "select", canvas("b-lumen-checkout", annos05, ov05), side05, overlays_win=menu05)
files["05-context-menu"] = page("Object context menu", s05)

# ================================================================ 06 group moves together + undo tooltip
annos06 = (callout(560, 168, 520, 92, ["Postcode needs 5 digits"], [(470, 508, 700, 260)], fs=32)
           + highlight(1060, 728, 444, 46) + arrow(1000, 985, 1130, 858, w=8) + step(44, 384, "1") + step(44, 733, "3")
           + f'<g opacity=".28">{rect(100, 512, 438, 66, sw=6, rx=10, shadow=False)}{step(44, 544, "2", shadow=False)}</g>'
           + f'<g transform="translate(560 330)">{rect(100, 512, 438, 66, sw=6, rx=10)}{step(44, 544, "2")}</g>')
ov06 = (sel(10, 500, 538, 90, rot=True, dashed=True) + chip(4, 300, "Group · 2 objects"))
ov06 = sel(572, 830, 534, 88, rot=True, dashed=True) + chip(520, 465, "Δ X +560 · Y +330") + cursor(500, 438, "move")
side06 = f"""
        <div class="group-title"><span class="heading">Group</span><span class="dim-label caption">2 objects</span></div>
        <div class="boxed-list">
          <div class="row"><span class="prefix">{icon("shape")}</span><div class="text"><span class="title">Rectangle</span></div></div>
          <div class="row"><span class="prefix">{icon("step")}</span><div class="text"><span class="title">Step 2</span></div></div>
        </div>
        <button class="btn full" style="justify-content:center">Ungroup <span class="dim-label" style="font-weight:400;margin-left:6px">Ctrl+Shift+G</span></button>
        {arrange_group("578", "842", "520", "70")}
        <span class="hint">Dragging any member moves the whole group. Double-click a member to edit it on its own.</span>"""
tt06 = tooltip(52, 52, "Undo Move Group", "Ctrl+Z")
s06 = editor("Checkout flow", SUB, "select", canvas("b-lumen-checkout", annos06, ov06), side06,
             overlays_win=tt06)
files["06-group-and-undo"] = page("Group selected, undo tooltip", s06)

# ================================================================ 07 zoom levels popover + 200 %
img07 = f'<img src="{C.SAMPLES}/b-lumen-checkout.png" alt="" style="width:1600px;height:1000px;max-width:none;position:absolute;left:-160px;top:-560px">'
annos07 = ""
zoom_pop = f"""<div class="popover arrow-down" style="right:{300+18+30}px;bottom:{107+18+52}px;width:236px;--arrow-x:118px">
      <div class="menu-item"><span class="label">Fit to Window</span><span class="accel">Ctrl+0</span></div>
      <div class="menu-item"><span class="label">Fit Width</span><span class="accel"></span></div>
      <div class="menu-sep"></div>
      <div class="menu-item"><span class="label">25 %</span></div>
      <div class="menu-item"><span class="label">50 %</span></div>
      <div class="menu-item"><span class="label">100 %</span><span class="accel">Ctrl+1</span></div>
      <div class="menu-item selected"><span class="label">200 %</span>{icon("check")}</div>
      <div class="menu-item"><span class="label">400 %</span></div>
      <div class="menu-item"><span class="label">800 %</span></div>
      <div class="menu-item"><span class="label">1600 %</span></div>
      <div class="menu-sep"></div>
      <div class="menu-item"><span class="label">Zoom In</span><span class="accel">Ctrl++</span></div>
      <div class="menu-item"><span class="label">Zoom Out</span><span class="accel">Ctrl+−</span></div>
    </div>"""
canvas07 = f"""<div class="canvas-area" style="display:block">
        <div style="position:absolute;inset:0;overflow:hidden">
          <img src="{C.SAMPLES}/b-lumen-checkout.png" alt="" style="position:absolute;width:3200px;height:2000px;left:-40px;top:-860px">
          <svg style="position:absolute;width:3200px;height:2000px;left:-40px;top:-860px" viewBox="0 0 1600 1000">{SVG_DEFS}{checkout_annos()}</svg>
        </div>
        <span style="position:absolute;right:3px;top:120px;width:6px;height:160px;border-radius:3px;background:var(--fill-3)"></span>
        <span style="position:absolute;bottom:3px;left:10px;width:260px;height:6px;border-radius:3px;background:var(--fill-3)"></span>
        {zoom_osd("200 %", active=True)}
      </div>"""
side07 = document_sidebar()
s07 = editor("Checkout flow", "1600 × 1000 · 200 %", "select", canvas07, side07, overlays_win=zoom_pop)
files["07-zoom-levels"] = page("Zoom levels", s07)

# ================================================================ 08 crop mode
# crop rect (image px): 60..1010, 160..960
cx, cy, cw, ch = 10, 170, 960, 800
X, Y, W, H = cx * .5, cy * .5, cw * .5, ch * .5
dim = "rgb(0 0 6 / 55%)"
ov08 = (f'<div class="ov" style="left:0;top:0;width:800px;height:{Y}px;background:{dim}"></div>'
        f'<div class="ov" style="left:0;top:{Y+H}px;width:800px;height:{500-Y-H}px;background:{dim}"></div>'
        f'<div class="ov" style="left:0;top:{Y}px;width:{X}px;height:{H}px;background:{dim}"></div>'
        f'<div class="ov" style="left:{X+W}px;top:{Y}px;width:{800-X-W}px;height:{H}px;background:{dim}"></div>'
        f'<div class="ov" style="left:{X}px;top:{Y}px;width:{W}px;height:{H}px;outline:2px solid #ffffff;box-shadow:0 0 0 1px rgb(0 0 6 / 40%)">'
        f'<span style="position:absolute;left:33.33%;top:0;bottom:0;width:1px;background:rgb(255 255 255 / 60%)"></span>'
        f'<span style="position:absolute;left:66.66%;top:0;bottom:0;width:1px;background:rgb(255 255 255 / 60%)"></span>'
        f'<span style="position:absolute;top:33.33%;left:0;right:0;height:1px;background:rgb(255 255 255 / 60%)"></span>'
        f'<span style="position:absolute;top:66.66%;left:0;right:0;height:1px;background:rgb(255 255 255 / 60%)"></span>'
        + "".join(
            f'<span style="position:absolute;left:{l};top:{t};width:{w};height:{h};background:#ffffff;box-shadow:0 0 0 1px rgb(0 0 6 / 35%);border-radius:2px"></span>'
            for l, t, w, h in [("-4px", "-4px", "22px", "6px"), ("-4px", "-4px", "6px", "22px"),
                               ("calc(100% - 18px)", "-4px", "22px", "6px"), ("calc(100% - 2px)", "-4px", "6px", "22px"),
                               ("-4px", "calc(100% - 2px)", "22px", "6px"), ("-4px", "calc(100% - 18px)", "6px", "22px"),
                               ("calc(100% - 18px)", "calc(100% - 2px)", "22px", "6px"), ("calc(100% - 2px)", "calc(100% - 18px)", "6px", "22px"),
                               ("calc(50% - 11px)", "-4px", "22px", "6px"), ("calc(50% - 11px)", "calc(100% - 2px)", "22px", "6px"),
                               ("-4px", "calc(50% - 11px)", "6px", "22px"), ("calc(100% - 2px)", "calc(50% - 11px)", "6px", "22px")])
        + '</div>'
        + chip(X + W / 2 - 52, Y + H + 12, "960 × 800 px")
        + cursor(X + W + 2, Y + H + 2, "move").replace("cursor", "cursor", 1))
side08 = f"""
        <div class="group-title"><span class="heading">Crop</span></div>
        <div class="grid2">
          <div class="cell"><span class="k">X</span><span class="entry">10</span></div>
          <div class="cell"><span class="k">Y</span><span class="entry">170</span></div>
          <div class="cell"><span class="k">W</span><span class="entry focus">960</span></div>
          <div class="cell"><span class="k">H</span><span class="entry">800</span></div>
        </div>
        {field("Aspect ratio", '<span class="dropdown full">Free</span>')}
        <div class="card-note">{icon("info")}<span>Annotations keep their position on the image. Objects outside the crop stay in the document and can be brought back with Undo.</span></div>
        <div class="hbox" style="--gap:12px"><button class="btn grow">Cancel <span class="dim-label" style="font-weight:400;margin-left:4px">Esc</span></button><button class="btn suggested-action grow">Crop <span style="font-weight:400;opacity:.8;margin-left:4px">Enter</span></button></div>"""
s08 = editor("Checkout flow", "Crop · 960 × 800", "crop", canvas("b-lumen-checkout", checkout_annos(), ov08), side08)
files["08-crop-mode"] = page("Crop mode", s08)

# ================================================================ 09 cropped result + rounded corners handle + resize dialog
crop_img = (f'<div style="position:absolute;inset:0;overflow:hidden;border-radius:12px">'
            f'<img src="{C.SAMPLES}/b-lumen-checkout.png" alt="" style="position:absolute;width:{1600*.6}px;height:{1000*.6}px;left:{-10*.6}px;top:{-170*.6}px;max-width:none">'
            f'<svg style="position:absolute;width:{1600*.6}px;height:{1000*.6}px;left:{-10*.6}px;top:{-170*.6}px" viewBox="0 0 1600 1000">{SVG_DEFS}{checkout_annos()}</svg></div>')
canvas09 = f"""<div class="canvas-area">
        <div class="canvas-image" style="width:{960*.6}px;height:{800*.6}px;border-radius:12px">{crop_img}
          <span class="ov hd active" style="left:22px;top:22px" title="Corner radius"></span>
          <span class="ov chip" style="left:-4px;top:-30px">Corner radius 20 px</span>
        </div>
        {zoom_osd("Fit · 60 %")}
      </div>"""
dlg09 = f"""<div class="dialog-host" style="z-index:30">
    <div class="dialog" style="width:420px;min-width:420px">
      <div class="headerbar"><div class="start"><button class="btn">Cancel</button></div><div class="title-widget"><span class="title">Resize Image</span></div><div class="end"><button class="btn suggested-action">Resize</button></div></div>
      <div class="dialog-body vbox" style="--gap:18px;padding-top:12px">
        <div class="boxed-list">
          <div class="row"><div class="text"><span class="title">Width</span></div><div class="suffix">{spin("960", 132, focus=True)}</div></div>
          <div class="row"><div class="text"><span class="title">Height</span></div><div class="suffix">{spin("800", 132)}</div></div>
          <div class="row"><div class="text"><span class="title">Keep aspect ratio</span></div><div class="suffix"><span class="switch on"></span></div></div>
          <div class="row"><div class="text"><span class="title">Unit</span></div><div class="suffix"><div class="toggle-group"><span class="toggle active">px</span><span class="toggle">%</span></div></div></div>
        </div>
        <div class="boxed-list"><div class="row"><div class="text"><span class="title">Scale annotations</span><span class="subtitle">Line widths and text sizes follow the image</span></div><div class="suffix"><span class="switch on"></span></div></div></div>
      </div>
    </div>
  </div>"""
s09 = editor("Checkout flow", "960 × 800 · Edited", "select", canvas09, document_sidebar().replace("1600 × 1000 px", "960 × 800 px").replace(spin("0 px", 128), spin("20 px", 128)),
             overlays_win=dlg09)
files["09-cropped-resize-dialog"] = page("Cropped, rounded, resize", s09)

# ================================================================ 10 canvas extension with transparent
canvas10 = f"""<div class="canvas-area">
        <div style="position:relative;width:{(1600+400)*.42}px;height:{1000*.42}px">
          <div class="checker" style="position:absolute;inset:0;box-shadow:0 0 0 1px rgb(0 0 6 / 10%), 0 2px 10px rgb(0 0 6 / 18%)"></div>
          <img src="{C.SAMPLES}/b-lumen-checkout.png" alt="" style="position:absolute;left:0;top:0;width:{1600*.42}px;height:{1000*.42}px">
          <svg style="position:absolute;inset:0;width:100%;height:100%;overflow:visible" viewBox="0 0 2000 1000">{SVG_DEFS}
            {rect(100, 512, 438, 66, sw=6, rx=10)}
            {callout(1630, 360, 340, 170, ["Free returns", "for 30 days"], [(1490, 860, 1700, 530)], fs=32)}
            {step(44, 384, "1")}{step(44, 544, "2")}{step(44, 733, "3")}
          </svg>
          <div class="ov" style="left:{1600*.42}px;top:0;bottom:0;width:0;border-left:1px dashed var(--accent-bg)"></div>
          <div class="ov" style="right:-1px;top:0;bottom:0;width:2px;background:var(--accent-bg)"></div>
          <div class="ov" style="right:-7px;top:calc(50% - 18px);width:12px;height:36px;border-radius:6px;background:#ffffff;border:1.5px solid var(--accent-bg)"></div>
          <span class="ov chip" style="right:14px;top:calc(50% + 26px)">+400 px</span>
          {cursor((1600+400)*.42 - 4, 1000*.42/2 + 40, "arrow")}
        </div>
        {zoom_osd("Fit · 42 %")}
      </div>"""
s10 = editor("Checkout flow", "2000 × 1000 · Edited", "select", canvas10, document_sidebar(canvas_sel="transparent").replace(spin("0 px", 128), spin("0 px", 128)))
files["10-canvas-extension-transparent"] = page("Canvas extension", s10)

# ================================================================ 11 tray drag onto canvas
ghost = (f'<div class="ov" style="left:470px;top:240px;width:240px;height:150px;opacity:.85;border-radius:6px;overflow:hidden;'
         f'box-shadow:0 0 0 2px var(--accent-bg), 0 8px 24px rgb(0 0 6 / 35%)"><img src="{C.SAMPLES}/e-relay-chat.png" alt="" style="width:100%;height:100%;object-fit:cover"></div>'
         + cursor(586, 316, "grab"))
canvas11 = canvas("b-lumen-checkout", checkout_annos(), ghost,
                  area_extra='style="box-shadow:inset 0 0 0 3px var(--accent-a50)"')
tray11 = tray(hover_idx=0, drag_idx=0)
s11 = editor("Checkout flow", SUB, "select", canvas11, document_sidebar(), tray_html=tray11,
             overlays_win=f'<div class="chip" style="position:absolute;left:{52+64+470}px;top:{47+73+400}px;z-index:30">Drop to insert image</div>')
files["11-tray-drag-to-canvas"] = page("Drag from tray", s11)

# ================================================================ 12 inserted image object
pip = (f'<image href="{C.SAMPLES}/e-relay-chat.png" x="940" y="480" width="480" height="300" preserveAspectRatio="xMidYMid slice" filter="url(#sh-big)"/>'
       f'<rect x="940" y="480" width="480" height="300" fill="none" stroke="#ffffff" stroke-width="6"/>')
ov12 = sel(940, 480, 480, 300, rot=True)
side12 = f"""
        <div class="group-title"><span class="heading">Image</span><span class="dim-label caption">From recent captures</span></div>
        {field("Border", swatches(selected="paper"))}
        {row_line("Border width", spin("6 px"))}
        {row_line("Corner radius", spin("0 px"))}
        {shadow_row(on=True)}
        {arrange_group("940", "480", "480", "300")}
        <span class="hint">Drag a corner handle to resize; the aspect ratio is kept. Hold Shift to resize freely.</span>"""
s12 = editor("Checkout flow", SUB, "select", canvas("b-lumen-checkout", checkout_annos() + pip, ov12), side12,
             overlays_win=toast("Image inserted", "Undo"))
files["12-inserted-image-object"] = page("Picture in picture", s12)

# ================================================================ 13 narrow 700 bottom sheet
C.ZOOM = 0.375
annos13 = checkout_annos()
ov13 = sel(1000 - 30, 858 - 30, 160, 157)
narrow_canvas = f"""<div class="canvas-area" style="align-items:flex-start;padding-top:40px">
        <div class="canvas-image" style="width:600px;height:375px">
          <img src="{C.SAMPLES}/b-lumen-checkout.png" alt="">
          <svg class="annotations" viewBox="0 0 1600 1000">{SVG_DEFS}{annos13}</svg>
          {ov13}
        </div>
      </div>"""
sheet13 = f"""<div class="bottom-sheet" style="padding:22px 18px 18px;left:52px">
      <div class="hbox" style="margin-bottom:12px"><span class="heading grow">Arrow</span><button class="btn flat">Select All Arrows</button></div>
      <div class="vbox" style="--gap:12px">
        {swatches(second_row=False)}
        <div class="hbox" style="--gap:12px"><span class="grow">Width</span>{spin("8 px")}</div>
        <div class="hbox" style="--gap:12px"><span class="grow">Line style</span>{toggles([("x:solid", "Solid"), ("x:dashed", "Dashed"), ("x:dotted", "Dotted")], 0, glyphs=True).replace('class="toggle-group"', 'class="toggle-group" style="width:200px"')}</div>
        <div class="hbox" style="--gap:12px"><span class="grow">Shadow</span><span class="switch on"></span></div>
      </div>
    </div>"""
s13 = f"""<div class="window" style="width:700px;height:800px">
  {headerbar("Checkout flow", SUB, narrow=True)}
  <div class="split-view">
    {tool_strip("select")}
    <div class="editor-col">{narrow_canvas}</div>
  </div>
  {sheet13}
</div>"""
files["13-narrow-bottom-sheet"] = page("Narrow window", s13, size=(700, 800),
                                       extra_css=" .tool-strip{overflow:hidden} .bottom-sheet .toggle-group>.toggle{flex:1}")
C.ZOOM = 0.5

# ================================================================ 14 crash recovery dialog
empty_canvas = f"""<div class="canvas-area"><div class="status-page compact" style="opacity:.6">{icon("image", "icon status-icon")}<div class="status-title">No Capture Open</div></div></div>"""
dlg14 = f"""<div class="dialog-host" style="z-index:30">
    <div class="dialog" style="width:460px;min-width:460px">
      <div class="dialog-body vbox" style="--gap:18px;padding:30px 24px 24px;text-align:center">
        <div class="vbox" style="--gap:10px;align-items:center">
          <span class="alert-heading" style="font-size:var(--fs-title-2);font-weight:var(--fw-heavy)">Restore Unsaved Captures?</span>
          <span>Annota closed unexpectedly. These documents were autosaved and can be restored.</span>
        </div>
        <div class="boxed-list" style="text-align:left">
          <div class="row"><span class="prefix"><span class="check checked"></span></span><img src="{C.SAMPLES}/b-lumen-checkout.png" alt="" style="width:64px;height:40px;object-fit:cover;border-radius:4px;box-shadow:0 0 0 1px var(--borders)"><div class="text"><span class="title">Checkout flow</span><span class="subtitle">Autosaved today at 10:42 · 7 objects</span></div></div>
          <div class="row"><span class="prefix"><span class="check checked"></span></span><img src="{C.SAMPLES}/d-tidewater-analytics.png" alt="" style="width:64px;height:40px;object-fit:cover;border-radius:4px;box-shadow:0 0 0 1px var(--borders)"><div class="text"><span class="title">September dashboard</span><span class="subtitle">Autosaved today at 10:41 · 12 objects</span></div></div>
        </div>
        <div class="hbox" style="--gap:12px">
          <button class="btn grow destructive-action flat" style="min-height:40px;border-radius:var(--radius-m);background:var(--fill-1)">Discard</button>
          <button class="btn grow suggested-action" style="min-height:40px;border-radius:var(--radius-m)">Restore</button>
        </div>
      </div>
    </div>
  </div>"""
s14 = f"""<div class="window" style="width:1280px;height:800px">
  {headerbar("Annota", "", True, True)}
  <div class="split-view">{tool_strip("select")}<div class="editor-col">{empty_canvas}{tray(current=None, open_docs=[])}</div>
  <div class="sidebar-pane end props"><div class="scroll"></div></div></div>
  {dlg14}
</div>"""
files["14-crash-recovery"] = page("Crash recovery", s14)

# ================================================================ 15 newer major version error
dlg15 = f"""<div class="dialog-host" style="z-index:30">
    <div class="dialog alert" style="width:420px;max-width:420px">
      <div class="alert-body">
        <span class="alert-heading">Newer Annota Version Needed</span>
        <span>“Release notes v2.annota” uses document format 2.0. It was saved with Annota 2.1 or newer.</span>
        <span>This is Annota 0.1, which opens format 1.x. Install Annota 2.1 or newer to open this file. The file was not changed.</span>
      </div>
      <div class="alert-actions"><button class="btn">Show in Files</button><button class="btn suggested-action">OK</button></div>
    </div>
  </div>"""
s15 = editor("Checkout flow", "1600 × 1000", "select", canvas("b-lumen-checkout", checkout_annos()), document_sidebar(),
             overlays_win=dlg15, undo_dis=True)
files["15-newer-format-error"] = page("Newer format error", s15)

# ================================================================ 16 many objects
import random
random.seed(7)
many = []
rows_y = [338, 374, 410, 446, 482, 518]
for i, yy in enumerate(rows_y):
    many.append(highlight(1156, yy - 18, 380, 34, opacity=.35))
for i in range(30):
    x = 345 + i * 25.8
    many.append(step(x, 700 - (i % 5) * 14 - 40, str(i + 1), r=11, shadow=False, color=["var(--annota-coral)", "var(--annota-azure)"][i % 2]))
for k, (x, y) in enumerate([(284, 109), (614, 109), (944, 109), (1274, 109)]):
    many.append(rect(x - 14, y - 22, 300, 96, sw=4, rx=10, color="var(--annota-teal)"))
    many.append(step(x + 270, y - 22, chr(65 + k), r=16, color="var(--annota-teal)"))
for a in [(1300, 600, 1440, 520), (1300, 640, 1460, 600), (1200, 640, 1240, 612)]:
    many.append(arrow(*a, w=6))
many.append(callout(320, 300, 440, 80, ["Peak on 26 Sep"], [(1040, 440, 700, 340)], fs=28, fill="var(--annota-violet)"))
many.append(rect(1150, 590, 380, 70, sw=4, rx=8))
side16 = f"""
        <div class="group-title"><span class="heading">Document</span><span class="dim-label caption">64 objects</span></div>
        <div class="boxed-list">
          <div class="row"><span class="prefix">{icon("step")}</span><div class="text"><span class="title">Steps</span></div><div class="suffix"><span class="badge neutral">34</span><button class="btn flat small">Select</button></div></div>
          <div class="row"><span class="prefix">{icon("highlighter")}</span><div class="text"><span class="title">Highlights</span></div><div class="suffix"><span class="badge neutral">6</span><button class="btn flat small">Select</button></div></div>
          <div class="row"><span class="prefix">{icon("shape")}</span><div class="text"><span class="title">Rectangles</span></div><div class="suffix"><span class="badge neutral">5</span><button class="btn flat small">Select</button></div></div>
          <div class="row"><span class="prefix">{icon("arrow")}</span><div class="text"><span class="title">Arrows</span></div><div class="suffix"><span class="badge neutral">3</span><button class="btn flat small">Select</button></div></div>
          <div class="row"><span class="prefix">{icon("callout")}</span><div class="text"><span class="title">Callouts</span></div><div class="suffix"><span class="badge neutral">1</span><button class="btn flat small">Select</button></div></div>
          <div class="row"><span class="prefix">{icon("image")}</span><div class="text"><span class="title">Other</span></div><div class="suffix"><span class="badge neutral">15</span><button class="btn flat small">Select</button></div></div>
        </div>
        <div class="boxed-list">
            <div class="row"><div class="text"><span class="title">Snap to guides</span><span class="subtitle">Ctrl+; · hold Alt to suspend</span></div><div class="suffix"><span class="switch on"></span></div></div>
            <div class="row"><div class="text"><span class="title">64 editable objects</span></div><div class="suffix"><button class="btn">Flatten All</button></div></div>
        </div>"""
s16 = editor("September dashboard", "1600 × 1000 · Edited", "select",
             canvas("d-tidewater-analytics", "".join(many)), side16, tray_html=tray(current="d-tidewater-analytics"))
files["16-many-objects"] = page("Many objects", s16)

# ================================================================ 17 main menu
menu17 = f"""<div class="popover" style="right:52px;top:44px;width:290px;--arrow-x:262px">
      <div class="menu-item"><span class="label">New Capture</span><span class="accel">Ctrl+N</span></div>
      <div class="menu-item"><span class="label">Open…</span><span class="accel">Ctrl+O</span></div>
      <div class="menu-item"><span class="label">Open Library</span><span class="accel">Ctrl+L</span></div>
      <div class="menu-sep"></div>
      <div class="menu-item"><span class="label">Save</span><span class="accel">Ctrl+S</span></div>
      <div class="menu-item"><span class="label">Save As…</span><span class="accel">Ctrl+Shift+S</span></div>
      <div class="menu-item"><span class="label">Export…</span><span class="accel">Ctrl+E</span></div>
      <div class="menu-sep"></div>
      <div class="menu-item"><span class="label">Resize Image…</span><span class="accel"></span></div>
      <div class="menu-item"><span class="label">Rotate Right</span><span class="accel">Ctrl+R</span></div>
      <div class="menu-item"><span class="label">Rotate Left</span><span class="accel">Ctrl+Shift+R</span></div>
      <div class="menu-item"><span class="label">Flip Horizontally</span></div>
      <div class="menu-item"><span class="label">Flip Vertically</span></div>
      <div class="menu-item hover"><span class="label">Flatten All</span><span class="accel">Ctrl+Shift+T</span></div>
      <div class="menu-sep"></div>
      <div class="menu-item"><span class="label">Preferences</span><span class="accel">Ctrl+,</span></div>
      <div class="menu-item"><span class="label">Keyboard Shortcuts</span><span class="accel">Ctrl+?</span></div>
      <div class="menu-item"><span class="label">About Annota</span></div>
    </div>"""
s17 = editor("Checkout flow", SUB, "select", canvas("b-lumen-checkout", checkout_annos()), document_sidebar(), overlays_win=menu17)
s17 = s17.replace(f'<button class="btn flat image-button">{icon("open-menu")}</button>', f'<button class="btn flat image-button active">{icon("open-menu")}</button>')
files["17-main-menu"] = page("Main menu", s17)

# ================================================================ 18 flatten all result
flat_img = (f'<img src="{C.SAMPLES}/b-lumen-checkout.png" alt="">')
s18 = editor("Checkout flow", SUB, "select", canvas("b-lumen-checkout", checkout_annos()),
             document_sidebar(objects=0).replace('<div class="row"><div class="text"><span class="title">0 editable objects</span></div><div class="suffix"><button class="btn">Flatten All</button></div></div>',
                                                 '<div class="row disabled"><div class="text"><span class="title">No editable objects</span><span class="subtitle">Annotations are part of the image</span></div><div class="suffix"><button class="btn" disabled>Flatten All</button></div></div>'),
             overlays_win=toast("7 objects flattened into the image", "Undo"))
files["18-flatten-all-result"] = page("Flatten all", s18)

for k, v in files.items():
    (OUT / f"{k}.html").write_text(v, encoding="utf-8")
print("\n".join(files))
