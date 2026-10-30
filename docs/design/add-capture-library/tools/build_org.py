"""Build add-library-organisation mockups:  cd docs/design/add-capture-library/tools && python3 build_org.py"""
from mock import *

CH = "add-library-organisation"
IT = ITEMS()
B = by_key(IT)

# annotation text + OCR content used by search scenarios
B["t2"].ann = [A_rect(392, 175, 576, 58), A_callout(700, 40, 300, 70, "Click Connect", 760, 175)]
B["t2"].name = "Harbor — connect NAS destination"

ORG_CSS = r"""
  .search-head { flex: 1; display: flex; justify-content: center; }
  .search-head { min-width: 0; padding: 0 12px; }
  .search-head .search-entry { width: 100%; max-width: 440px; }
  .tag-row { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; padding: 10px 12px; }
  .pv .group-label { font-size: var(--fs-caption-heading); font-weight: var(--fw-bold); opacity: var(--dim-opacity); padding: 0 2px; margin-bottom: -6px; }
  .tagc.add { background: transparent; box-shadow: inset 0 0 0 1px var(--borders); font-weight: var(--fw-bold); }
  .ac-row { display: flex; align-items: center; gap: 10px; min-height: 34px; padding: 0 10px; border-radius: var(--radius-s); }
  .ac-row .label { flex: 1; }
  .ac-row .n { opacity: var(--dim-opacity); font-size: var(--fs-small); font-variant-numeric: tabular-nums; }
  .ac-row.hover { background: var(--fill-flat-hover); }
  .ac-row.selected { background: var(--accent-a12); }
  .info-line { display: flex; align-items: center; gap: 8px; padding: 10px 18px; font-size: var(--fs-small); box-shadow: inset 0 -1px var(--separator); background: var(--view-bg); }
  .info-line .icon { opacity: .7; }
  .dup-group { display: flex; gap: 12px; align-items: flex-start; }
  .dup-card { width: 150px; display: flex; flex-direction: column; gap: 6px; }
  .dup-card .frame { height: 100px; border-radius: 9px; background: var(--row-hover); box-shadow: inset 0 0 0 1px var(--separator); display: grid; place-items: center; position: relative; }
  .dup-card.keep .frame { box-shadow: inset 0 0 0 2px var(--success-bg); }
  .dup-card .frame .badge { position: absolute; top: 6px; left: 6px; }
  .dup-card .caption { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
"""

IMPORTED = [
    Item("s1", "oct12", "09:14", "Screenshot From 2026-10-12 09-14-33", "a", (250, 120, 1100, 760), "File", "GNOME Screenshot", "", "", "402 KB", plain=True),
    Item("s2", "oct12", "09:13", "Screenshot From 2026-10-12 09-13-02", "d", (0, 0, 1600, 1000), "File", "GNOME Screenshot", "", "", "880 KB", plain=True),
    Item("s3", "oct12", "08:51", "Screenshot From 2026-10-12 08-51-40", "e", (72, 0, 1228, 1000), "File", "GNOME Screenshot", "", "", "610 KB", plain=True),
    Item("s4", "oct09", "17:30", "Screenshot From 2026-10-09 17-30-11", "b", (0, 0, 1600, 1000), "File", "GNOME Screenshot", "", "", "702 KB", plain=True),
    Item("s5", "oct09", "16:02", "Screenshot From 2026-10-09 16-02-55", "c", (0, 0, 1600, 1000), "File", "GNOME Screenshot", "", "", "530 KB", plain=True),
    Item("s6", "oct09", "11:47", "Screenshot From 2026-10-09 11-47-19", "f", (0, 600, 1600, 1000), "File", "GNOME Screenshot", "", "", "490 KB", plain=True),
    Item("s7", "oct09", "10:12", "Screenshot From 2026-10-09 10-12-08", "b", (1040, 188, 480, 708), "File", "GNOME Screenshot", "", "", "140 KB", plain=True),
]
SHOT_DAYS = [("oct12", "Monday", "12 October 2026"), ("oct09", "Friday", "9 October 2026")]


def W(name, title, body, size=(1280, 800), flush=True, css=""):
    write(CH, name, page(title, body, size, flush, ORG_CSS + css))


def gh(title="All Captures", sub="31 captures", preview_on=False, mode="grid", search=True, sort="Capture Date"):
    return headerbar(content_start(mode), title, sub,
                     end=content_end(mode, 1, sort, preview_on, search=search), close=not preview_on)


def search_head(q, n, preview_on=False, focus=True):
    entry = (f'<span class="search-entry{" focus" if focus else ""}">{ic("search")}<span class="grow">{esc(q)}'
             f'{"<span class=caret style=display:inline-block;width:1px;height:17px;background:currentColor;vertical-align:-3px;margin-left:1px></span>" if focus else ""}</span>'
             f'<span class="caption dim-label" style="margin-right:4px">{n}</span>{ic("edit-clear")}</span>')
    end = btn("search", cls="flat active") + btn("sidebar-show-right", cls="flat" + (" active" if preview_on else ""))
    wc = "" if preview_on else '<div class="window-controls"><span class="wc">' + ic("close") + "</span></div>"
    return (f'<div class="headerbar"><div class="start" style="flex:0 0 auto">{content_start()}</div>'
            f'<div class="search-head">{entry}</div><div class="end" style="flex:0 0 auto">{end}{wc}</div></div>')


def lw(content_html, side=None, pv="", overlays=""):
    return window((side if side is not None else sidebar_org()) + content_html + pv, 1280, 800, overlays)


def tags_html(tags, add=True):
    t = "".join(f'<span class="tagc"><span class="tag-dot" style="--c:{TAG_COLORS.get(x, "var(--annota-azure)")};width:8px;height:8px"></span>{esc(x)}</span>' for x in tags)
    a = f'<span class="tagc add">{ic("plus")}Add Tag</span>' if add else ""
    return t + a


def org_actions(it, cols=("Release notes 0.4",), tags=None, tag_focus=False):
    tags = it.tags if tags is None else tags
    col = "".join(f'<span class="tagc">{ic("folder")}{esc(c)}</span>' for c in cols) or '<span class="caption dim-label">Not in a collection</span>'
    return (f'<div class="actions"><button class="btn suggested-action grow">Open in Editor</button>{btn("folder")}{btn("export")}</div>'
            f'<div class="group-label">Tags</div><div class="boxed-list"><div class="tag-row{" focus-ring" if tag_focus else ""}">{tags_html(tags)}</div></div>'
            f'<div class="group-label">Collections</div><div class="boxed-list"><div class="tag-row">{col}</div></div>')


def pv(it, fav_on=None, img=None, cols=("Release notes 0.4",), tags=None, hide_props=False, title_html=None):
    return preview_pane(it, fav_btn=True, fav_on=it.fav if fav_on is None else fav_on, img_html=img,
                        actions=org_actions(it, cols, tags), hide_props=hide_props, title_html=title_html)


# ---------------------------------------------------------------- 01 overview: full sidebar + preview with tags
W("01-library-organised", "Library — organised",
  lw(content(gh(preview_on=True), grid(IT, states={"t1": {"selected", "showfav"}, "y1": {"showfav"}})), pv=pv(B["t1"], cols=("Release notes 0.4", "Onboarding guide"))))

# ---------------------------------------------------------------- 02 search: annotation text
hit = B["t2"]
r_conn = [hit, B["w4"], B["o1"]]
for i in (B["w4"], B["o1"]):
    pass
match_t2 = '<div class="match"><span class="src">Annotation</span>Click <mark>Connect</mark></div>'
match_w4 = '<div class="match"><span class="src">Window title</span>Harbor — <mark>Connect</mark>ion</div>'
match_o1 = '<div class="match"><span class="src">Screen text</span>Last <mark>connect</mark>ed 02:00</div>'
hit_img_it = Item("x", "today", "", "", "a", hit.crop, "", "", ann=hit.ann + [A_mark(820, 50, 165, 50)])
res_groups = [("today", "Today", "Friday, 30 October"), ("wed", "Wednesday", "28 October"), ("oct18", "18 October 2026", "Sunday")]
grid_html = grid(r_conn, groups=res_groups, states={"t2": {"selected"}},
                 per_tile={"t2": {"match": match_t2}, "w4": {"match": match_w4}, "o1": {"match": match_o1}}, scroll=None)
W("02-search-annotation-text", "Library — search connect",
  lw(content(search_head("connect", "3 results", preview_on=True), grid_html),
     pv=pv(hit, img=timg(hit_img_it, 280, 180), cols=())))

# ---------------------------------------------------------------- 03 search: screen text (OCR)
inv_items = [B["t5"], B["m2"]]
B["t5"].ann = [A_blur(497, 162, 292, 22), A_blur(700, 278, 510, 22), A_mark(1048, 508, 58, 26)]
m5 = '<div class="match"><span class="src">Screen text</span>…0417, <mark>invoice</mark> goes to…</div>'
m6 = '<div class="match"><span class="src">Screen text</span><mark>Invoice</mark> address</div>'
m2 = '<div class="match"><span class="src">Tag</span><mark>invoice</mark>s</div>'
B["m2"].tags = ["invoices"]
g3 = grid(inv_items, groups=[("today", "Today", "Friday, 30 October"), ("mon", "Monday", "26 October")],
          states={"t5": {"selected"}}, per_tile={"t5": {"match": m5}, "m2": {"match": m2}}, scroll=None)
ocr_note = (f'<div class="info-line">{ic("ocr-text")}<span>Searching names, tags, applications, window titles, web addresses, annotation text and <b>screen text</b>.</span>'
            f'<span class="spacer"></span><span class="caption dim-label">412 of 10,412 captures still being read</span><span class="spinner"></span></div>')
mt = (f'<span class="title-4">{esc(B["t5"].name)}</span>{btn("pen", cls="flat small")}')
pv3 = pv(B["t5"], cols=())
pv3 = pv3.replace('<div class="group-label">Tags</div>',
                  '<div class="group-label">Found in Screen Text</div><div class="boxed-list"><div class="tag-row" style="font-size:13px">“…booking reference is RH-2026-0417, <mark>invoice</mark> goes to the club address.”</div></div><div class="group-label">Tags</div>', 1)
W("03-search-screen-text", "Library — search invoice",
  lw(content(search_head("invoice", "2 results", preview_on=True), ocr_note + g3), pv=pv3))

# ---------------------------------------------------------------- 04 combined filters with chips
fx = [i for i in IT if i.app == "Firefox" and i.day in ("today", "yesterday", "wed", "tue", "mon") and not i.dur]
chips = ('<div class="filter-bar">'
         f'<span class="chip"><span class="k">Date</span> Last 7 days<span class="x">{ic("close")}</span></span>'
         f'<span class="chip"><span class="k">Type</span> Image<span class="x">{ic("close")}</span></span>'
         f'<span class="chip"><span class="k">App</span> Firefox<span class="x">{ic("close")}</span></span>'
         f'<span class="chip add">{ic("plus")}Filter</span><span class="spacer"></span>'
         f'<span class="caption dim-label">{len(fx)} of 31</span><button class="btn flat text-button small">Save as Smart Collection…</button>'
         f'<button class="btn flat text-button small">Clear All</button></div>')
W("04-filters-combined", "Library — filters",
  lw(content(search_head("", "", focus=False).replace('<span class="grow"></span>', '<span class="grow placeholder">Search captures</span>'), chips + grid(fx, scroll=(0, .5)))))

# ---------------------------------------------------------------- 05 add-filter popover (category list + date sub-page)
fmenu = (f'<div class="popover" style="left:560px;top:96px;width:300px;--arrow-x:40px">'
         f'<div class="menu-section-title">Add Filter</div>'
         + "".join(f'<div class="menu-item{" hover" if l == "Website" else ""}">{ic(i)}<span class="label">{l}</span><svg class="icon" style="opacity:.55"><use href="#i-go-next"/></svg></div>'
                   for i, l in [("timer", "Date"), ("image", "Type"), ("capture-window", "Application"), ("share", "Website"), ("tag", "Tag"), ("folder", "Collection")])
         + f'<div class="menu-sep"></div><div class="menu-item"><span class="check" style="width:16px;height:16px"></span><span class="label">Has Annotations</span></div></div>')
sub = (f'<div class="popover no-arrow" style="left:868px;top:150px;width:290px">'
       f'<div class="hbox" style="padding:3px 6px 6px">{btn("go-previous", cls="flat small")}<span class="heading" style="font-size:14px">Website</span></div>'
       f'<span class="search-entry focus" style="width:100%;margin-bottom:6px">{ic("search")}<span class="grow">lum<span style="display:inline-block;width:1px;height:16px;background:currentColor;vertical-align:-3px"></span></span></span>'
       f'<div class="ac-row hover"><span class="check checked" style="width:16px;height:16px"></span><span class="label"><b>lum</b>en-outfitters.example</span><span class="n">9</span></div>'
       f'<div class="ac-row"><span class="check" style="width:16px;height:16px"></span><span class="label">tidewater.example</span><span class="n">6</span></div>'
       f'<div class="ac-row"><span class="check" style="width:16px;height:16px"></span><span class="label">fieldnotes.example</span><span class="n">3</span></div></div>')
chips5 = chips.replace(f'<span class="chip add">', f'<span class="chip add" style="box-shadow:inset 0 0 0 2px var(--accent-a50)">')
W("05-filter-add-popover", "Library — add filter",
  lw(content(search_head("", "", focus=False).replace('<span class="grow"></span>', '<span class="grow placeholder">Search captures</span>'), chips5 + grid(fx, scroll=(0, .5))), overlays=fmenu + sub))

# date range popover (custom)
def _cal():
    cells = [(d, True) for d in (28, 29, 30)] + [(d, False) for d in range(1, 32)] + [(1, True)]
    out = []
    for d, other in cells:
        st = ""
        if not other and d in (12, 18):
            st = "background:var(--accent-bg);color:var(--accent-fg);font-weight:700;"
        elif not other and 12 < d < 18:
            st = "background:var(--accent-a12);"
        if other:
            st += "opacity:.35;"
        out.append(f'<span style="width:32px;height:30px;display:grid;place-items:center;border-radius:6px;font-size:13px;{st}">{d}</span>')
    return "".join(out)


cal_days = _cal()
datepop = (f'<div class="popover" style="left:300px;top:96px;width:300px;--arrow-x:56px">'
           f'<div class="menu-section-title">Date</div>'
           f'<div class="menu-item"><span class="radio" style="width:16px;height:16px"></span><span class="label">Today</span></div>'
           f'<div class="menu-item"><span class="radio" style="width:16px;height:16px"></span><span class="label">Last 7 Days</span></div>'
           f'<div class="menu-item"><span class="radio" style="width:16px;height:16px"></span><span class="label">Last 30 Days</span></div>'
           f'<div class="menu-item selected"><span class="radio checked" style="width:16px;height:16px"></span><span class="label">Custom Range</span></div>'
           f'<div class="hbox" style="justify-content:space-between;padding:6px 6px 2px">{btn("go-previous", cls="flat small")}<span class="heading" style="font-size:14px">October 2026</span>{btn("go-next", cls="flat small")}</div>'
           f'<div style="display:grid;grid-template-columns:repeat(7,32px);gap:4px 6px;justify-content:center;padding:4px 0 6px">'
           + "".join(f'<span class="caption dim-label" style="text-align:center">{d}</span>' for d in "MTWTFSS") + cal_days + '</div>'
           f'<div class="hbox" style="padding:4px 6px 4px"><span class="caption dim-label grow">12 – 18 October</span><button class="btn suggested-action text-button small">Apply</button></div></div>')
chips_d = ('<div class="filter-bar">'
           f'<span class="chip" style="box-shadow:inset 0 0 0 2px var(--accent-a50)"><span class="k">Date</span> 12 – 18 Oct<span class="x">{ic("close")}</span></span>'
           f'<span class="chip add">{ic("plus")}Filter</span><span class="spacer"></span><span class="caption dim-label">3 of 31</span></div>')
oct_items = [B["o1"], B["o2"], B["o3"]]
W("06-filter-date-range", "Library — date range",
  lw(content(search_head("", "", focus=False).replace('<span class="grow"></span>', '<span class="grow placeholder">Search captures</span>'),
             chips_d + grid(oct_items, scroll=None)), overlays=datepop))

# ---------------------------------------------------------------- 07 bulk tag with autocomplete
sel5 = ["t1", "t7", "y3", "w2", "u3"]
st = {k: {"multi"} for k in B if B[k]}
for k in sel5:
    st[k] = {"multi", "selected"}
bar = (f'<div class="action-bar"><span class="count">5 selected</span><button class="btn flat text-button">Select All</button><span class="spacer"></span>'
       f'<button class="btn active" style="padding:5px 14px">{ic("tag")}Tag…</button>'
       f'<button class="btn" style="padding:5px 14px">{ic("folder")}Add to Collection…</button>'
       f'<button class="btn" style="padding:5px 14px">{ic("star")}Favourite</button>'
       f'<button class="btn" style="padding:5px 14px">{ic("export")}Export…</button>'
       f'<button class="btn image-button" style="color:var(--destructive)">{ic("trash")}</button></div>')
sel_header = headerbar('<button class="btn text-button">Cancel</button>', "5 Selected", "of 31 captures",
                       end=sort_btn() + btn("sidebar-show-right", cls="flat"), close=True)
tagpop = (f'<div class="popover arrow-down" style="left:600px;top:434px;width:320px;--arrow-x:136px">'
          f'<div class="menu-section-title">Tags for 5 Captures</div>'
          f'<div style="display:flex;flex-wrap:wrap;gap:6px;padding:6px 6px 8px">'
          f'<span class="tagc"><span class="tag-dot" style="--c:{TAG_COLORS["checkout"]};width:8px;height:8px"></span>checkout <span class="dim-label">4 of 5</span>{ic("close", "icon x")}</span>'
          f'<span class="tagc"><span class="tag-dot" style="--c:{TAG_COLORS["bug"]};width:8px;height:8px"></span>bug <span class="dim-label">2 of 5</span>{ic("close", "icon x")}</span></div>'
          f'<span class="entry focus" style="width:100%">{ic("tag")}rel<span class="caret" style="margin-left:0"></span></span>'
          f'<div style="padding-top:6px"><div class="ac-row selected"><span class="tag-dot" style="--c:{TAG_COLORS['release-notes']}"></span><span class="label"><b>rel</b>ease-notes</span><span class="n">1</span><kbd class="key">Enter</kbd></div>'
          f'<div class="ac-row">{ic("plus")}<span class="label">Create Tag “rel”</span></div></div></div>')
B["y3"].tags = ["checkout", "release-notes"]
side7 = sidebar_org(tag_counts=[("bug", 6), ("checkout", 4), ("docs", 2), ("dev", 4), ("release-notes", 1), ("report", 4), ("support", 2)], cut=True)
W("07-bulk-tag-autocomplete", "Library — tag 5 captures",
  lw(content(sel_header, grid(IT, states=st) + bar), side=side7, overlays=tagpop))

# ---------------------------------------------------------------- 08 tagged → sidebar release-notes 5
side8 = sidebar_org(selected="tag-release-notes", tag_counts=[("bug", 6), ("checkout", 4), ("docs", 2), ("dev", 4), ("release-notes", 5), ("report", 4), ("support", 2)], cut=True)
rn = [B[k] for k in sel5]
W("08-tag-view", "Library — tag release-notes",
  lw(content(gh("release-notes", "5 captures · tag"), grid(rn, scroll=None)), side=side8,
     overlays=toast("Tag “release-notes” added to 5 captures", "Undo")))

# ---------------------------------------------------------------- 09 tag context menu + rename
tmenu = menu([("Rename Tag…", "F2", "hover"), ("Change Colour", ""), "sep", ("Delete Tag…", "")], 150, 552, 220, arrow_x=60)
W("09-tag-menu", "Library — tag menu",
  lw(content(gh("release-notes", "5 captures · tag"), grid(rn, scroll=None)), side=side8, overlays=tmenu))

rename = (f'<div class="dialog-host"><div class="dialog alert" style="width:400px;max-width:400px"><div class="alert-body" style="gap:14px">'
          f'<div class="alert-heading">Rename Tag?</div><div>The new name is written into all 5 captures that carry the tag.</div>'
          f'<span class="entry focus" style="text-align:left">release-notes-0.4<span class="caret" style="margin-left:1px"></span></span></div>'
          f'<div class="alert-actions"><button class="btn">Cancel</button><button class="btn suggested-action">Rename</button></div></div></div>')
W("10-tag-rename-dialog", "Library — rename tag",
  lw(content(gh("release-notes", "5 captures · tag"), grid(rn, scroll=None)), side=side8, overlays=rename))

# ---------------------------------------------------------------- 11 drag into collection
st11 = {"y1": {"selected"}, "y8": {"selected"}}
ghost = (f'<div class="ghost" style="left:150px;top:330px"><div class="stack" style="transform:rotate(-3deg)">{timg(B["y1"], 96, 84)}</div>'
         f'<span class="badge">2</span></div>'
         f'<svg class="cursor-arrow" style="left:206px;top:392px" viewBox="0 0 20 20"><path d="M3 2 V16 L6.6 12.8 L9 18 L11.4 17 L9 11.8 H14 Z" fill="#fff" stroke="#000" stroke-width="1.2" stroke-linejoin="round"/></svg>'
         f'<div class="tooltip" style="left:236px;top:412px">Add 2 captures to “Release notes 0.4”</div>')
W("11-drag-into-collection", "Library — add to collection",
  lw(content(gh(), grid(IT, states=st11)), side=sidebar_org(drop="rn"), overlays=ghost))

# ---------------------------------------------------------------- 12 collection view + context menu add to collection
col_items = [B[k] for k in ["t1", "t7", "y3", "y1", "y8", "w2", "w4", "u3", "o2", "m1", "t4", "y5"]]
ctx = (menu([("Open in Editor", "Enter"), ("Quick Look", "Space"), "sep", ("Add to Favourites", "Ctrl+B"), ("Edit Tags…", "Ctrl+T"),
             ("Add to Collection", "", "hover"), ("Remove from “Release notes 0.4”", ""), "sep", ("Rename…", "F2"), ("Move to Trash", "Del")],
            560, 214, 300, arrow_x=24)
       + '<div class="popover no-arrow" style="left:866px;top:388px;width:240px">'
       + '<div class="menu-item"><span class="check checked" style="width:16px;height:16px"></span><span class="label">Release notes 0.4</span></div>'
       + '<div class="menu-item hover"><span class="check" style="width:16px;height:16px"></span><span class="label">Onboarding guide</span></div>'
       + f'<div class="menu-sep"></div><div class="menu-item">{ic("plus")}<span class="label">New Collection…</span></div></div>')
ctx = ctx.replace('<span class="label">Add to Collection</span>', f'<span class="label">Add to Collection</span><svg class="icon" style="opacity:.55"><use href="#i-go-next"/></svg>')
colhead = headerbar(content_start(), "Release notes 0.4", "12 captures · collection",
                    end=btn("search", cls="flat") + sort_btn("Order Added") + btn("view-more", cls="flat") + btn("sidebar-show-right", cls="flat"), close=True)
col_grid = '<div class="grid-scroll"><div style="height:18px"></div><div class="tiles">' + "".join(tile(i, ("selected",) if i.key == "y3" else ()) for i in col_items) + '</div></div>'
W("12-collection-view", "Library — collection",
  lw(content(colhead, col_grid), side=sidebar_org(selected="rn"), overlays=ctx))

# ---------------------------------------------------------------- 13 delete collection (12 items stay)
alert = (f'<div class="dialog-host"><div class="dialog alert" style="width:400px;max-width:400px"><div class="alert-body">'
         f'<div class="alert-heading">Delete Collection?</div><div>“Onboarding guide” is removed. Its 12 captures stay in the library and keep their tags.</div></div>'
         f'<div class="alert-actions"><button class="btn">Cancel</button><button class="btn destructive-action">Delete</button></div></div></div>')
onb = [B[k] for k in ["t2", "y5", "w4", "o1", "y6", "w6", "m2", "u1", "t3", "y7", "y4", "m3"]]
onb_head = headerbar(content_start(), "Onboarding guide", "12 captures · collection", end=btn("search", cls="flat") + sort_btn("Order Added") + btn("view-more", cls="flat active") + btn("sidebar-show-right", cls="flat"), close=True)
W("13-delete-collection", "Library — delete collection",
  lw(content(onb_head, '<div class="grid-scroll"><div style="height:18px"></div><div class="tiles">' + "".join(tile(i) for i in onb) + '</div></div>'),
     side=sidebar_org(selected="onb"), overlays=alert))
W("14-collection-deleted", "Library — collection deleted",
  lw(content(gh(), grid(IT)),
     side=sidebar_org(collections=[("folder", "Release notes 0.4", "12", "rn"), ("search", "Bugs, last 30 days", "6", "smart")]),
     overlays=toast("Collection “Onboarding guide” deleted — 12 captures kept", "Undo")))

# ---------------------------------------------------------------- 15 favourites
favs = [B["t1"], B["y1"], B["t4"], B["w2"]]
for f in favs:
    f.fav = True
W("15-favourites", "Library — favourites",
  lw(content(gh("Favourites", "4 captures"), grid(favs, states={k.key: {"showfav"} for k in favs} | {"t4": {"showfav", "selected"}}, scroll=None)),
     side=sidebar_org(selected="fav"), pv=pv(B["t4"], fav_on=True, cols=())))
B["t4"].fav = False
# favourite via star (hover state + tooltip)
star_tip = '<div class="tooltip" style="left:1060px;top:44px">Add to Favourites <span class="shortcut">Ctrl+B</span></div>'
pv16 = pv(B["t7"], fav_on=False, cols=()).replace(f'<button class="btn image-button flat"', f'<button class="btn image-button flat hover"', 1)
W("16-favourite-star", "Library — mark favourite",
  lw(content(gh(preview_on=True), grid(IT, states={"t7": {"selected"}})), pv=pv16, overlays=star_tip))

# ---------------------------------------------------------------- 17 save search dialog
save_dlg = (f'<div class="dialog-host"><div class="dialog" style="width:480px;max-width:480px">'
            + headerbar('<button class="btn text-button">Cancel</button>', "New Smart Collection", end='<button class="btn text-button suggested-action">Save</button>')
            + f'<div class="dialog-body vbox" style="--gap:18px"><div class="boxed-list">'
            f'<div class="row entry-row focus"><div class="text"><span class="title">Name</span><span>Bugs, last 30 days<span class="caret" style="margin-left:1px;vertical-align:-3px"></span></span></div></div></div>'
            f'<div class="preferences-group"><div class="group-header"><div class="text"><span class="heading">Rules</span><span class="description">Captures that match all rules appear automatically, including new ones.</span></div></div>'
            f'<div class="boxed-list"><div class="tag-row" style="padding:12px">'
            f'<span class="chip plain"><span class="k">Tag</span>&nbsp;bug</span><span class="chip plain"><span class="k">Date</span>&nbsp;Last 30 days</span></div></div></div>'
            f'<div class="caption dim-label" style="padding:0 4px">6 captures match now · stored as part of the library folder</div></div></div></div>')
chips17 = ('<div class="filter-bar">'
           f'<span class="chip"><span class="k">Tag</span> bug<span class="x">{ic("close")}</span></span>'
           f'<span class="chip"><span class="k">Date</span> Last 30 days<span class="x">{ic("close")}</span></span>'
           f'<span class="chip add">{ic("plus")}Filter</span><span class="spacer"></span><span class="caption dim-label">6 of 31</span>'
           f'<button class="btn flat text-button small active">Save as Smart Collection…</button></div>')
bugs = [i for i in IT if "bug" in i.tags]
W("17-save-search-dialog", "Library — save smart collection",
  lw(content(search_head("", "", focus=False).replace('<span class="grow"></span>', '<span class="grow placeholder">Search captures</span>'), chips17 + grid(bugs, scroll=None)),
     overlays=save_dlg))

# ---------------------------------------------------------------- 18 smart collection auto-updates
newbug = Item("nb", "today", "14:48", "Postcode field still accepts spaces", "b", (80, 500, 912, 120), "Region", "Firefox",
              "Checkout — Lumen Outfitters", "lumen-outfitters.example/checkout", "38 KB", [A_rect(28, 20, 460, 54)], tags=["bug"])
smart = [newbug] + bugs
smart_head = headerbar(content_start(), "Bugs, last 30 days", "7 captures · smart collection",
                       end=f'<button class="btn flat text-button">Edit Rules…</button>' + btn("sidebar-show-right", cls="flat active"), close=False)
rules = ('<div class="filter-bar">'
         '<span class="chip plain"><span class="k">Tag</span>&nbsp;bug</span><span class="chip plain"><span class="k">Date</span>&nbsp;Last 30 days</span>'
         '<span class="spacer"></span><span class="caption dim-label">Updates automatically</span></div>')
W("18-smart-collection", "Library — smart collection",
  lw(content(smart_head, rules + grid(smart, states={"nb": {"selected"}}, scroll=None)),
     side=sidebar_org(selected="smart", collections=[("folder", "Release notes 0.4", "12", "rn"), ("folder", "Onboarding guide", "12", "onb"), ("search", "Bugs, last 30 days", "7", "smart")],
                      tag_counts=[("bug", 7), ("checkout", 4), ("docs", 2), ("dev", 4), ("report", 4), ("support", 2)]),
     pv=pv(newbug, cols=())))

# ---------------------------------------------------------------- 19 preferences: folders, retention, duplicates
def prefs(retention_on=False):
    ret_rows = (f'<div class="row"><div class="text"><span class="title">Clean Up Old Captures</span><span class="subtitle">Move untagged captures that are not favourites and not in a collection to the trash</span></div><div class="suffix"><span class="switch{" on" if retention_on else ""}"></span></div></div>'
                + (f'<div class="row"><div class="text"><span class="title">After</span></div><div class="suffix"><span class="spin" style="width:140px"><span class="value">30 days</span>{btn("minus")}{btn("plus")}</span></div></div>'
                   f'<div class="row property"><div class="text"><span class="title">Next clean-up</span><span class="subtitle">At next start · 14 captures eligible</span></div></div>' if retention_on else ""))
    return f"""<div class="dialog-host"><div class="dialog" style="width:640px;max-width:640px;height:740px">
  {headerbar("", "Preferences", end=f'<button class="btn flat circular" style="width:24px;min-width:24px;min-height:24px;background:var(--fill-1)">{ic("close")}</button>')}
  <div style="display:flex;justify-content:center;padding:0 0 6px"><div class="toggle-group flat"><span class="toggle">{ic("capture-region")}Capture</span><span class="toggle">{ic("pen")}Editor</span><span class="toggle active">{ic("library")}Library</span></div></div>
  <div class="preferences-page" style="padding-top:12px;gap:22px">
    <div class="preferences-group">
      <div class="group-header"><div class="text"><span class="heading">Additional Folders</span><span class="description">Images in these folders are shown in the library and stay where they are.</span></div><button class="btn flat image-button">{ic("plus")}</button></div>
      <div class="boxed-list">
        <div class="row"><div class="prefix">{ic("folder")}</div><div class="text"><span class="title">Screenshots</span><span class="subtitle mono-path">~/Pictures/Screenshots · 86 images</span></div><div class="suffix"><button class="btn flat image-button">{ic("close")}</button></div></div>
        <div class="row"><div class="prefix">{ic("folder")}</div><div class="text"><span class="title">Design reviews</span><span class="subtitle mono-path">/mnt/nas/projects/reviews · indexing 212 of 540</span></div><div class="suffix"><span class="spinner"></span><button class="btn flat image-button">{ic("close")}</button></div></div>
      </div>
    </div>
    <div class="preferences-group">
      <div class="group-header"><div class="text"><span class="heading">Housekeeping</span></div></div>
      <div class="boxed-list">{ret_rows}
        <div class="row"><div class="text"><span class="title">Find Duplicates</span><span class="subtitle">Captures with identical pixels</span></div><div class="suffix"><button class="btn text-button">Find…</button></div></div>
      </div>
    </div>
    <div class="preferences-group">
      <div class="group-header"><div class="text"><span class="heading">Search</span></div></div>
      <div class="boxed-list"><div class="row"><div class="text"><span class="title">Search Screen Text</span><span class="subtitle">Read text in captures on this computer with Tesseract (English, German)</span></div><div class="suffix"><span class="switch on"></span></div></div></div>
    </div>
  </div></div></div>"""


W("19-preferences-folders-retention", "Preferences — library organisation",
  lw(content(gh(), grid(IT)), overlays=prefs(True)))

# folder chooser picking ~/Pictures/Screenshots
fc = (f'<div class="dialog-host"><div class="dialog" style="width:760px;max-width:760px;height:520px">'
      + headerbar('<button class="btn text-button">Cancel</button>', "Add Folder to Library", end='<button class="btn text-button suggested-action">Add</button>')
      + f'<div style="display:flex;flex:1;min-height:0"><div class="sidebar" style="width:200px;background:var(--sidebar-bg)"><div class="sidebar-list">'
      + nav("library", "Home") + nav("image", "Pictures", "", "selected") + nav("folder", "Documents") + nav("folder", "annota on nas")
      + '</div></div><div class="view" style="flex:1;padding:12px 18px;display:flex;flex-direction:column;gap:2px">'
      + '<div class="caption dim-label" style="padding:0 0 8px">~/Pictures</div>'
      + "".join(f'<div class="row {c}" style="min-height:40px;border-radius:6px">{ic("folder")}<div class="text"><span class="title">{n}</span></div><span class="caption dim-label">{m}</span></div>'
                for n, m, c in [("Annota", "Library folder", "disabled"), ("Screenshots", "86 images", "selected"), ("Wallpapers", "12 images", ""), ("Holiday 2026", "340 images", "")])
      + '</div></div></div></div>')
W("20-add-folder-chooser", "Preferences — add folder",
  lw(content(gh(), grid(IT)), overlays=fc))

# ---------------------------------------------------------------- 21 GNOME screenshots folder in library
shot_head = headerbar(content_start(), "Screenshots", "86 images · ~/Pictures/Screenshots",
                      end=btn("search", cls="flat") + sort_btn() + btn("sidebar-show-right", cls="flat active"), close=False)
info = (f'<div class="info-line">{ic("info")}<span>Indexed in place — files are not moved or changed. Tags and collections are kept in a sidecar file next to each image.</span></div>')
W("21-imported-folder", "Library — imported folder",
  lw(content(shot_head, info + grid(IMPORTED, groups=SHOT_DAYS, states={"s1": {"selected"}}, scroll=(0, .1))),
     side=sidebar_org(selected="f-shots"),
     pv=preview_pane(IMPORTED[0], folder="~/Pictures/Screenshots", fav_btn=True, actions=org_actions(IMPORTED[0], (), []))))

# ---------------------------------------------------------------- 22 retention toast on startup
W("22-retention-toast", "Library — retention",
  lw(content(gh(sub="17 captures"), grid([i for i in IT if i.tags or i.fav or i.day in ("today", "yesterday")])),
     side=sidebar_org().replace('<span class="label">All Captures</span><span class="count">31</span>', '<span class="label">All Captures</span><span class="count">17</span>').replace('<span class="label">Trash</span><span class="count">2</span>', '<span class="label">Trash</span><span class="count">16</span>'),
     overlays=toast("14 old captures moved to Trash (older than 30 days, untagged)", "Review")))

# ---------------------------------------------------------------- 23 duplicates dialog
def dupcard(it, keep, label):
    b = '<span class="badge success">Keep</span>' if keep else ""
    return (f'<div class="dup-card{" keep" if keep else ""}"><div class="frame">{timg(it, 134, 84)}{b}</div>'
            f'<span class="caption">{esc(label)}</span><span class="caption dim-label" style="margin-top:-4px">{it.fsize}</span></div>')


dA = Item("dA", "today", "", "", "d", (264, 83, 1302, 112), "", "", fsize="96 KB")
dB = Item("dB", "today", "", "", "b", (1040, 188, 480, 708), "", "", fsize="142 KB")
dlg = (f'<div class="dialog-host"><div class="dialog" style="width:640px;max-width:640px">'
       + headerbar('<button class="btn text-button">Cancel</button>', "Duplicates", "2 groups · 3 extra copies · 412 KB", end='<button class="btn text-button destructive-action">Trash 3 Copies</button>')
       + '<div class="dialog-body vbox" style="--gap:12px;padding-top:12px">'
       + '<div class="caption dim-label">Identical pixels. The copy that has tags, a collection or is a favourite is kept; otherwise the oldest.</div>'
       + '<div class="boxed-list"><div class="row" style="align-items:flex-start;padding:12px"><div class="dup-group">'
       + dupcard(dA, True, "KPI row September") + dupcard(dA, False, "2026-10-30 11-05-02") + dupcard(dA, False, "Screenshot From 2026-10-30…")
       + '</div></div><div class="row" style="align-items:flex-start;padding:12px"><div class="dup-group">'
       + dupcard(dB, True, "Order summary") + dupcard(dB, False, "Order summary (copy)")
       + '</div></div></div></div></div></div>')
W("23-duplicates-dialog", "Library — duplicates", lw(content(gh(), grid(IT)), overlays=dlg))

# ---------------------------------------------------------------- 24 moved library restores tags (indexing)
idx_banner = ('<div class="banner"><span class="spinner"></span><span class="banner-title">Reading library folder — 1,920 of 10,412 · tags, favourites and collections are restored from the files</span></div>')
side24 = sidebar_org(tag_counts=[("bug", 2), ("checkout", 1), ("docs", 1), ("report", 1)],
                     collections=[("folder", "Release notes 0.4", "4", "rn"), ("folder", "Onboarding guide", "3", "onb"), ("search", "Bugs, last 30 days", "2", "smart")])
side24 = side24.replace('<span class="label">Annota</span><span class="count">31</span>', '<span class="label">Annota</span><span class="spinner"></span>').replace('<span class="label">All Captures</span><span class="count">31</span>', '<span class="label">All Captures</span><span class="count">1,920</span>')
W("24-moved-library-indexing", "Library — restoring from files",
  lw(content(gh(sub="1,920 of 10,412 read"), idx_banner + grid(IT[:12], scroll=(0, .9))), side=side24))

# ---------------------------------------------------------------- 25 no results
noresults = (f'<div class="status-page compact">{ic("search", "icon status-icon")}<div class="status-title">No Results</div>'
             f'<div class="status-description">Nothing matches “waterfall” with the current filters.</div>'
             f'<button class="btn pill">Clear Filters</button></div>')
chips25 = ('<div class="filter-bar">'
           f'<span class="chip"><span class="k">Type</span> Video<span class="x">{ic("close")}</span></span>'
           f'<span class="chip add">{ic("plus")}Filter</span><span class="spacer"></span><span class="caption dim-label">0 of 31</span></div>')
W("25-search-no-results", "Library — no results", lw(content(search_head("waterfall", "0 results"), chips25 + noresults)))

# ---------------------------------------------------------------- 26 OCR unavailable
noocr = ('<div class="banner warning"><span class="banner-title">Screen text is not searched — the Tesseract OCR package is not installed.</span><button class="btn">How to Install</button></div>')
W("26-search-ocr-unavailable", "Library — OCR unavailable",
  lw(content(search_head("invoice", "1 result"), noocr + grid([B["m2"]], groups=[("mon", "Monday", "26 October")], per_tile={"m2": {"match": m2}}, scroll=None))))

# ---------------------------------------------------------------- 27 type filter incl. step guides + template
sg1 = Item("g1", "today", "12:30", "Connect a NAS — step guide", "a", (250, 120, 1100, 760), "Step guide", "Annota", fsize="1.2 MB",
           ann=[A_step(410, 150, 1), A_step(410, 210, 2)])
sg2 = Item("g2", "wed", "16:02", "Checkout walkthrough", "b", (0, 0, 1600, 1000), "Step guide", "Annota", fsize="2.4 MB",
           ann=[A_step(130, 330, 1), A_step(130, 690, 2), A_step(1100, 790, 3)])
sg3 = Item("g3", "mon", "10:22", "Release 0.4 before/after", "d", (264, 83, 1302, 600), "Template", "Annota", fsize="640 KB")
typepop = (f'<div class="popover" style="left:300px;top:96px;width:240px;--arrow-x:46px"><div class="menu-section-title">Type</div>'
           f'<div class="menu-item"><span class="check" style="width:16px;height:16px"></span>{ic("image")}<span class="label">Image</span><span class="accel">24</span></div>'
           f'<div class="menu-item"><span class="check" style="width:16px;height:16px"></span>{ic("record")}<span class="label">Video</span><span class="accel">2</span></div>'
           f'<div class="menu-item hover"><span class="check checked" style="width:16px;height:16px"></span>{ic("step")}<span class="label">Step Guide</span><span class="accel">2</span></div>'
           f'<div class="menu-item"><span class="check checked" style="width:16px;height:16px"></span>{ic("template")}<span class="label">Template</span><span class="accel">1</span></div></div>')
chips27 = ('<div class="filter-bar">'
           f'<span class="chip" style="box-shadow:inset 0 0 0 2px var(--accent-a50)"><span class="k">Type</span> Step guide, Template<span class="x">{ic("close")}</span></span>'
           f'<span class="chip add">{ic("plus")}Filter</span><span class="spacer"></span><span class="caption dim-label">3 of 31</span></div>')
W("27-filter-type-step-guides", "Library — step guides",
  lw(content(search_head("", "", focus=False).replace('<span class="grow"></span>', '<span class="grow placeholder">Search captures</span>'),
             chips27 + grid([sg1, sg2, sg3], groups=[("today", "Today", "Friday, 30 October"), ("wed", "Wednesday", "28 October"), ("mon", "Monday", "26 October")],
                            per_tile={"g1": {"meta": f'{ic("step")}<span>6 steps · 12:30</span>'}, "g2": {"meta": f'{ic("step")}<span>3 steps · 16:02</span>'}, "g3": {"meta": f'{ic("template")}<span>Before/after · 10:22</span>'}}, scroll=None)),
     overlays=typepop))

# ---------------------------------------------------------------- 28/29 narrow
nh = (f'<div class="headerbar">{"<div class=start style=flex:0>" + btn("sidebar-show", cls="flat") + "</div>"}'
      f'<div class="search-head"><span class="search-entry focus" style="width:100%">{ic("search")}<span class="grow">connect</span>{ic("edit-clear")}</span></div>'
      f'<div class="end" style="flex:0 0 auto">{btn("view-more", cls="flat")}<div class="window-controls"><span class="wc">{ic("close")}</span></div></div></div>')
nchips = ('<div class="filter-bar" style="padding:8px 12px">'
          f'<span class="chip"><span class="k">Date</span> Last 30 days<span class="x">{ic("close")}</span></span>'
          f'<span class="chip add">{ic("plus")}Filter</span></div>')
W("28-narrow-search", "Library — narrow search",
  window(content(nh, nchips + grid(r_conn, groups=res_groups, per_tile={"t2": {"match": match_t2}, "w4": {"match": match_w4}, "o1": {"match": match_o1}}, scroll=None)), 700, 800),
  size=(700, 800))

sheet = (f'<div class="backdrop-dim"></div><div class="bottom-sheet" style="padding:24px 18px 18px">'
         f'<div class="heading" style="text-align:center;margin-bottom:12px">Tags for “Checkout postcode error”</div>'
         f'<div style="display:flex;flex-wrap:wrap;gap:6px;padding:0 0 12px">{tags_html(["bug", "checkout"], add=False)}</div>'
         f'<span class="entry focus" style="width:100%">{ic("tag")}re<span class="caret" style="margin-left:0"></span></span>'
         f'<div style="padding-top:6px"><div class="ac-row selected" style="min-height:44px"><span class="tag-dot" style="--c:{TAG_COLORS['release-notes']}"></span><span class="label"><b>re</b>lease-notes</span><span class="n">5</span></div>'
         f'<div class="ac-row" style="min-height:44px"><span class="tag-dot" style="--c:{TAG_COLORS['report']}"></span><span class="label"><b>re</b>port</span><span class="n">4</span></div></div></div>')
nh2 = headerbar(btn("sidebar-show", cls="flat"), "All Captures", "31 captures", end=btn("search", cls="flat") + btn("view-more", cls="flat"), close=True)
W("29-narrow-tag-sheet", "Library — narrow tag editing",
  window(content(nh2, grid(IT, states={"t1": {"selected"}})), 700, 800, overlays=sheet), size=(700, 800))

print("ok")
