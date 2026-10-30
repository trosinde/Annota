#!/usr/bin/env python3
"""Render kit sample screens and the component showcase to PNG.

Usage:  python3 docs/design/kit/tools/render-samples.py
  sample-content/*.html -> sample-content/png/<name>.png (1600x1000; f-* full page 1600x2600)
  components.html       -> png/components-light@1x.png, png/components-dark@1x.png
"""
import pathlib
import re

from playwright.sync_api import sync_playwright

KIT = pathlib.Path(__file__).resolve().parent.parent


def size_of(path):
    m = re.search(r'name="viewport-size"\s+content="(\d+)x(\d+)"', path.read_text(encoding="utf-8"))
    return (int(m.group(1)), int(m.group(2))) if m else (1600, 1000)


def settle(page):
    page.wait_for_load_state("networkidle")
    page.evaluate("document.fonts.ready.then(() => true)")
    page.wait_for_timeout(150)


def main():
    with sync_playwright() as pw:
        b = pw.chromium.launch()
        out = KIT / "sample-content" / "png"
        out.mkdir(parents=True, exist_ok=True)
        for f in sorted((KIT / "sample-content").glob("*.html")):
            w, h = size_of(f)
            page = b.new_page(viewport={"width": w, "height": min(h, 1000)}, color_scheme="light")
            page.goto(f.as_uri())
            settle(page)
            page.screenshot(path=str(out / f"{f.stem}.png"), full_page=h > 1000,
                            clip=None if h > 1000 else {"x": 0, "y": 0, "width": w, "height": h})
            page.close()
            print(out / f"{f.stem}.png")
        comp = KIT / "components.html"
        if comp.exists():
            (KIT / "png").mkdir(exist_ok=True)
            for theme in ("light", "dark"):
                page = b.new_page(viewport={"width": 1440, "height": 900}, color_scheme=theme)
                page.goto(comp.as_uri())
                page.evaluate("t => document.documentElement.dataset.theme = t", theme)
                settle(page)
                target = KIT / "png" / f"components-{theme}@1x.png"
                page.screenshot(path=str(target), full_page=True)
                page.close()
                print(target)
        b.close()


if __name__ == "__main__":
    main()
