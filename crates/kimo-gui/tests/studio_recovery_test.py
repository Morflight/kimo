"""Startup recovery and crash-report consent against a synthetic native adapter."""
import asyncio
from playwright.async_api import async_playwright
from studio_actions_test import serve

async def main():
    server = serve()
    try:
        async with async_playwright() as playwright:
            browser = await playwright.chromium.launch()
            for restore, failure, send, count, project in [(True,False,True,3,'/fixture/edit.kimo'),(True,True,False,2,None),(False,False,False,1,None)]:
                page = await browser.new_page()
                errors=[]
                page.on('pageerror',lambda error:errors.append(str(error)))
                await page.add_init_script('''window.startupCalls=[];window.__TAURI__={
                  core:{invoke:async(command,args)=>{
                    startupCalls.push({command,args});
                    const {bridge}=await import('/ui/dev/mock-bridge.js');
                    if(command==='recovery_status')return {autosaved_at:1700000000000,project_path:PROJECT};
                    if(command==='recovery_restore'){if(FAILURE)throw Error('synthetic restore failure');return null;}
                    if(command==='recovery_discard')return null;
                    if(command==='crash_pending')return Array.from({length:COUNT},(_,i)=>({file:'fixture-'+i+'.log',text:Array.from({length:20},(_,n)=>'line '+n).join('\\n')}));
                    if(command==='crash_answer')return null;
                    return bridge.invoke(command,args);
                  }},
                  event:{listen:async(name,callback)=>{const {bridge}=await import('/ui/dev/mock-bridge.js');return bridge.listen(name,payload=>callback({payload}));}}
                };'''.replace('PROJECT', 'null' if project is None else repr(project)).replace('FAILURE',str(failure).lower()).replace('COUNT',str(count)))
                await page.goto(f'http://127.0.0.1:{server.server_address[1]}/ui/studio.html')
                await page.wait_for_selector('#confirm.on')
                assert 'Restore your unsaved work' in await page.inner_text('#cf-title')
                assert ('an unsaved project' if project is None else 'edit') in await page.inner_text('#cf-body')
                await page.click('#cf-yes' if restore else '#cf-no')
                await page.wait_for_function('document.querySelector("#cf-title").textContent.includes("unexpectedly")')
                assert await page.locator('#cf-code').is_visible()
                assert (await page.inner_text('#cf-code')).splitlines()==['line '+str(i) for i in range(12)]
                await page.click('#cf-yes' if send else '#cf-no')
                await page.wait_for_selector('body[data-ready="1"]')
                calls=await page.evaluate('startupCalls')
                names=[c['command'] for c in calls]
                assert ('recovery_restore' in names)==restore
                assert ('recovery_discard' in names)==(not restore)
                answers=[c['args'] for c in calls if c['command']=='crash_answer']
                assert answers==[{'file':f'fixture-{i}.log','send':send and i==0} for i in range(count)]
                assert not await page.locator('#cf-code').is_visible()
                assert await page.inner_text('#cf-code')==''
                if restore and not failure:
                    assert await page.evaluate('__kimo.S.dirty') is True
                    assert 'Restored' in await page.inner_text('#toast')
                if failure:
                    assert 'synthetic restore failure' in await page.inner_text('#toast')
                assert not errors,errors
                await page.close()
            print('PASS startup restore/discard/error and per-log crash-report consent without opening or sending a report')
            await browser.close()
    finally:
        server.shutdown()
        server.server_close()

if __name__=='__main__':
    asyncio.run(main())
