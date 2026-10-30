#!/usr/bin/env python3
"""Build Annota symbolic icons (16x16) -> icons/*.svg, icons/sprite.svg and icons.js.

Original drawings for Annota (CC-BY-SA-4.0). Style: 2 px strokes centred on the
1 px grid, round caps and joins, single colour (currentColor).
Run:  python3 docs/design/kit/tools/build-icons.py
"""
import json
import math
import pathlib

KIT = pathlib.Path(__file__).resolve().parent.parent
OUT = KIT / "icons"

S = 'fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"'
F = 'fill="currentColor" stroke="none"'


def star(cx, cy, ro, ri, n=5, rot=-90):
    pts = []
    for i in range(n * 2):
        r = ro if i % 2 == 0 else ri
        a = math.radians(rot + i * 180 / n)
        pts.append(f"{cx + r * math.cos(a):.2f} {cy + r * math.sin(a):.2f}")
    return "M" + " L".join(pts) + " Z"


def gear():
    # 8 teeth, outer r 7, root r 5.2, hole r 2.2 (evenodd)
    pts = []
    n = 8
    for i in range(n):
        base = i * 360 / n
        for da, r in ((-13, 5.4), (-8, 7.2), (8, 7.2), (13, 5.4)):
            a = math.radians(base + da)
            pts.append(f"{8 + r * math.cos(a):.2f} {8 + r * math.sin(a):.2f}")
    outer = "M" + " L".join(pts) + " Z"
    hole = "M8 5.6 a2.4 2.4 0 1 0 0 4.8 a2.4 2.4 0 1 0 0 -4.8 Z"
    return f'<path {F} fill-rule="evenodd" d="{outer} {hole}"/>'


BRACKETS = ("M1.5 5 V2.5 a1 1 0 0 1 1 -1 H5 M11 1.5 h2.5 a1 1 0 0 1 1 1 V5 "
            "M14.5 11 v2.5 a1 1 0 0 1 -1 1 H11 M5 14.5 H2.5 a1 1 0 0 1 -1 -1 V11")
POINTER = "M3.5 1.8 V13.2 L6.4 10.6 L8.4 14.6 L10.2 13.7 L8.3 9.8 L12.2 9.6 Z"

ICONS = {
    # ---- annotation tools
    "select": f'<path {F} stroke="currentColor" stroke-width="1" stroke-linejoin="round" d="{POINTER}"/>',
    "arrow": f'<path {S} d="M2.5 13.5 L12.5 3.5 M6 3 H13 V10"/>',
    "line": f'<path {S} d="M3.5 12.5 L12.5 3.5"/><circle {F} cx="3" cy="13" r="2"/><circle {F} cx="13" cy="3" r="2"/>',
    "pen": f'<path {S} d="M10.5 2.5 L13.5 5.5 L6 13 H3 V10 Z M8.5 4.5 L11.5 7.5"/>',
    "shape": f'<rect {S} x="2" y="6" width="8" height="8" rx="1"/><path {S} d="M8.2 5.2 A3.5 3.5 0 1 1 10.8 9.8"/>',
    "text": f'<path {F} d="M2.5 2 H13.5 V5.5 H11.5 V4 H9 V12 H10.5 V14 H5.5 V12 H7 V4 H4.5 V5.5 H2.5 Z"/>',
    "callout": f'<path {S} d="M3 2 H13 a1 1 0 0 1 1 1 V10 a1 1 0 0 1 -1 1 H8.5 L5 14 V11 H3 a1 1 0 0 1 -1 -1 V3 a1 1 0 0 1 1 -1 Z"/>',
    "step": f'<circle {S} cx="8" cy="8" r="6"/><path {S} d="M6.8 6 L8.3 5 V11"/>',
    "highlighter": f'<path {S} d="M10 2 L14 6 L8.5 11.5 H6 L4.5 10 V7.5 Z M4.5 10 L2.5 12"/><rect {F} x="7" y="13" width="8" height="2.5" rx="1" opacity=".5"/>',
    "blur": "".join(f'<circle {F} cx="{x}" cy="{y}" r="{r}"/>' for x, y, r in [
        (3, 3, 1.6), (8, 3, 1.2), (13, 3, .8), (3, 8, 1.2), (8, 8, 1.6), (13, 8, 1.2),
        (3, 13, .8), (8, 13, 1.2), (13, 13, 1.6)]),
    "crop": f'<path {S} d="M4.5 1 V11.5 H15 M1 4.5 H11.5 V15"/>',
    "magnify": f'<circle {S} cx="6.5" cy="6.5" r="5"/><path {S} stroke-width="2.5" d="M10.5 10.5 L14 14"/><rect {F} x="4.5" y="4.5" width="4" height="4" rx="1"/>',
    "spotlight": f'<path {F} fill-rule="evenodd" d="M2.5 2 H13.5 a1.5 1.5 0 0 1 1.5 1.5 V12.5 a1.5 1.5 0 0 1 -1.5 1.5 H2.5 a1.5 1.5 0 0 1 -1.5 -1.5 V3.5 a1.5 1.5 0 0 1 1.5 -1.5 Z M8 4.5 a3.5 3.5 0 1 0 0 7 a3.5 3.5 0 1 0 0 -7 Z"/>',
    "stamp": f'<circle {S} cx="8" cy="3.8" r="2.3"/><path {S} d="M8 6.5 V9 M3 9 H13 a1 1 0 0 1 1 1 V11.5 H2 V10 a1 1 0 0 1 1 -1 Z M3 14.5 H13"/>',
    "cut-out": f'<circle {S} cx="4" cy="12" r="2"/><circle {S} cx="12" cy="12" r="2"/><path {S} d="M5.4 10.4 L11 1.5 M10.6 10.4 L5 1.5"/>',
    "fill": f'<path {S} d="M2 8 L7 3 L12 8 L7 13 Z M2 8 H12"/><path {F} d="M14 10.5 C14 10.5 12.4 12.6 12.4 13.6 a1.6 1.6 0 0 0 3.2 0 C15.6 12.6 14 10.5 14 10.5 Z"/>',
    "eraser": f'<path {S} d="M9 2.5 L13.5 7 L7.5 13 H5 L2.5 10.5 Z M5.5 6 L10 10.5 M9.5 13 H14"/>',
    "magic-wand": f'<path {S} d="M2 14 L9.5 6.5"/><path {S} stroke-width="1.5" d="M12 1.5 V4.5 M10.5 3 H13.5"/><circle {F} cx="14" cy="8.5" r="1"/><circle {F} cx="7" cy="2" r="1"/><circle {F} cx="11" cy="9.5" r=".8"/>',
    "selection": f'<path {S} d="M2 5 V3 a1 1 0 0 1 1 -1 H5 M11 2 H13 a1 1 0 0 1 1 1 V5 M14 11 V13 a1 1 0 0 1 -1 1 H11 M5 14 H3 a1 1 0 0 1 -1 -1 V11 M7.5 2 H8.5 M7.5 14 H8.5 M2 7.5 V8.5 M14 7.5 V8.5"/>',
    # ---- capture / recording
    "record": f'<circle {F} cx="8" cy="8" r="4"/><circle fill="none" stroke="currentColor" stroke-width="1.5" cx="8" cy="8" r="6.75"/>',
    "stop": f'<rect {F} x="3" y="3" width="10" height="10" rx="2"/>',
    "pause": f'<rect {F} x="3" y="2" width="3.5" height="12" rx="1"/><rect {F} x="9.5" y="2" width="3.5" height="12" rx="1"/>',
    "capture-region": f'<path {S} d="{BRACKETS}"/><rect {F} x="5" y="5.5" width="6" height="5" rx="1"/>',
    "capture-window": f'<rect {S} x="1" y="2" width="14" height="12" rx="2"/><path {F} d="M3 2 H13 a2 2 0 0 1 2 2 V6 H1 V4 a2 2 0 0 1 2 -2 Z"/>',
    "capture-screen": f'<rect {S} x="1" y="2" width="14" height="9" rx="1.5"/><path {S} d="M8 11 V14 M5.5 14 H10.5"/>',
    "timer": f'<circle {S} cx="8" cy="9" r="5.5"/><path {S} d="M8 9 V6 M6.5 1.5 H9.5 M12.6 3.6 L13.6 4.6"/>',
    "camera": f'<path {S} d="M2.5 4.5 H5 L6.5 2.5 H9.5 L11 4.5 H13.5 a1 1 0 0 1 1 1 V12.5 a1 1 0 0 1 -1 1 H2.5 a1 1 0 0 1 -1 -1 V5.5 a1 1 0 0 1 1 -1 Z"/><circle {S} cx="8" cy="8.8" r="2.3"/>',
    "microphone": f'<rect {S} x="5.5" y="1.5" width="5" height="8" rx="2.5"/><path {S} d="M3 7.5 a5 5 0 0 0 10 0 M8 12.5 V14.5"/>',
    "speaker": f'<path {F} d="M1.5 6 a1 1 0 0 1 1 -1 H4.5 L8 2 V14 L4.5 11 H2.5 a1 1 0 0 1 -1 -1 Z"/><path {S} d="M10.5 5.5 a3.5 3.5 0 0 1 0 5 M12.5 3 a7 7 0 0 1 0 10"/>',
    "cursor": f'<path fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" d="M6 4.5 V14 L8.4 11.8 L10 15 L11.5 14.3 L9.9 11.1 L13.2 10.9 Z"/><path {S} stroke-width="1.5" d="M2 4 L3.5 5 M4 1.5 L4.6 3 M1.5 7.5 L3 7.5"/>',
    # ---- library / documents
    "library": f'<rect {S} x="1.5" y="4.5" width="10" height="10" rx="1.5"/><path {S} d="M4.5 1.5 H13 a1.5 1.5 0 0 1 1.5 1.5 V11.5"/><path {F} d="M3 13 L6 9.5 L8 11.5 L9 10.5 L10 11.5 V13 Z"/>',
    "template": f'<rect {S} x="2" y="2" width="12" height="12" rx="1.5"/><path {S} d="M2 6 H14 M6.5 6 V14"/>',
    "image": f'<rect {S} x="1.5" y="2.5" width="13" height="11" rx="1.5"/><circle {F} cx="5.5" cy="6.5" r="1.5"/><path {F} d="M2.5 12.5 L6.5 8.5 L9 11 L10.5 9.5 L13.5 12.5 Z"/>',
    "folder": f'<path {S} d="M2.5 3 H6 L7.5 4.5 H13.5 a1 1 0 0 1 1 1 V12.5 a1 1 0 0 1 -1 1 H2.5 a1 1 0 0 1 -1 -1 V4 a1 1 0 0 1 1 -1 Z"/>',
    "trash": f'<path {S} d="M2 4 H14 M6 4 V2.5 H10 V4 M3.5 4 L4.3 13.6 a1 1 0 0 0 1 .9 H10.7 a1 1 0 0 0 1 -.9 L12.5 4"/>',
    "tag": f'<path {S} d="M2 2.5 a.5 .5 0 0 1 .5 -.5 H7.5 L14 8.5 L8.5 14 L2 7.5 Z"/><circle {F} cx="5.2" cy="5.2" r="1.3"/>',
    "star": f'<path {S} stroke-width="1.8" d="{star(8, 8.6, 6.6, 2.9)}"/>',
    "grid-view": "".join(f'<rect {F} x="{x}" y="{y}" width="5.5" height="5.5" rx="1.2"/>' for x in (1.5, 9) for y in (1.5, 9)),
    "list-view": "".join(f'<rect {F} x="1.5" y="{y}" width="2.5" height="2.5" rx=".8"/><rect {F} x="6" y="{y + .25}" width="8.5" height="2" rx="1"/>' for y in (2, 6.75, 11.5)),
    # ---- actions
    "undo": f'<path {S} d="M5.5 2.5 L2 6 L5.5 9.5 M2 6 H9.5 a4 4 0 0 1 0 8 H7"/>',
    "redo": f'<path {S} d="M10.5 2.5 L14 6 L10.5 9.5 M14 6 H6.5 a4 4 0 0 0 0 8 H9"/>',
    "zoom-in": f'<circle {S} cx="7" cy="7" r="5"/><path {S} d="M10.7 10.7 L14 14 M5 7 H9 M7 5 V9"/>',
    "zoom-out": f'<circle {S} cx="7" cy="7" r="5"/><path {S} d="M10.7 10.7 L14 14 M5 7 H9"/>',
    "search": f'<circle {S} cx="7" cy="7" r="5"/><path {S} d="M10.7 10.7 L14 14"/>',
    "copy": f'<rect {S} x="5.5" y="5.5" width="9" height="9" rx="1.5"/><path {S} d="M2 10.5 V3 a1 1 0 0 1 1 -1 H10.5"/>',
    "share": f'<circle {S} cx="12" cy="3.5" r="2"/><circle {S} cx="4" cy="8" r="2"/><circle {S} cx="12" cy="12.5" r="2"/><path {S} stroke-width="1.6" d="M5.8 7 L10.2 4.5 M5.8 9 L10.2 11.5"/>',
    "export": f'<path {S} d="M8.5 2 H3 a1 1 0 0 0 -1 1 V13 a1 1 0 0 0 1 1 H13 a1 1 0 0 0 1 -1 V7.5 M8 8 L14 2 M10 2 H14 V6"/>',
    "save": f'<path {S} d="M8 1.5 V9.5 M4.5 6.5 L8 10 L11.5 6.5 M2 11 V13 a1 1 0 0 0 1 1 H13 a1 1 0 0 0 1 -1 V11"/>',
    "open-menu": f'<path {S} d="M2 4 H14 M2 8 H14 M2 12 H14"/>',
    "view-more": "".join(f'<circle {F} cx="8" cy="{y}" r="1.6"/>' for y in (3, 8, 13)),
    "effects": f'<path {F} d="M7 1.5 C7.5 5.3 8.8 6.6 12.5 7 C8.8 7.4 7.5 8.7 7 12.5 C6.5 8.7 5.2 7.4 1.5 7 C5.2 6.6 6.5 5.3 7 1.5 Z"/><path {F} d="M12.5 10 C12.7 11.6 13.2 12.1 14.8 12.3 C13.2 12.5 12.7 13 12.5 14.6 C12.3 13 11.8 12.5 10.2 12.3 C11.8 12.1 12.3 11.6 12.5 10 Z"/>',
    "ocr-text": f'<path {S} d="{BRACKETS}"/><path {S} d="M5 6 H11 M5 10 H9"/>',
    "redact": f'<rect {F} x="1" y="2" width="14" height="5" rx="1.2"/><path {S} stroke-width="1.6" d="M1.8 10.5 H14.2 M1.8 14 H9"/>',
    "settings": gear(),
    "keyboard": f'<rect {S} x="1" y="3" width="14" height="10" rx="1.5"/>' + "".join(
        f'<rect {F} x="{x - .75}" y="{y - .75}" width="1.5" height="1.5" rx=".3"/>' for x in (4, 6.67, 9.33, 12) for y in (6,)) +
        f'<rect {F} x="3.25" y="8.25" width="1.5" height="1.5" rx=".3"/><rect {F} x="11.25" y="8.25" width="1.5" height="1.5" rx=".3"/><rect {F} x="6" y="8.4" width="4" height="1.3" rx=".5"/>',
    "plus": f'<path {S} d="M8 2.5 V13.5 M2.5 8 H13.5"/>',
    "minus": f'<path {S} d="M2.5 8 H13.5"/>',
    "close": f'<path {S} d="M4 4 L12 12 M12 4 L4 12"/>',
    "check": f'<path {S} d="M2.5 8.5 L6 12 L13.5 4"/>',
    "warning": f'<path {F} fill-rule="evenodd" d="M6.9 2.1 a1.25 1.25 0 0 1 2.2 0 L15 12.9 a1.25 1.25 0 0 1 -1.1 1.85 H2.1 A1.25 1.25 0 0 1 1 12.9 Z M7 5.5 H9 L8.75 10 H7.25 Z M8 11 a1.1 1.1 0 1 0 0 2.2 a1.1 1.1 0 1 0 0 -2.2 Z"/>',
    "info": f'<path {F} fill-rule="evenodd" d="M8 1 a7 7 0 1 0 0 14 a7 7 0 1 0 0 -14 Z M7 7 H9 V12 H7 Z M8 3.6 a1.1 1.1 0 1 0 0 2.2 a1.1 1.1 0 1 0 0 -2.2 Z"/>',
    # ---- navigation / misc
    "go-previous": f'<path {S} d="M10 3 L5 8 L10 13"/>',
    "go-next": f'<path {S} d="M6 3 L11 8 L6 13"/>',
    "pan-down": f'<path {S} d="M4 6 L8 10 L12 6"/>',
    "sidebar-show": f'<rect {S} x="1" y="2" width="14" height="12" rx="2"/><path {F} d="M3 2 H6 V14 H3 a2 2 0 0 1 -2 -2 V4 a2 2 0 0 1 2 -2 Z"/>',
    "sidebar-show-right": f'<rect {S} x="1" y="2" width="14" height="12" rx="2"/><path {F} d="M10 2 H13 a2 2 0 0 1 2 2 V12 a2 2 0 0 1 -2 2 H10 Z"/>',
    "eyedropper": f'<path {S} d="M9 4 L12 7 M10.2 2.8 a1.9 1.9 0 0 1 3 3 L11.5 7.5 L8.5 4.5 Z M9.5 5.5 L3 12 V13 H4 L10.5 6.5"/>',
    "delete-text": f'<path {S} d="M5.5 3 H13.5 a1 1 0 0 1 1 1 V12 a1 1 0 0 1 -1 1 H5.5 L1.5 8 Z M7.5 6 L11.5 10 M11.5 6 L7.5 10"/>',
    "drag-handle": "".join(f'<circle {F} cx="{x}" cy="{y}" r="1.3"/>' for x in (5.5, 10.5) for y in (3.5, 8, 12.5)),
    "pin": f'<path {S} d="M6 2 H10 M7 2 V6.5 L4.5 9.5 H11.5 L9 6.5 V2 M8 9.5 V14.5"/>',
    "play": f'<path {F} d="M4 2.8 a1 1 0 0 1 1.5 -.86 L13.5 7.1 a1 1 0 0 1 0 1.73 L5.5 14.06 A1 1 0 0 1 4 13.2 Z"/>',
    "edit-clear": f'<path {F} fill-rule="evenodd" d="M8 1 a7 7 0 1 0 0 14 a7 7 0 1 0 0 -14 Z M5.2 4.5 L8 7.3 L10.8 4.5 L11.5 5.2 L8.7 8 L11.5 10.8 L10.8 11.5 L8 8.7 L5.2 11.5 L4.5 10.8 L7.3 8 L4.5 5.2 Z"/>',
}


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    for old in OUT.glob("*.svg"):
        old.unlink()
    symbols = []
    for name, body in ICONS.items():
        (OUT / f"{name}.svg").write_text(
            f'<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16">{body}</svg>\n')
        symbols.append(f'<symbol id="i-{name}" viewBox="0 0 16 16">{body}</symbol>')
    sprite = ('<svg xmlns="http://www.w3.org/2000/svg" style="display:none">\n'
              + "\n".join(symbols) + "\n</svg>\n")
    (OUT / "sprite.svg").write_text(sprite)
    js = f"""/* Annota design kit — icons.js (generated by tools/build-icons.py, do not edit)
 * Chromium blocks external <use href="file.svg#id"> on file:// pages, so this script
 * inlines the sprite into every page. Use: <svg class="icon"><use href="#i-arrow"/></svg>
 * (hrefs written as ".../sprite.svg#i-arrow" are rewritten to "#i-arrow" automatically).
 */
(function () {{
  var SPRITE = {json.dumps(sprite)};
  window.ANNOTA_ICONS = {json.dumps(list(ICONS))};
  function inject() {{
    if (document.getElementById("annota-sprite")) return;
    var holder = document.createElement("div");
    holder.id = "annota-sprite";
    holder.style.cssText = "position:absolute;width:0;height:0;overflow:hidden";
    holder.innerHTML = SPRITE;
    document.body.insertBefore(holder, document.body.firstChild);
    var uses = document.querySelectorAll("use");
    for (var i = 0; i < uses.length; i++) {{
      var h = uses[i].getAttribute("href") || uses[i].getAttribute("xlink:href") || "";
      var k = h.indexOf("#");
      if (k > 0) uses[i].setAttribute("href", h.slice(k));
    }}
    document.documentElement.dataset.iconsReady = "1";
  }}
  if (document.body) inject(); else document.addEventListener("DOMContentLoaded", inject);
}})();
"""
    (KIT / "icons.js").write_text(js)
    print(f"{len(ICONS)} icons written to {OUT}")


if __name__ == "__main__":
    main()
