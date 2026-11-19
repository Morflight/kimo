"""Stale model replies and native preview lifecycle at the real browser boundary."""
import asyncio
from playwright.async_api import async_playwright
from studio_actions_test import serve

async def main():
    server=serve()
    try:
        async with async_playwright() as playwright:
            browser=await playwright.chromium.launch()
            page=await browser.new_page(viewport={'width':1280,'height':800})
            page.set_default_timeout(10000)
            errors=[]
            page.on('pageerror',lambda error:errors.append(str(error)))
            async def future_markup(route):
                response=await route.fetch()
                body=(await response.text()).replace('</body>','<span id="future-icon" data-icon="owned-future-icon"></span></body>')
                await route.fulfill(response=response,body=body)
            await page.route('**/studio.html*',future_markup)
            await page.goto(f'http://127.0.0.1:{server.server_address[1]}/ui/studio.html?stream=1')
            await page.wait_for_selector('body[data-ready="1"]')
            await page.click('#wel-skip')
            assert await page.locator('#future-icon svg').count()==1
            assert await page.locator('#future-icon svg').inner_html()==''
            await page.evaluate('''()=>{
              window.boundaryInvoke=__kimoBridge.invoke;window.boundaryCalls=[];
              __kimoBridge.invoke=async(c,a)=>{
                boundaryCalls.push({command:c,args:a});
                if(c==='cli_models')return new Promise(resolve=>window.finishOldModels=resolve);
                return boundaryInvoke(c,a);
              };
              __kimo.S.models={};__kimo.S.app.settings.agent='codex';__kimo.show('agent');
            }''')
            await page.click('#agent-pick [data-agent="codex"]')
            await page.wait_for_function('typeof finishOldModels==="function"')
            await page.click('#agent-pick [data-agent="api"]')
            await page.evaluate('''()=>{
              finishOldModels({models:[{id:'obsolete-model',label:'Obsolete model'}],efforts:[]});
            }''')
            await page.wait_for_function('__kimo.S.models.codex?.models?.length===1')
            assert 'Change in Connections' in await page.inner_text('#model-bar')
            assert 'Obsolete model' not in await page.inner_text('#model-bar')
            await page.evaluate('''async()=>{
              __kimo.show('edit');window.exportBoundary={text:true,done:false,polls:0};
              const previous=__kimoBridge.invoke;
              __kimoBridge.invoke=async(c,a)=>{
                if(c==='engine_read'&&a.args.action==='get_project')return {ok:true,data:{width:64,height:64,fps:24,tracks:[{id:'owned',clips:[{id:'owned-title',start:0,duration:1,source:{type:'text',text:'Owned'}}]}]}};
                if(c==='engine_call'&&a.args.action==='export_job_start')return exportBoundary.text
                  ?{ok:true,text:'Owned acknowledgement without a job id'}:{ok:true,data:{job_id:'owned-export'}};
                if(c==='engine_read'&&a.args.action==='job_status'){
                  exportBoundary.polls++;return exportBoundary.done?{ok:true,data:{state:'cancelled'}}:{ok:false};
                }
                return previous(c,a);
              };
              await __kimo.loadProject();
            }''')
            await page.click('#ed-export')
            await page.fill('#ex-path','/fixture/owned.mp4')
            await page.click('#ex-go')
            assert await page.inner_text('#ex-err')=='The export did not start.'
            await page.evaluate('''()=>{
              exportBoundary.text=false;window.exportTimers=[];window.ownedSetInterval=window.setInterval;
              window.setInterval=(callback,...args)=>{exportTimers.push(callback);return ownedSetInterval(callback,...args);};
            }''')
            await page.click('#ex-go')
            await page.evaluate('()=>{window.setInterval=ownedSetInterval;}')
            assert await page.evaluate('exportTimers.length')==1
            await page.wait_for_function('exportBoundary.polls>0')
            assert await page.inner_text('#ex-status')=='Rendering…'
            assert await page.locator('#ex-go').is_disabled()
            await page.evaluate('exportBoundary.done=true')
            await page.wait_for_function('document.querySelector("#ex-err").textContent==="cancelled"')
            assert await page.locator('#ex-go').is_enabled()
            await page.evaluate('''async()=>{
              const polls=exportBoundary.polls;await exportTimers[0]();
              if(exportBoundary.polls!==polls)throw Error('finished export polled without a job');
            }''')
            await page.click('#ex-close')
            await page.evaluate('''()=>{
              const providers=__kimo.S.app.providers,select=document.querySelector('#key-provider');
              const model=document.querySelector('#key-model').value,base=document.querySelector('#key-base').value;
              __kimo.S.app.providers=providers.filter(p=>p.id!==select.value);
              select.dispatchEvent(new Event('change',{bubbles:true}));
              if(document.querySelector('#key-model').value!==model||document.querySelector('#key-base').value!==base)throw Error('removed provider overwrote current fields');
              __kimo.S.app.providers=providers;
            }''')
            await page.evaluate('''async()=>{
              __kimo.show('edit');await __kimo.PB.ensure(0);__kimo.PB.pause();
            }''')
            assert await page.evaluate('__kimo.PB.started&&!__kimo.PB.playing')
            await page.evaluate('''async()=>{
              if(!await __kimo.PB.play(0))throw Error('owned stream failed to start');
              await __kimo.viewerRefresh();
              if(!__kimo.PB.playing&&!__kimo.PB.buffering)throw Error('frame refresh interrupted stream playback');
              __kimo.PB.pause();
            }''')

            await page.evaluate("__mock.emit('project-changed',{actor:'you',action:'set_property'})")
            await page.wait_for_function('!__kimo.PB.started&&boundaryCalls.some(c=>c.command==="preview_stop")')
            await page.evaluate('''async()=>{
              await __kimo.PB.stop();boundaryCalls.length=0;
              __kimoBridge.invoke=async(c,a)=>{
                boundaryCalls.push({command:c,args:a});
                if(c==='engine_read'&&a.args.action==='get_project')return {ok:true,data:{width:64,height:64,fps:24,tracks:[],assets:{owned:{id:'owned',path:'/fixture/owned.mp4',duration:1}}}};
                if(c==='engine_call'&&a.args.action==='add_track')return {ok:true,data:{track_id:'owned-drop-track'}};
                if(c==='engine_call'&&a.args.action==='add_clip')return {ok:true,data:{}};
                return boundaryInvoke(c,a);
              };
              await __kimo.loadProject();
            }''')
            await page.evaluate('__kimo.V.dur=0;__kimo.V.t=1')
            await page.keyboard.press('Home')
            assert await page.evaluate('__kimo.V.t')==0
            source=await page.locator('#bin-list .asset').bounding_box()
            destination=await page.locator('#tl-ruler').bounding_box()
            await page.mouse.move(source['x']+30,source['y']+20)
            await page.mouse.down()
            await page.mouse.move(destination['x']+100,destination['y']+10,steps=6)
            await page.mouse.up()
            await page.wait_for_function('boundaryCalls.some(c=>c.args?.args?.action==="add_clip")')
            args=await page.evaluate('boundaryCalls.find(c=>c.args?.args?.action==="add_clip").args.args.args')
            assert args['track_id']=='owned-drop-track' and args['asset_id']=='owned'
            await page.evaluate('''async()=>{
              window.ownedTextProject={width:64,height:64,fps:24,tracks:[{id:'text-track',clips:[]}]};
              const previous=__kimoBridge.invoke;
              __kimoBridge.invoke=async(c,a)=>{
                if(c==='engine_read'&&a.args.action==='get_project')return {ok:true,data:structuredClone(ownedTextProject)};
                if(c==='engine_call'&&a.args.action==='add_text'){
                  const args=a.args.args;
                  ownedTextProject.tracks[0].clips.push({id:'new-title',start:args.start,duration:args.duration,source:{type:'text',text:args.text}});
                  return {ok:true,data:{clip_id:'new-title'}};
                }
                return previous(c,a);
              };
              __kimo.V.t=0;await __kimo.loadProject();
            }''')
            await page.click('#ed-text')
            await page.wait_for_function('document.activeElement?.dataset.f==="text"')
            assert await page.input_value('#insp-body [data-f="text"]')=='Your title'
            assert await page.evaluate('__kimo.S.sel')=='new-title'

            assert not errors,errors
            await browser.close()
            print('PASS forward-compatible icon, stale model reply, paused preview invalidation and media drop creating a track')
    finally:
        server.shutdown();server.server_close()

if __name__=='__main__':
    asyncio.run(main())
