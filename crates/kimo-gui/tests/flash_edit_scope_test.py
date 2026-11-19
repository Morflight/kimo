"""Deterministic flash-expiry/edit ordering regression; no real engine is contacted."""
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
            await page.add_init_script("""(() => {
                const timeout = window.setTimeout.bind(window);
                window.heldExpiry = null;
                window.setTimeout = (fn, ms, ...args) => timeout(() => {
                    if (fn.name === 'flashExpired') window.heldExpiry = fn;
                    else fn(...args);
                }, ms);
            })();""")
            await page.goto(f'http://127.0.0.1:{server.server_port}/ui/index.html?feed=off')
            await page.wait_for_function('window.__kimo')
            import json
            events = json.loads((ROOT / 'tests/ev_engine_diff.json').read_text())
            await page.evaluate('events => { for (const e of events) __kimo.apply(e); __kimo.apply({type:"mode", mode:"edit", activity:"#calls"}); }', events)
            await page.wait_for_function('window.heldExpiry !== null')
            await page.wait_for_timeout(300)
            result = await page.evaluate("""async () => {
                const before = {scoped: __kimo.metrics.scoped, header: document.querySelector('#pill-act').textContent};
                __kimo.apply({type:'tool_start', id:'expiry-edit', name:'split_clip'});
                __kimo.apply({type:'tool_end', id:'expiry-edit', name:'split_clip', dur_ms:1});
                // Deliver the expired timer while the edit is queued for paint.
                const expire = heldExpiry; window.runExpiry = expire; heldExpiry = null; expire();
                await new Promise(requestAnimationFrame);
                const edit = {scoped: __kimo.metrics.scoped, header: document.querySelector('#pill-act').textContent};
                await new Promise(requestAnimationFrame);
                return {before, edit, after: document.querySelector('#pill-act').textContent,
                        calls: __kimo.state.callCount};
            }""")
            assert result['edit']['scoped'] == result['before']['scoped'] + 1, result
            assert result['edit']['header'] == result['before']['header'], result
            assert result['after'] == f"{result['calls']} calls", result
            assert result['after'] != result['before']['header'], result
            queued = await page.evaluate("""async () => {
                __kimo.apply({type:'tool_start',id:'prepare-catchup',name:'move_clip'});
                __kimo.apply({type:'tool_end',id:'prepare-catchup',name:'move_clip',dur_ms:1});
                await new Promise(requestAnimationFrame);
                const before={scoped:__kimo.metrics.scoped,header:document.querySelector('#pill-act').textContent};
                // A catch-up frame is pending, but the next edit has not arrived yet.
                runExpiry();
                __kimo.apply({type:'tool_start',id:'after-expiry',name:'split_clip'});
                __kimo.apply({type:'tool_end',id:'after-expiry',name:'split_clip',dur_ms:1});
                await new Promise(requestAnimationFrame);
                const edit={scoped:__kimo.metrics.scoped,header:document.querySelector('#pill-act').textContent};
                await new Promise(requestAnimationFrame);
                return {before,edit,after:document.querySelector('#pill-act').textContent,calls:__kimo.state.callCount};
            }""")
            assert queued['edit']['scoped'] == queued['before']['scoped'] + 1, queued
            assert queued['edit']['header'] == queued['before']['header'], queued
            assert queued['after'] == f"{queued['calls']} calls", queued
            assert queued['after'] != queued['before']['header'], queued
            assert not errors, errors
            print('PASS expiry before the next edit also preserves scoped paint during pending catch-up')
            print('PASS expired flash preserves scoped edit paint and header catches up on the following frame')
            await browser.close()
    finally:
        server.shutdown()
        server.server_close()

if __name__ == '__main__':
    asyncio.run(main())
