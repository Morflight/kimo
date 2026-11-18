"""Frame steps preserve a paused audio clock's sub-millisecond position."""
import asyncio
import functools
import http.server
from pathlib import Path
import threading
from playwright.async_api import async_playwright


async def main():
    handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=str(Path(__file__).resolve().parents[1]))
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        async with async_playwright() as playwright:
            browser = await playwright.chromium.launch()
            page = await browser.new_page(viewport={'width': 1280, 'height': 800})
            errors = []
            page.on('pageerror', lambda error: errors.append(str(error)))
            await page.goto(f'http://127.0.0.1:{server.server_address[1]}/ui/studio.html?stream=1')
            await page.wait_for_selector('body[data-ready="1"]')
            await page.keyboard.press('Escape')
            await page.evaluate("window.__kimo.show('edit')")
            await page.wait_for_selector('#viewer[data-state=ready]')
            failures = []
            for start in [3.219263, 5.111111]:
                # Feed the real clock callback a valid paused position. Keyboard
                # events and the complete viewer/preview path remain unmocked.
                await page.evaluate('(t) => window.__kimo.PB.o.onTime(t)', start)
                fps = await page.evaluate('window.__kimo.V.fps')
                await page.keyboard.press('ArrowRight')
                after = await page.evaluate('window.__kimo.V.t')
                if abs(after - (start + 1 / fps)) >= 1e-9:
                    failures.append(f'forward step drift: {start} -> {after}')
                await page.keyboard.down('k')
                await page.keyboard.press('j')
                await page.keyboard.press('j')
                await page.keyboard.up('k')
                after = await page.evaluate('window.__kimo.V.t')
                if abs(after - (start - 1 / fps)) >= 1e-9:
                    failures.append(f'two backward steps drift: {start} -> {after}')
                assert await page.get_attribute('#viewer', 'data-playing') == '0'
            assert not errors, errors
            await browser.close()
            assert not failures, failures
            print('PASS frame steps retain sub-millisecond clock precision')
    finally:
        server.shutdown()
        server.server_close()


if __name__ == '__main__':
    asyncio.run(main())
