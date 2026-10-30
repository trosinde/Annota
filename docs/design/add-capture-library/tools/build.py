"""Build add-capture-library mockups:  cd docs/design/add-capture-library/tools && python3 build.py"""
from mock import *

CH = "add-capture-library"
IT = ITEMS()
B = by_key(IT)


def lib_window(content_html, side=None, pv="", overlays="", w=1280, h=800):
    return window((side if side is not None else sidebar_mvp()) + content_html + pv, w, h, overlays)


def gh(mode="grid", preview_on=False, sort="Capture Date", title="All Captures", sub="31 captures", zoom=1,
       start="", end_extra="", sort_cls=""):
    return headerbar(start or content_start(mode, zoom), title, sub, end=content_end(mode, zoom, sort, preview_on, extra=end_extra, sort_cls=sort_cls),
                     close=not preview_on)


def W(name, title, body, size=(1280, 800), flush=True, css=""):
    write(CH, name, page(title, body, size, flush, css))


# ---------------------------------------------------------------- 01 grid
W("01-library-grid", "Library",
  lib_window(content(gh(), grid(IT, states={"t1": {"hover"}, "y3": {"focus"}}))))

# ---------------------------------------------------------------- 02 grid + preview (window capture metadata)
W("02-library-preview", "Library — preview",
  lib_window(content(gh(preview_on=True), grid(IT, tw=150, states={"t2": {"selected"}})),
             pv=preview_pane(B["t2"])))

# ---------------------------------------------------------------- 03 list view
def list_view(items, sort_col="date", desc=True, sel=(), group=True, rows_limit=None):
    cols = [("Name", "name", "38%"), ("Captured", "date", "16%"), ("Application", "app", "16%"),
            ("Size", "size", "10%"), ("Type", "type", "12%"), ("Dimensions", "dim", "")]
    th = []
    for lab, k, w in cols:
        arrow = ic("pan-down") if k == sort_col else ""
        st = f' style="width:{w}"' if w else ""
        th.append(f'<th class="{"sorted" if k == sort_col else ""}"{st}>{lab}{arrow}</th>')
    rows = []
    groups = DAYS if group else [(None, None, None)]
    n = 0
    for gk, lab, sub in groups:
        its = [i for i in items if (gk is None or i.day == gk)]
        if not its:
            continue
        if gk is not None:
            rows.append(f'<tr class="grp"><td colspan="6">{esc(lab)}<span class="caption">{esc(sub)}</span></td></tr>')
        for i in its:
            if rows_limit and n >= rows_limit:
                break
            n += 1
            cls = "sel" if i.key in sel else ""
            dname = f"{dayname(i.day)}, {i.time}" if not group else i.time
            kind = i.kind + (" · " + i.dur if i.dur else "")
            rows.append(f'<tr class="{cls}"><td><div class="namecell"><span class="tbox">{timg(i, 40, 28, svg=False)}</span>'
                        f'<span class="ellipsize">{esc(i.name)}</span></div></td><td class="dim">{dname}</td>'
                        f'<td>{esc(i.app)}</td><td class="num">{i.fsize}</td><td class="dim">{kind}</td><td class="dim num">{i.px}</td></tr>')
    return (f'<div class="lv"><table><thead><tr>{"".join(th)}</tr></thead><tbody>{"".join(rows)}</tbody></table>'
            f'<span class="scrollbar" style="top:40px;height:22%"></span></div>')


W("03-library-list", "Library — list",
  lib_window(content(gh("list"), list_view(IT, sel=("t4",)))))


# ---------------------------------------------------------------- 04 sort menu (sort by size, descending)
def size_kb(i):
    s = i.fsize
    return float(s.split()[0]) * (1024 if "MB" in s else 1)


by_size = sorted(IT, key=size_kb, reverse=True)
sort_menu = menu([("#", "Sort By"), ("radio", "Capture Date", False), ("radio", "Modified Date", False),
                  ("radio", "Name", False), ("radio", "File Size", True, "hover"), ("radio", "Application", False), "sep",
                  ("radio", "Ascending", False), ("radio", "Descending", True)], 990, 48, 230, arrow_x=150)
W("04-sort-menu", "Library — sort",
  lib_window(content(gh("list", sort="File Size", sort_cls="active"), list_view(by_size, "size", group=False)),
             overlays=sort_menu))

# ---------------------------------------------------------------- 05 large thumbnails (zoom step 3)
W("05-thumbnail-zoom-large", "Library — large thumbnails",
  lib_window(content(gh(zoom=2), grid(IT, tw=230, th=168, scroll=(0.0, 0.2)))))

# ---------------------------------------------------------------- 06 quick look
ql_item = B["t1"]
ql = (f'<div class="ql"><div class="ql-top"><div class="grow vbox" style="--gap:0"><span class="t">{esc(ql_item.name)}</span>'
      f'<span class="s">Today, 14:22 · Firefox · {ql_item.px} · 1 of 31</span></div>'
      f'<button class="btn osd text-button">Open in Editor</button><button class="btn osd circular">{ic("close")}</button></div>'
      f'<div class="ql-img">{timg(ql_item, 1080, 560)}'
      f'<button class="btn osd circular nav" style="left:18px">{ic("go-previous")}</button>'
      f'<button class="btn osd circular nav" style="right:18px">{ic("go-next")}</button></div>'
      f'<div class="ql-hint"><span><kbd class="key">Space</kbd> Close</span><span><kbd class="key">←</kbd> <kbd class="key">→</kbd> Previous / next</span>'
      f'<span><kbd class="key">Enter</kbd> Open in editor</span></div></div>')
W("06-quick-look", "Library — quick look",
  lib_window(content(gh(), grid(IT, states={"t1": {"selected"}})), overlays=ql))

# ---------------------------------------------------------------- 07 multi-select + action bar
sel5 = ["t1", "t4", "t7", "y1", "y3"]
st = {k: {"multi"} for k in B}
for k in sel5:
    st[k] = {"multi", "selected"}
bar = (f'<div class="action-bar"><span class="count">5 selected</span>'
       f'<button class="btn flat text-button">Select All</button><span class="spacer"></span>'
       f'<button class="btn text-button">Open</button>'
       f'<button class="btn" style="padding:5px 14px">{ic("export")}Export…</button>'
       f'<button class="btn" style="padding:5px 14px">{ic("folder")}Copy to Folder…</button>'
       f'<button class="btn" style="padding:5px 14px;color:var(--destructive)">{ic("trash")}Move to Trash</button></div>')
sel_header = headerbar(f'<button class="btn text-button">Cancel</button>', "5 Selected", "of 31 captures",
                       end=sort_btn() + btn("sidebar-show-right", cls="flat"), close=True)
W("07-multi-select", "Library — multi-select",
  lib_window(content(sel_header, grid(IT, states=st) + bar)))

# ---------------------------------------------------------------- 08 batch export dialog (5 items JPG)
strip = "".join(f'<span style="width:72px;height:52px;display:grid;place-items:center;border-radius:6px;background:var(--fill-1)">{timg(B[k], 64, 44)}</span>' for k in sel5)
dlg = f"""<div class="dialog-host"><div class="dialog" style="width:560px;max-width:560px">
  {headerbar('<button class="btn text-button">Cancel</button>', "Export 5 Captures", end='<button class="btn text-button suggested-action">Export</button>', cls="")}
  <div class="dialog-body vbox" style="--gap:18px">
    <div class="hbox" style="--gap:6px;justify-content:center">{strip}</div>
    <div class="boxed-list">
      <div class="row"><div class="text"><span class="title">Format</span></div><div class="suffix"><div class="toggle-group"><span class="toggle">PNG</span><span class="toggle active">JPG</span><span class="toggle">WebP</span><span class="toggle">PDF</span></div></div></div>
      <div class="row"><div class="text"><span class="title">Quality</span></div><div class="suffix" style="width:220px"><div class="scale grow"><div class="trough"><div class="fill" style="width:85%"></div><div class="knob" style="left:85%"></div></div><span class="value">85</span></div></div></div>
      <div class="row"><div class="text"><span class="title">Folder</span><span class="subtitle mono-path">~/Pictures/Annota/Exports</span></div><div class="suffix">{btn("folder", cls="flat")}</div></div>
      <div class="row entry-row focus"><div class="text"><span class="title">File name pattern</span><span>{{name}}-{{n}}<span class="caret" style="margin-left:1px;vertical-align:-3px"></span></span></div></div>
    </div>
    <div class="vbox" style="--gap:4px;padding:0 4px"><span class="caption-heading dim-label">Preview</span>
      <span class="caption monospace">Checkout postcode error-1.jpg</span><span class="caption monospace">KPI row September-2.jpg</span>
      <span class="caption monospace dim-label">… 3 more · annotations are flattened</span></div>
  </div></div></div>"""
W("08-bulk-export-dialog", "Library — export 5",
  lib_window(content(sel_header, grid(IT, states=st) + bar), overlays=dlg))

# ---------------------------------------------------------------- 09 item context menu
ctx = menu([("Open in Editor", "Enter"), ("Quick Look", "Space"), "sep",
            ("Rename…", "F2"), ("Duplicate", "Ctrl+D"), ("Copy Image", "Ctrl+C"), ("Export…", "Ctrl+E"),
            ("Show in Files", ""), "sep", ("Move to Trash", "Del")], 520, 214, 250, arrow_x=24)
W("09-item-context-menu", "Library — context menu",
  lib_window(content(gh(), grid(IT, states={"t3": {"selected"}})), overlays=ctx))

# ---------------------------------------------------------------- 10 rename inline (F2)
ren = ('<span class="entry focus" style="width:100%;min-height:28px;padding:0 6px;font-size:13px;margin-top:-4px">'
       '<span style="background:var(--accent-bg);color:var(--accent-fg);border-radius:2px">KPI row September</span><span class="caret" style="height:15px;margin-left:0"></span></span>')
W("10-rename-inline", "Library — rename",
  lib_window(content(gh(preview_on=True), grid(IT, states={"t4": {"selected", "rename"}}, per_tile={"t4": {"extra": ren}})),
             pv=preview_pane(B["t4"])))

# ---------------------------------------------------------------- 11 delete with undo toast
IT_del = [i for i in IT if i.key != "t3"]
W("11-delete-undo-toast", "Library — deleted",
  lib_window(content(gh(sub="30 captures"), grid(IT_del, states={"t4": {"selected"}})),
             side=sidebar_mvp(counts={"all": "30", "trash": "3"}),
             overlays=toast("“2026-10-30 13-41-07” moved to Trash", "Undo")))

# ---------------------------------------------------------------- 12 drag out to another app
relay = B["t5"]
drag_css = ".mock-frame{align-items:flex-start;justify-content:flex-start}"
relay_win = (f'<div class="window" style="position:absolute;left:640px;top:300px;width:600px;height:460px">'
             f'{headerbar("", "Relay", "Jonas Okafor", close=True)}'
             f'<div style="flex:1;position:relative;background:#f6f5fb;overflow:hidden" data-theme="light">'
             f'<span class="timg" style="position:absolute;left:-251px;top:-44px;width:1000px;height:625px;background-image:url({K}sample-content/png/e-relay-chat.png);background-size:1000px 625px"></span>'
             f'<div style="position:absolute;left:16px;right:16px;bottom:14px;height:96px;border-radius:12px;background:var(--accent-a12);outline:2px dashed var(--accent-bg);outline-offset:-2px;display:grid;place-items:center;color:var(--accent);font-weight:700">Drop to attach</div></div></div>')
lib_small = (f'<div class="window" style="position:absolute;left:24px;top:24px;width:900px;height:560px">'
             f'<div class="split-view">{sidebar_mvp()}{content(gh(), grid(IT, tw=150, states={"t1": {"selected"}}))}</div></div>')
ghost = (f'<div class="ghost" style="left:700px;top:528px"><div class="stack">{timg(B["t1"], 120, 76)}</div></div>'
         f'<svg class="cursor-arrow" style="left:760px;top:580px" viewBox="0 0 20 20"><path d="M3 2 V16 L6.6 12.8 L9 18 L11.4 17 L9 11.8 H14 Z" fill="#fff" stroke="#000" stroke-width="1.2" stroke-linejoin="round"/></svg>')
W("12-drag-out", "Library — drag out",
  f'<div style="position:relative;width:1280px;height:800px">{lib_small}{relay_win}{ghost}</div>', flush=False, css=drag_css)

# ---------------------------------------------------------------- 13 empty state
empty = f"""<div class="status-page">
  <svg class="icon status-icon"><use href="#i-library"/></svg>
  <div class="status-title">No Captures Yet</div>
  <div class="status-description">Press <kbd class="key">Shift</kbd> <kbd class="key">Print</kbd> anywhere to capture a region.
  Every capture is kept here automatically, in <span class="monospace" style="font-size:13px">~/Pictures/Annota</span>.</div>
  <button class="btn pill suggested-action">Take Capture</button>
  <button class="btn flat text-button" style="margin-top:12px">Change Library Folder…</button>
</div>"""
W("13-empty-state", "Library — empty",
  lib_window(content(headerbar("", "All Captures", end="", close=True), empty),
             side=sidebar_mvp(counts={"all": "", "trash": ""}, foot=side_foot("~/Pictures/Annota", "Local folder · empty"))))

# ---------------------------------------------------------------- 14 NAS offline
off_banner = ('<div class="banner warning"><span class="banner-title">Library server not reachable — showing the last known captures, read-only. '
              'New captures are kept on this computer.</span><button class="btn">Retry</button></div>')
fallback_items = [B["t1"], B["t4"], B["t7"]]
fb_head = ('<div class="day-head" style="padding-top:18px"><span class="heading">Waiting for Library Server</span>'
           '<span class="caption">3 captures in ~/.local/share/annota/pending · moved automatically when /mnt/nas/annota is back</span></div>')
fb_tiles = '<div class="tiles">' + "".join(tile(i, ()) for i in fallback_items) + '</div>'
rest = [i for i in IT if i.key not in ("t1", "t4", "t7")]
for i in rest:
    pass
off_grid = (grid(rest, groups=[("today", "Today", "Friday, 30 October"), ("yesterday", "Yesterday", "Thursday, 29 October"), ("wed", "Wednesday", "28 October")])
            .replace('<div class="grid-scroll" style="padding-top:0px">', '<div class="grid-scroll" style="padding-top:0px">' + fb_head + fb_tiles, 1))
W("14-nas-offline", "Library — server offline",
  lib_window(content(gh(sub="28 captures · read-only"), off_banner + off_grid),
             side=sidebar_mvp(counts={"all": "28", "trash": "2"}, foot=side_foot("/mnt/nas/annota", "Not reachable · 3 pending", off=True))))

# ---------------------------------------------------------------- 15 NAS back online
W("15-nas-back-online", "Library — server back",
  lib_window(content(gh(sub="31 captures"), grid(IT, states={"t1": {"selected"}, "t4": {"selected"}, "t7": {"selected"}})),
             side=sidebar_mvp(foot=side_foot("/mnt/nas/annota", "Connected · 31 captures · 1.8 GB")),
             overlays=toast("Library server is back — 3 captures moved to /mnt/nas/annota", "Show")))

# ---------------------------------------------------------------- 16 folder sync (external delete + external add)
new_png = Item("n1", "today", "14:31", "Screenshot from 2026-10-30 14-31-12", "a", (642, 236, 576, 176), "File", "Files",
               "", "", "88 KB", plain=True)
IT_sync = [new_png] + IT
W("16-folder-sync", "Library — external changes",
  lib_window(content(gh(sub="31 captures"), grid(IT_sync, states={"t3": {"gone"}}))))

# ---------------------------------------------------------------- 17/18 preferences: library
def prefs(auto_on=True, focus_switch=False):
    sw_cls = "switch on" if auto_on else "switch"
    if focus_switch:
        sw_cls += " focus"
    sub = ("Captures are saved to the library folder before the editor opens"
           if auto_on else "Only captures you save yourself appear in the library")
    loc_dis = "" if auto_on else ""
    return f"""<div class="dialog-host"><div class="dialog" style="width:640px;max-width:640px;height:700px">
  {headerbar("", "Preferences", close=False, end=f'<button class="btn flat circular" style="width:24px;min-width:24px;min-height:24px;background:var(--fill-1)">{ic("close")}</button>')}
  <div style="display:flex;justify-content:center;padding:0 0 6px"><div class="toggle-group flat"><span class="toggle">{ic("capture-region")}Capture</span><span class="toggle">{ic("pen")}Editor</span><span class="toggle active">{ic("library")}Library</span></div></div>
  <div class="preferences-page" style="padding-top:12px;gap:24px">
    <div class="preferences-group">
      <div class="group-header"><div class="text"><span class="heading">Storage</span></div></div>
      <div class="boxed-list">
        <div class="row {'focus' if focus_switch else ''}"><div class="text"><span class="title">Keep Every Capture</span><span class="subtitle">{sub}</span></div><div class="suffix"><span class="{sw_cls}"></span></div></div>
        <div class="row{loc_dis}"><div class="prefix">{ic("folder")}</div><div class="text"><span class="title">Library Folder</span><span class="subtitle mono-path">/mnt/nas/annota</span></div><div class="suffix"><button class="btn text-button">Change…</button></div></div>
        <div class="row property"><div class="text"><span class="title">Contents</span><span class="subtitle">31 captures · 1.8 GB · network share, connected</span></div></div>
      </div>
    </div>
    <div class="preferences-group">
      <div class="group-header"><div class="text"><span class="heading">When the Library Folder Is Unavailable</span><span class="description">For a folder on your own server or NAS. Annota never connects to anything itself — it uses the share mounted by your system.</span></div></div>
      <div class="boxed-list">
        <div class="row"><div class="prefix">{ic("folder")}</div><div class="text"><span class="title">Local Fallback Folder</span><span class="subtitle mono-path">~/.local/share/annota/pending</span></div><div class="suffix"><button class="btn flat image-button">{ic("folder")}</button></div></div>
        <div class="row"><div class="text"><span class="title">Move Pending Captures Automatically</span><span class="subtitle">When the library folder is reachable again</span></div><div class="suffix"><span class="switch on"></span></div></div>
      </div>
    </div>
    <div class="preferences-group">
      <div class="group-header"><div class="text"><span class="heading">Index</span></div></div>
      <div class="boxed-list"><div class="row"><div class="text"><span class="title">Rebuild Library Index</span><span class="subtitle">Thumbnails and search data are recreated from the files</span></div><div class="suffix"><button class="btn text-button">Rebuild</button></div></div></div>
    </div>
  </div></div></div>"""


W("17-preferences-library", "Preferences — library",
  lib_window(content(gh(), grid(IT)), overlays=prefs(True)))
W("18-preferences-storage-off", "Preferences — storage off",
  lib_window(content(gh(), grid(IT)), overlays=prefs(False, focus_switch=True)))

# ---------------------------------------------------------------- 19 editor tray fed from library
def tray(cur="t1", hover=None, sel=()):
    open_docs = ["t1", "t2"]
    recent = ["t3", "t4", "t5", "t6"]
    parts = ['<div class="tray"><span class="tray-label">Open</span>']
    for k in open_docs:
        c = "cur" if k == cur else ("sel" if k in sel else "")
        dot = '<span class="dot"></span>' if k == "t2" else ""
        parts.append(f'<div class="ti {c}">{timg(B[k], 100, 68)}{dot}</div>')
    parts.append('<span class="sepv"></span><span class="tray-label">Recent</span>')
    for k in recent:
        c = "hover" if k == hover else ("sel" if k in sel else "")
        ck = f'<span class="ck">{ic("check")}</span>' if k in sel else ""
        d = f'<span class="dur" style="right:4px;bottom:4px;height:18px">{ic("play")}{B[k].dur}</span>' if B[k].dur else ""
        parts.append(f'<div class="ti {c}">{timg(B[k], 100, 68)}{ck}{d}</div>')
    parts.append(f'<div class="more">{ic("library")}Open Library<span class="caption">Ctrl+L</span></div></div>')
    return "".join(parts)


def editor(tray_html, overlays="", subtitle="1600 × 1000 · In Library", title="Checkout postcode error"):
    it = B["t1"]
    canvas = (f'<div class="content-pane" style="position:relative"><div class="canvas-area">'
              f'<div class="canvas-image" style="width:700px;height:261px">{timg(it, 700, 261)}</div>'
              f'<div class="osd-toolbar" style="position:absolute;right:18px;bottom:18px">{btn("zoom-out")}<span class="label">77 %</span>{btn("zoom-in")}</div></div>'
              f'{tray_html}</div>')
    return (f'<div class="window" style="width:1280px;height:800px">{editor_header(title, subtitle)}'
            f'<div class="split-view">{tool_strip()}{canvas}{props_arrow()}</div>{overlays}</div>')


tip = '<div class="tooltip" style="left:520px;top:604px">Harbor backup destination · Today 13:58</div>'
W("19-editor-tray", "Editor — tray from library", editor(tray(hover="t4"), overlays=tip), css=EDITOR_CSS)

emenu = menu([("New Capture", "Ctrl+N"), ("Open…", "Ctrl+O"), ("Library", "Ctrl+L", "hover"), "sep",
              ("Save As…", "Shift+Ctrl+S"), ("Show in Library", ""), "sep",
              ("Preferences", "Ctrl+,"), ("Keyboard Shortcuts", "Ctrl+?"), ("About Annota", "")], 975, 48, 270, arrow_x=246)
W("20-editor-menu-library", "Editor — open library", editor(tray(), overlays=emenu), css=EDITOR_CSS)

# ---------------------------------------------------------------- 21/22 narrow
narrow_head = headerbar(btn("sidebar-show", cls="flat"), "All Captures", "31 captures",
                        end=view_toggle() + btn("view-more", cls="flat"), close=True)
W("21-narrow-grid", "Library — narrow",
  window(content(narrow_head, grid(IT, tw=150, th=112, states={"t1": {"selected"}})), 700, 800), size=(700, 800))

it = B["t1"]
np_head = headerbar(btn("go-previous", cls="flat"), it.name, "Today, 14:22", end=btn("star", cls="flat") + btn("view-more", cls="flat"), close=True)
np_body = (f'<div class="pv-body" style="padding:12px 18px 18px;gap:18px">'
           f'<div class="pv-img" style="height:280px">{timg(it, 620, 250)}</div>'
           f'<div class="actions" style="display:flex;gap:6px"><button class="btn suggested-action grow" style="flex:1">Open in Editor</button>{btn("copy")}{btn("folder")}{btn("export")}</div>'
           f'<div class="boxed-list">{prop("Type", "Region")}{prop("Application", "Firefox")}'
           f'{prop("Source", "https://" + it.url)}{prop("Dimensions", it.px + " · scale 1×")}{prop("File size", it.fsize + " · .annota")}'
           f'{prop("Location", "/mnt/nas/annota/" + filename(it), mono=True)}</div></div>')
W("22-narrow-preview", "Library — narrow preview",
  window(content(np_head, np_body, cls="", style="background:var(--window-bg)"), 700, 800), size=(700, 800),
  css=".boxed-list .row{min-height:0;padding:7px 12px}")

nside = (f'<div style="position:absolute;inset:0;z-index:20;display:flex"><div class="sidebar-pane sidebar lib-side" style="box-shadow:var(--shadow-dialog);height:100%">'
         f'{headerbar(btn("capture-region", cls="flat"), "Annota", end=btn("open-menu", cls="flat"))}'
         f'<div class="sidebar-list">{nav("library", "All Captures", "31", "selected")}{nav("timer", "Recent")}{nav("trash", "Trash", "2")}</div>'
         f'{side_foot()}</div><div class="backdrop-dim" style="position:relative;flex:1;z-index:0"></div></div>')
W("23-narrow-sidebar", "Library — narrow sidebar",
  window(content(narrow_head, grid(IT, tw=150, th=112)), 700, 800, overlays=nside), size=(700, 800))

# ---------------------------------------------------------------- 24 large library (lazy thumbnails)
def sk_tile(tw=150, th=112, loaded=None):
    if loaded:
        return tile(loaded)
    return (f'<div class="tile" style="--tw:{tw}px;--th:{th}px"><div class="frame"><span class="skeleton" style="width:110px;height:74px"></span></div>'
            f'<div class="name"><span class="skeleton" style="display:inline-block;width:110px;height:10px"></span></div>'
            f'<div class="meta"><span class="skeleton" style="display:inline-block;width:52px;height:8px"></span></div></div>')


lg = ['<div class="grid-scroll">']
lg.append('<div class="day-head"><span class="heading">Monday</span><span class="caption">14 September 2026 · 38 captures</span></div><div class="tiles">')
for n, i in enumerate(IT[:14]):
    lg.append(sk_tile(loaded=i if n < 6 else None))
lg.append('</div><div class="day-head"><span class="heading">13 September 2026</span><span class="caption">Sunday · 12 captures</span></div><div class="tiles">')
for _ in range(7):
    lg.append(sk_tile())
lg.append('</div><span class="scrollbar" style="top:46%;height:3%"></span></div>')
W("24-large-library", "Library — 10,000 items",
  lib_window(content(gh(sub="10,412 captures"), "".join(lg)),
             side=sidebar_mvp(counts={"all": "10,412", "trash": "2"}, foot=side_foot("/mnt/nas/annota", "Connected · 38 GB"))))

# ---------------------------------------------------------------- 25 trash
trash_items = [B["u3"], B["o2"]]
tb = (f'<div class="banner"><span class="banner-title">Items are in the system trash. Annota shows only captures from the library folder.</span><button class="btn">Empty Trash…</button></div>'
      + '<div class="grid-scroll"><div class="day-head"><span class="heading">Trashed Today</span></div><div class="tiles">'
      + "".join(tile(i, ("selected",) if i.key == "u3" else (), meta=f'{ic("trash")}<span>from {dayname(i.day)}</span>') for i in trash_items)
      + "</div></div>")
tactions = (f'<div class="action-bar"><span class="count">1 selected</span><span class="spacer"></span>'
            f'<button class="btn text-button">Restore</button><button class="btn text-button destructive-action">Delete Permanently…</button></div>')
W("25-trash", "Library — trash",
  lib_window(content(headerbar("", "Trash", "2 captures", end=view_toggle(), close=True), tb + tactions),
             side=sidebar_mvp(selected="trash")))

# ---------------------------------------------------------------- 26 video preview
W("26-video-preview", "Library — video",
  lib_window(content(gh(preview_on=True), grid(IT, states={"w3": {"selected"}}, scroll=(0.36, 0.32)).replace('padding-top:0px', 'padding-top:0px;margin-top:-640px')),
             pv=preview_pane(B["w3"])))

print("ok")
