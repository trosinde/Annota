import sys; sys.path.insert(0, __file__.rsplit("/", 1)[0])
from common import *

C = "add-text-recognition"
SC, OFF = 0.55, 700  # field-notes: 55 % zoom, scrolled to native y=700

PARA = ("A tide table tells you when; the water itself tells you how. Wind from the west piles water\n"
        "against the breakwater and can lift high tide by half a metre. A long calm spell does the\n"
        "opposite. Locals learn to read both the dial and the colour of the channel before they\n"
        "leave the slip.")

BLOCKS = [(390, 695, 820, 82), (390, 806, 300, 36), (390, 848, 820, 112), (390, 1248, 820, 84),
          (390, 1360, 340, 34), (390, 1402, 820, 84), (390, 1502, 820, 52), (390, 1578, 820, 84)]


def blocks_svg(selected=None, show=True):
    out = ""
    if show:
        for i, (x, y, w, h) in enumerate(BLOCKS):
            if i == selected:
                continue
            out += (f'<rect x="{x - 8}" y="{y - 6}" width="{w + 16}" height="{h + 12}" rx="8" fill="var(--accent-a12)" '
                    f'stroke="var(--accent-bg)" stroke-opacity=".45" stroke-width="2" stroke-dasharray="8 6"/>')
    if selected is not None:
        x, y, w, h = BLOCKS[selected]
        out += (f'<rect x="{x - 10}" y="{y - 8}" width="{w + 20}" height="{h + 16}" rx="8" fill="var(--accent-a25)" '
                f'stroke="var(--accent-bg)" stroke-width="3"/>')
        for hx, hy in [(x - 10, y - 8), (x + w + 10, y - 8), (x - 10, y + h + 8), (x + w + 10, y + h + 8)]:
            out += f'<circle cx="{hx}" cy="{hy}" r="9" fill="#fff" stroke="var(--accent-bg)" stroke-width="3"/>'
    return ov(out, 1600, 2600)


def field_canvas(inner):
    return native("f-field-notes", SC, 880, 700, inner, offset_y=OFF, img_h=2600)


def local_note(text="Runs on this computer. The image never leaves it."):
    return f'<div class="local-note">{icon("info")}<span>{text}</span></div>'


def ocr_props(state="selected", found="8 text areas found"):
    lang = ('<div class="boxed-list">'
            '<div class="row"><div class="text"><span class="title">Language</span></div>'
            '<div class="suffix"><span class="dropdown flat">English + German</span></div></div>'
            '<div class="row"><div class="text"><span class="title">Show text areas</span></div>'
            '<div class="suffix"><span class="switch on"></span></div></div></div>')
    if state == "selected":
        sel = (f'<div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Selection</span>'
               f'<span class="description">4 lines · 61 words</span></div></div>'
               f'<div class="card" style="padding:12px;font-size:var(--fs-small);line-height:1.45;max-height:150px;overflow:hidden">'
               f'{PARA.replace(chr(10), " ")}</div>'
               f'<button class="btn text-button suggested-action" style="width:100%">{icon("copy")}Grab Text…<kbd class="key" style="margin-left:auto;background:rgb(255 255 255 / 20%);box-shadow:none">Ctrl+Shift+C</kbd></button>'
               f'<button class="btn text-button" style="width:100%">{icon("text")}Edit Text</button></div>')
    else:
        sel = ('<div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Selection</span></div></div>'
               '<div class="hint">Drag across text or click a highlighted area. Double-click a line to edit it in place.</div></div>')
    return (f'<div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Recognise Text</span>'
            f'<span class="description">{found}</span></div></div>{lang}</div>{sel}'
            f'{local_note()}')


# 01 — selection-based grab: recognised areas + one selected paragraph
write(C, "01-select-text-area",
      editor("The tide clock", "1600 × 2600", "ocr-text", field_canvas(blocks_svg(selected=2)), ocr_props(), zoom="55 %"),
      "select text area")

# 02 — grab text dialog (selection)
def grab_dialog(text_html, footer, title="Grab Text", copy_label="Copy", copy_disabled=False, width=760, body_extra=""):
    dis = " disabled" if copy_disabled else ""
    return f"""<div class="dialog-host"><div class="dialog" style="width:{width}px;max-width:{width}px">
<div class="headerbar"><div class="start"><button class="btn text-button">Cancel</button></div>
<div class="title-widget"><span class="title">{title}</span></div>
<div class="end"><button class="btn text-button suggested-action"{dis}>{copy_label}</button></div></div>
<div class="dialog-body vbox" style="--gap:12px">
{text_html}
{body_extra}
<div class="hbox" style="--gap:12px">
  <span class="dropdown" style="min-width:200px">English + German</span>
  <span class="check-label"><span class="check checked"></span>Keep line breaks</span>
  <span class="spacer"></span>
  <span class="caption dim-label">{footer}</span>
</div>
{local_note("Recognised on this computer with Tesseract. Works offline.")}
</div></div></div>"""


TEXTBOX = ('<div class="card" style="padding:14px 16px;font-size:var(--fs-small);line-height:1.65;white-space:pre-wrap;min-height:170px;'
           'box-shadow:inset 0 0 0 1px var(--borders);background:var(--view-bg)">{}</div>')

write(C, "02-grab-text-dialog",
      editor("The tide clock", "1600 × 2600", "ocr-text", field_canvas(blocks_svg(selected=2)), ocr_props(),
             overlay=grab_dialog(TEXTBOX.format(PARA + '<span class="entry" style="display:inline;padding:0;background:none"><span class="caret" style="margin-left:1px;vertical-align:-3px"></span></span>'),
                                 "4 lines · 61 words"), zoom="55 %"),
      "grab text dialog")

# 03 — copied: toast
write(C, "03-text-copied-toast",
      editor("The tide clock", "1600 × 2600", "ocr-text", field_canvas(blocks_svg(selected=2)), ocr_props(),
             overlay=f'<div class="toast-overlay" style="left:52px;right:300px"><div class="toast"><span class="toast-title">4 lines copied to clipboard</span>'
                     f'<button class="btn flat circular">{icon("close")}</button></div></div>', zoom="55 %"),
      "text copied")

# 04 — whole image, recognising (loading)
loading = ('<div class="card" style="padding:14px 16px;min-height:170px;box-shadow:inset 0 0 0 1px var(--borders);background:var(--view-bg);'
           'display:flex;flex-direction:column;align-items:center;justify-content:center;gap:12px">'
           '<span class="spinner large"></span><span>Recognising text…</span>'
           '<div class="progress" style="width:260px"><div class="bar" style="width:58%"></div></div>'
           '<span class="caption dim-label">Whole image · 1600 × 2600</span></div>')
write(C, "04-grab-whole-image-loading",
      editor("The tide clock", "1600 × 2600", "select", field_canvas(""), "", zoom="55 %",
             overlay=grab_dialog(loading, "", copy_disabled=True)).replace(
          '<div class="sidebar-pane end props"><div class="scroll"></div></div>',
          '<div class="sidebar-pane end props"><div class="scroll"><div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Select</span><span class="description">Click an object to edit it</span></div></div></div></div></div>'),
      "grab text loading")

# 05 — no text found (empty)
empty = ('<div class="card" style="min-height:200px;box-shadow:inset 0 0 0 1px var(--borders);background:var(--view-bg);display:flex">'
         f'<div class="status-page compact" style="padding:18px">{icon("ocr-text", "icon status-icon")}'
         '<div class="status-title" style="font-size:var(--fs-title-4)">No Text Found</div>'
         '<div class="status-description" style="margin-bottom:0">Try a larger zoom when capturing, or pick another language. '
         'Handwriting and very small text are hard to read.</div></div></div>')
dash = native("d-tidewater-analytics", 0.5, 800, 500,
              ov('<rect x="290" y="300" width="820" height="600" rx="6" fill="var(--accent-a25)" stroke="var(--accent-bg)" stroke-width="4"/>'))
write(C, "05-no-text-found",
      editor("Tidewater dashboard", "1600 × 1000", "ocr-text", dash, ocr_props("none", "No text areas found"),
             overlay=grab_dialog(empty, "0 lines", copy_disabled=True)),
      "no text found")

# 06 — edit text in place: before (editing)
PATCH_STYLE = "font-family:var(--font-ui);font-size:14.5px;color:#7a7a7f;background:#fff;letter-spacing:.01em"


def harbor_canvas(inner, scale=0.68, w=880, h=600, offx=260, offy=100):
    return (f'<div class="canvas-image" style="width:{w}px;height:{h}px;overflow:hidden">'
            f'<div class="native" style="width:1600px;height:1000px;transform:scale({scale}) translate(-{offx}px,-{offy}px)">'
            f'<img src="{S}a-harbor-backup.png" style="width:1600px;height:1000px" alt="">{inner}</div></div>')


typo = '<div class="patch" style="left:653px;top:605px;height:20px;width:250px;{s}">{t}</div>'
before_inner = (typo.format(s=PATCH_STYLE, t="Recieve a warning before pruning")
                # in-place edit field
                + '<div class="patch" style="left:645px;top:601px;height:28px;width:262px;outline:3px solid var(--accent-bg);'
                  'border-radius:4px;background:#fff;padding-left:8px;font-family:var(--font-ui);font-size:14.5px;color:#7a7a7f">'
                  'Recie<span style="display:inline-block;width:2px;height:18px;background:var(--accent-bg);margin:0 1px"></span>ve a warning before pruning</div>'
                + ov('<path d="M653 625 q3 3 6 0 t6 0 t6 0 t6 0 t6 0 t6 0 t6 0 t6 0" stroke="var(--destructive)" stroke-width="2" fill="none"/>'))
SWATCH_ROW = ('<div class="row"><div class="text"><span class="title">{l}</span><span class="subtitle">{s}</span></div>'
              '<div class="suffix"><span class="swatch small square" style="--c:{c}"></span></div></div>')


def edit_props(state):
    head = ('<div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Edit Text</span>'
            '<span class="description">{}</span></div></div>').format(
        "Type to replace the recognised line" if state == "before" else "Replaced text stays editable")
    orig = ('<div class="boxed-list"><div class="row property"><div class="text"><span class="title">Recognised</span>'
            '<span class="subtitle">Recieve a warning before pruning</span></div></div></div>')
    style = ('<div class="boxed-list">'
             '<div class="row"><div class="text"><span class="title">Font</span></div><div class="suffix"><span class="dropdown flat">Sans (matched)</span></div></div>'
             '<div class="row"><div class="text"><span class="title">Size</span><span class="subtitle">Estimated from the image</span></div><div class="suffix"><span class="spin" style="width:110px"><span class="value">14</span>'
             f'<button class="btn">{icon("minus")}</button><button class="btn">{icon("plus")}</button></span></div></div>'
             + SWATCH_ROW.format(l="Text colour", s="Sampled · #7A7A7F", c="#7a7a7f")
             + SWATCH_ROW.format(l="Background", s="Sampled · #FFFFFF", c="#ffffff")
             + '</div>')
    btns = ('<div class="hbox" style="--gap:6px"><button class="btn text-button grow">Revert</button>'
            + ('<button class="btn text-button suggested-action grow">Done</button>' if state == "before" else "") + '</div>')
    hint = '<div class="hint">Enter applies · Esc cancels</div>' if state == "before" else ""
    return head + orig + style + btns + hint + '</div>' + local_note()


write(C, "06-edit-text-before",
      editor("Harbor preferences", "1600 × 1000 · Edited", "ocr-text", harbor_canvas(before_inner), edit_props("before"), zoom="68 %"),
      "edit text before")

after_inner = (typo.format(s=PATCH_STYLE, t="Receive a warning before pruning")
               + ov('<rect x="648" y="603" width="254" height="26" fill="none" stroke="var(--accent-bg)" stroke-width="2.5"/>'
                    + "".join(f'<circle cx="{x}" cy="{y}" r="8" fill="#fff" stroke="var(--accent-bg)" stroke-width="3"/>'
                              for x, y in [(648, 603), (902, 603), (648, 629), (902, 629)])))
write(C, "07-edit-text-after",
      editor("Harbor preferences", "1600 × 1000 · Edited", "select", harbor_canvas(after_inner), edit_props("after"), zoom="68 %",
             overlay=f'<div class="toast-overlay" style="left:52px;right:300px"><div class="toast"><span class="toast-title">Text replaced</span>'
                     f'<button class="btn flat text-button">Undo</button><button class="btn flat circular">{icon("close")}</button></div></div>'),
      "edit text after")

# 08 — text capture mode: region selection on desktop
sel = (1280 * 0.0, 0)
desk = f"""<div class="desktop">
{shell_panel()}
<img class="screen-img" src="{S}e-relay-chat.png" style="height:800px" alt="">
<div class="scrim" style="left:0;top:32px;width:1280px;height:96px"></div>
<div class="scrim" style="left:0;top:128px;width:380px;height:84px"></div>
<div class="scrim" style="left:832px;top:128px;width:448px;height:84px"></div>
<div class="scrim" style="left:0;top:212px;width:1280px;height:588px"></div>
<div style="position:absolute;left:380px;top:128px;width:452px;height:84px;outline:2px solid #fff;box-shadow:0 0 0 1px rgb(0 0 6 / 40%)"></div>
<div style="position:absolute;left:380px;top:220px;" class="osd-toolbar"><span class="label">452 × 84</span></div>
<div class="osd-toolbar pill" style="position:absolute;left:50%;transform:translateX(-50%);bottom:36px;gap:6px">
  {icon("ocr-text")}<span class="label">Select text to copy</span><span class="sep"></span>
  <span class="label" style="font-weight:400;opacity:.8">Enter to confirm · Esc to cancel</span>
</div>
</div>"""
write(C, "08-text-capture-select-region", desk, "text capture region", flush=True)

notif = f"""<div class="desktop">
{shell_panel()}
<img class="screen-img" src="{S}e-relay-chat.png" style="height:800px" alt="">
<div class="shell-notif">
  <img src="../../kit/app-icon/annota.svg" width="36" height="36" alt="" style="flex:none">
  <div class="grow"><div class="hbox"><span class="t grow">Text copied</span><span class="b">now</span></div>
  <div class="b">3 lines from your selection are on the clipboard.<br>“Morning! For the venue booking, the contact is …”</div>
  <div class="acts"><button class="btn">Show Text</button><button class="btn">Open in Editor</button></div></div>
</div>
</div>"""
write(C, "09-text-capture-notification", notif, "text capture notification")

# 10 — OCR preferences
LANGS = [("English", "eng · installed", True, True), ("German", "deu · installed · system language", True, True),
         ("French", "fra · installed", False, True), ("Spanish", "Not installed", False, False)]
lang_rows = "".join(
    f'<div class="row{"" if inst else " "}"><div class="prefix"><span class="check{" checked" if on else ""}" style="{"" if inst else "opacity:.4"}"></span></div>'
    f'<div class="text"><span class="title">{n}</span><span class="subtitle">{s}</span></div>'
    + ('' if inst else '<div class="suffix"><code class="caption" style="padding:3px 6px;border-radius:4px;background:var(--fill-1)">sudo apt install tesseract-ocr-spa</code></div>')
    + '</div>' for n, s, on, inst in LANGS)
prefs = f"""<div class="dialog-host"><div class="dialog" style="width:640px;max-width:640px;height:700px">
<div class="headerbar"><div class="start"></div><div class="title-widget">
<div class="toggle-group flat"><span class="toggle">{icon("settings")}General</span><span class="toggle">{icon("capture-region")}Capture</span><span class="toggle active">{icon("ocr-text")}Text Recognition</span></div></div>
<div class="end"><div class="window-controls"><span class="wc">{icon("close")}</span></div></div></div>
<div style="overflow:hidden;flex:1"><div class="preferences-page" style="padding-top:12px">
<div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Languages</span>
<span class="description">Annota reads text in every checked language. Fewer languages are faster.</span></div></div>
<div class="boxed-list"><div class="row"><div class="text"><span class="title">Follow system language</span><span class="subtitle">Adds German automatically</span></div><div class="suffix"><span class="switch on"></span></div></div></div>
<div class="boxed-list">{lang_rows}</div></div>
<div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Result</span></div></div>
<div class="boxed-list">
<div class="row"><div class="text"><span class="title">Keep line breaks</span></div><div class="suffix"><span class="switch on"></span></div></div>
<div class="row"><div class="text"><span class="title">Join words split by hyphens</span></div><div class="suffix"><span class="switch"></span></div></div>
<div class="row"><div class="text"><span class="title">Show text before copying</span><span class="subtitle">Off: Ctrl+Shift+C copies right away</span></div><div class="suffix"><span class="switch on"></span></div></div>
</div></div>
{local_note("Text recognition runs on this computer with Tesseract. Annota never sends images anywhere.")}
</div></div></div></div>"""
write(C, "10-preferences-ocr-languages",
      editor("The tide clock", "1600 × 2600", "select", field_canvas(""), "", overlay=prefs, zoom="55 %"),
      "OCR preferences")

# 11 — Tesseract missing
banner = (f'<div class="banner warning"><span class="banner-title">Text recognition needs Tesseract. Install it with '
          f'<code style="padding:2px 6px;border-radius:4px;background:var(--fill-1)">sudo apt install tesseract-ocr</code></span>'
          f'<button class="btn">Copy Command</button></div>')
missing_props = (f'<div class="status-page compact" style="padding:36px 6px">{icon("ocr-text", "icon status-icon")}'
                 '<div class="status-title" style="font-size:var(--fs-title-4)">Text Recognition Unavailable</div>'
                 '<div class="status-description">Install the <b>tesseract-ocr</b> package, then try again. No restart needed.</div>'
                 '<button class="btn text-button">Try Again</button></div>')
write(C, "11-tesseract-not-installed",
      editor("The tide clock", "1600 × 2600", "ocr-text", field_canvas(""), missing_props, banner=banner, zoom="55 %"),
      "tesseract missing")

# 12 — narrow: grab text as bottom sheet
narrow = f"""<div class="window" style="width:700px;height:800px">
{headerbar("The tide clock", "1600 × 2600")}
<div class="content-pane" style="position:relative;flex:1;display:flex">
<div class="canvas-area">{native("f-field-notes", 0.4, 640, 700, blocks_svg(selected=2), offset_y=OFF, img_h=2600)}</div>
</div>
<div class="backdrop-dim"></div>
<div class="bottom-sheet" style="padding:24px 18px 18px">
<div class="hbox" style="margin-bottom:12px"><button class="btn text-button">Cancel</button><span class="spacer"></span><span class="heading">Grab Text</span><span class="spacer"></span><button class="btn text-button suggested-action">Copy</button></div>
{TEXTBOX.format(PARA.replace(chr(10), " ")).replace('white-space:pre-wrap;','')}
<div class="hbox" style="margin-top:12px"><span class="dropdown">English + German</span><span class="spacer"></span><span class="caption dim-label">4 lines · 61 words</span></div>
<div style="margin-top:12px">{local_note("Recognised on this computer with Tesseract. Works offline.")}</div>
</div>
</div>"""
write(C, "12-grab-text-narrow", narrow, "grab text narrow", size="700x800")
print("ok")
