"""Real Chromium proof of spectator pan/zoom, picked-call overflow and shell visibility."""
import asyncio
import functools
import http.server
from pathlib import Path
import socketserver
import threading

CRATE = Path(__file__).resolve().parents[1]

class Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *args):
        pass

async def settle(page):
    await page.evaluate('() => new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))')

async def main():
    from playwright.async_api import async_playwright
    server = socketserver.ThreadingTCPServer(('127.0.0.1', 0), functools.partial(Quiet, directory=str(CRATE)))
    server.daemon_threads = True
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        async with async_playwright() as p:
            browser = await p.chromium.launch()
            page = await browser.new_page(viewport={'width':1376,'height':768})
            errors = []
            page.on('pageerror', lambda e: errors.append(str(e)))
            await page.add_init_script('''
              window.shell = {checks: [], minimized:false, visible:true, reject:false};
              window.__TAURI__ = {event:{listen:()=>Promise.resolve(()=>{})},window:{getCurrentWindow:()=>({
                isMinimized:async()=>{if(shell.reject)throw Error('fixture');return shell.minimized;},
                isVisible:async()=>shell.visible,
                onResized:f=>shell.checks.push(f),onFocusChanged:f=>shell.checks.push(f)
              })}};
            ''')
            await page.goto(f'http://127.0.0.1:{server.server_address[1]}/ui/index.html?feed=off')
            await page.wait_for_function('window.__kimo && shell.checks.length===2')
            await page.evaluate('''() => {
              __kimo.apply({type:'mode',mode:'edit'});
              __kimo.apply({type:'view',data:{tl:{ruler:{t0:0,t1:40,step:5},tracks:Array.from({length:12},(_,i)=>({id:'t'+i,label:'Track '+i,clips:[{id:'c'+i,label:'Clip '+i,s:0,e:40}]}))}}});
            }''')
            await settle(page)
            timeline = page.locator('[data-panel=tl]')
            async def wheel(**kwargs):
                await timeline.evaluate('(el,o)=>el.dispatchEvent(new WheelEvent("wheel",{bubbles:true,cancelable:true,clientX:600,...o}))',kwargs)
                await settle(page)
            assert await timeline.locator('.tlbar:not(.v)').count() == 0
            await wheel(deltaY=-200,ctrlKey=True)
            thumb = timeline.locator('.tlbar:not(.v) i')
            initial = await thumb.evaluate('e=>({left:parseFloat(e.style.left),width:parseFloat(e.style.width)})')
            await wheel(deltaX=100)
            moved = await thumb.evaluate('e=>parseFloat(e.style.left)')
            assert moved > initial['left'], (moved,initial)
            await wheel(deltaY=-2,shiftKey=True,deltaMode=1)
            assert await thumb.evaluate('e=>parseFloat(e.style.left)') < moved
            await wheel(deltaY=1,deltaMode=2)
            assert await timeline.locator('.trk .lab').first.inner_text() != 'Track 0'
            await wheel(deltaY=-100000)
            assert await timeline.locator('.trk .lab').first.inner_text() == 'Track 0'
            await wheel(deltaY=1)  # sub-row accumulation must not scroll yet
            assert await timeline.locator('.trk .lab').first.inner_text() == 'Track 0'
            await wheel(deltaY=10000,metaKey=True)
            assert await timeline.locator('.tlbar:not(.v)').count() == 0
            unchanged = await timeline.inner_html()
            await wheel(deltaX=100)
            assert await timeline.inner_html() == unchanged
            print('PASS wheel zoom, pointer-relative pan, line/page deltas and bounded track scrolling')

            await page.evaluate('''() => {
              __kimo.apply({type:'tool_start',id:'picked',name:'inspect_fixture',args:Object.fromEntries(Array.from({length:30},(_,i)=>['arg '+i,'value '+i]))});
              __kimo.apply({type:'tool_end',id:'picked',dur_ms:12,result:'completed fixture'});
            }''')
            await settle(page)
            await page.locator('.call[data-call=picked]').click()
            await settle(page)
            assert await page.evaluate('__kimo.state.pick') == 'picked'
            inspector = page.locator('[data-panel=insp]')
            assert 'more' in await inspector.inner_text()
            bounds = await inspector.evaluate('el=>({panel:el.getBoundingClientRect().bottom,last:el.querySelector(".kv").lastElementChild.getBoundingClientRect().bottom})')
            assert bounds['last'] <= bounds['panel'], bounds
            await page.keyboard.press('Escape')
            await settle(page)
            assert await page.evaluate('__kimo.state.pick') is None
            print('PASS picked-call overflow stays inside the inspector and Escape restores the view')

            for minimized,visible in [(True,True),(False,False)]:
                await page.evaluate('async ([m,v])=>{shell.minimized=m;shell.visible=v;await shell.checks[0]();}',[minimized,visible])
                before = await page.evaluate('__kimo.metrics.renders')
                await page.evaluate('__kimo.apply({type:"stats",cpu:42})')
                await page.wait_for_timeout(50)
                assert await page.evaluate('__kimo.metrics.renders') == before
                await page.evaluate('async()=>{shell.minimized=false;shell.visible=true;await shell.checks[1]();}')
                await settle(page)
                assert await page.evaluate('__kimo.metrics.renders') > before
            await page.evaluate('async()=>{shell.reject=true;await shell.checks[0]();shell.reject=false;}')
            assert not errors, errors
            print('PASS shell minimization/invisibility suspend rendering and shell errors remain contained')
            await browser.close()
    finally:
        server.shutdown()
        server.server_close()

if __name__ == '__main__':
    asyncio.run(main())
