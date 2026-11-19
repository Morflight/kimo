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
            # A failed native shuttle must not silently fall back to forward playback.
            await page.evaluate('''async()=>{
              await __kimo.PB.stop();window.savedPlay=__kimo.PB.play;
              __kimo.PB.supported=true;__kimo.PB.play=async()=>false;
              Object.assign(__kimo.V,{t:.1,dur:3,draft:null});draftCalls.length=0;
            }''')
            await page.keyboard.press('j')
            await page.wait_for_timeout(30)
            assert await page.evaluate('draftCalls.length')==0
            assert await page.locator('#viewer').get_attribute('data-playing')=='0'
            # Stopping while native playback is starting must prevent a late fallback.
            await page.evaluate('()=>{__kimo.PB.play=()=>new Promise(resolve=>window.finishNativePlay=resolve)}')
            await page.click('#vw-play')
            await page.wait_for_function('typeof finishNativePlay==="function"')
            await page.click('#vw-play')
            await page.evaluate('finishNativePlay(false)')
            await page.wait_for_timeout(30)
            assert await page.evaluate('draftCalls.length')==0
            assert await page.locator('#viewer').get_attribute('data-playing')=='0'
            await page.evaluate('''()=>{
              __kimo.PB.play=savedPlay;__kimo.PB.supported=false;
              window.savedDraftInvoke=__kimoBridge.invoke;
              __kimoBridge.invoke=async(c,a)=>{
                if(c==='viewer_draft'){draftCalls.push(a);throw Error('owned draft render failure');}
                return savedDraftInvoke(c,a);
              };
            }''')
            await page.click('#vw-play')
            await page.wait_for_selector('#viewer[data-state="error"]')
            assert 'owned draft render failure' in await page.inner_text('#vw-err')
            assert await page.evaluate('draftCalls.length')==1
            # A late rendered chunk after Stop cannot replace the current draft.
            await page.evaluate('''()=>{
              __kimoBridge.invoke=async(c,a)=>c==='viewer_draft'
                ?new Promise(resolve=>window.finishLateDraft=resolve):savedDraftInvoke(c,a);
            }''')
            await page.click('#vw-play')
            await page.wait_for_function('typeof finishLateDraft==="function"')
            await page.click('#vw-play')
            await page.evaluate('finishLateDraft({rev:"late",start:0,end:3,frame_urls:[]})')
            await page.wait_for_timeout(30)
            assert await page.evaluate('__kimo.V.draft') is None
            # Crossed loop endpoints clear the obsolete opposite endpoint.
            await page.evaluate('__kimo.V.t=1;__kimo.PB.loop={on:false,in:null,out:null}')
            await page.keyboard.press('o')
            await page.evaluate('__kimo.V.t=2')
            await page.keyboard.press('i')
            assert await page.evaluate('JSON.stringify(__kimo.PB.loop)')=='{"on":true,"in":2,"out":null}'
            await page.evaluate('__kimo.V.t=.5')
            await page.keyboard.press('o')
            assert await page.evaluate('JSON.stringify(__kimo.PB.loop)')=='{"on":true,"in":null,"out":0.5}'
            await page.keyboard.press('Control+Shift+x')
            assert await page.evaluate('__kimo.PB.loop.on') is False
            await page.evaluate('()=>{__kimoBridge.invoke=savedDraftInvoke}')
            print('PASS failed shuttle, cancelled native/draft starts, explicit draft error and crossed loop endpoints')
            await page.keyboard.press('Control+l')
            assert await page.evaluate('__kimo.PB.loop.on') is True
            await page.keyboard.press('Control+l')
            assert await page.evaluate('__kimo.PB.loop.on') is False
            before=await page.evaluate('__kimo.V.t')
            fps=await page.evaluate('__kimo.V.fps')
            await page.keyboard.down('k')
            await page.keyboard.press('l')
            await page.keyboard.up('k')
            assert abs(await page.evaluate('__kimo.V.t')-before-1/fps)<1e-9
            # Player callbacks are the transport boundary consumed by Studio.
            await page.evaluate('''async()=>{
              await __kimo.PB.stop();__kimo.V.w=4096;
            }''')
            assert await page.evaluate('__kimo.PB.o.scale()')==.25
            await page.evaluate('__kimo.V.w=0')
            assert await page.evaluate('__kimo.PB.o.scale()')==.5
            await page.evaluate('''()=>{
              __kimo.PB.rate=-2;__kimo.PB.loop.on=true;
              __kimo.PB.o.onState('playing',{size:'320x180',fps:0,skip:5,proxies:true});
            }''')
            text=await page.inner_text('#vw-msg')
            for expected in ['−2×','… fps','every 5th frame','proxies','looping']:
                assert expected in text,(expected,text)
            await page.evaluate("__kimo.PB.o.onWarm('ready')")
            assert await page.inner_text('#vw-prepare-action')=='Clear prepared preview'
            await page.evaluate("__kimo.PB.o.onWarm('idle')")
            assert await page.inner_text('#vw-prepare-status')==''
            assert await page.evaluate("""()=>{
              __kimo.V.frames.clear();__kimo.PB.o.onState('scrub',{cached:true});
              return document.querySelector('#vw-msg').textContent;
            }""")=='Draft frame (buffered)'
            # Full-quality frame retention evicts the oldest entry, not the newest.
            await page.evaluate('''async()=>{
              await __kimo.PB.stop();__kimo.PB.loop.on=false;__kimo.PB.rate=1;
              const canvas=document.createElement('canvas');canvas.width=2;canvas.height=2;
              const url=canvas.toDataURL();
              window.shownFixture={duration:3,fps:0,rev:'owned',width:320,height:180,empty:false,changed:true,
                frame:{rev:'owned',index:300,url,cached:true},before:{url,time:.5}};
              __kimo.V.frames=new Map(Array.from({length:300},(_,i)=>['owned:'+i,{url}]));
              __kimo.V.t=1;__kimo.V.draft=null;
              __kimoBridge.invoke=async(c,a)=>c==='viewer_frame'?structuredClone(shownFixture):savedDraftInvoke(c,a);
              await __kimo.viewerRefresh();
            }''')
            assert await page.evaluate('__kimo.V.fps')==30
            assert await page.evaluate('__kimo.V.frames.size')==300
            assert not await page.evaluate('__kimo.V.frames.has("owned:0")')
            assert await page.evaluate('__kimo.V.frames.has("owned:1")&&__kimo.V.frames.has("owned:300")')
            await page.click('#vw-ab button[data-v="before"]')
            assert await page.locator('#viewer').get_attribute('data-view')=='before'
            await page.evaluate('__kimo.viewerRefresh()')
            assert await page.locator('#viewer').get_attribute('data-view')=='after'
            await page.evaluate('()=>{__kimoBridge.invoke=savedDraftInvoke}')
            print('PASS preview status callbacks, resolution policy, bounded full-quality cache and changed-frame comparison')
            # Exercise the actual HTML video fallback with an owned 2x2 recording.
            await page.evaluate('''async()=>{
              await __kimo.PB.stop();clearTimeout(__kimo.V.timer);
              const canvas=document.createElement('canvas');canvas.width=2;canvas.height=2;
              const ctx=canvas.getContext('2d');ctx.fillStyle='blue';ctx.fillRect(0,0,2,2);
              const stream=canvas.captureStream(10),recorder=new MediaRecorder(stream,{mimeType:'video/webm'}),chunks=[];
              recorder.ondataavailable=e=>chunks.push(e.data);
              const done=new Promise(resolve=>recorder.onstop=resolve);
              recorder.start();let n=0;const paint=setInterval(()=>{ctx.fillStyle=++n%2?'blue':'red';ctx.fillRect(0,0,2,2);},50);
              await new Promise(resolve=>setTimeout(resolve,3000));clearInterval(paint);recorder.stop();await done;
              stream.getTracks().forEach(t=>t.stop());
              window.videoFixtureURL=URL.createObjectURL(new Blob(chunks,{type:'video/webm'}));
              __kimoBridge.invoke=async(c,a)=>c==='viewer_draft'
                ?{rev:'video-owned',start:a.start,end:a.start+3,fps:10,url:videoFixtureURL}:savedDraftInvoke(c,a);
              Object.assign(__kimo.V,{t:0,dur:6,fps:10,rev:'video-owned',draft:null});
              __kimo.PB.supported=false;
              const video=document.querySelector('#vw-video');window.realVideoPlay=video.play;
              video.play=async()=>{throw Error('owned decoder unavailable');};
            }''')
            await page.click('#vw-play')
            await page.wait_for_function('document.querySelector("#vw-msg").textContent.includes("Frames still work")')
            assert await page.locator('#viewer').get_attribute('data-playing')=='0'
            await page.evaluate("()=>{document.querySelector('#vw-video').play=realVideoPlay;}")
            await page.click('#vw-play')
            await page.wait_for_function('document.querySelector("#viewer").dataset.playing==="1"')
            await page.keyboard.press('ArrowRight')
            assert await page.locator('#viewer').get_attribute('data-playing')=='0'
            # Scrubbing an available video chunk selects video, including a frame
            # refresh while the same chunk remains available.
            await page.evaluate('''async()=>{
              const scrub=document.querySelector('#vw-scrub');scrub.value='0.1';scrub.oninput();
              __kimoBridge.invoke=async(c,a)=>c==='viewer_frame'
                ?{...shownFixture,rev:'video-owned',changed:false,frame:{...shownFixture.frame,rev:'video-owned'}}
                :c==='viewer_draft'?{rev:'video-owned',start:a.start,end:a.start+3,fps:10,url:videoFixtureURL}:savedDraftInvoke(c,a);
              await __kimo.viewerRefresh();
            }''')
            assert await page.locator('#vw-video').is_visible()
            await page.evaluate('''()=>{
              const previous=__kimoBridge.invoke;window.endedStarts=[];
              __kimoBridge.invoke=async(c,a)=>{if(c==='viewer_draft')endedStarts.push(a.start);return previous(c,a);};
              __kimo.V.t=0;__kimo.V.dur=6;__kimo.V.fps=10;
            }''')
            await page.click('#vw-play')
            await page.wait_for_function('document.querySelector("#viewer").dataset.playing==="1"')
            await page.evaluate('''()=>{
              const video=document.querySelector('#vw-video');video.pause();__kimo.V.t=3;video.onended();
            }''')
            await page.wait_for_function('__kimo.V.draft.start===3&&document.querySelector("#viewer").dataset.playing==="1"')
            assert await page.evaluate('endedStarts')==[3]
            await page.evaluate('''()=>{
              const video=document.querySelector('#vw-video');video.pause();__kimo.V.t=6;video.onended();
            }''')
            assert await page.locator('#viewer').get_attribute('data-playing')=='0'
            assert await page.evaluate('endedStarts')==[3]
            await page.keyboard.press('k')

            await page.evaluate('''()=>{
              __kimoBridge.invoke=savedDraftInvoke;
              document.querySelector('#vw-video').pause();URL.revokeObjectURL(videoFixtureURL);
            }''')
            print('PASS real video draft, decoder rejection, frame stepping and cached video scrubbing')
            # Stopped engines must receive no frame request, and obsolete replies
            # must never replace a newer successful frame or expose a stale error.
            await page.evaluate('''async()=>{
              await __kimo.PB.stop();clearTimeout(__kimo.V.timer);__kimo.V.draft=null;
              window.viewerRequests=0;window.savedEngine=__kimo.S.app.engine;
              __kimoBridge.invoke=async(c,a)=>{if(c==='viewer_frame'){viewerRequests++;return shownFixture;}return savedDraftInvoke(c,a);};
              __kimo.S.app.engine={state:'stopped'};await __kimo.viewerRefresh();
            }''')
            assert await page.evaluate('viewerRequests')==0
            await page.evaluate('''async()=>{
              __kimo.S.app.engine=savedEngine;
              __kimoBridge.invoke=async(c,a)=>{
                if(c==='viewer_frame')return new Promise((resolve,reject)=>window.finishOldFrame={resolve,reject});
                return savedDraftInvoke(c,a);
              };
              window.oldRefresh=__kimo.viewerRefresh();
            }''')
            await page.wait_for_function('typeof finishOldFrame==="object"')
            await page.evaluate('''async()=>{
              __kimoBridge.invoke=async(c,a)=>c==='viewer_frame'?shownFixture:savedDraftInvoke(c,a);
              await __kimo.viewerRefresh();finishOldFrame.reject(Error('owned obsolete frame error'));await oldRefresh;
            }''')
            assert await page.locator('#viewer').get_attribute('data-state')=='ready'
            assert 'owned obsolete frame error' not in await page.inner_text('#vw-err')
            await page.evaluate('''async()=>{
              __kimoBridge.invoke=async(c,a)=>{if(c==='viewer_frame')throw 'superseded';return savedDraftInvoke(c,a);};
              await __kimo.viewerRefresh();
              __kimoBridge.invoke=async(c,a)=>c==='viewer_frame'?{...shownFixture,before:null}:savedDraftInvoke(c,a);
              await __kimo.viewerRefresh();
              document.querySelector('#vw-ab button[data-v="before"]').onclick();
            }''')
            assert await page.locator('#viewer').get_attribute('data-view')=='after'
            assert await page.locator('#viewer').get_attribute('data-state')=='ready'
            await page.evaluate('()=>{__kimoBridge.invoke=savedDraftInvoke;}')
            print('PASS stopped engine, obsolete frame rejection, superseded render and unavailable comparison')
            await page.evaluate('''async()=>{
              __kimoBridge.invoke=async(c,a)=>c==='viewer_frame'?shownFixture:savedDraftInvoke(c,a);
              __kimo.V.t=.1;__kimo.V.draft={rev:shownFixture.rev,start:0,end:3,fps:0,frame_urls:[shownFixture.frame.url]};
              await __kimo.viewerRefresh();
              document.querySelector('#vw-ab button[data-v="before"]').onclick();
              const scrub=document.querySelector('#vw-scrub');scrub.value='.2';scrub.oninput();
            }''')
            assert await page.locator('#viewer').get_attribute('data-view')=='after'
            assert await page.locator('#viewer').get_attribute('data-media')=='image'
            assert await page.locator('#vw-img').get_attribute('src')==await page.evaluate('shownFixture.frame.url')
            for enabled,start,expected in [(True,.25,.25),(True,None,0),(False,None,0)]:
                actual=await page.evaluate('''({enabled,start})=>{
                  __kimo.V.t=__kimo.V.dur;__kimo.PB.loop={on:enabled,in:start,out:null};
                  document.querySelector('#vw-play').onclick();
                  const at=__kimo.V.t;document.querySelector('#vw-play').onclick();return at;
                }''',{'enabled':enabled,'start':start})
                assert actual==expected
            await page.evaluate('()=>{__kimoBridge.invoke=savedDraftInvoke;}')
            print('PASS sequence-frame refresh, comparison scrubbing and exact restart at loop boundaries')

            assert not errors,errors
            await browser.close()
            print('PASS fallback draft crosses exactly one chunk boundary and stops at the timeline end')
    finally:
        server.shutdown();server.server_close()

if __name__=='__main__':
    asyncio.run(main())
