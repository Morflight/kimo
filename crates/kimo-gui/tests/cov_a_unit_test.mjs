import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {dirname, resolve} from 'node:path';
import vm from 'node:vm';
const directory = fileURLToPath(new URL('../ui/', import.meta.url));
const context = vm.createContext({performance:{now:()=>1000}}), modules = new Map();
function get(filename) {
  if (!modules.has(filename)) {
    const access=filename===resolve(directory,'router.js')?'\nexport { COV_H as testHelpers };':filename===resolve(directory,'cov_a.js')?'\nexport { rows as testRows, panelOf as testPanel, capList as testCap, mcClips as testMulticam };':'';
    modules.set(filename,new vm.SourceTextModule(readFileSync(filename,'utf8')+access,{identifier:filename,context}));
  }
  return modules.get(filename);
}
// Enter through router, as the UI does, so the real cyclic imports initialize in order.
const root=get(resolve(directory,'router.js'));
await root.link((name,parent)=>get(resolve(dirname(parent.identifier),name)));
await root.evaluate();
const modes=get(resolve(directory,'modes.js'));
if(modes.status==='unlinked') await modes.link(()=>{throw Error('unexpected mode dependency');});
await modes.evaluate();
const {MODES}=modes.namespace;
const {covAOps,covAPre,covASee,covAHistory,covAPost,testRows,testPanel,testCap,testMulticam}=get(resolve(directory,'cov_a.js')).namespace;
const H=root.namespace.testHelpers, plain=x=>JSON.parse(JSON.stringify(x));
const fresh=(mode='graph')=>({mode,explicitMode:true,views:{},preview:{},liveTL:{tracks:[{id:'v1',label:'Video',clips:[{id:'c1',label:'cam.mp4',s:0,e:10}]}]}});
const event=(tool,args={},result={})=>({tool_name:tool,call_id:'test',phase:'complete',payload:{args,result}});
const blocks=s=>Object.values(s.views[s.mode]||{}).find(p=>p.blocks)?.blocks.filter(b=>b.k).map(b=>[b.k,b.v,...(b.from!=null?[b.from]:[])])||[];
const run=(s,tool,args={},result={})=>covAOps(s,MODES,event(tool,args,result),H);
for(const [tool,expected] of [
 ['remove_marker',[['Marker','removed']]],['detect_cuts',[['Cuts','none found'],['Source','timeline']]],
 ['detect_beats',[['Beats','0']]],['bind_beats',[['Property','value'],['Hit','+0.1 · pulse'],['Beats','0']]],
 ['remove_silence_cuts',[['Removed','0 ranges · 0 s']]],['set_time_mode',[['Time','normal']]],
 ['sync_clips',[['Synced','0 clips'],['By','audio']]],['burn_in',[['Burn-in','off','on']]],
 ['transition_render_raw',[['Transition','transition']]],['match_events',[['Matched','0 of ? → ?'],['Within','0'],['Mode','nearest']]],
]) {const s=fresh();assert.equal(run(s,tool),true);assert.deepEqual(plain(blocks(s)),expected,tool);}
for(const tool of ['unknown','add_marker','add_multicam','switch_angle'])assert.equal(run(fresh(),tool),false);
for(const value of [null,{},'',NaN,Infinity,'bad'])assert.equal(run(fresh(),'add_marker',{time:value}),false);
const markers=fresh();for(let i=0;i<66;i++)run(markers,'add_marker',{time:i,name:i===65?'  Last  ':undefined},{marker_id:i});
assert.equal(markers.liveTL.markers.length,64);assert.equal(markers.liveTL.markers[0].t,2);assert.equal(markers.liveTL.markers.at(-1).name,'Last');
run(markers,'add_marker',{time:7,duration:-1},{marker_id:65});assert.equal(markers.liveTL.markers.at(-1).d,0);
run(markers,'remove_marker',{marker_id:65});assert.equal(markers.liveTL.markers.at(-1).gone,1000);
run(markers,'remove_marker',{marker_id:2});assert.equal(markers.liveTL.markers[0].gone,1000);
const cuts=fresh();run(cuts,'detect_cuts',{source:'file:missing.mp4'},{cuts:[null,{}, {time:'2'}]});assert.deepEqual(plain(cuts.liveTL.cuts.ts),[{t:2,kind:'cut'}]);
run(cuts,'detect_cuts',{source:'clip:missing'},{cuts:[]});assert.deepEqual(plain(blocks(cuts)),[['Cuts','none found'],['Source','clip']]);
run(cuts,'detect_beats',{},{});assert.deepEqual(plain(blocks(cuts)),[['Beats','0']]);
for(const [span,step] of [[10,2],[20,5],[40,10]]) for(const curve of ['pulse','swing','kick','flash']) {
 const s=fresh();s.liveTL.tracks[0].clips[0].e=span;
 run(s,'bind_beats',{clip_id:'c1',beats:[null,-1,0,1,2,span],curve,amount:-0.5});
 assert.equal(s.views.graph.graph.ruler.step,step);
 assert.deepEqual(plain(s.liveTL.tracks[0].clips[0].keys.map(k=>k.t)),[0,1,2]);
 assert.equal(s.views.graph.graph.curves[0].keys.length,3);
 assert.ok(blocks(s).some(r=>r[0]==='Hit'&&r[1]===`-0.5 · ${curve}`));
}
const pulse=fresh();run(pulse,'bind_beats',{clip_id:'c1',bpm:60,every:2,offset:1});assert.deepEqual(plain(pulse.liveTL.tracks[0].clips[0].keys.map(k=>k.t)),[1,3,5,7,9]);
for(const mode of ['freeze','reverse','normal','normal'])run(pulse,'set_time_mode',{clip_id:'c1',mode});
assert.equal(pulse.liveTL.tracks[0].clips[0].fx,undefined);assert.deepEqual(plain(blocks(pulse)),[['Clip','cam.mp4'],['Time','normal']]);
const sync=fresh();run(sync,'sync_clips',{clip_ids:['missing','c1']},{offsets_seconds:[0,null]});assert.equal(sync.liveTL.tracks[0].clips[0].s,0);
run(sync,'sync_clips',{clip_ids:['c1']},{offsets_seconds:[0]});assert.deepEqual(plain(blocks(sync)),[['Synced','1 clips'],['By','audio'],['cam.mp4','0 s']]);
for(const operator of [{},{type:'wipe'},null]) {const s=fresh();run(s,'transition_render_raw',{operator});assert.equal(blocks(s)[0][1],operator?.type||'transition');}
for(const mean of [-1,0,1]) {const s=fresh();run(s,'match_events',{a:[1],b:[2]},{pairs:[null,{}],stats:{mean_offset_s:mean}});assert.ok(blocks(s).some(r=>r[0]==='Mean offset'&&r[1]===(mean<0?'−1 s':mean>0?'+1 s':'0 s')));}
console.log('PASS timeline defaults, capped markers, beat curves and synchronization preserve measured output');
assert.deepEqual(plain(testCap([1,null,2])),[1,2]);
const directRows={};testRows(directRows,'Title',[['A','B']], 'Subtitle',H);assert.equal(directRows.sub,'Subtitle');
const unavailable=fresh('absent');assert.equal(run(unavailable,'burn_in'),true);assert.deepEqual(plain(unavailable.views.absent),{});
for(const tl of [{},{tracks:[{}, {clips:[{}]}]}]) {const s=fresh();s.liveTL=tl;run(s,'detect_beats',{path:'file.mp4',min_gap:0.2});assert.ok(blocks(s).some(r=>r[0]==='Min gap'&&r[1]==='0.2 s'));}
const raw=fresh();raw.liveTL.tracks[0].clips[0].label=null;run(raw,'bind_beats',{clip_id:'c1',beats:[1]});assert.equal(raw.views.graph.graph.lanes[0],'clip · value');
run(raw,'sync_clips',{clip_ids:['c1']},{offsets_seconds:[1]});assert.equal(blocks(raw)[2][0],'clip');
const emptySee=fresh();covASee(emptySee,{});assert.equal(emptySee._capGroups,undefined);
for(const ev of [{phase:'start'}, {phase:'start',payload:{args:1}}, {phase:'complete'}, {phase:'error'}])assert.equal(covAPre(fresh(),ev,H),ev);
const cleaned=covAPre(fresh(),{phase:'complete',payload:{from:[1],param:'x'}},H);assert.deepEqual(plain(cleaned),{phase:'complete',payload:{},target:{param:null}});
const pre=fresh();pre.liveTL.markers=[{id:'m',t:2}];
let prepared=covAPre(pre,{phase:'start',call_id:'pre',payload:{args:{marker_id:'m',angles:['missing'],clip_ids:[],beats:[1,2],optional:undefined}}},H);
assert.deepEqual(plain(prepared.payload.args),{marker:'unnamed @ 2 s',beats:'2 beats'});
delete pre.liveTL.markers;prepared=covAPre(pre,{phase:'start',payload:{args:{marker_id:'m'}}},H);assert.deepEqual(plain(prepared.payload.args),{});
const cached=fresh();cached._editArgs={test:{time:5}};assert.equal(covAOps(cached,MODES,{tool_name:'add_marker',call_id:'test'},H),true);assert.equal(cached.liveTL.markers[0].t,5);
assert.equal(covAOps(fresh(),MODES,{tool_name:'add_marker'},H),false);
assert.equal(run(fresh(),'add_marker',{},null),false);
const silence=fresh();silence.liveTL.tracks[0].clips=[{s:0,e:1},{s:6,e:8},{s:1,e:3},{s:3,e:6},{s:3,e:4},{label:'split',s:1,e:7,thumb:'thumbnail'}];
run(silence,'remove_silence_cuts',{track_id:'v1',ranges:[null,[2,2],[2,5]]});
assert.deepEqual(plain(silence.liveTL.tracks[0].clips.map(c=>[c.s,c.e])),[[0,1],[1,2],[1,2],[2,3],[2,4],[3,5]]);
assert.equal(silence.liveTL.tracks[0].clips.find(c=>c.id==='split_r').thumb,undefined);
const emptyTrack=fresh();delete emptyTrack.liveTL.tracks[0].clips;run(emptyTrack,'remove_silence_cuts',{track_id:'v1',ranges:[[1,2]]});assert.deepEqual(plain(emptyTrack.liveTL.tracks[0].clips),[]);
for(const tl of [{},{tracks:[{}]}]) {const s=fresh();s.liveTL=tl;run(s,'add_multicam',{angles:['missing']},{start:0,duration:5,multicam_id:'m'});assert.ok(blocks(s).some(r=>r[0]==='Audio'&&r[1]==='clip'));}
const multi=fresh();run(multi,'add_multicam',{}, {start:0,duration:10,multicam_id:'m'});
for(const angle of [2,2,3])run(multi,'switch_angle',{multicam_id:'m',t:2,angle});
assert.ok(blocks(multi).some(r=>r[0]==='Audio'&&r[1]==='angle 1 (fixed)'));
assert.equal(multi.liveTL._mc.m.cuts.length,1);
run(multi,'switch_angle',{multicam_id:'m',t:4,angle:3});assert.equal(multi.liveTL.tracks.find(t=>t.id==='multicam').clips.length,2);
const mt={};testMulticam(mt,{s:0,e:2,track:'test',names:[],cuts:[]},1000,null);assert.equal(mt.clips[0].label,'clip · angle 1');
mt.clips.push({s:-1,e:3});testMulticam(mt,{s:0,e:2,track:'test',names:[],cuts:[]},1000,1);assert.equal(mt.clips.length,2);
const hist=fresh();delete hist.liveTL.tracks;const mutation=event('add_marker',{time:1},{marker_id:'m'});mutation.payload.mutated=true;
covAHistory(hist,MODES,mutation,H,()=>run(hist,'add_marker',{time:1},{marker_id:'m'}));
covAHistory(hist,MODES,{tool_name:'undo'},H,()=>false);assert.equal(hist.liveTL.markers[0].gone,1000);
delete hist.liveTL.markers;covAHistory(hist,MODES,event('redo'),H,()=>false);assert.equal(hist.liveTL.markers[0].t,1);
for(let i=0;i<50;i++)covAHistory(hist,MODES,{tool_name:'other',payload:{mutated:true}},H,()=>true);
assert.equal(hist._covHist.undo.length,48);covAHistory(hist,MODES,event('undo',{}, {undone:1,undo_left:0}),H,()=>false);assert.equal(hist._covHist.undo.length,0);
const post=(data,o={})=>covAPost(data,x=>x*10,o,x=>x>=0&&x<100,t=>t===1000);
assert.equal(post({markers:[{t:-1},{t:1,gone:1}],cuts:{},joints:{t:1000}}),'');
assert.equal(post({cuts:{ts:[{t:20}]},joints:{t:1000,at:[20]}}),'');
assert.equal(post({markers:[{t:1}]}),'<i class="tlmk" style="left:10px;top:15px;height:13px"></i>');
console.log('PASS sparse rail arguments, ripple boundaries, multicam defaults and history alignment');
const sparseTracks=fresh();sparseTracks.liveTL.tracks.push({id:'empty'});
run(sparseTracks,'sync_clips',{clip_ids:['c1']},{offsets_seconds:[0]});assert.equal(sparseTracks.liveTL.tracks[1].clips,undefined);
const saved=event('add_marker',{time:2},{marker_id:'keep'});saved.payload.mutated=true;
covAHistory(sparseTracks,MODES,saved,H,()=>run(sparseTracks,'add_marker',{time:2},{marker_id:'keep'}));
const again=event('add_marker',{time:3},{marker_id:'next'});again.payload.mutated=true;
covAHistory(sparseTracks,MODES,again,H,()=>run(sparseTracks,'add_marker',{time:3},{marker_id:'next'}));
covAHistory(sparseTracks,MODES,event('undo'),H,()=>false);
assert.equal(sparseTracks.liveTL.markers.find(m=>m.id==='keep').gone,undefined);
assert.equal(sparseTracks.liveTL.markers.find(m=>m.id==='next').gone,1000);
const coveredSpan={clips:[{id:'inside',s:0,e:1},{id:'overlap',s:0,e:3}]};
testMulticam(coveredSpan,{s:0,e:2,track:'test',names:[],cuts:[]},1000,null);
assert.deepEqual(plain(coveredSpan.clips.map(c=>c.id)),['overlap','mc:test:0']);
for(const [operator,expected] of [[{},'op'],[{type:'wipe'},'wipe']])assert.equal(covAPre(fresh(),{phase:'start',payload:{args:{operator}}},H).payload.args.operator,expected);
const emptySync=fresh();delete emptySync.liveTL.tracks;run(emptySync,'sync_clips');assert.deepEqual(plain(blocks(emptySync)),[['Synced','0 clips'],['By','audio']]);
const effects=fresh();effects.liveTL.tracks[0].clips[0].fx=['reverse'];
const timeEdit=event('set_time_mode',{clip_id:'c1',mode:'normal'});timeEdit.payload.mutated=true;
covAHistory(effects,MODES,timeEdit,H,()=>run(effects,'set_time_mode',{clip_id:'c1',mode:'normal'}));
covAHistory(effects,MODES,event('undo'),H,()=>false);assert.deepEqual(plain(effects.liveTL.tracks[0].clips[0].fx),['reverse']);
const {cSnap,cPush}=get(resolve(directory,'project_ops.js')).namespace;
const lane=fresh(), beforeLane=cSnap(lane,MODES);lane.liveTL.tracks[0].clips[0].label='renamed';const afterLane=cSnap(lane,MODES);
lane._covHist={undo:[null],redo:[]};cPush(lane,beforeLane,afterLane,'set_meta',{});
assert.equal(covAHistory(lane,MODES,event('undo'),H,()=>false),true);assert.equal(lane.liveTL.tracks[0].clips[0].label,'cam.mp4');
assert.equal(covAHistory(lane,MODES,event('redo'),H,()=>false),true);assert.equal(lane.liveTL.tracks[0].clips[0].label,'renamed');
