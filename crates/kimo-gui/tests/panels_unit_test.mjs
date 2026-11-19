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

for(const [mark,pattern] of [
 [{quad:[[0,0],[1,0],[1,1],[0,1]],roi:true},/stroke-dasharray="6 4"/],
 [{rect:[.1,.2,.3,.4],em:true,label:'Target'},/Target/],
 [{brackets:true,label:'HUD'},/HUD/],[{brackets:true},/<path/],
 [{to:[.1,.2,.3,.4]},/stroke-dasharray="4 4"/],
 [{text:'Counter',at:[.5,.5],box:true},/Counter/],[{text:0,at:[.5,.5]},/>0<\/text>/],
 [{shape:'ellipse',rect:[.1,.2,.3,.4]},/<ellipse cx="100" cy="80"/],
 [{shape:'rect',rect:[.1,.2,.3,.4]},/<rect/],
 [{path:[[0,0],[1,1]],closed:true},/<polygon/],[{path:[[0,0],[1,1]]},/<polyline/],
 [{squares:3},/opacity=".85"/],[{squares:3,seed:42},/opacity=".85"/],
 [{crop:.5},/<rect x="100" y="50" width="200" height="100"/],
 [{vline:.25},/x1="100" x2="100"/],
 [{pts:[{p:[.5,.5],v:[.1,.1]},{p:[0,0]}]},/x2="160" y2="80"/],
 [{rings:[{p:[.5,.5],r:12}]},/r="12"/],
 [{boxes:[[0,0,.2,.2],[.1,.1,.3,.3]]},/stroke-opacity="0.35"/],[{boxes:[]},/^$/],
 [{pt:[.5,.5]},/r="24"/],[{pt:[.5,.5],r:.1},/r="20"/],
 [{poly:[[0,0],[1,0],[1,1]]},/<polygon points="0,0 400,0 400,200"/],
 [{light:[.5,.5],kind:'subject'},/stroke-dasharray="3 3"/],
 [{light:[.5,.5],kind:'spot',label:'Key'},/Key/],
 [{cam:{}},/<path/],[{cam:{cam_fov:45,cam_pan:20,cam_tilt:10},keys:[0,20]},/tilt 10°/],
 [{body:[.5,.5],label:'Body'},/Body/],[{body:[.5,.5],r:.05},/r="20"/]
])assert.match(finite(overlay([mark],400,200)),pattern);
const transforms=finite(gizmo({prev:{},next:{x:10,y:20},null:true,parent:{x:40,y:10},read:['position']},400,200,{width:400,height:200},0));assert.match(transforms,/stroke-dasharray="3 3"/);assert.match(transforms,/position/);
assert.match(finite(render('tree',{items:[{name:'Selected',sel:true,icon:'cam'},{name:'Nested',d:1,icon:'unknown'}]},{guide:15})),/Nested/);
assert.match(finite(render('list',{rows:[{icon:'bulb',k:'Key',v:'2'},{icon:'custom',k:'Other',v:'1'}]},{chips:true,lines:true})),/custom/);
assert.match(finite(render('profile',{pts:[[0,0],[1,1]],thumbs:[{label:'Missing',sel:true}]},{ax:10,x1:100,base:200,top:40,tx:120,tw:60,gap:10,ty:40,th:50,labY:110,smooth:false,hollow:false})),/ptile sel/);
console.log('PASS overlay geometry for graphics, tracking, camera and physics, plus default tree/list/profile styling');
for(const data of [
 {src:'fixture://frame',falsecolor:{},progress:{halt:true,left:'stopped'},thumbs:[{dim:true},{src:'fixture://thumb',check:true}]},
 {stale:3,refused:'budget',refusedSrc:'inferred'}, {stale:4,refused:'engine stopped'},
 {sub:'<b>trusted</b>',subHtml:true}
]) {const html=finite(render('viewer',data));if(data.falsecolor)assert.match(html,/clip-path:inset\(0 0 0 50%\)/);if(data.refused)assert.match(html,/render refused/);if(data.sub)assert.match(html,/<b>trusted<\/b>/);}
for(const status of ['error','lost','ok']) {
 const rows=pickedRows({status,error:status==='error'?'failure':null,diff:[{k:'Position',from:1,v:2}],args:{empty:null,array:[1,2],object:{a:1,b:2,c:3,d:4}},result:{items:[1,2],value:3,empty:null}});
 assert.equal(rows.find(r=>r.k==='Status').v,status==='lost'?'no result (feed dropped)':status);assert.ok(rows.some(r=>r.sec==='Diff'));assert.ok(rows.some(r=>r.sec==='Result'));
}
assert.match(finite(render('keys',{strip:[{pts:[[0,0],[1,1]]}],lanes:[]},{stripBg:'#123',headLine:30})),/#123/);
assert.match(finite(render('curves',{curves:[{pts:[[0,0],[1,1]]}]},{band:[10,20]})),/fill="#171715"/);
assert.match(finite(render('diffbatch',{diff:[['?','unknown']]})),/unknown/);
assert.match(finite(gizmo({prev:{},next:{},path:[[100,100],[100,100]],read:[]},400,200,{},0)),/M140 40 C140 40 140 40 140 40/);
const trackDefaults=finite(render('timeline',{tracks:[{label:'Music',chip:true,mute:true,solo:true,wave:{},beats:{list:[1,2]},env:[[0,.2],[2,.8]],clips:[{s:0,e:1,label:'Cover',cover:'fixture://cover'}]}],playhead:2},{lane:true}));assert.match(trackDefaults,/labchip/);assert.match(trackDefaults,/class="s">S/);assert.match(trackDefaults,/fixture:\/\/cover/);
console.log('PASS missing viewer defaults, refusal states, selected-call diagnostics and timeline metadata');

assert.match(finite(render('diffbatch',{diff:[['+','partial']]},{col:{},txt:{}})),/color:var\(--text\)/);
const wheelData={wheels:[{name:'Lift',vals:['0','0','0'],em:true,ring:45}]};
const firstRing=finite(render('wheels',wheelData,{conic:true,r:30}));assert.equal((firstRing.match(/stroke="rgb\(/g)||[]).length,72);
for(let radius=31;radius<100;radius++)assert.match(finite(render('wheels',wheelData,{conic:true,r:radius})),new RegExp(`A${radius} ${radius} `));
assert.equal(render('wheels',wheelData,{conic:true,r:30}),firstRing,'eviction cannot change the wheel palette or geometry');
assert.match(finite(render('wheels',wheelData)),/url\(#ringE\)/);
const rigDefaultOptions={cx:100,cy:100,R:50,cols:[10,200,350,580],hy:170,ry:200,rh:30,bar:[280,420],thumbs:{x:10,y:260,w:60,h:30,gx:10,gy:5,ly:8}};
const rigDefaults=finite(render('lightrig',{lights:[{name:'Top',a:90},{name:'Right',a:0,em:true},{name:'Left',a:180}],rows:[['A','5000K','1',0,false,'2'],['B','','',.5,true,'']],thumbs:[{label:'Missing'}]},rigDefaultOptions));assert.match(rigDefaults,/Top/);assert.match(rigDefaults,/font-size:17px/);assert.match(rigDefaults,/border-radius:6px/);
const emptyRig=finite(render('lightrig',{thumbs:[]},rigDefaultOptions));assert.match(emptyRig,/<circle cx="100" cy="100" r="50"/);assert.match(emptyRig,/LIGHT/);assert.match(emptyRig,/FALLOFF/);assert.doesNotMatch(emptyRig,/<line /,'a rig without reported lights draws no light rays');assert.doesNotMatch(emptyRig,/class="cream"/,'a rig without reported rows renders no row values');
console.log('PASS conic-ring cache eviction preserves exact markup and measured light-rig defaults remain finite');
assert.equal(wave(100,40,-49297/9301),wave(100,40,(1-49297)/9301),'zero RNG state uses the deterministic seed-one fallback');
state.calls=[{name:'progress',status:'running',t:900,progress:{done:1,total:4}}];
const railDefaults=finite(render('agent',{}, {progress:true,curH:50}));assert.match(railDefaults,/padding-top:9px/);assert.match(railDefaults,/width:25.0%/);assert.doesNotMatch(railDefaults,/data-call=/);state.calls=[];
assert.match(finite(KINDS.viewer({spec:{},data:{},state:{gizmo:{t:1000,prev:{},next:{},read:[]}},w:600,h:300})),/class="ov gz"/);
assert.match(finite(render('loudness',{meters:[{l:'L'}]})),/background:#8d887e/);
for(const opts of [undefined,{colW:[100,100,100,100,100],footLine:200}]){
 const data={head:['name','preset','duration','status','time'],rows:['✕ failed','✓ done','queued','● running'].map(status=>['Name','preset','-',status,'-']),foot:'4 jobs'};
 let html=finite(render('renders',data,opts));assert.match(html,/failed/);assert.match(html,/queued/);assert.match(html,/4 jobs/);
 data.footEm='absent';assert.match(finite(render('renders',data,opts)),/4 jobs/);data.footEm='4';assert.match(finite(render('renders',data,opts)),/<span class="cream">4<\/span>/);
}
assert.match(finite(render('lightrig',{rows:[['Fill','','1',.3,false,'']],thumbs:[{label:'Empty'}]})),/#bdb8ae/);
assert.match(finite(render('scopes',{wave:{bins:2,cols:['zz']},img:{kind:'waveform',src:'fixture://wave'}})),/fixture:\/\/wave/);
const selected=pickedRows({status:'ok',args:[null,{a:1},'text'],result:{small:{a:1},flag:false}});assert.ok(selected.every(r=>r.k!=='#1'));assert.ok(selected.some(r=>r.k==='#2'&&r.v==='{a}')); assert.ok(selected.some(r=>r.k==='small'&&r.v==='{a}'));
console.log('PASS deterministic RNG fallback, rail progress defaults, render statuses/footers and sparse diagnostic values');
const detailedTimeline=finite(render('timeline',{ruler:{t0:0,t1:20,step:5,align:'right'},playhead:2,razor:{at:2,t:1000},tracks:[{label:'Solo',solo:true,lane:'#333',area:{},wave:{env:[[0,.2],[20,.8]],beats:[1,2]},clips:[{s:-5,e:-1,label:'Off left'},{s:25,e:30,label:'Off right'}]}]}));assert.match(detailedTimeline,/translateX\(-100%\)/);assert.match(detailedTimeline,/stroke-width="1.5"/);assert.match(detailedTimeline,/class="razor"/);assert.doesNotMatch(detailedTimeline,/Off left|Off right/);
const flashingClips=Array.from({length:35},(_,i)=>({s:i*.1+(i>20?3:0),e:i*.1+.1+(i>20?3:0),_flash:1000}));
const mergedFlash=finite(render('timeline',{tracks:[{clips:flashingClips}]}));assert.equal((mergedFlash.match(/class="clip crun flash"/g)||[]).length,2);
const panelArgs={spec:{title:'Fixture'},state,data:{ruler:{t0:0,t1:20000,step:5000},playhead:100,tracks:Array.from({length:15},(_,i)=>({clips:[{s:0,e:1,label:'Clip'+i}]}))},w:600,h:150,pan:{z:2,k:0}};
const zoomed=finite(KINDS.timeline(panelArgs));assert.equal(panelArgs.pan.t,0);assert.match(zoomed,/class="tlbar"/);assert.match(zoomed,/class="tlbar v"/);assert.doesNotMatch(zoomed,/Clip14/);
const plainPan=finite(KINDS.timeline({...panelArgs,pan:{z:1,k:1}}));assert.match(plainPan,/class="playhead"/);
panels.namespace.TL_SMALL.on=true;assert.match(finite(render('timeline',{tracks:[{}]},{trackY:40})),/top:34px;height:30px/);panels.namespace.TL_SMALL.on=false;
assert.match(finite(render('keys',{graph:{pts:[0,1],max:1}})),/stroke-width="1.3"/);
assert.match(finite(render('stack',{nodes:[{name:'Centered'}]},{center:true})),/font-size:12px/);
console.log('PASS timeline clipping, flashing-run compaction, zoom/scroll defaults and compact track layout');
const blankThumb=finite(render('profile',{thumbs:[{label:'Missing source'}]}));
assert.match(blankThumb,/Missing source/);assert.match(blankThumb,/stroke="#2a2a28"/);
assert.deepEqual(JSON.parse(JSON.stringify(pickedRows({args:'scalar argument'}))),[{sec:'Call'},{k:'Status',v:'ok',ch:false},{sec:'Args'},{k:'args',v:'scalar argument'}]);
console.log('PASS missing-thumbnail placeholder and scalar call argument summary');
for (const args of [null, undefined]) {
 assert.deepEqual(JSON.parse(JSON.stringify(pickedRows({args}))),[{sec:'Call'},{k:'Status',v:'ok',ch:false}], 'absent arguments must not create empty inspector sections');
}
const unselectedSwatch=finite(block({widget:'swatches',items:[{hi:'#fff',mid:'#888',lo:'#000'}]},120,80));
assert.match(unselectedSwatch,/stroke="#2a2a28" stroke-width="1"/);
assert.match(unselectedSwatch,/fill="url\(#sw0\)"/);
assert.doesNotMatch(unselectedSwatch,/<image/);
