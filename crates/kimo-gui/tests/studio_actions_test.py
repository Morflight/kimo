"""Real Chromium controls against a recording, synthetic native-command adapter.

Checks UI command arguments and error/status presentation, not engine rendering or
filesystem writes. The fixture never accesses a real project or exports a file.
"""
import asyncio
import functools
import http.server
from pathlib import Path
import threading
from playwright.async_api import async_playwright

def serve():
    class QuietHandler(http.server.SimpleHTTPRequestHandler):
        def log_message(self, *args):
            pass
    handler = functools.partial(QuietHandler, directory=Path(__file__).resolve().parents[1])
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


async def main():
    server = serve()
    errors = []
    try:
        async with async_playwright() as playwright:
            browser = await playwright.chromium.launch()
            page = await browser.new_page(viewport={'width': 1280, 'height': 800})
            page.set_default_timeout(10000)
            page.on('pageerror', lambda error: errors.append(str(error)))
            await page.route('**/*', lambda route: route.continue_() if route.request.url.startswith('http://127.0.0.1') else route.abort())
            await page.goto(f'http://127.0.0.1:{server.server_address[1]}/ui/studio.html')
            await page.wait_for_selector('body[data-ready="1"]')
            await page.click('#wel-skip')
            await page.evaluate('''async () => {
              const original=__kimoBridge.invoke;
              window.fixture={requests:[],pick:[],fail:null,jobState:'done',project:{width:640,height:360,fps:25,assets:{a:{id:'a',path:'/fixture/shot.mp4',duration:8}},tracks:[{id:'t',name:'Video',clips:[{id:'c',start:0,duration:8,source:{type:'media',asset_id:'a'},effects:[]}]}]}};
              __kimoBridge.invoke=async (command,args={})=>{
                fixture.requests.push({command,args:structuredClone(args)});
                if(fixture.fail===command)throw Error('fixture failure '+command);
                if(command==='pick')return fixture.pick;
                if(command==='project_save')return args.path;
                if(command==='project_open'||command==='project_new'||command==='reveal')return null;
                if(command==='engine_read'||command==='engine_call'){
                  const {action,args:a}=args.args;
                  if(action==='get_project')return {ok:true,data:structuredClone(fixture.project)};
                  if(action==='import_media')return {ok:true,data:{asset_id:'a'}};
                  if(action==='duplicate_clip')return {ok:true,data:{clip_id:'duplicate'}};
                  if(action==='add_track')return {ok:true,data:{track_id:'t'}};
                  if(action==='add_text')return {ok:true,data:{clip_id:'c'}};
                  if(action==='export_job_start')return {ok:true,data:{job_id:'job'}};
                  if(action==='job_status')return {ok:true,data:{state:fixture.jobState,frames_done:20,frames_total:20,chunks:{done:1,total:1},eta:{seconds:0},result:{path:'/fixture/out.mp4'},error:fixture.jobState==='failed'?'synthetic encoder failure':null}};
                  return {ok:true,data:{}};
                }
                return original(command,args);
              };
              __kimo.show('edit');await __kimo.loadProject();
            }''')
            await page.wait_for_selector('[data-clip="c"]')
            assert 'shot.mp4' in await page.inner_text('#tl-tracks')
            await page.click('[data-clip="c"] .cl')
            await page.evaluate('__kimo.V.t=2.12345')
            await page.click('#ed-split')
            request = await page.evaluate('fixture.requests.find(r=>r.args.args?.action==="split_clip")')
            assert request['args']['args']['args'] == {'clip_id': 'c', 'at': 2.123}
            await page.click('#ed-dup')
            await page.wait_for_function('__kimo.S.sel==="duplicate"')
            await page.evaluate('__kimo.S.sel="c"')
            await page.click('#ed-delete')
            await page.wait_for_function('__kimo.S.sel===null')
            for selector, action in [('#ed-track', 'add_track'), ('#ed-marker', 'add_marker'), ('#ed-text', 'add_text'), ('#ed-undo', 'undo'), ('#ed-redo', 'redo')]:
                await page.click(selector)
                await page.wait_for_function('(a)=>fixture.requests.some(r=>r.args.args?.action===a)', arg=action)
            zoom = await page.input_value('#tl-zoom')
            await page.click('#tl-zin')
            assert float(await page.input_value('#tl-zoom')) > float(zoom)
            await page.click('#tl-zout')
            await page.click('#tl-fit')
            await page.evaluate('fixture.pick=["/fixture/new.mp4"]')
            await page.click('#bin-import')
            await page.wait_for_function('fixture.requests.some(r=>r.args.args?.action==="import_media")')
            await page.evaluate('fixture.pick=["/fixture/saved.kimo"];__kimo.S.dirty=true')
            await page.click('#ed-save')
            await page.wait_for_function('__kimo.S.dirty===false')
            assert await page.evaluate('fixture.requests.some(r=>r.command==="project_save"&&r.args.path==="/fixture/saved.kimo")')
            await page.evaluate('fixture.pick=["/fixture/open.kimo"];__kimo.S.dirty=true')
            await page.click('#ed-open')
            await page.click('#cf-no')
            assert not await page.evaluate('fixture.requests.some(r=>r.command==="project_open")')
            await page.click('#ed-open')
            await page.click('#cf-yes')
            await page.wait_for_function('fixture.requests.some(r=>r.command==="project_open")')
            await page.click('#ed-export')
            await page.fill('#ex-path', 'relative.mp4')
            await page.click('#ex-go')
            assert 'full path' in await page.inner_text('#ex-err')
            await page.fill('#ex-path', '/fixture/out.mp4')
            await page.click('#ex-q [data-crf="16"]')
            await page.click('#ex-go')
            await page.wait_for_function('document.querySelector("#ex-status").textContent.startsWith("Done")')
            assert await page.evaluate('fixture.requests.some(r=>r.args.args?.action==="export_job_start"&&r.args.args.args.crf===16)')
            await page.click('#ex-reveal')
            assert await page.evaluate('fixture.requests.some(r=>r.command==="reveal"&&r.args.path==="/fixture/out.mp4")')
            await page.evaluate('fixture.jobState="failed"')
            await page.click('#ex-go')
            await page.wait_for_function('document.querySelector("#ex-err").textContent.includes("synthetic encoder failure")')
            await page.click('#ex-close')
            await page.click('#tb-proj')
            assert await page.is_visible('#ctx')
            await page.keyboard.press('Escape')
            assert not await page.is_visible('#ctx')
            await page.click('#brand')
            assert await page.is_visible('#about')
            await page.click('#ab-close')
            assert not errors, errors
            await page.evaluate("__kimo.show('edit');__kimo.T.z=50;__kimo.V.t=0;__kimo.loadProject()")
            await page.wait_for_selector('[data-clip="c"]')
            await page.locator('#tl-snap').uncheck()
            async def drag_clip(part, delta, action, expected):
                await page.evaluate('fixture.requests=[];__kimo.T.z=50')
                element = page.locator('[data-clip="c"] '+part)
                box = await element.bounding_box()
                x,y = box['x']+box['width']/2,box['y']+box['height']/2
                await page.mouse.move(x,y)
                await page.mouse.down()
                await page.mouse.move(x+delta,y,steps=5)
                await page.mouse.up()
                await page.wait_for_function('(action)=>fixture.requests.some(r=>r.args.args?.action===action)',arg=action)
                actual = await page.evaluate('(action)=>fixture.requests.find(r=>r.args.args?.action===action).args.args.args',action)
                assert actual == expected, (part,actual,expected)
                await page.evaluate('__kimo.loadProject()')
            await drag_clip('.cl',50,'move_clip',{'clip_id':'c','start':1})
            await drag_clip('.r',-50,'trim_clip',{'clip_id':'c','duration':7})
            await drag_clip('.l',50,'trim_clip',{'clip_id':'c','duration':7,'in_point':1})
            assert await page.evaluate('fixture.requests.some(r=>r.args.args?.action==="move_clip"&&r.args.args.args.start===1)')
            await page.locator('#tl-snap').check()
            await page.evaluate('__kimo.V.t=1')
            await drag_clip('.cl',48,'move_clip',{'clip_id':'c','start':1})
            await page.evaluate('fixture.project.tracks[0].locked=true;__kimo.loadProject();fixture.requests=[]')
            await page.click('[data-clip="c"] .cl')
            assert 'locked' in await page.inner_text('#toast')
            assert not await page.evaluate('fixture.requests.some(r=>r.args.args?.action==="move_clip")')
            await page.evaluate('fixture.project.tracks[0].locked=false;__kimo.loadProject()')
            await page.locator('.thead .tn').dblclick()
            await page.locator('.thead .tn input').fill(' Renamed track ')
            await page.locator('.thead .tn input').press('Enter')
            await page.wait_for_function('fixture.requests.some(r=>r.args.args?.action==="set_track"&&r.args.args.args.name==="Renamed track")')
            await page.evaluate('fixture.requests=[]')
            await page.click('.thead [data-tk="muted"]')
            await page.wait_for_function('fixture.requests.some(r=>r.args.args?.action==="set_track"&&r.args.args.args.muted===true)')
            await page.evaluate('fixture.requests=[];__kimo.S.sel=null;__kimo.V.t=1')
            await page.click('#ed-split')
            await page.wait_for_function('fixture.requests.some(r=>r.args.args?.action==="split_clip")')
            await page.evaluate('fixture.requests=[];__kimo.V.t=9;__kimo.S.sel=null')
            await page.locator('.asset .add').click()
            await page.wait_for_function('fixture.requests.some(r=>r.args.args?.action==="add_clip")')
            assert await page.evaluate('fixture.requests.find(r=>r.args.args?.action==="add_clip").args.args.args') == {'track_id':'t','source_type':'media','asset_id':'a','start':9,'duration':8}
            print('PASS real pointer move/trim, snapping, locked track, rename/mute, playhead split and media insertion')
            await page.locator('#tl-zoom').evaluate("el=>{el.value='60';el.dispatchEvent(new Event('input',{bubbles:true}))}")
            assert await page.evaluate('__kimo.T.z') == 60
            await page.locator('#tl-scroll').hover()
            await page.keyboard.down('Control')
            await page.mouse.wheel(0,-100)
            await page.keyboard.up('Control')
            await page.wait_for_function('__kimo.T.z>60')
            await page.locator('#tl-scroll').evaluate('el=>el.scrollLeft=0')
            ruler = await page.locator('#tl-ruler').bounding_box()
            await page.mouse.move(ruler['x']+100,ruler['y']+10)
            await page.mouse.down()
            await page.mouse.move(ruler['x']+140,ruler['y']+10,steps=3)
            await page.mouse.up()
            assert 0 < await page.evaluate('__kimo.V.t') < 8
            height = await page.locator('#tl').bounding_box()
            await page.mouse.move(height['x']+100,height['y']+2)
            await page.mouse.down()
            await page.mouse.move(height['x']+100,height['y']-28,steps=3)
            await page.mouse.up()
            assert await page.locator('#editor').evaluate("el=>parseFloat(el.style.getPropertyValue('--tlh'))") > height['height']
            await page.evaluate('fixture.requests=[]')
            await page.locator('.asset').click(button='right')
            await page.locator('#ctx').get_by_role('button',name='Show in folder',exact=True).click()
            await page.wait_for_function('fixture.requests.some(r=>r.command==="reveal"&&r.args.path==="/fixture/shot.mp4")')
            await page.locator('.asset').click(button='right')
            await page.locator('#ctx').get_by_role('button',name='Add at the end',exact=True).click()
            await page.wait_for_function('fixture.requests.some(r=>r.args.args?.action==="add_clip"&&r.args.args.args.start===8)')
            await page.locator('.asset').click(button='right')
            await page.locator('#ctx').get_by_role('button',name='Ask the agent to use it',exact=True).click()
            assert 'asset a' in await page.input_value('#chat-in')
            await page.evaluate("__kimo.show('edit');fixture.requests=[]")
            await page.locator('.thead').click(button='right')
            await page.locator('#ctx').get_by_role('button',name='Close gaps on this track',exact=True).click()
            await page.wait_for_function('fixture.requests.some(r=>r.args.args?.action==="close_gaps")')
            assert await page.evaluate('fixture.requests.find(r=>r.args.args?.action==="close_gaps").args.args.args') == {'track_ids':['t']}
            print('PASS zoom slider/wheel, ruler seek, timeline resize and media/track context actions')
            # Inspector controls must forward typed values to the appropriate engine action.
            await page.evaluate("fixture.project.tracks[0].clips[0].effects=[{id:'fx',kind:'blur'}];__kimo.show('edit');__kimo.S.sel='c';__kimo.S.transitions=['dissolve'];__kimo.loadProject()")
            await page.wait_for_selector('[data-f="start"]')
            async def field(selector, value, action, expected):
                await page.evaluate('fixture.requests=[]')
                await page.locator(selector).evaluate("(el,value)=>{el.value=value;el.dispatchEvent(new Event('input',{bubbles:true}));el.dispatchEvent(new Event('change',{bubbles:true}));}", value)
                await page.wait_for_function('(action)=>fixture.requests.some(r=>r.args.args?.action===action)', arg=action)
                actual = await page.evaluate('(action)=>fixture.requests.find(r=>r.args.args?.action===action).args.args.args', action)
                assert actual == expected, (selector, actual, expected)
            for name, value, action, expected in [
                ('start','-2','move_clip',{'start':0}),
                ('duration','3.5','trim_clip',{'duration':3.5}),
                ('in_point','-1','trim_clip',{'in_point':0}),
                ('speed','120','set_speed',{'speed':100}),
                ('opacity','0.4','set_property',{'property':'opacity','value':0.4}),
                ('scale','1.5','set_property',{'property':'scale','value':1.5}),
                ('x','12','set_property',{'property':'x','value':12}),
                ('y','-8','set_property',{'property':'y','value':-8}),
                ('rotation','45','set_property',{'property':'rotation','value':45}),
                ('volume','0.7','set_audio',{'volume':0.7}),
                ('fade_in','0.5','set_audio',{'fade_in':0.5}),
                ('fade_out','0.8','set_audio',{'fade_out':0.8}),
                ('addfx','glow','add_effect',{'kind':'glow'}),
                ('trans','dissolve','add_transition',{'kind':'dissolve','duration':0.5}),
            ]:
                await field(f'[data-f="{name}"]',value,action,{'clip_id':'c',**expected})
            await page.evaluate('fixture.requests=[]')
            await page.locator('[data-f="mute"]').check()
            await page.wait_for_function('fixture.requests.some(r=>r.args.args?.action==="set_audio")')
            assert await page.evaluate('fixture.requests.find(r=>r.args.args?.action==="set_audio").args.args.args') == {'clip_id':'c','mute':True}
            await page.click('[data-rmfx="fx"]')
            await page.wait_for_function('fixture.requests.some(r=>r.args.args?.action==="remove_effect")')
            assert await page.evaluate('fixture.requests.find(r=>r.args.args?.action==="remove_effect").args.args.args') == {'clip_id':'c','effect_id':'fx'}
            await page.evaluate("fixture.project.tracks[0].clips[0].source={type:'text',text:'Title',size:24,color:[1,.5,0]};__kimo.loadProject()")
            await page.wait_for_selector('[data-f="text"]')
            await field('[data-f="text"]','Updated title','set_text',{'clip_id':'c','text':'Updated title'})
            await field('[data-f="size"]','36','set_text',{'clip_id':'c','size':36})
            await field('[data-f="color"]','#123456','set_text',{'clip_id':'c','color':'#123456'})
            await page.locator('[data-f="text"]').focus()
            await page.keyboard.press('Control+Enter')
            assert await page.locator('[data-f="text"]').evaluate('(el)=>document.activeElement!==el')
            await page.evaluate('__kimo.S.sel=null;__kimo.loadProject()')
            await page.wait_for_selector('#pj-preset')
            await field('#pj-preset','1080x1920','project_set',{'width':1080,'height':1920})
            await field('#pj-fps','60','project_set',{'fps':60})
            await page.evaluate('fixture.requests=[];__kimo.S.dirty=true')
            await page.click('#ed-new')
            await page.click('#cf-no')
            assert not await page.evaluate('fixture.requests.some(r=>r.command==="project_new")')
            await page.click('#ed-new')
            await page.click('#cf-yes')
            await page.wait_for_function('fixture.requests.some(r=>r.command==="project_new")')
            assert await page.evaluate('__kimo.S.sel') is None
            assert not errors, errors
            print('PASS inspector values, text/audio/effects, project format and new-project consent')
            print('PASS timeline commands, imports, save/open confirmation, export validation/status and menus')
            await page.close()
            native = await browser.new_page(viewport={'width': 1280, 'height': 800})
            native.set_default_timeout(10000)
            native.on('pageerror', lambda error: errors.append(str(error)))
            await native.add_init_script("""window.__TAURI__={
              core:{invoke:async(command,args)=>{const {bridge}=await import('/ui/dev/mock-bridge.js');return bridge.invoke(command,args);}},
              event:{listen:async(name,callback)=>{const {bridge}=await import('/ui/dev/mock-bridge.js');return bridge.listen(name,payload=>callback({payload}));}}
            };""")
            await native.goto(f'http://127.0.0.1:{server.server_address[1]}/ui/studio.html')
            await native.wait_for_selector('body[data-ready="1"]')
            assert await native.evaluate('__kimoBridge.kind') == 'tauri'
            await native.click('#wel-skip')
            await native.evaluate("__mock.emit('update-available',{state:'available',version:'test',notes:'Fixture notes'});__kimo.S.dirty=true")
            await native.click('#tb-update')
            assert 'autosaved' in await native.inner_text('#cf-body')
            await native.click('#cf-no')
            await native.click('#tb-update')
            await native.click('#cf-yes')
            await native.wait_for_function('document.querySelector("#toast").textContent.includes("update_install")')
            assert await native.is_enabled('#tb-update')
            await native.evaluate("__mock.emit('update-progress',{done:1,total:2})")
            assert '50%' in await native.inner_text('#tb-update')
            await native.evaluate("__mock.emit('update-installing',{})")
            assert 'Installing' in await native.inner_text('#tb-update')
            await native.evaluate("__mock.emit('update-available',{state:'current'})")
            assert not await native.is_visible('#tb-update')
            await native.evaluate("__kimo.show('connections');Object.defineProperty(navigator,'clipboard',{configurable:true,value:{writeText:async(text)=>window.copied=text}})")
            await native.click('#more-mcp > summary')
            await native.click('#cfg-copy')
            assert await native.evaluate('copied===document.querySelector("#cfg-json").textContent')
            await native.evaluate("navigator.clipboard.writeText=async()=>{throw Error('denied')};document.execCommand=(command)=>{window.copyFallback=command;return true}")
            await native.click('#cfg-copy')
            assert await native.evaluate('copyFallback') == 'copy'
            assert not errors, errors
            await native.evaluate("""() => {
              const invoke=__kimoBridge.invoke;window.modelRequests=[];
              __kimoBridge.invoke=async(command,args)=>{modelRequests.push({command,args});return invoke(command,args)};
              __kimo.S.models.codex={models:[{id:'fast',label:'Fast (newest)',efforts:['low'],default_effort:'low'},{id:'deep',label:'Deep',efforts:['high']}],efforts:['low','high'],source:'fixture catalogue'};
              __kimo.show('agent');
            }""")
            await native.click('.apick[data-agent="codex"]')
            await native.wait_for_selector('#model-sel')
            await native.select_option('#effort-sel','high')
            await native.select_option('#model-sel','fast')
            await native.wait_for_function('__kimo.S.app.settings.agent_models.codex==="fast"')
            assert await native.evaluate('__kimo.S.app.settings.agent_efforts.codex') == ''
            assert 'Fast' in await native.inner_text('#ask-agent-t')
            assert '(newest)' not in await native.inner_text('#ask-agent-t')
            await native.select_option('#model-sel','__other')
            assert await native.is_visible('#model-other')
            await native.fill('#model-other',' custom-model ')
            await native.press('#model-other','Enter')
            await native.wait_for_function('__kimo.S.app.settings.agent_models.codex==="custom-model"')
            await native.select_option('#effort-sel','low')
            await native.wait_for_function('__kimo.S.app.settings.agent_efforts.codex==="low"')
            assert await native.evaluate('modelRequests.some(r=>r.command==="agent_select"&&r.args.model==="custom-model"&&r.args.select===false)')
            await native.select_option('#model-sel','')
            await native.wait_for_function('__kimo.S.app.settings.agent_models.codex===""')
            assert await native.evaluate('__kimo.S.app.settings.agent_efforts.codex') == 'low'
            await native.select_option('#effort-sel','')
            await native.evaluate("__kimo.show('live')")
            await native.click('#live-clear')
            assert await native.inner_text('#live-count') == '0 actions'
            await native.click('#live-graph')
            await native.wait_for_function('modelRequests.some(r=>r.command==="open_spectator")')
            assert not errors, errors
            await native.evaluate("__kimo.show('connections')")
            await native.click('#more-api > summary')
            await native.click('#key-clear')
            await native.wait_for_function('document.querySelector("#key-msg").textContent==="Forgotten."')
            await native.click('#key-use')
            await native.wait_for_function('__kimo.S.app.settings.agent==="api"')
            assert 'Change in Connections' in await native.inner_text('#model-bar')
            await native.evaluate("""() => {
              const invoke=__kimoBridge.invoke;
              __kimoBridge.invoke=async(command,args)=>{
                if(command==='agent_send')throw Error('Please sign in to the fixture provider');
                return invoke(command,args);
              };
            }""")
            await native.fill('#chat-in','Describe this edit')
            await native.press('#chat-in','Enter')
            await native.wait_for_selector('#msgs .err')
            assert 'sign in' in await native.inner_text('#msgs .err')
            assert await native.is_visible('#msgs .err [data-go="connections"]')
            assert await native.is_enabled('#chat-send')
            await native.click('#agent-new')
            await native.wait_for_selector('#chat-hello')
            assert 'New conversation' in await native.inner_text('#chat-hello')
            await native.evaluate("__kimo.show('edit')")
            await native.fill('#ask-in','Try again')
            await native.locator('#askbar').evaluate('(form)=>form.requestSubmit()')
            await native.wait_for_function('!__kimo.S.busy&&document.querySelector("#toast").textContent.includes("sign in")')
            await native.click('#ask-agent')
            await native.wait_for_function('__kimo.S.tab==="agent"')
            await native.evaluate("__mock.emit('tauri://drag-enter',{})")
            assert await native.is_visible('#dropcover')
            await native.evaluate("__mock.emit('tauri://drag-leave',{})")
            assert not await native.is_visible('#dropcover')
            await native.evaluate("__mock.emit('tauri://drag-drop',{paths:[]})")
            await native.wait_for_function('__kimo.S.tab==="edit"')
            assert not await native.is_visible('#dropcover')
            assert not errors, errors
            print('PASS forgotten key, API selection, failed agent turn/reset, ask bar and native drag events')
            print('PASS model selection, custom names, incompatible effort reset and Live controls')
            print('PASS native bridge event envelope, update consent/error/progress and clipboard fallback')
            await browser.close()
    finally:
        server.shutdown()
        server.server_close()

if __name__ == '__main__':
    asyncio.run(main())
