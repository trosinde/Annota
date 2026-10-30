import sys, random; sys.path.insert(0, __file__.rsplit("/", 1)[0])
from common import *

C = "add-smart-redaction"
SC = 0.55

# detections in native 1600×1000 coordinates of e-relay-chat.png
ITEMS = {
    "email": [("hannah.wexler@riverside-hall.example", (496, 160, 294, 24)),
              ("jonas.okafor@fernway.example", (1320, 197, 214, 21))],
    "phone": [("+49 30 5550 1287", (1153, 256, 59, 22)), ("+49 30 5550 1287", (699, 278, 82, 22)),
              ("+49 30 5550 9934", (1320, 245, 132, 21))],
    "ip": [("192.168.40.17", (294, 136, 75, 18)), ("192.168.40.17", (498, 374, 118, 22)),
           ("192.168.40.17", (664, 607, 106, 22))],
    "date": [("Today", (822, 92, 58, 23)), ("Yesterday", (327, 249, 60, 17)), ("Yesterday", (327, 315, 60, 17))],
    "token": [("RH-2026-0417", (935, 511, 113, 22))],
}
CATS = [("email", "E-mail addresses", 2), ("phone", "Phone numbers", 3), ("ip", "IP addresses", 3),
        ("url", "Web addresses", 0), ("card", "Payment cards", 0), ("iban", "IBANs", 0),
        ("date", "Dates", 3), ("token", "Keys and tokens", 1), ("face", "Faces", 0)]
LABEL = {"email": "E-mail", "phone": "Phone", "ip": "IP address", "date": "Date", "token": "Token"}


def mosaic(x, y, w, h, seed, cell=10):
    rnd = random.Random(seed)
    out = []
    for yy in range(0, h, cell):
        for xx in range(0, w, cell):
            g = rnd.randint(150, 235)
            out.append(f'<rect x="{x + xx}" y="{y + yy}" width="{min(cell, w - xx)}" height="{min(cell, h - yy)}" fill="rgb({g} {g} {min(255, g + 12)})"/>')
    return "".join(out)


def redactions(style="solid", off=(), tags=True, selected=None, off_items=()):
    svg, divs = "", ""
    i = 0
    for cat, items in ITEMS.items():
        if cat in off:
            continue
        for n, (label, (x, y, w, h)) in enumerate(items):
            i += 1
            if (cat, n) in off_items:
                svg += (f'<rect x="{x - 3}" y="{y - 3}" width="{w + 6}" height="{h + 6}" rx="3" fill="none" '
                        f'stroke="var(--annota-ink)" stroke-opacity=".5" stroke-width="2" stroke-dasharray="6 4"/>')
                continue
            if style == "solid":
                svg += f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="2" fill="var(--annota-ink)"/>'
            elif style == "pixelate":
                svg += mosaic(x, y, w, h, i)
            else:
                divs += (f'<div style="position:absolute;left:{x}px;top:{y}px;width:{w}px;height:{h}px;'
                         f'backdrop-filter:blur(7px);-webkit-backdrop-filter:blur(7px);border-radius:2px"></div>')
            if tags == 'all' or (tags and selected == (cat, n)):
                tw = len(LABEL[cat]) * 8.6 + 16
                svg += (f'<g><rect x="{x - 4}" y="{y - 26}" width="{tw}" height="19" rx="9.5" fill="var(--annota-coral)"/>'
                        f'<text x="{x + 4}" y="{y - 12}" font-size="13" font-weight="700" fill="#fff" '
                        f'font-family="Inter, sans-serif">{LABEL[cat]}</text></g>')
            if tags and selected != (cat, n):
                svg += f'<rect x="{x - 3}" y="{y - 3}" width="{w + 6}" height="{h + 6}" rx="3" fill="none" stroke="var(--annota-coral)" stroke-width="2.5"/>'
            if selected == (cat, n):
                svg += (f'<rect x="{x - 4}" y="{y - 4}" width="{w + 8}" height="{h + 8}" fill="none" stroke="var(--accent-bg)" stroke-width="3"/>'
                        + "".join(f'<circle cx="{a}" cy="{b}" r="7" fill="#fff" stroke="var(--accent-bg)" stroke-width="3"/>'
                                  for a, b in [(x - 4, y - 4), (x + w + 4, y - 4), (x - 4, y + h + 4), (x + w + 4, y + h + 4)]))
    return divs + ov(svg)


def canvas(inner, w=880, h=550):
    return native("e-relay-chat", SC, w, h, inner)


def style_toggle(active):
    t = lambda n: f'<span class="toggle{" active" if n == active else ""}" style="flex:1">{n}</span>'
    return f'<div class="toggle-group" style="display:flex">{t("Solid")}{t("Pixelate")}{t("Blur")}</div>'


ROWCSS = """
  .cat-list .row { min-height: 44px; padding: 4px 12px; }
  .cat-list .row .title { font-size: var(--fs-small); }
  .cat-list .row.empty .title, .cat-list .row.empty .badge { opacity: .5; }
  .cat-list .item { min-height: 36px; padding: 2px 12px 2px 42px; background: rgb(0 0 6 / 3%); }
  .cat-list .item .title { font-family: var(--font-mono); font-size: 12px; }
  .warn-note { display: flex; gap: 8px; align-items: flex-start; padding: 10px 12px; border-radius: var(--radius-m); background: rgb(229 165 10 / 18%); font-size: var(--fs-small); line-height: 1.4; }
  .warn-note .icon { color: var(--warning); margin-top: 1px; }
"""


def cat_list(off=(), expand="email", off_items=()):
    rows = []
    for key, name, n in CATS:
        on = n > 0 and key not in off
        count = n - sum(1 for c, _ in off_items if c == key) if on else n
        badge = f'<span class="badge {"neutral" if not on else ""}">{count if on else n}</span>'
        chev = icon("pan-down") if key == expand else ""
        rows.append(f'<div class="row{" empty" if n == 0 else ""}"><div class="prefix" style="width:16px">{chev}</div>'
                    f'<div class="text"><span class="title">{name}</span></div>'
                    f'<div class="suffix">{badge if n else "<span class=caption style=opacity:.5>None</span>"}'
                    f'<span class="switch{" on" if on else ""}{" disabled" if n == 0 else ""}" style="transform:scale(.85)"></span></div></div>')
        if key == expand:
            for i, (label, _) in enumerate(ITEMS[key]):
                chk = "" if (key, i) in off_items else " checked"
                rows.append(f'<div class="row item"><div class="text"><span class="title ellipsize">{label}</span></div>'
                            f'<div class="suffix"><span class="check{chk}"></span></div></div>')
    return '<div class="boxed-list cat-list">' + "".join(rows) + '</div>'


def props(style="Solid", off=(), expand="email", total=12, off_items=(), warn=False):
    w = ('<div class="warn-note">' + icon("warning") +
         '<span>Blur and pixelate can sometimes be undone by others. Use Solid for passwords, keys and personal data.</span></div>') if warn else ""
    return (f'<div class="preferences-group" style="gap:6px"><div class="group-header"><div class="text"><span class="heading">Smart Redact</span>'
            f'<span class="description">{total} items found · checked on this computer</span></div></div></div>'
            f'<div class="vbox" style="--gap:6px"><span class="sec-title">Style for all</span>{style_toggle(style)}</div>{w}'
            f'{cat_list(off, expand, off_items)}'
            f'<button class="btn text-button destructive-action flat" style="width:100%;background:var(--fill-1)">Permanently Redact…</button>')


def ed(canvas_html, side, overlay="", title="Venue chat", sub="1600 × 1000 · Edited"):
    return f"<style>{ROWCSS}</style>" + editor(title, sub, "redact", canvas_html, side, overlay=overlay, zoom="55 %")


# 01 scanning
scan_side = (f'<div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Smart Redact</span>'
             f'<span class="description">Looking for personal data…</span></div></div>'
             f'<div class="card pad-2 vbox" style="--gap:12px;align-items:center;padding:24px 12px">'
             f'<span class="spinner large"></span><span>Reading text and looking for faces</span>'
             f'<div class="progress" style="width:200px"><div class="bar" style="width:40%"></div></div>'
             f'<span class="caption dim-label">E-mail, phone, IP, web address, card, IBAN, date, key, face</span></div>'
             f'<button class="btn text-button" style="width:100%">Cancel</button>'
             f'<div class="local-note">{icon("info")}<span>Runs on this computer. Nothing is uploaded, and it works offline.</span></div></div>')
scan_line = ov('<rect x="0" y="420" width="1600" height="6" fill="var(--accent-bg)" opacity=".7"/>'
               '<rect x="0" y="0" width="1600" height="420" fill="var(--accent-a12)"/>')
write(C, "01-detecting", ed(canvas(scan_line), scan_side), "detecting")

# 02 review: email found + tagged, solid
write(C, "02-review-detections",
      ed(canvas(redactions("solid", selected=("email", 0))), props("Solid")), "review detections")

# 03 dates excluded
write(C, "03-exclude-dates",
      ed(canvas(redactions("solid", off=("date",))), props("Solid", off=("date",), expand="token", total=8,
                                                            off_items=(("token", 0),)) )
      .replace(redactions("solid", off=("date",)), redactions("solid", off=("date",), off_items=(("token", 0),))),
      "exclude dates")

# 04 pixelate with warning
write(C, "04-style-pixelate-warning",
      ed(canvas(redactions("pixelate", off=("date",), tags=False)), props("Pixelate", off=("date",), expand=None, total=9, warn=True)),
      "pixelate warning")

# 05 blur
write(C, "05-style-blur",
      ed(canvas(redactions("blur", off=("date",), tags=False)), props("Blur", off=("date",), expand=None, total=9, warn=True)),
      "blur")

# 06 permanent redaction confirmation
alert = ('<div class="dialog-host"><div class="dialog alert" style="width:420px;max-width:420px">'
         '<div class="alert-body"><div class="alert-heading">Permanently Redact?</div>'
         '<div>The pixels under 9 solid redactions are removed from the original image when you save. '
         'They cannot be recovered, not even with Undo after saving.</div>'
         '<span class="check-label" style="justify-content:center;margin-top:6px;font-size:var(--fs-small)"><span class="check checked"></span>Switch blur and pixelate to Solid</span></div>'
         '<div class="alert-actions"><button class="btn">Cancel</button><button class="btn destructive-action">Redact and Save</button></div></div></div>')
write(C, "06-permanent-redaction-confirm",
      ed(canvas(redactions("solid", off=("date",), tags=False)), props("Solid", off=("date",), expand=None, total=9), overlay=alert),
      "permanent confirm")

# 07 after permanent redaction: toast + subtitle
done_side = props("Solid", off=("date",), expand=None, total=9).replace(
    '<button class="btn text-button destructive-action flat" style="width:100%;background:var(--fill-1)">Permanently Redact…</button>',
    f'<div class="card" style="padding:12px;display:flex;gap:10px;align-items:flex-start">{icon("check")}'
    '<div class="vbox" style="--gap:2px"><span class="heading" style="font-size:var(--fs-small)">Permanently redacted</span>'
    '<span class="hint">Hidden pixels were removed from the saved file.</span></div></div>')
write(C, "07-permanently-redacted",
      ed(canvas(redactions("solid", off=("date",), tags=False)), done_side, sub="1600 × 1000 · Saved · Permanently redacted",
         overlay=f'<div class="toast-overlay" style="left:52px;right:300px"><div class="toast"><span class="toast-title">Saved “Venue chat.annota” · original pixels removed</span>'
                 f'<button class="btn flat circular">{icon("close")}</button></div></div>'),
      "permanently redacted")

# 08 export dialog with warning
exp = f"""<div class="dialog-host"><div class="dialog" style="width:560px;max-width:560px">
<div class="headerbar"><div class="start"><button class="btn text-button">Cancel</button></div><div class="title-widget"><span class="title">Export Image</span></div><div class="end"><button class="btn text-button suggested-action">Export</button></div></div>
<div class="dialog-body vbox" style="--gap:18px">
<div class="warn-note" style="font-size:var(--fs-body)">{icon("warning")}<div class="vbox" style="--gap:6px"><b>9 redactions use Pixelate</b><span style="font-size:var(--fs-small)">Pixelated text can sometimes be reconstructed. Solid hides it for sure.</span>
<div class="hbox"><button class="btn small text-button" style="padding:2px 10px">Use Solid for All</button></div></div></div>
<div class="boxed-list">
<div class="row entry-row"><div class="text"><span class="title">File name</span><span>Venue chat.png</span></div></div>
<div class="row"><div class="text"><span class="title">Format</span></div><div class="suffix"><div class="toggle-group"><span class="toggle active">PNG</span><span class="toggle">JPG</span><span class="toggle">WebP</span><span class="toggle">PDF</span></div></div></div>
<div class="row disabled" style="opacity:1"><div class="text"><span class="title">Flatten annotations</span><span class="subtitle">Always on: the image contains redactions</span></div><div class="suffix">{icon("pin")}<span class="switch on disabled"></span></div></div>
</div></div></div></div>"""
write(C, "08-export-warning",
      ed(canvas(redactions("pixelate", off=("date",), tags=False)), props("Pixelate", off=("date",), expand=None, total=9, warn=True), overlay=exp),
      "export warning")

# 09 nothing found
none_side = (f'<div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Smart Redact</span>'
             f'<span class="description">0 items found · checked on this computer</span></div></div></div>'
             f'<div class="status-page compact" style="padding:24px 6px;flex:none">{icon("redact", "icon status-icon")}'
             '<div class="status-title" style="font-size:var(--fs-title-4)">Nothing Sensitive Found</div>'
             '<div class="status-description">No e-mail addresses, numbers, keys or faces were detected. '
             'You can still draw redactions with <kbd class="key">B</kbd>.</div>'
             '<button class="btn text-button">Check Again</button></div>')
write(C, "09-nothing-found",
      ed(native("d-tidewater-analytics", SC, 880, 550), none_side, title="Tidewater dashboard", sub="1600 × 1000"),
      "nothing found")

# 10 face detected: profile photo variant (geometric avatar), face category expanded, blur on faces
face_cats = [(k, n, 1 if k == "face" else c) for k, n, c in CATS]
CATS[:] = face_cats
ITEMS["face"] = [("Profile photo · 86 × 86 px", (1408, 26, 86, 86))]
LABEL["face"] = "Face"
av = ov(avatar_svg(1408, 26, 86, "circle", "#bcd0dc"))
face_red = ov('<defs><filter id="fb"><feGaussianBlur stdDeviation="9"/></filter><clipPath id="fc"><circle cx="1451" cy="69" r="47"/></clipPath></defs>'
              '<g clip-path="url(#fc)"><g filter="url(#fb)">' + avatar_svg(1408, 26, 86, "circle", "#bcd0dc").replace('id="av140826circle"', 'id="av2"').replace('#av140826circle', '#av2') + '</g></g>'
              '<circle cx="1451" cy="69" r="50" fill="none" stroke="var(--accent-bg)" stroke-width="3"/>'
              '<g><rect x="1506" y="34" width="56" height="19" rx="9.5" fill="var(--annota-coral)"/><text x="1514" y="48" font-size="13" font-weight="700" fill="#fff" font-family="Inter, sans-serif">Face</text></g>')
side = props("Blur", off=("date",), expand="face", total=10, warn=True).replace(
    '<div class="vbox" style="--gap:6px"><span class="sec-title">Style for all</span>',
    '<div class="vbox" style="--gap:6px"><span class="sec-title">Style for all</span>')
write(C, "10-face-detected",
      ed(canvas(av + redactions("blur", off=("date", "face"), tags=False) + face_red),
         side.replace('<span class="title ellipsize">Profile photo', '<span class="title ellipsize" style="font-family:var(--font-ui)">Profile photo')
         ),
      "face detected")
print("ok")
