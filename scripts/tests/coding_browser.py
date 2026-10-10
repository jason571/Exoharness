"""Real Chromium acceptance INSIDE the cached browser container.

URL comes from an isolated test's real media router, never a production account.
Only requests to that exact signed project subtree are allowed by Playwright.
"""
import json
import os
from urllib.parse import urlsplit
from playwright.sync_api import sync_playwright

url = os.environ["EXOHARNESS_CODING_TEST_URL"]
parsed = urlsplit(url)
parts = parsed.path.split("/")
prefix = "/".join(parts[:4]) + "/"
failures = []
with sync_playwright() as runtime:
    browser = runtime.chromium.launch(executable_path="/usr/bin/chromium", args=["--no-sandbox", "--disable-dev-shm-usage", "--disable-background-networking"])
    context = browser.new_context(viewport={"width": 1280, "height": 800})
    def scope(route):
        requested = urlsplit(route.request.url)
        if (requested.scheme, requested.netloc) == (parsed.scheme, parsed.netloc) and requested.path.startswith(prefix):
            route.continue_()
        else:
            failures.append("request outside signed test project")
            route.abort()
    context.route("**/*", scope)
    page = context.new_page()
    page.on("pageerror", lambda error: failures.append(str(error)))
    page.on("requestfailed", lambda request: failures.append("resource failed"))
    page.on("response", lambda response: failures.append("HTTP resource error") if response.status >= 400 else None)
    for width in [1280, 390]:
        page.set_viewport_size({"width": width, "height": 800})
        page.goto(url, wait_until="networkidle")
        page.wait_for_function("document.body.dataset.ready === 'true'")
        assert page.title() == "多文件网站首页"
        assert page.locator("body").evaluate("e => getComputedStyle(e).color") == "rgb(10, 20, 30)"
        assert page.locator("#cover").evaluate("e => e.complete && e.naturalWidth > 0")
        assert page.evaluate("document.documentElement.scrollWidth <= innerWidth")
        rectangles = page.locator("article").evaluate_all("items => items.map(e => ({x:e.getBoundingClientRect().x,y:e.getBoundingClientRect().y}))")
        assert len(set(item["x"] for item in rectangles)) == (2 if width == 1280 else 1)
        page.locator("#toggle").click()
        assert page.locator("#toggle").inner_text() == "状态已切换"
        assert page.locator("body").get_attribute("data-toggled") == "true"
        page.locator("#details").click()
        page.wait_for_load_state("networkidle")
        page.wait_for_function("document.body.dataset.ready === 'true'")
        assert page.title() == "二级页面"
        assert page.locator("body").evaluate("e => getComputedStyle(e).color") == "rgb(10, 20, 30)"
        assert page.locator("#cover").evaluate("e => e.complete && e.naturalWidth > 0")
        page.locator("#home").click()
        page.wait_for_load_state("networkidle")
        assert page.title() == "多文件网站首页"
    assert not failures, failures
    browser.close()
print(json.dumps({"home": "passed", "secondary": "passed", "css": "passed", "module_js": "passed", "image": "passed", "interaction": "passed", "desktop_mobile": "passed", "console_resource_errors": 0}))
