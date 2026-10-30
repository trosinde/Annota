#!/usr/bin/env python3
"""Render Annota design mockups (HTML) to PNG in light and dark style at 1x and 2x.

Usage:  python3 tools/render-mockups.py docs/design/<change-id> [--only 01-editor]

Each docs/design/<id>/src/*.html is rendered to docs/design/<id>/png/<name>-<theme>@<scale>x.png.
A mockup may declare its viewport with <meta name="viewport-size" content="1280x800">.
Dark style is applied by setting data-theme="dark" on <html> and prefers-color-scheme: dark.
Requires: pip install playwright && playwright install chromium
"""
import argparse
import pathlib
import re
import sys

from playwright.sync_api import sync_playwright

DEFAULT_SIZE = (1280, 800)


def viewport_of(html: str):
    m = re.search(r'name="viewport-size"\s+content="(\d+)x(\d+)"', html)
    return (int(m.group(1)), int(m.group(2))) if m else DEFAULT_SIZE


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("design_dir")
    ap.add_argument("--only", help="render only files whose name starts with this prefix")
    args = ap.parse_args()

    base = pathlib.Path(args.design_dir).resolve()
    src, out = base / "src", base / "png"
    files = sorted(p for p in src.glob("*.html") if not args.only or p.name.startswith(args.only))
    if not files:
        print(f"no mockups found in {src}", file=sys.stderr)
        return 1
    out.mkdir(parents=True, exist_ok=True)

    with sync_playwright() as pw:
        browser = pw.chromium.launch()
        for f in files:
            w, h = viewport_of(f.read_text(encoding="utf-8"))
            for theme in ("light", "dark"):
                for scale in (1, 2):
                    page = browser.new_page(viewport={"width": w, "height": h},
                                            device_scale_factor=scale, color_scheme=theme)
                    page.goto(f.as_uri())
                    page.evaluate("t => document.documentElement.dataset.theme = t", theme)
                    page.wait_for_load_state("networkidle")
                    target = out / f"{f.stem}-{theme}@{scale}x.png"
                    page.screenshot(path=str(target), full_page=False)
                    page.close()
                    print(target.relative_to(base.parent.parent.parent))
        browser.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
