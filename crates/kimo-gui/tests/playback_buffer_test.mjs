import {fileURLToPath} from 'node:url';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import assert from 'node:assert/strict';
const source = readFileSync(process.env.KIMO_PLAYBACK_SOURCE || new URL('../ui/playback.js', import.meta.url),'utf8');
function setup(end=10,stride=4) {
 let ms=0,id=0;const jobs=new Map();
 const ctx={window:{},performance:{now:()=>ms},requestAnimationFrame:f=>{jobs.set(++id,f);return id;},cancelAnimationFrame:id=>jobs.delete(id),clearTimeout(){},setTimeout(){}};
 vm.createContext(ctx);vm.runInContext(source,ctx, {filename: fileURLToPath(new URL('../ui/playback.js', import.meta.url))});
 const p=new ctx.window.KimoPlayer({fps:()=>60,duration:()=>end,scale:()=>.5,onTime(){},onState(){},canvas:{width:1,height:1,getContext:()=>({drawImage(){}})}});
 p.info={audio_on:false};p.playing=true;p.run={start:0,dir:1,stride};
 return {p,fill(a,b){for(let i=a;i<b;i++)p.frames.set(i*stride,{bmp:{width:1,height:1},time:i*stride/60});},step(dt){ms+=dt;const callbacks=[...jobs.values()];jobs.clear();callbacks.forEach(f=>f());}};
}
const h=setup();h.fill(0,8);h.p.rebuffer();h.step(10);assert.equal(h.p.buffering,true,'eight images must not restart a starved player');
h.fill(8,32);h.step(10);assert.equal(h.p.buffering,false,'two batches permit playback');
// At 15 preview fps, 16-frame batches every 900 ms sustain playback once a reserve exists.
for(let t=10;t<=3000;t+=10){if(t%900===0){const batch=t/900;h.fill(32+(batch-1)*16,32+batch*16);}h.step(10);assert.equal(h.p.buffering,false,`batch jitter must not stall at ${t}ms`);}
const tail=setup(.2);tail.fill(0,3);tail.p.rebuffer();tail.step(10);assert.equal(tail.p.buffering,false,'short timeline must not wait for nonexistent frames');
console.log('ok buffer waits for a reserve, absorbs batch jitter, and starts short tails');
