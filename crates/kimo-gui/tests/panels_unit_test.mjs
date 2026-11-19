import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {dirname,resolve} from 'node:path';
import vm from 'node:vm';
const directory=fileURLToPath(new URL('../ui/',import.meta.url)), modules=new Map();
const context=vm.createContext({performance:{now:()=>1000},document:{documentElement:{}},getComputedStyle:()=>({getPropertyValue:()=>''})});
function get(filename){if(!modules.has(filename))modules.set(filename,new vm.SourceTextModule(readFileSync(filename,'utf8')+(filename===resolve(directory,'panels.js')?'\nexport {overlay,gizmo,block,wave};':''),{identifier:filename,context}));return modules.get(filename);}
const root=get(resolve(directory,'router.js'));await root.link((name,parent)=>get(resolve(dirname(parent.identifier),name)));await root.evaluate();
const panels=get(resolve(directory,'panels.js'));await panels.link((name,parent)=>get(resolve(dirname(parent.identifier),name)));await panels.evaluate();
const {KINDS,overlay,gizmo,block,wave,fmtArgs,pickedRows,panRows}=panels.namespace;
const state={calls:[],session:{still:true},mode:'edit',callCount:0};
const render=(kind,data={},opts)=>KINDS[kind]({spec:{title:'Fixture',...(opts?{opts}:{})},data,state,w:600,h:300});
const finite=html=>{assert.doesNotMatch(html,/NaN|Infinity|undefined/);return html;};
assert.equal(fmtArgs(null),'');assert.equal(fmtArgs('raw'),'raw');assert.equal(fmtArgs([null,{a:1,b:null},'x']), 'a: 1, x');
assert.equal(fmtArgs({mask_id:'m',clip_id:'c',time:1.234},'set_mask_path_key'),'m, t: 1.2');
assert.equal(fmtArgs({path:'C:\\tmp\\frame.png',amount:1.23456}),'frame.png, amount: 1.235');
assert.equal(fmtArgs({nested:{x:1},bad:Infinity}),'nested: {"x":1}');
for(const lens of [{lens:[0,0]},{lens:[.4,-.1],c:[0,.25]},{lens:[-.2,.1],c:[]}]) {
 const html=finite(overlay([lens],400,200));assert.equal((html.match(/<polyline/g)||[]).length,12);
 if(lens.lens[0]===0)assert.match(html,/points="50,0 50,10/);
}
for(const a of [0,45,-45,270,-270]) {
 const html=finite(gizmo({prev:{},next:{rot_x:a,rot_y:a},three:true,read:['rotation']},400,200,{width:400,height:200},0));
 assert.equal((html.match(/stroke-width="2.2"/g)||[]).length,a?2:0);assert.match(html,/rotation/);
}
for(const seed of [undefined,42]) {const html=finite(block({widget:'solve',seed},200));assert.equal((html.match(/<circle/g)||[]).length,261);assert.match(html,/stroke-dasharray="2 4"/);}
assert.equal(wave(100,40,7),wave(100,40,7));assert.notEqual(wave(100,40,7),wave(100,40,8));assert.match(wave(1,10,0,.5,1,false),/^M.*Z$/);
const waveHtml=finite(render('timeline',{tracks:[{label:'Audio',wave:{}}]}));assert.match(waveHtml,/Audio/);assert.match(waveHtml,/<path d="M/);
assert.match(finite(render('media',{items:[{name:'Sound',wave:true}]})),/<svg viewBox="0 0 38 24"/);
for(const kind of ['waveform','vectorscope']) {const html=finite(render('scopes',{img:{kind,src:'fixture://scope',tc:'00:01'}}));assert.match(html,/scopedock/);assert.match(html,/fixture:\/\/scope/);assert.match(html,/00:01/);}
const clips=Array.from({length:20},(_,i)=>({s:i*.2,e:(i+1)*.2}));
const first=finite(render('timeline',{ruler:{t0:0,t1:20,step:5},tracks:[{clips}]}));assert.equal((first.match(/class="clip crun/g)||[]).length,1);assert.match(first,/linear-gradient/);
assert.equal(render('timeline',{ruler:{t0:0,t1:20,step:5},tracks:[{clips}]}),first,'unchanged dense runs preserve markup');
assert.match(finite(render('timeline',{tracks:[{clips:[{s:0,e:.01}]}]})),/class="clip crun/);
assert.match(finite(render('stack',{nodes:[{name:'Source'},{name:'Modifier',em:true}]})),/Modifier/);
const profile=finite(render('profile',{pts:[[0,0],[1,1]],cursor:.5,thumbs:[{label:'Start',src:'fixture://start'},{label:'End',sel:true}]}));assert.match(profile,/fixture:\/\/start/);assert.match(profile,/End/);
const lights=finite(render('lightrig',{lights:[{name:'Key',a:0,em:true},{name:'Fill',a:180}],rows:[['Key','5600K','80%',.8,true,'soft']],thumbs:[{label:'Lit',src:'fixture://lit',sel:true}]}));assert.match(lights,/Key/);assert.match(lights,/Fill/);assert.match(lights,/5600K/);
console.log('PASS panel formatting, lens grid, 3D arcs, solve cloud, waveform determinism, dense timeline seams and compact layouts');
for(const kind of Object.keys(KINDS))assert.match(finite(render(kind)),/Fixture/,`${kind} retains its title with empty data`);
for(const opts of [undefined,{stripBg:'#222',stripEmOp:.5,stripX:2,stripR:3,stripPad:5,rTickDx:1,rulerHl:true,sepX:20,divX:180,headLine:25,altBg:'#222',selX:180,selGlow:['#444',10,20],laneSep:'#555',laneBg:'#111',areaR:true,keyLine:'#666',emKeyScale:2}]) {
 const html=finite(render('keys',{metaChip:'Keys',strip:[{pts:[[0,0],[20,1]],em:true},{pts:[[0,1],[20,0]]}],graph:{pts:[0,1,0],max:1,thr:.5},playhead:5,lanes:[
  {label:'Selected',sel:true,keys:[0,{t:5,em:true}],line:true,bar:[[0,5]],cache:[4,12],area:[[0,0],[5,1]],dots:[2,4]},
  {label:'Normal',keys:[1,{t:2}],area:[[1,0],[3,1]],color:'#fff',stroke:'#aaa',r:3,dotColor:'#bbb',dots:[1]},
  {label:'Alternate',html:true,keys:[]} ]},opts));
 assert.match(html,/Selected/);assert.match(html,/Normal/);assert.match(html,/playhead/);assert.match(html,/var\(--ember\)/);
}
for(const beats of [{list:[1,2]},{t0:0,step:5}]) {
 const html=finite(render('curves',{beats,playhead:5,lanes:['Opacity'],curves:[{pts:[[0,0],[10,1]],keys:[[0,0],[10,1,'red','blue']],handles:[[[0,0],[2,.5]]],em:true},{pts:[[0,1],[10,0]],smooth:true,color:'green',w:2,dash:'2 2',cls:'curve'}]}));assert.match(html,/Opacity/);assert.match(html,/stroke="blue"/);assert.match(html,/stroke="green"/);
}
assert.match(finite(render('transcript',{lines:[{spk:'A',tc:'0',text:'<escaped>'},{spk:'B',tc:'1',html:'<b>safe</b>'}]})),/&lt;escaped&gt;/);
for(const opts of [undefined,{nameCream:true}]) {const html=finite(render('history',{items:[{id:'one',name:'Saved',src:'fixture://saved',live:true,dim:true,sel:true},{id:'two',name:'Next'}]},opts));assert.match(html,/Saved/);assert.match(html,/Next/);assert.match(html,/opacity:.45/);}
for(const opts of [undefined,{col:{'+':'green','~':'orange','-':'red'},txt:{'+':'white','~':'white','-':'white'},metaInline:true,nameC:'red',lookC:'green',activeC:'blue',doneC:'white'}]) {
 const html=finite(render('diffbatch',{diff:[['+','new'],['~','changed'],['-','removed']],batch:{title:'Batch',meta:'3',rows:[['a','look','queued'],['b','look','grading'],['c','look','done']]}},opts));assert.match(html,/changed/);assert.match(html,/queued/);assert.match(html,/grading/);assert.match(html,/done/);
}
for(const widget of [
 {widget:'eq'}, {widget:'eq',bands:[{f:200,type:'highpass',n:3},{f:12000,type:'lowpass',n:2},{f:200,g:2,type:'lowshelf'},{f:8000,g:-2,type:'highshelf'},{f:1000,g:4,q:2,sel:true,_flash:1000},{f:4000,g:-3,dim:true}]},
 {widget:'comp',inDb:-6,_grT:1000,_flashT:1000,_flashR:1000},
 {widget:'comp',threshold:-12,ratio:4,boxW:90,boxH:70,boxY:5,gap:10,fs:14},
 {widget:'swatches',items:[{hi:'white',mid:'gray',lo:'black',sel:true},{src:'fixture://swatch',hi:'white',mid:'gray',lo:'black'}]},
 {widget:'thumbs',items:[{label:'One',src:'fixture://one',w:80,sel:true},{label:'Two',h:70}]},
 {widget:'thumbs',items:[{},{}]},{widget:'unknown'}]) assert.match(finite(block(widget,240)),/class="widget/);
for(const b of [{sec:'Section',h:10},{gap:12},{list:['1 numbered','plain'],sel:0},{curve:[[0,0],[1,1]],ch:'g'},{curve:[[0,1]],ch:'b'},{curve:[[0,1]],ch:'r'},{code:'<code>'},{cap:['Label']},{cap:['Label','Value']},{widget:'unknown',src:'fixture://image',style:'opacity:.5'},{html:'<b>trusted</b>'},{k:'Gain',v:'3',from:'2',chip:true,emb:true,bar:true,dim:true,tight:true,h:20,kStyle:'color:red',vStyle:'color:blue'}])assert.ok(finite(block(b,200)).length);
console.log('PASS empty panels, key lanes, curves, transcripts, history, batch status and inspector widget variants');

assert.match(block({widget:'comp',gap:10},240),/Threshold/);assert.equal(block({gap:10},240),'<div style="height:10px"></div>');
