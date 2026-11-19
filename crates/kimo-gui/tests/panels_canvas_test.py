"""Canvas pixel regressions using the complete panel module in real Chromium."""
import asyncio
from playwright.async_api import async_playwright
from studio_actions_test import serve

async def main():
    server = serve()
    try:
        async with async_playwright() as playwright:
            browser = await playwright.chromium.launch()
            page = await browser.new_page()
            await page.route('**/canvas.html', lambda route: route.fulfill(content_type='text/html', body='<link rel="stylesheet" href="/ui/app.css"><main id="panel"></main>'))
            await page.goto(f'http://127.0.0.1:{server.server_address[1]}/canvas.html')
            result = await page.evaluate('''async () => {
              const {KINDS,POST,PRE}=await import('/ui/panels.js');
              const el=document.querySelector('#panel');
              const check=(ok,message)=>{if(!ok)throw Error(message)};
              const pixel=(cv,x,y)=>[...cv.getContext('2d').getImageData(x,y,1,1).data];
              const draw=(kind,data={},opts)=>{const args={spec:{title:'Fixture',...(opts?{opts}:{})},data,w:kind==='scopes'?1024:160,h:kind==='scopes'?300:120,state:{calls:[],session:{still:true}}};el.innerHTML=KINDS[kind](args);POST[kind]?.(el,args);return args;};
              POST.cie(el,{spec:{},w:160,h:120});POST.viewer(el);POST.scopes(el,{spec:{},data:{},w:160,h:120});
              const snapshots=[];
              for(let i=0;i<7;i++){
                const opts={scx:110+i,scy:110+i,ox:20,oy:110,fillOp:.5};
                const args=draw('cie',{gamuts:[{pts:[[.1,.1],[.6,.2],[.3,.7]],dash:true,label:'Triangle'}, {pts:[[.2,.2],[.3,.2],[.2,.3]]}],points:[[.3,.3],[.4,.4,'ring']]},opts);
                const cv=el.querySelector('canvas');const center=pixel(cv,55,74);
                check(center[3]===128,'CIE opacity follows configured value');check(center.slice(0,3).some(v=>v>0),'CIE interior contains colour');check(pixel(cv,0,0)[3]===0,'CIE exterior stays transparent');
                const before=cv.toDataURL();POST.cie(el,args);check(cv.toDataURL()===before,'CIE cached repaint preserves pixels');snapshots.push(before);
              }
              check(new Set(snapshots).size===7,'CIE geometry changes its pixels');
              draw('cie');check(el.querySelector('canvas').width===160,'default CIE canvas width');
              Object.defineProperty(window,'devicePixelRatio',{configurable:true,value:0});
              draw('cie');check(el.querySelector('canvas').width===160,'missing device scale falls back to one');

              const scope=draw('scopes',{wave:{bins:4,cols:['0z90','900z']}});
              const cv=el.querySelector('canvas');check(cv&&cv.width===1024,'scope raster exists');
              const baseline=cv.toDataURL();POST.scopes(el,scope);check(cv.toDataURL()===baseline,'scope repaint clears previous pixels');
              draw('scopes',{wave:{bins:4,cols:['0z90','900z']},vec:{pts:[[0,0,'#ff0000']],targets:[[.2,.2]],skin:[[0,0],[.3,.2]]}},{splitLine:true});
              check(el.querySelector('canvas').toDataURL()!==baseline,'vector points and skin line change raster');
              // Distinct in-memory pictures exercise eviction without accessing user media.
              const outputs=[];
              for(let i=0;i<11;i++){
                const src=document.createElement('canvas');src.width=16;src.height=16;const c=src.getContext('2d');c.fillStyle=`rgb(${20+i*20},${20+i*20},${20+i*20})`;c.fillRect(0,0,16,16);
                el.innerHTML='<div class="frame"><img><canvas class="fc" width="16" height="16"></canvas></div>';
                const im=el.querySelector('img');POST.viewer(el);im.src=src.toDataURL();await im.decode();
                POST.viewer(el);const target=el.querySelector('canvas'),first=target.toDataURL();POST.viewer(el);check(target.toDataURL()===first,'false-colour cache preserves pixels');
                check(pixel(target,8,8)[3]===255,'false-colour output remains opaque');outputs.push(first);
              }
              check(new Set(outputs).size>8,'different exposures remain distinguishable');
              for(const width of [20,45,120,260,600]) {
                el.style.cssText=`position:relative;width:${width}px;height:300px`;
                const missing={name:'missing id',args:'one two three',status:'ok'};
                const call={id:'sparse',name:'set_property',args:'hero, scale: 100% → 120%, extra long argument',status:'ok'};
                const args={spec:{title:'Fixture'},state:{mode:'edit',session:{still:true},calls:[missing,call]},data:{},w:width,h:300};
                PRE.agent(el,args);
                check(!missing._fit,'a row without an identifier cannot acquire another call fit');
                check(call._fit&&call._fit.k,'identified row receives a measured fit even without duration');
                const fit=call._fit;PRE.agent(el,args);check(call._fit===fit,'unchanged geometry reuses its measured fit');
                el.innerHTML=KINDS.agent(args);
                check(el.querySelector('[data-call="sparse"] .sig').title==='set_property(hero, scale: 100% → 120%, extra long argument)','full call remains available in title');
              }
              // Sweep real layout boundaries where measured text first fits: preserve the
              // complete accessible call while changing only its visible compact rendering.
              for(const name of ['x','look','preview_material']) for(let width=24;width<=240;width++) {
                el.style.cssText=`position:relative;width:${width}px;height:300px`;
                const call={id:'boundary',name,args:'subject with several words, threshold: 0.5',status:'ok',dur_ms:300};
                const args={spec:{title:'Fixture'},state:{mode:'edit',session:{still:true},calls:[call]},data:{},w:width,h:300};
                PRE.agent(el,args);el.innerHTML=KINDS.agent(args);
                const sig=el.querySelector('[data-call="boundary"] .sig');
                check(sig.title===`${name}(subject with several words, threshold: 0.5)`,'compact layout retains the full accessible call');
                check(call._fit&&typeof call._fit.tight==='boolean','layout boundary has a measured fit');
              }
              // Fractional CSS widths exercise subpixel rounding at the ellipsis boundary.
              for(let eighths=192;eighths<=1920;eighths++) {
                const width=eighths/8;el.style.cssText=`position:relative;width:${width}px;height:300px`;
                const call={id:'fractional',name:'look',args:'frame quality assessment, threshold: 0.5',status:'ok',dur_ms:300};
                const args={spec:{title:'Fixture'},state:{mode:'edit',session:{still:true},calls:[call]},data:{},w:width,h:300};
                PRE.agent(el,args);el.innerHTML=KINDS.agent(args);
                check(el.querySelector('.sig').title==='look(frame quality assessment, threshold: 0.5)','subpixel fitting retains the full call');
                check(call._fit&&typeof call._fit.tight==='boolean','subpixel fitting records its layout choice');
              }
              return {cie:snapshots.length,exposures:outputs.length};
            }''')
            assert result == {'cie': 7, 'exposures': 11}, result
            print('PASS real CIE raster/cache, waveform/vector raster repaint and false-colour exposure cache')
            await browser.close()
    finally:
        server.shutdown()
        server.server_close()

if __name__ == '__main__':
    asyncio.run(main())
