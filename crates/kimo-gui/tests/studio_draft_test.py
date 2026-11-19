"""Fallback draft playback crosses chunk boundaries using synthetic image frames."""
import asyncio
from playwright.async_api import async_playwright
from studio_actions_test import serve

async def main():
    server=serve()
    try:
        async with async_playwright() as playwright:
            browser=await playwright.chromium.launch()
            page=await browser.new_page(viewport={'width':1280,'height':800})
            errors=[]
            page.on('pageerror',lambda error:errors.append(str(error)))
            await page.goto(f'http://127.0.0.1:{server.server_address[1]}/ui/studio.html')
            await page.wait_for_selector('body[data-ready="1"]')
            await page.click('#wel-skip')
            await page.evaluate("__kimo.show('edit')")
            await page.wait_for_selector('#viewer[data-state="ready"]')
            await page.evaluate('''async()=>{
              await __kimo.PB.stop();__kimo.PB.supported=false;
              const canvas=document.createElement('canvas');canvas.width=2;canvas.height=2;
              const ctx=canvas.getContext('2d');ctx.fillStyle='red';ctx.fillRect(0,0,2,2);
              const url=canvas.toDataURL();window.draftCalls=[];
              const draft=(start,end)=>({rev:'fixture',start,end,fps:25,frame_urls:Array(5).fill(url)});
              const invoke=__kimoBridge.invoke;
              __kimoBridge.invoke=async(command,args)=>{
                if(command==='viewer_draft'){draftCalls.push(args);return draft(args.start,.4);}
                return invoke(command,args);
              };
              Object.assign(__kimo.V,{t:0,dur:.4,fps:25,rev:'fixture',draft:draft(0,.2)});
            }''')
            await page.click('#vw-play')
            await page.wait_for_function('draftCalls.length>0',timeout=3000)
            assert await page.evaluate('draftCalls') == [{'start':.2,'seconds':4}]
            await page.wait_for_function('document.querySelector("#viewer").dataset.playing==="0"',timeout=3000)
            assert abs(await page.evaluate('__kimo.V.t')-.4)<1e-9
            assert await page.evaluate('draftCalls.length')==1
            await page.evaluate("""async()=>{
              const invoke=__kimoBridge.invoke;
              __kimoBridge.invoke=async(c,a)=>c==='engine_read'&&a.args.action==='get_project'
                ?{ok:true,data:{width:640,height:360,fps:25,tracks:[{id:'t',clips:[{id:'c',start:0,duration:1,source:{type:'text',text:'Fixture'}}]}]}}
                :invoke(c,a);
              await __kimo.loadProject();
            }""")
            await page.locator('[data-clip] .cl').first.click()
            selected=await page.evaluate('__kimo.S.sel')
            assert selected is not None
            await page.click('#brand')
            await page.keyboard.press('Escape')
            assert not await page.locator('#about').is_visible()
            assert await page.evaluate('__kimo.S.sel') == selected
            await page.keyboard.press('Escape')
            assert await page.evaluate('__kimo.S.sel') is None
            assert not errors,errors
            await browser.close()
            print('PASS fallback draft crosses exactly one chunk boundary and stops at the timeline end')
    finally:
        server.shutdown();server.server_close()

if __name__=='__main__':
    asyncio.run(main())
