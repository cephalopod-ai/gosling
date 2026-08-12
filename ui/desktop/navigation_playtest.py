from __future__ import annotations

import json
import re
from pathlib import Path

from playwright.sync_api import sync_playwright


BASE_URL = "http://127.0.0.1:4179/navigation-playtest.html"
EVIDENCE_DIR = Path("/tmp/gosling-navigation-playtest.CK9zQn")
CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"


def relative_luminance(rgb: str) -> float:
    values = [int(value) / 255 for value in rgb.removeprefix("rgb(").removesuffix(")").split(", ")]
    channels = [value / 12.92 if value <= 0.04045 else ((value + 0.055) / 1.055) ** 2.4 for value in values]
    return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2]


def contrast_ratio(first: str, second: str) -> float:
    lighter, darker = sorted((relative_luminance(first), relative_luminance(second)), reverse=True)
    return (lighter + 0.05) / (darker + 0.05)


def open_case(browser, turns: int, width: int, height: int, reduced_motion: bool = False):
    context = browser.new_context(
        viewport={"width": width, "height": height},
        reduced_motion="reduce" if reduced_motion else "no-preference",
    )
    page = context.new_page()
    console_errors: list[str] = []
    page.on("console", lambda message: console_errors.append(message.text) if message.type == "error" else None)
    page.goto(f"{BASE_URL}?turns={turns}")
    page.wait_for_load_state("networkidle")
    return context, page, console_errors


def main() -> None:
    results: dict[str, object] = {}
    EVIDENCE_DIR.mkdir(parents=True, exist_ok=True)

    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(headless=True, executable_path=CHROME)

        context, page, errors = open_case(browser, 5, 1280, 820)
        navigation = page.get_by_role("navigation", name="Thread navigation")
        navigation.wait_for()
        turn_four = page.get_by_role("button", name=re.compile(r"^Turn 4 of 5:"))
        turn_four.click()
        page.wait_for_timeout(500)
        results["normal"] = {
            "navigation_visible": navigation.is_visible(),
            "active_turn": page.locator('[aria-current="location"]').get_attribute("aria-label"),
            "scroll_top": page.locator("[data-playtest-viewport]").evaluate("node => node.scrollTop"),
            "console_errors": errors,
        }
        page.screenshot(path=str(EVIDENCE_DIR / "normal-turn-4.png"))
        context.close()

        context = browser.new_context(viewport={"width": 1280, "height": 820})
        page = context.new_page()
        page.goto(f"{BASE_URL}?turns=1&long=1")
        page.wait_for_load_state("networkidle")
        results["single_long_turn"] = {
            "navigation_count": page.get_by_role("navigation", name="Thread navigation").count(),
            "scroll_height": page.locator("[data-playtest-viewport]").evaluate("node => node.scrollHeight"),
            "client_height": page.locator("[data-playtest-viewport]").evaluate("node => node.clientHeight"),
        }
        page.screenshot(path=str(EVIDENCE_DIR / "single-long-turn.png"))
        context.close()

        context, page, errors = open_case(browser, 1, 1280, 820)
        results["single_turn"] = {
            "navigation_count": page.get_by_role("navigation", name="Thread navigation").count(),
            "scroll_height": page.locator("[data-playtest-viewport]").evaluate("node => node.scrollHeight"),
            "client_height": page.locator("[data-playtest-viewport]").evaluate("node => node.clientHeight"),
            "console_errors": errors,
        }
        page.screenshot(path=str(EVIDENCE_DIR / "single-turn.png"))
        context.close()

        responsive: dict[str, object] = {}
        for label, width in (("desktop", 1280), ("tablet", 900), ("mobile", 390)):
            context, page, errors = open_case(browser, 5, width, 820)
            responsive[label] = {
                "width": width,
                "navigation_visible": page.get_by_role("navigation", name="Thread navigation").is_visible(),
                "body_scroll_width": page.locator("body").evaluate("node => node.scrollWidth"),
                "body_client_width": page.locator("body").evaluate("node => node.clientWidth"),
                "console_errors": errors,
            }
            page.screenshot(path=str(EVIDENCE_DIR / f"responsive-{label}.png"))
            context.close()
        results["responsive"] = responsive

        context, page, errors = open_case(browser, 100, 1280, 820)
        marker_boxes = page.locator('button[aria-label^="Turn "]').evaluate_all(
            "nodes => nodes.map(node => { const r = node.getBoundingClientRect(); return {height:r.height, top:r.top}; })"
        )
        unique_tops = len({round(box["top"], 1) for box in marker_boxes})
        marker_color = page.locator('button[aria-label^="Turn "] span').first.evaluate(
            "node => getComputedStyle(node).backgroundColor"
        )
        surface_color = page.locator("main").evaluate("node => getComputedStyle(node).backgroundColor")
        scroll_measurement = page.locator("[data-playtest-viewport]").evaluate(
            """async node => {
              let rectCalls = 0;
              const original = Element.prototype.getBoundingClientRect;
              Element.prototype.getBoundingClientRect = function(...args) {
                rectCalls += 1;
                return original.apply(this, args);
              };
              const started = performance.now();
              for (let index = 0; index < 20; index += 1) {
                node.scrollTop = (node.scrollHeight - node.clientHeight) * index / 19;
                node.dispatchEvent(new Event('scroll'));
                await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
              }
              const durationMs = performance.now() - started;
              Element.prototype.getBoundingClientRect = original;
              return { durationMs, rectCalls };
            }"""
        )
        results["hundred_turns"] = {
            "marker_count": len(marker_boxes),
            "min_marker_height": min(box["height"] for box in marker_boxes),
            "max_marker_height": max(box["height"] for box in marker_boxes),
            "unique_marker_tops": unique_tops,
            "inactive_marker_color": marker_color,
            "surface_color": surface_color,
            "inactive_marker_contrast": round(contrast_ratio(marker_color, surface_color), 2),
            "twenty_scrolls_duration_ms": round(scroll_measurement["durationMs"], 1),
            "rect_calls": scroll_measurement["rectCalls"],
            "console_errors": errors,
        }
        page.screenshot(path=str(EVIDENCE_DIR / "hundred-turns.png"))
        context.close()

        context, page, errors = open_case(browser, 5, 1280, 820)
        focus_order: list[str | None] = []
        for _ in range(8):
            page.keyboard.press("Tab")
            focus_order.append(page.locator(":focus").get_attribute("aria-label"))
        results["keyboard"] = {
            "focus_order": focus_order,
            "console_errors": errors,
        }
        context.close()

        context, page, errors = open_case(browser, 5, 1280, 820, reduced_motion=True)
        page.get_by_role("button", name=re.compile(r"^Turn 3 of 5:")).click()
        results["reduced_motion"] = {
            "requested_behavior": page.evaluate("window.navigationPlaytest.lastBehavior"),
            "console_errors": errors,
        }
        context.close()

        context, page, errors = open_case(browser, 5, 1280, 820)
        latest = page.get_by_role("button", name="Jump to latest")
        latest.click(click_count=4, delay=20)
        page.wait_for_timeout(600)
        viewport = page.locator("[data-playtest-viewport]")
        results["rapid_repeat"] = {
            "clicks_received": page.evaluate("window.navigationPlaytest.latestClicks"),
            "distance_from_bottom": viewport.evaluate(
                "node => node.scrollHeight - node.scrollTop - node.clientHeight"
            ),
            "console_errors": errors,
        }
        context.close()

        browser.close()

    print(json.dumps(results, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
