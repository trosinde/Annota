"""Build add-capture-library mockups.  python3 build.py"""
from mock import *

CH = "add-capture-library"
IT = ITEMS()
B = by_key(IT)

def lib_window(content_html, side=None, pv=None, overlays="", w=1280, h=800):
    return window((side if side is not None else sidebar_mvp()) + content_html + (pv or ""), w, h, overlays)

def grid_header(mode="grid", preview_on=False, sort="Capture Date", title="All Captures", sub="31 captures", zoom=1, start=None, end_extra="", sort_cls=""):
    return headerbar(start or "", title, sub, end=content_end(mode, zoom, sort, preview_on, extra=end_extra, sort_cls=sort_cls), close=not preview_on)

# 01 grid default
body = lib_window(content(grid_header(), grid(IT, states={"t1": {"hover"}})))
write(CH, "01-library-grid", page("Library", body))
