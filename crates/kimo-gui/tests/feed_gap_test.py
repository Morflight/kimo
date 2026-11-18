"""Deterministic transport/paint ordering regression; no real engine is contacted."""
import asyncio
import functools
import http.server
from pathlib import Path
import threading
from playwright.async_api import async_playwright

ROOT = Path(__file__).resolve().parents[1]

class Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *args):
        pass

async def main():
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), functools.partial(Quiet, directory=str(ROOT)))
    threading.Thread(target=server.serve_forever, daemon=True).start()
    errors = []
    try:
        async with async_playwright() as p:
            browser = await p.chromium.launch()
            page = await browser.new_page()
            page.on('pageerror', lambda e: errors.append(str(e)))
            await page.add_init_script('''window.EventSource = class {
                static CLOSED = 2;
                constructor() { window.testSource = this; this.readyState = 1; }
                close() { this.readyState = 2; }
            };''')
            await page.goto(f'http://127.0.0.1:{server.server_port}/ui/index.html?feed=sse')
            await page.wait_for_function('window.__kimo && window.testSource')
            await page.evaluate('''async () => {
                const s = window.testSource;
                window.sendTestEvent = event => s.onmessage({data: JSON.stringify(event)});
                s.onopen();
                // Start and drop happen in one task before the app can paint.
                sendTestEvent({type:'tool_start', id:'interrupted', name:'timeline'});
                s.onerror();
                await new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)));
                s.onopen();
                sendTestEvent({type:'tool_start', id:'next', name:'timeline'});
                sendTestEvent({type:'tool_end', id:'next', dur_ms:2});
            }''')
            await page.wait_for_function("__kimo.state.calls.find(c => c.id === 'next')?.status === 'ok'")
            calls = await page.evaluate('__kimo.state.calls.map(c => [c.id, c.status, c.dur_ms])')
            assert calls == [['interrupted', 'lost', None], ['next', 'ok', 2]], calls
            print('PASS queued start before drop becomes lost when the serial engine advances')
            await page.evaluate("sendTestEvent({type:'tool_end', id:'interrupted', dur_ms:7})")
            await page.wait_for_function("__kimo.state.calls.find(c => c.id === 'interrupted')?.status === 'ok'")
            calls = await page.evaluate('__kimo.state.calls.map(c => [c.id, c.status, c.dur_ms])')
            assert calls == [['interrupted', 'ok', 7], ['next', 'ok', 2]], calls
            assert not errors, errors
            print('PASS replayed completion settles the interrupted call without duplicate rows or page errors')
            await browser.close()
    finally:
        server.shutdown()
        server.server_close()

if __name__ == '__main__':
    asyncio.run(main())
