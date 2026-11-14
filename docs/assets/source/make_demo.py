import json, zipfile, sys, io
from PIL import Image
D = sys.argv[1]
B = json.load(open(f"{D}/boxes.json"))
FONT = "Cantarell"
RED = "#e01b24ff"; WHITE = "#ffffffff"; NONE = "#00000000"
SH0 = {"enabled": False, "color": "#000000ff", "opacity": 0.3, "angle": 90, "distance": 3, "blur": 4}
SH1 = dict(SH0, enabled=True)
objs = []; nid = [0]
def add(o):
    nid[0] += 1; o["id"] = nid[0]; objs.append(o)
def pad(b, p):
    return b["x"] - p, b["y"] - p, b["width"] + 2 * p, b["height"] + 2 * p

def redact(b):
    x, y, w, h = pad(b, 5)
    add({"type": "redaction", "kind": "pixelate", "intensity": 70, "shape": "rectangle",
         "x": x, "y": y, "width": w, "height": h, "rotation": 0, "color": "#000000ff"})

def step(n, cx, cy):
    add({"type": "step", "x": cx, "y": cy, "radius": 17, "shape": "circle", "color": RED,
         "label_color": WHITE, "ring_color": WHITE, "ring_width": 2.5, "sequence": 0, "number": n,
         "counting": "numbers", "value": None, "opacity": 1, "shadow": SH1})

d = B["days"]; x, y, w, h = pad(d, 3)
add({"type": "highlighter", "x": x, "y": y, "width": w, "height": h, "rotation": 0, "color": "#f6d32dff", "opacity": 0.55})
redact(B["mail"]); redact(B["key"])
a = B["auto"]; f = B["freq"]; s = B["save"]
step(1, a["x"] - 26, a["y"] + a["height"] / 2)
step(2, f["x"] - 28, f["y"] + f["height"] / 2)
step(3, s["x"] + s["width"] + 26, s["y"] + s["height"] / 2)
add({"type": "callout", "text": "Turn this on", "x": 1104, "y": 286, "width": 160, "height": 52, "rotation": 0,
     "shape": "rounded_rectangle", "tails": [{"x": a["x"] + a["width"] + 6, "y": a["y"] + a["height"] / 2}],
     "fill": RED, "outline_color": NONE, "outline_width": 0, "padding": 10, "corner_radius": 10,
     "font_family": FONT, "font_size": 18, "bold": True, "text_color": WHITE, "alignment": "center",
     "opacity": 1, "shadow": SH1})
st = {"x": 1186, "y": 520}; en = {"x": f["x"] + f["width"] + 8, "y": f["y"] + f["height"] / 2 + 4}
add({"type": "arrow", "start": st, "end": en, "middle": {"x": (st["x"] + en["x"]) / 2 + 22, "y": (st["y"] + en["y"]) / 2 + 14},
     "color": RED, "width": 5, "start_style": "none", "end_style": "triangle", "start_size": 3, "end_size": 3,
     "line_style": "solid", "opacity": 1, "shadow": SH0})
add({"type": "text", "text": "Daily is safer", "x": 1112, "y": 528, "rotation": 0, "font_family": FONT, "font_size": 18,
     "bold": True, "italic": False, "underline": False, "strikethrough": False, "alignment": "left",
     "color": RED, "outline_color": NONE, "outline_width": 0, "opacity": 1, "shadow": SH0})

img = Image.open(f"{D}/base.png"); W, H = img.size
doc = {"format": "1.0", "requires": "0.1", "generator": "showcase demo",
       "image": {"file": "original.png", "width": W, "height": H, "corner_radius": 0},
       "canvas": {"x": 0, "y": 0, "width": W, "height": H, "background": {"type": "transparent"}},
       "metadata": {"title": "Backup settings", "scale_factor": 1}, "objects": objs}
th = img.copy(); th.thumbnail((256, 256)); tb = io.BytesIO(); th.save(tb, "PNG")
with zipfile.ZipFile(f"{D}/backup-settings.annota", "w", zipfile.ZIP_DEFLATED) as z:
    z.write(f"{D}/base.png", "original.png")
    z.writestr("document.json", json.dumps(doc, indent=1))
    z.writestr("thumbnail.png", tb.getvalue())
print("ok", len(objs), "objects")
