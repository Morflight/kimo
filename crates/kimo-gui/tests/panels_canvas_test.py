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
            await page.route('**/canvas.html', lambda route: route.fulfill(content_type='text/html', body='<main id="panel"></main>'))
            await page.goto(f'http://127.0.0.1:{server.server_address[1]}/canvas.html')
            result = await page.evaluate('''async () => {
              const {KINDS,POST}=await import('/ui/panels.js');
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
