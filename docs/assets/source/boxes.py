import json,sys
from playwright.sync_api import sync_playwright
html,out,png=sys.argv[1:4]
with sync_playwright() as p:
    b=p.chromium.launch();pg=b.new_page(viewport={"width":1280,"height":800});pg.goto("file://"+html);pg.wait_for_timeout(300)
    r={i:pg.locator("#"+i).bounding_box() for i in ["warn","mail","auto","freq","save","key","days"]}
    pg.screenshot(path=png);b.close()
json.dump(r,open(out,"w"),indent=1);print(json.dumps(r))
