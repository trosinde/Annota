import sys
sys.path.insert(0, "/tmp/claude-0/-home-claude/a6fbb290-1e39-5b02-b596-1e25d7216ff2/scratchpad/asx")
from asx_common import *

C = "bootstrap-app-shell"

MAIN_MENU = [("New Capture", "Ctrl+N"), ("Open…", "Ctrl+O"), ("Capture Library", "Ctrl+L"), "-",
             ("Preferences", "Ctrl+,"), ("Keyboard Shortcuts", "Ctrl+?"), ("About Annota", None)]

# ---------------------------------------------------------------- 01 welcome
WELCOME_CSS = """
  .welcome { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; background: var(--window-bg); }
  .welcome .app-icon { width: 128px; height: 128px; margin-bottom: 24px; }
  .welcome .status-title { font-size: var(--fs-title-1); font-weight: var(--fw-heavy); margin-bottom: 12px; }
  .welcome .status-description { max-width: 460px; text-align: center; margin-bottom: 30px; }
  .mode-row { display: flex; gap: 12px; margin-bottom: 30px; }
  .mode-card { width: 156px; height: 108px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; border-radius: var(--radius-l); font-weight: var(--fw-bold); }
  .mode-card .kbd-row { font-weight: var(--fw-regular); }
  .mode-card.focus { outline: 2px solid var(--accent-a50); outline-offset: 1px; }
  .hint { display: flex; align-items: center; gap: 6px; font-size: var(--fs-small); }
"""
welcome = f"""<div class="window" style="width:1280px;height:800px">
{headerbar("Annota", "", minimal=True)}
<div class="welcome">
  <img class="app-icon" src="{APPICON}" alt="">
  <div class="status-title">Capture Anything</div>
  <div class="status-description">Pick what to capture. The capture opens here, ready to mark up. Every annotation stays editable until you export.</div>
  <div class="mode-row">
    <div class="card activatable mode-card focus">{ic("capture-region","icon large")}<span>Region</span><span class="kbd-row"><kbd class="key">Print</kbd></span></div>
    <div class="card activatable mode-card">{ic("capture-window","icon large")}<span>Window</span><span class="kbd-row"><kbd class="key">Alt</kbd><kbd class="key">Print</kbd></span></div>
    <div class="card activatable mode-card">{ic("capture-screen","icon large")}<span>Screen</span><span class="kbd-row"><kbd class="key">Shift</kbd><kbd class="key">Print</kbd></span></div>
  </div>
  <div class="hint dim-label">{ic("folder")}<span>or open an image or .annota file with</span><kbd class="key">Ctrl</kbd><kbd class="key">O</kbd><span>— or drop it here</span></div>
</div>
</div>"""
write(C, "01-first-launch-welcome", page("welcome", welcome, css=WELCOME_CSS))

# ---------------------------------------------------------------- 02 main menu
menu = popover_menu([MAIN_MENU[0], ("Open…", "Ctrl+O", "hover"), *MAIN_MENU[2:]],
                    "right:44px;top:50px;width:290px;--arrow-x:250px")
menu = menu.replace('<div class="popover"', '<div class="popover"', 1)
w2 = welcome.replace('<button class="btn flat image-button ">', '<button class="btn flat image-button active">')
w2 = w2[: w2.rfind("</div>")] + menu + "\n</div>"
write(C, "02-main-menu", page("main menu", w2, css=WELCOME_CSS))

# ---------------------------------------------------------------- 03/04 preferences
PREF_CSS = WELCOME_CSS + """
  .prefs { width: 640px; height: 680px; max-width: 640px; }
  .prefs .headerbar { min-height: 47px; }
  .prefs .body { flex: 1; overflow: hidden; background: var(--dialog-bg); }
  .prefs .preferences-page { padding: 12px 24px 24px; max-width: none; gap: 24px; }
  .row .dropdown.flat { font-weight: var(--fw-regular); }
  .pattern-entry { font-family: var(--font-mono); font-size: 14px; }
  .switcher { margin: 0 auto; }
  .row .radio { margin-right: 4px; }
  .popover.list .menu-item { gap: 8px; }
"""


def prefs_dialog(page_name, content, extra="", body_extra=""):
    gen = "active" if page_name == "general" else ""
    cap = "active" if page_name == "capture" else ""
    return f"""<div class="dialog-host"><div class="dialog prefs">
  <div class="headerbar">
    <div class="start"><button class="btn flat image-button">{ic("search")}</button></div>
    <div class="title-widget"><div class="toggle-group switcher"><span class="toggle {gen}">{ic("settings")} General</span><span class="toggle {cap}">{ic("camera")} Capture</span></div></div>
    <div class="end"><div class="window-controls"><span class="wc">{ic("close")}</span></div></div>
  </div>
  <div class="body" style="position:relative"><div class="preferences-page">{content}</div>{body_extra}</div>
  {extra}
</div></div>"""


GENERAL = f"""
<div class="preferences-group">
  <div class="group-header"><div class="text"><span class="heading">Saving</span><span class="description">Where Ctrl+S puts new captures</span></div></div>
  <div class="boxed-list">
    <div class="row"><div class="text"><span class="title">Save folder</span><span class="subtitle">~/Pictures/Annota</span></div><div class="suffix"><button class="btn flat image-button" title="Choose Folder…">{ic("folder")}</button></div></div>
    <div class="row entry-row"><div class="text"><span class="title">File name pattern</span><span class="pattern-entry">Annota %Y-%m-%d %H-%M-%S</span></div><div class="suffix"><button class="btn flat image-button" title="Pattern Help">{ic("info")}</button></div></div>
  </div>
  <div class="caption dim-label" style="padding:0 2px">Example: Annota 2026-10-30 14-22-08.annota</div>
</div>
<div class="preferences-group">
  <div class="group-header"><div class="text"><span class="heading">Export</span></div></div>
  <div class="boxed-list">
    <div class="row"><div class="text"><span class="title">Default format</span><span class="subtitle">Pre-selected in the Export dialog</span></div><div class="suffix"><span class="dropdown flat">JPG</span></div></div>
    <div class="row"><div class="text"><span class="title">Export folder</span><span class="subtitle">~/Pictures/Annota/Exports</span></div><div class="suffix"><button class="btn flat image-button">{ic("folder")}</button></div></div>
  </div>
</div>
<div class="preferences-group">
  <div class="group-header"><div class="text"><span class="heading">Appearance</span></div></div>
  <div class="boxed-list">
    <div class="row"><div class="text"><span class="title">Style</span><span class="subtitle">Follows the system light or dark style and accent colour</span></div><div class="suffix"><span class="value">System</span></div></div>
  </div>
</div>"""

write(C, "03-preferences-general", page("preferences general",
      welcome[: welcome.rfind("</div>")] + prefs_dialog("general", GENERAL) + "\n</div>", css=PREF_CSS))

# 03b — format dropdown open (persisted choice JPG)
fmt_pop = f"""<div class="popover list" style="right:52px;top:330px;width:200px;--arrow-x:150px">
  <div class="menu-item"><span class="label">PNG</span></div>
  <div class="menu-item selected"><span class="label">JPG</span>{ic("check")}</div>
  <div class="menu-item"><span class="label">WebP</span></div>
  <div class="menu-item"><span class="label">PDF</span></div>
</div>"""
GENERAL_OPEN = GENERAL.replace('<span class="dropdown flat">JPG</span>', '<span class="dropdown flat hover">JPG</span>')
dlg = prefs_dialog("general", GENERAL_OPEN, body_extra=fmt_pop)
write(C, "04-preferences-format-dropdown", page("preferences dropdown",
      welcome[: welcome.rfind("</div>")] + dlg + "\n</div>", css=PREF_CSS))

CAPTURE = f"""
<div class="preferences-group">
  <div class="group-header"><div class="text"><span class="heading">After Capture</span><span class="description">What happens when a capture is taken</span></div></div>
  <div class="boxed-list">
    <div class="row"><div class="prefix"><span class="radio checked"></span></div><div class="text"><span class="title">Open in editor</span></div></div>
    <div class="row focus"><div class="prefix"><span class="radio"></span></div><div class="text"><span class="title">Copy to clipboard</span><span class="subtitle">No window opens</span></div></div>
    <div class="row"><div class="prefix"><span class="radio"></span></div><div class="text"><span class="title">Open in editor and copy to clipboard</span></div></div>
  </div>
</div>
<div class="preferences-group">
  <div class="group-header"><div class="text"><span class="heading">Capture Defaults</span></div></div>
  <div class="boxed-list">
    <div class="row"><div class="text"><span class="title">Default mode</span></div><div class="suffix"><span class="dropdown flat">Region</span></div></div>
    <div class="row"><div class="text"><span class="title">Delay</span><span class="subtitle">Seconds to wait before capturing (0–60)</span></div><div class="suffix">{spin("0", 110)}</div></div>
    <div class="row"><div class="text"><span class="title">Include pointer</span></div><div class="suffix"><span class="switch"></span></div></div>
  </div>
</div>
<div class="preferences-group">
  <div class="group-header"><div class="text"><span class="heading">Shortcut</span></div></div>
  <div class="boxed-list">
    <div class="row"><div class="text"><span class="title">Capture region</span><span class="subtitle">Registered as a system shortcut</span></div><div class="suffix"><kbd class="key">Print</kbd></div></div>
  </div>
</div>"""
write(C, "05-preferences-capture", page("preferences capture",
      welcome[: welcome.rfind("</div>")] + prefs_dialog("capture", CAPTURE) + "\n</div>", css=PREF_CSS))

# ---------------------------------------------------------------- 06 about
ABOUT_CSS = WELCOME_CSS + """
  .about { width: 360px; max-width: 360px; min-width: 360px; height: 620px; }
  .about .headerbar { background: transparent; box-shadow: none; position: absolute; left: 0; right: 0; top: 0; }
  .about .about-body { display: flex; flex-direction: column; align-items: center; padding: 36px 24px 24px; gap: 0; text-align: center; }
  .about .about-body img { width: 128px; height: 128px; margin-bottom: 18px; }
  .about .app-name { font-size: var(--fs-title-1); font-weight: var(--fw-heavy); }
  .about .dev { margin: 6px 0 12px; }
  .about .version { background: var(--fill-1); color: var(--accent); border-radius: var(--radius-pill); padding: 4px 12px; font-weight: var(--fw-bold); font-size: var(--fs-small); margin-bottom: 24px; }
  .about .boxed-list { width: 100%; text-align: left; margin-bottom: 18px; }
  .about .row { min-height: 46px; }
"""
about = f"""<div class="dialog-host"><div class="dialog about">
  <div class="headerbar"><div class="start"></div><div class="title-widget"></div><div class="end"><div class="window-controls"><span class="wc">{ic("close")}</span></div></div></div>
  <div class="about-body">
    <img src="{APPICON}" alt="Annota app icon">
    <div class="app-name">Annota</div>
    <div class="dev">The Annota Contributors</div>
    <div class="version">0.1.0</div>
    <div class="boxed-list">
      <div class="row"><div class="text"><span class="title">Website</span></div><div class="suffix">{ic("go-next")}</div></div>
      <div class="row"><div class="text"><span class="title">Report an Issue</span></div><div class="suffix">{ic("go-next")}</div></div>
    </div>
    <div class="boxed-list">
      <div class="row"><div class="text"><span class="title">Credits</span></div><div class="suffix">{ic("go-next")}</div></div>
      <div class="row"><div class="text"><span class="title">Legal</span></div><div class="suffix">{ic("go-next")}</div></div>
    </div>
    <div class="caption dim-label" style="max-width:300px">Works entirely on this computer. No account, no tracking.<br><br>© 2026 The Annota Contributors · GPL-3.0-or-later</div>
  </div>
</div></div>"""
write(C, "06-about-dialog", page("about", welcome[: welcome.rfind("</div>")] + about + "\n</div>", css=ABOUT_CSS))

# ---------------------------------------------------------------- 07 shortcuts window
SC_CSS = """
  .sc-body { flex: 1; display: flex; gap: 48px; padding: 24px 48px; background: var(--window-bg); overflow: hidden; }
  .sc-col { flex: 1; display: flex; flex-direction: column; gap: 20px; }
  .sc-group h3 { margin: 0 0 8px; font-size: var(--fs-heading); font-weight: var(--fw-bold); color: var(--accent); }
  .sc-row { display: flex; align-items: center; justify-content: space-between; min-height: 30px; font-size: var(--fs-small); gap: 12px; }
  .sc-row .kbd-row kbd.key { min-width: 24px; height: 24px; }
"""
def sc(groups):
    out = []
    for name, rows in groups:
        r = "".join(f'<div class="sc-row"><span>{l}</span><span class="kbd-row">{"".join(f"<kbd class=key>{k}</kbd>" for k in keys)}</span></div>' for l, keys in rows)
        out.append(f'<div class="sc-group"><h3>{name}</h3>{r}</div>')
    return "".join(out)

col1 = sc([("General", [("New capture", ["Ctrl", "N"]), ("Open file", ["Ctrl", "O"]), ("Capture library", ["Ctrl", "L"]),
                        ("Preferences", ["Ctrl", ","]), ("Keyboard shortcuts", ["Ctrl", "?"]), ("Close window", ["Ctrl", "W"]), ("Quit", ["Ctrl", "Q"])]),
           ("Capture", [("Capture region", ["Print"]), ("Capture window", ["Alt", "Print"]), ("Capture screen", ["Shift", "Print"])])])
col2 = sc([("Document", [("Save", ["Ctrl", "S"]), ("Save as", ["Ctrl", "Shift", "S"]), ("Export", ["Ctrl", "E"]),
                         ("Copy image", ["Ctrl", "C"]), ("Undo", ["Ctrl", "Z"]), ("Redo", ["Ctrl", "Shift", "Z"]),
                         ("Zoom in / out", ["Ctrl", "+"]), ("Best fit", ["Ctrl", "0"])])])
col3 = sc([("Tools", [("Select", ["V"]), ("Arrow", ["A"]), ("Line", ["L"]), ("Pen", ["P"]), ("Shape", ["U"]), ("Text", ["T"]),
                      ("Callout", ["K"]), ("Step", ["S"]), ("Highlighter", ["H"]), ("Redact", ["B"])])])
col4 = sc([("More Tools", [("Crop", ["C"]), ("Magnify", ["M"]), ("Spotlight", ["G"]), ("Stamp", ["I"]), ("Cut-out", ["X"]),
                           ("Fill", ["F"]), ("Eraser", ["E"]), ("Magic wand", ["W"]), ("Selection", ["Q"])])])
shortcuts = f"""<div class="window" style="width:1180px;height:700px">
  <div class="headerbar">
    <div class="start"></div>
    <div class="title-widget"><span class="title">Shortcuts</span></div>
    <div class="end"><button class="btn flat image-button">{ic("search")}</button><div class="window-controls"><span class="wc">{ic("close")}</span></div></div>
  </div>
  <div class="sc-body"><div class="sc-col">{col1}</div><div class="sc-col">{col2}</div><div class="sc-col">{col3}</div><div class="sc-col">{col4}</div></div>
</div>"""
write(C, "07-keyboard-shortcuts", page("shortcuts", shortcuts, css=SC_CSS, body_cls="mock-frame"))

# ---------------------------------------------------------------- 08 editor opened via --open
ed = editor(title="shot.annota", subtitle="~/Pictures/Annota · 1600 × 1000", hb_kwargs={})
write(C, "08-editor-open-file", page("open file", ed))

# ---------------------------------------------------------------- 09/10 GNOME overview
OV_CSS = """
  body { background: #1b1b1f; }
  .overview { width: 1280px; height: 800px; position: relative; background: radial-gradient(900px 500px at 50% 0%, #3a3d4a, #1b1b1f 70%); color: #fff; font-family: var(--font-ui); overflow: hidden; }
  .topbar { height: 30px; background: #000; display: flex; align-items: center; justify-content: space-between; padding: 0 12px; font-size: 14px; font-weight: 700; }
  .topbar .pill { width: 34px; height: 8px; border-radius: 4px; background: #fff; display: inline-block; }
  .topbar .dot { width: 6px; height: 6px; border-radius: 3px; background: rgb(255 255 255 / 50%); display: inline-block; margin-left: 4px; }
  .ov-search { width: 320px; height: 36px; margin: 22px auto 0; border-radius: 18px; background: rgb(255 255 255 / 12%); display: flex; align-items: center; gap: 8px; padding: 0 14px; font-size: 15px; }
  .ov-search .ph { opacity: .55; }
  .grid { display: grid; grid-template-columns: repeat(6, 132px); gap: 46px 36px; justify-content: center; margin-top: 52px; }
  .app { display: flex; flex-direction: column; align-items: center; gap: 10px; font-size: 14px; padding: 12px 0; border-radius: 18px; }
  .app.hl { background: rgb(255 255 255 / 12%); }
  .app .ai { width: 84px; height: 84px; border-radius: 18px; display: grid; place-items: center; color: #fff; box-shadow: 0 3px 0 rgb(0 0 0 / 25%), 0 6px 14px rgb(0 0 0 / 30%); }
  .app .ai .icon { width: 40px; height: 40px; }
  .app img { width: 92px; height: 92px; margin: -4px; }
  .dash { position: absolute; left: 50%; bottom: 18px; transform: translateX(-50%); display: flex; gap: 10px; padding: 10px; border-radius: 22px; background: rgb(255 255 255 / 12%); }
  .dash .ai { width: 52px; height: 52px; border-radius: 12px; display: grid; place-items: center; }
  .dash .ai .icon { width: 26px; height: 26px; color: #fff; }
  .dash img { width: 56px; height: 56px; margin: -2px; }
  .page-dots { position: absolute; bottom: 104px; left: 50%; transform: translateX(-50%); display: flex; gap: 10px; }
  .page-dots span { width: 8px; height: 8px; border-radius: 4px; background: rgb(255 255 255 / 30%); }
  .page-dots span:first-child { background: #fff; }
  .results { width: 760px; margin: 28px auto 0; border-radius: 18px; background: rgb(255 255 255 / 10%); padding: 18px; display: flex; gap: 18px; }
  .results .app { width: 132px; }
  .tip { position: absolute; background: rgb(0 0 0 / 80%); padding: 6px 12px; border-radius: 8px; font-size: 13px; }
"""
APPS = [("Files", "folder", "#3a7bd5"), ("Text Editor", "text", "#58a55c"), ("Terminal", "keyboard", "#3d3d44"),
        ("Settings", "settings", "#6a6f7c"), ("Image Viewer", "image", "#d9822b"), ("Calendar", "timer", "#c64545"),
        ("Videos", "play", "#2f6fa6"), ("Sound Recorder", "microphone", "#b5367a"), ("Help", "info", "#4c8f8a"),
        ("Archive Manager", "folder", "#8a6d3b"), ("Disk Usage", "grid-view", "#6b4fa3")]

def app_tile(name, icon, col, hl=False):
    return f'<div class="app{" hl" if hl else ""}"><div class="ai" style="background:{col}">{ic(icon)}</div><span>{name}</span></div>'

tiles = [app_tile(*a) for a in APPS[:4]]
tiles.insert(2, f'<div class="app hl"><img src="{APPICON}" alt=""><span>Annota</span></div>')
tiles += [app_tile(*a) for a in APPS[4:]]
dash = (f'<div class="dash"><div class="ai" style="background:#3a7bd5">{ic("folder")}</div>'
        f'<div class="ai" style="background:#58a55c">{ic("text")}</div><img src="{APPICON}" alt="">'
        f'<div class="ai" style="background:#6a6f7c">{ic("settings")}</div>'
        f'<div class="ai" style="background:#3d3d44">{ic("grid-view")}</div></div>')
topbar = '<div class="topbar"><span><span class="pill"></span><span class="dot"></span><span class="dot"></span></span><span>Fri 30 Oct  14:22</span><span>⏻</span></div>'
ov = f"""<div class="overview">{topbar}
<div class="ov-search">{ic("search")}<span class="ph">Type to search</span></div>
<div class="grid">{"".join(tiles)}</div>
<div class="tip" style="left:465px;top:286px">Capture and annotate screenshots</div>
<div class="page-dots"><span></span><span></span></div>
{dash}
</div>"""
write(C, "09-gnome-app-grid", page("app grid", ov, css=OV_CSS, body_cls=""))

ov2 = f"""<div class="overview">{topbar}
<div class="ov-search" style="background:rgb(255 255 255 / 18%)">{ic("search")}<span>anno</span><span style="width:1px;height:18px;background:#fff;margin-left:-6px"></span></div>
<div class="results">
  <div class="app hl"><img src="{APPICON}" alt=""><span>Annota</span></div>
</div>
<div style="width:760px;margin:12px auto 0;font-size:13px;opacity:.7;padding-left:18px">Also found by: screenshot, capture, annotate, markup, Bildschirmfoto</div>
{dash}
</div>"""
write(C, "10-gnome-search", page("app search", ov2, css=OV_CSS, body_cls=""))

# ---------------------------------------------------------------- 11 terminal CLI
TERM_CSS = """
  .term { background: #1e1e22; color: #e6e6e6; flex: 1; padding: 14px 18px; font-family: var(--font-mono); font-size: 14px; line-height: 1.55; white-space: pre; overflow: hidden; }
  .term .p { color: #8ff0a4; } .term .d { color: #99c1f1; } .term .e { color: #f66151; font-weight: 700; } .term .m { color: #9a9a9a; } .term .b { font-weight: 700; }
  .term .cur { display: inline-block; width: 9px; height: 17px; background: #e6e6e6; vertical-align: -3px; }
  .window.console .headerbar { background: #2e2e32; color: #fff; box-shadow: inset 0 -1px rgb(0 0 6 / 36%); }
  .window.console .wc { background: rgb(255 255 255 / 10%); color: #fff; }
  .window.console { background: #1e1e22; }
"""
P = '<span class="p">mara@workstation</span>:<span class="d">~</span>$ '
term = f"""<div class="window console" style="width:900px;height:560px">
  <div class="headerbar"><div class="start"><button class="btn flat image-button" style="color:#fff">{ic("plus")}</button></div><div class="title-widget"><span class="title">mara@workstation: ~</span></div><div class="end"><button class="btn flat image-button" style="color:#fff">{ic("search")}</button><button class="btn flat image-button" style="color:#fff">{ic("open-menu")}</button><div class="window-controls"><span class="wc">{ic("close")}</span></div></div></div>
  <div class="term">{P}annota --version
annota 0.1.0
{P}annota --capture banana
<span class="e">error:</span> invalid value '<span class="b">banana</span>' for '<span class="b">--capture &lt;MODE&gt;</span>'
  [possible values: <span class="p">region</span>, <span class="p">window</span>, <span class="p">screen</span>]

Usage: annota [OPTIONS]

For more information, try '<span class="b">--help</span>'.
{P}echo $?
2
{P}annota --capture region --delay 90
<span class="e">error:</span> invalid value '<span class="b">90</span>' for '<span class="b">--delay &lt;SECONDS&gt;</span>': 90 is not in 0..=60

For more information, try '<span class="b">--help</span>'.
{P}annota --open shot.annota
{P}<span class="cur"></span></div>
</div>"""
write(C, "11-cli-errors", page("terminal", term, size="1280x800", css=TERM_CSS, body_cls="mock-frame"))
print("ok")
