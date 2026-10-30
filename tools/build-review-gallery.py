#!/usr/bin/env python3
"""Build a single-file design review gallery from all mockups.

Usage:  python3 tools/build-review-gallery.py [-o docs/design/review.html]

Reads docs/design/<change>/png/*-light@1x.png, converts them to WebP (1280 px max),
embeds them in tools/design-gen/review-gallery/template.html and writes one HTML file
(~11 MB) that works offline. Requires: pip install pillow
The output is git-ignored; rebuild it whenever mockups change.
"""
import argparse
import base64
import glob
import io
import json
import os
import pathlib
import re

from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parent.parent
TEMPLATE = ROOT / "tools/design-gen/review-gallery/template.html"

# Roadmap order and German review titles (phase: MVP, P2, P3).
ORDER = [
    ("bootstrap-app-shell", "App-Grundgerüst & Paket", "MVP"),
    ("add-screen-capture", "Bildschirm aufnehmen", "MVP"),
    ("add-annotation-editor", "Editor & Dateiformat", "MVP"),
    ("add-core-annotation-tools", "Kern-Werkzeuge", "MVP"),
    ("add-style-system", "Stilvorlagen & Themes", "MVP"),
    ("add-export-share", "Export & Teilen", "MVP"),
    ("add-capture-library", "Bibliothek", "MVP"),
    ("add-advanced-annotation-tools", "Erweiterte Werkzeuge", "P2"),
    ("add-image-effects", "Bildeffekte", "P2"),
    ("add-capture-presets", "Aufnahme-Vorlagen", "P2"),
    ("add-text-recognition", "Texterkennung", "P2"),
    ("add-smart-redaction", "Automatisch schwärzen", "P2"),
    ("add-library-organisation", "Bibliothek: Suche, Tags, Sammlungen", "P2"),
    ("add-templates", "Seitenvorlagen", "P2"),
    ("add-screen-recording", "Bildschirmaufnahme (Video)", "P2"),
    ("add-scrolling-capture", "Scrollende Aufnahme", "P3"),
    ("add-step-capture", "Schritt-Aufnahme", "P3"),
    ("add-smart-editing", "Smart Move & Vereinfachen", "P3"),
]


def webp_data_uri(path: str):
    im = Image.open(path).convert("RGB")
    if im.width > 1280:
        im = im.resize((1280, round(im.height * 1280 / im.width)))
    buf = io.BytesIO()
    im.save(buf, "WEBP", quality=74, method=6)
    return "data:image/webp;base64," + base64.b64encode(buf.getvalue()).decode(), im.size


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("-o", "--output", default=str(ROOT / "docs/design/review.html"))
    args = ap.parse_args()

    data, images = [], {}
    for cid, title, phase in ORDER:
        shots = []
        for png in sorted(glob.glob(str(ROOT / f"docs/design/{cid}/png/*-light@1x.png"))):
            name = os.path.basename(png).replace("-light@1x.png", "")
            key = f"{cid}--{name}.webp"
            images[key], (w, h) = webp_data_uri(png)
            shots.append([name, key, w, h])
        data.append({"id": cid, "t": title, "p": phase, "s": shots})

    html = TEMPLATE.read_text(encoding="utf-8")
    html = html.replace("/*DATA*/[]", json.dumps(data, ensure_ascii=False))
    html = html.replace("var DATA=", "var IMG=" + json.dumps(images) + ";\n  var DATA=", 1)
    html = html.replace("src=\"img/'+s[1]+'\"", "src=\"'+IMG[s[1]]+'\"")
    html = html.replace("img.src='img/'+f.s[1]", "img.src=IMG[f.s[1]]")
    if not re.search(r"IMG\[", html):
        raise SystemExit("template placeholders not found")
    pathlib.Path(args.output).write_text(html, encoding="utf-8")
    total = sum(len(d["s"]) for d in data)
    print(f"{args.output}: {total} screens, {os.path.getsize(args.output) / 1e6:.1f} MB")


if __name__ == "__main__":
    main()
