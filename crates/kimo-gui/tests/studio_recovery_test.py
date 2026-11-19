"""Startup recovery and crash-report consent against a synthetic native adapter."""
import asyncio
import json
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
            for welcome_action in ['connect','import']:
                page=await browser.new_page()
                errors=[]
                page.on('pageerror',lambda error:errors.append(str(error)))
                await page.add_init_script("""window.setupCalls=[];window.setupSkipped=false;
                  window.__TAURI__={core:{invoke:async(command,args)=>{
                    setupCalls.push({command,args});
                    const {bridge}=await import('/ui/dev/mock-bridge.js');
                    if(command==='setup_status')return {ready:false,done:setupSkipped,parts:[{id:'ffmpeg',name:'FFmpeg',note:'fixture',pinned:true,size:1000000,satisfied:null}]};
                    if(command==='setup_start')return await new Promise(resolve=>window.finishSetup=resolve);
                    if(command==='setup_pause'){finishSetup({ready:false});return null;}
                    if(command==='setup_skip'){setupSkipped=true;return null;}
                    if(command==='pick')return ['/fixture/import.mp4'];
                    if(command==='engine_call'&&args.args.action==='import_media')return {ok:true,data:{asset_id:'fixture'}};
                    return bridge.invoke(command,args);
                  }},event:{listen:async(name,callback)=>{const {bridge}=await import('/ui/dev/mock-bridge.js');return bridge.listen(name,payload=>callback({payload}));}}};""")
                await page.goto(f'http://127.0.0.1:{server.server_address[1]}/ui/studio.html')
                await page.wait_for_selector('body[data-ready="1"]')
                assert await page.locator('#setup').is_visible()
                assert not await page.locator('#welcome').is_visible()
                await page.click('#setup-go')
                await page.wait_for_function('typeof finishSetup==="function"')
                assert await page.locator('#setup-go').is_disabled()
                await page.click('#setup-pause')
                await page.wait_for_function('document.querySelector("#setup-err").textContent.includes("FFmpeg is still missing")')
                assert await page.locator('#setup-go').is_enabled()
                await page.click('#setup-later')
                await page.wait_for_selector('#welcome.on')
                assert not await page.locator('#setup').is_visible()
                await page.click('#wel-'+welcome_action)
                await page.wait_for_function('setupCalls.some(c=>c.command==="onboarded_set")')
                assert not await page.locator('#welcome').is_visible()
                assert await page.evaluate('__kimo.S.app.settings.onboarded') is True
                if welcome_action=='connect':
                    assert await page.locator('#tab-connections').is_visible()
                else:
                    await page.wait_for_function('setupCalls.some(c=>c.command==="engine_call"&&c.args.args.action==="import_media")')
                    imported=await page.evaluate('setupCalls.find(c=>c.command==="engine_call"&&c.args.args.action==="import_media").args.args.args')
                    assert imported=={'path':'/fixture/import.mp4'}
                assert not errors,errors
                await page.close()
            print('PASS setup pause/missing dependency/skip and welcome connection/import actions with synthetic commands')
            variants=[(['crossfade',{'kind':'wipe'},{'name':'iris'},{}],['crossfade','wipe','iris']),
                      ({'transitions':['dissolve']},['dissolve']),({'kinds':['dip']},['dip']),
                      (None,[]),('refused',[])]
            for index,(transition_reply,expected) in enumerate(variants):
                page=await browser.new_page()
                errors=[]
                page.on('pageerror',lambda error:errors.append(str(error)))
                config=json.dumps({'index':index,'transitions':transition_reply})
                await page.add_init_script('window.startupFixture='+config+';'+'''window.startupRequests=[];
                  window.__TAURI__={core:{invoke:async(command,args)=>{
                    startupRequests.push({command,args});
                    const {bridge}=await import('/ui/dev/mock-bridge.js');
                    if(command==='engine_read'&&args.args.action==='list_transitions')return startupFixture.transitions==='refused'
                      ?{ok:false,text:'owned transition lookup failure'}:{ok:true,data:startupFixture.transitions};
                    if(command==='project_open')return null;
                    if(command==='update_install')return null;
                    if(command==='cli_register')throw Error('owned registration failure');
                    if(command==='cli_models'&&startupFixture.index===3)throw Error('owned model lookup failure');
                    if(command==='cli_status')return startupFixture.index===2?[]:[{id:'codex',name:'Codex',plan:'owned fixture',installed:true,signed_in:null,sign_in_note:'Status unavailable',registered:false}];
                    const answer=await bridge.invoke(command,args);
                    if(command==='app_state'){
                      answer.settings.onboarded=true;
                      if(startupFixture.index===4&&startupRequests.filter(r=>r.command==='app_state').length===1)answer.engine={state:'stopped'};
                      answer.settings.recent=['/fixture/recent.kimo'];
                      answer.settings.agent='codex';answer.settings.agent_models={codex:'owned-custom-model'};
                      answer.settings.agent_efforts={codex:'high'};
                      if(startupFixture.index===2){
                        answer.settings.agent='api';answer.settings.provider='anthropic';answer.settings.models={anthropic:''};
                        answer.providers=answer.providers.map(p=>p.id==='anthropic'?{...p,default_model:''}:p);
                        answer.keys=answer.keys.map(k=>k.provider==='anthropic'?{...k,set:true,hint:'owned fixture'}:k);
                      }
                    }
                    return answer;
                  }},event:{listen:async(name,callback)=>{const {bridge}=await import('/ui/dev/mock-bridge.js');return bridge.listen(name,payload=>callback({payload}));}}};''')
                await page.goto(f'http://127.0.0.1:{server.server_address[1]}/ui/studio.html')
                await page.wait_for_selector('body[data-ready="1"]')
                assert await page.evaluate('__kimo.S.transitions')==expected
                if index==4:
                    await page.wait_for_function('startupRequests.filter(r=>r.command==="app_state").length>=2&&__kimo.S.app.engine.state==="running"')
                assert await page.inner_text('#home-recent .n')=='recent'
                await page.click('#home-recent [data-open]')
                await page.wait_for_function('startupRequests.some(r=>r.command==="project_open")')
                assert await page.evaluate('startupRequests.find(r=>r.command==="project_open").args.path')=='/fixture/recent.kimo'
                await page.click('#nav a[data-tab="home"]')
                await page.locator('#tab-home [data-go="agent"]').first.click()
                await page.wait_for_function('__kimo.S.tab==="agent"')
                if index==2:
                    assert 'not set' in await page.inner_text('#model-bar')
                    assert not await page.locator('#nav-conn').is_visible()
                else:
                    await page.wait_for_selector('#model-other:not([hidden])')
                    assert await page.input_value('#model-other')=='owned-custom-model'
                    assert await page.input_value('#model-sel')=='__other'
                    if index==3:
                        assert 'owned model lookup failure' in await page.inner_text('#model-src')
                    else:
                        await page.select_option('#model-sel','test-model')
                        await page.wait_for_function('__kimo.S.app.settings.agent_models.codex==="test-model"')
                        assert await page.evaluate('__kimo.S.app.settings.agent_efforts.codex')=='high'
                    await page.click('#nav a[data-tab="connections"]')
                    assert 'Status unavailable' in await page.inner_text('#cli-cards #cli-codex')
                    assert await page.locator('#cli-cards #cli-codex [data-cli="signin"]').is_visible()
                    assert 'high effort' in await page.inner_text('#cli-cards #cli-codex')
                if index==0:
                    # A failed external registration must remain visible and re-enable retry.
                    await page.click('#cli-cards #cli-codex [data-cli="register"]')
                    await page.wait_for_function('document.querySelector("#cli-msg").textContent.includes("owned registration failure")')
                    assert await page.locator('#cli-cards #cli-codex [data-cli="register"]').is_enabled()
                    await page.evaluate("__kimo.S.app.settings.agent='api'")
                    await page.click('#cli-refresh')
                    await page.click('#cli-cards #cli-codex [data-cli="use"]')
                    await page.wait_for_function('startupRequests.some(r=>r.command==="agent_select"&&r.args.agent==="codex")&&__kimo.S.app.settings.agent==="codex"')
                    assert await page.evaluate('__kimo.S.app.settings.agent')=='codex'
                    # A clean project does not claim an autosave; an owned native adapter
                    # acknowledges installation without downloading or restarting anything.
                    await page.evaluate("async()=>{__kimo.S.update=null;await document.querySelector('#tb-update').onclick();}")
                    assert not await page.locator('#confirm').is_visible()
                    await page.evaluate("()=>{__kimo.S.dirty=false;__mock.emit('update-available',{state:'available',version:'99.0.0'});}")
                    await page.click('#tb-update')
                    assert 'autosaved' not in await page.inner_text('#cf-body')
                    assert await page.inner_text('#cf-title')=='Install kimo 99.0.0?'
                    await page.click('#cf-yes')
                    await page.wait_for_function('startupRequests.some(r=>r.command==="update_install")')
                    assert await page.locator('#tb-update').is_disabled()
                    assert await page.inner_text('#tb-update')=='Downloading…'
                    await page.evaluate("__mock.emit('update-installing')")
                    assert await page.inner_text('#tb-update')=='Installing…'
                    await page.click('#brand')
                    await page.locator('#about').click(position={'x':5,'y':5})
                    assert not await page.locator('#about').is_visible()
                    await page.evaluate("document.querySelector('#tb-proj').dispatchEvent(new Event('click',{bubbles:true,cancelable:true}))")
                    assert await page.locator('#ctx').is_visible()
                    assert await page.evaluate('Math.abs(document.querySelector("#ctx").getBoundingClientRect().left-Math.min(document.querySelector("#tb-proj").getBoundingClientRect().left,innerWidth-230))<1')
                    await page.locator('#ctx hr').click()
                    assert await page.locator('#ctx').is_visible()
                    await page.keyboard.press('Escape')
                    await page.evaluate("__mock.emit('tauri://drag-drop')")
                    assert await page.evaluate('__kimo.S.tab')=='edit'
                    await page.evaluate("__mock.emit('tauri://drag-drop',{paths:['/fixture/drag.kimo']})")
                    await page.wait_for_function('startupRequests.some(r=>r.command==="project_open"&&r.args.path==="/fixture/drag.kimo")')
                assert not errors,errors
                await page.close()
            print('PASS transition reply variants, recent projects, home links, unknown sign-in state and model fallbacks')
            await browser.close()
    finally:
        server.shutdown()
        server.server_close()

if __name__=='__main__':
    asyncio.run(main())
