import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {dirname, resolve} from 'node:path';
import vm from 'node:vm';
const directory = fileURLToPath(new URL('../ui/', import.meta.url));
const context = vm.createContext({performance:{now:()=>1000}}), modules = new Map();
function get(filename) {
  if (!modules.has(filename)) {
    const access=filename===resolve(directory,'router.js')?'\nexport { COVB_H as testHelpers };':filename===resolve(directory,'cov_b.js')?'\nexport { keyList as testKeyList, numOrNull as testNumber, clone as testClone };':'';
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
const {covbLearn,covbOps,covbHistory,testKeyList,testNumber,testClone}=get(resolve(directory,'cov_b.js')).namespace;
const H=root.namespace.testHelpers;
const plain=x=>JSON.parse(JSON.stringify(x));
const realm=x=>{context.seed=JSON.stringify(x);return vm.runInContext('JSON.parse(seed)',context);};
const fresh=(mode='graph')=>realm({mode,explicitMode:true,views:{},preview:{},session:{width:640,height:360}});
const event=(tool,args={},result={},changed,phase='complete')=>realm({tool_name:tool,call_id:'fixture',phase,payload:{args,result,changed}});
const blocks=s=>Object.values(s.views[s.mode]).find(v=>v.blocks)?.blocks.filter(b=>b.k).map(b=>[b.k,b.v,...(b.from!=null?[b.from]:[])])||[];
for(const [value,expected] of [[null,null],[{},null],[true,null],['bad',null],['2.5',2.5],[0,0]])assert.equal(testNumber(value),expected);
assert.deepEqual(plain(testKeyList([null,{},'ignored','…+3',{t:0,v:1,ease:'linear'},{t:2,v:Infinity}])),{keys:[[0,1,'linear']],more:3});
assert.deepEqual(plain(testKeyList(null)),{keys:[],more:0});
const original=realm({a:[{b:1}],url:'data:shared'}),copied=testClone(original);copied.a[0].b=2;assert.equal(original.a[0].b,1);assert.equal(copied.url,original.url);
const learned=fresh();
covbLearn(learned,event('add_text_animator',{preset:'rise'}, {},undefined,'start'));
const complete=event('add_text_animator',undefined,{animator_id:'an1'});delete complete.payload.args;covbLearn(learned,complete);
assert.equal(learned._covbAnim.an1,'rise');assert.deepEqual(plain(learned._covbAnimArgs),{});
let e=event('key',{}, {},{clip:{'props.x.keys':[[],[{t:0,v:1}]],'text_fx.animators[an1].weight.keys':[[],[{t:1,v:2}]],'mask.layers[m1].name':['','Face'],ignored:true}});e.target={clip_id:'c1'};covbLearn(learned,e);
assert.deepEqual(plain(learned._covbCurves['c1|x']),{keys:[[0,1,null]],more:0});assert.equal(learned._covbMasks.m1,'Face');
e.payload.changed.clip['props.x.keys']=[[],[]];covbLearn(learned,e);assert.equal(learned._covbCurves['c1|x'],undefined);
console.log('PASS key learning preserves valid keys and names and removes emptied curves');
for(const [tool,args,expected] of [
 ['move_key',{property:'x'},[['Property','position x'],['Keys','0']]],
 ['remove_key',{property:'x'},[['Property','position x'],['Keys','0','0']]],
 ['smooth_curve',{property:'x'},[['Property','position x'],['Sigma','2 keys'],['Ends','free'],['Keys','0']]],
 ['simplify_curve',{property:'x'},[['Property','position x'],['Keys','0','0'],['Tolerance','0.25']]],
 ['bake_property',{property:'x'},[['Property','position x'],['Baked','0 keys ·  s'],['Keys','0','0']]],
 ['apply_motion_preset',{preset:'rise'},[['Preset','rise']]],
 ['copy_curve',{from_property:'x',to_property:'y'},[['From','position x'],['To','position y']]],
 ['set_animator_keyframe',{prop:'weight'},[['Property','weight'],['Keys','0','0']]],
 ['motion_extract',{},[['Frames','0']]],
 ['motion_diff',{},[]],
 ['remove_motion_path',{},[['Motion path','none','on']]],
 ['solve_constraints',{},[['Items','0'],['Box','640×360']]],
 ['compute_text_bounds',{},[]],
 ['raycast_mesh_raw',{},[['Result','miss']]],
 ['compile_scene3d_descriptor',{},[]],
 ['add_managed_linear_effect',{kind:'bloom'},[['Effect','bloom']]],
 ['preview_subject_match',{},[['Preview','before / after']]],
]) {
 const s=fresh();assert.equal(covbOps(s,MODES,event(tool,args),H),true,tool);assert.deepEqual(plain(blocks(s)),expected,tool);
}
console.log('PASS sparse graph and measurement results have explicit default inspector rows');
const curves=fresh();
const before=[{t:0,v:0},{t:1,v:10}],after=[{t:0,v:0},{t:2,v:20}];
covbOps(curves,MODES,event('move_key',{property:'x',time:1,new_time:2},{},{clip:{'props.x.keys':[before,after]}}),H);
assert.deepEqual(plain(blocks(curves)),[['Property','position x'],['Key time','2 s','1 s'],['Value','20','10'],['Keys','2']]);
assert.equal(curves.views.graph.graph.playhead,2);assert.ok(curves.views.graph.graph.curves.some(c=>c.cls==='cvb-mark cvb-gone'));
covbOps(curves,MODES,event('smooth_curve',{property:'x',pin_ends:true},{},{clip:{'props.x.keys':[before,after]}}),H);
assert.ok(blocks(curves).some(r=>r[0]==='Ends'&&r[1]==='pinned'));
const copy=fresh();copy._covbCurves=realm({'src|x':{keys:[[0,0],[1,10]],more:0}});
covbOps(copy,MODES,event('copy_curve',{from_clip:'src',from_property:'x',to_clip:'dst',to_property:'y',reverse:true,invert:true,time_scale:2,time_offset:1,value_scale:2,value_offset:-1},{keys:2}),H);
assert.deepEqual(plain(copy._covbCurves['dst|y']),{keys:[[1,-1],[3,19]],more:0});
assert.ok(blocks(copy).some(r=>r[0]==='Flip'&&r[1]==='reversed, inverted'));
const history=fresh();history.liveTL=realm({});history.views.graph=realm({custom:{value:'before'}});
const step=event('move_key');step.payload.mutated=true;covbHistory(history,MODES,step,H);history.views.graph.custom.value='after';history.views.graph.extra=realm({temporary:true});
covbHistory(history,MODES,event('undo',{}, {undone:1}),H);assert.equal(history.views.graph.custom.value,'before');assert.equal(history.views.graph.extra,undefined);
covbHistory(history,MODES,event('redo',{}, {redone:1}),H);assert.equal(history.views.graph.custom.value,'after');assert.equal(history.views.graph.extra.temporary,true);
covbHistory(history,MODES,event('restore'),H);assert.deepEqual(plain(history._covbHist),{undo:[],redo:[]});
const external=event('unrelated');external.payload.mutated=true;for(let i=0;i<70;i++)covbHistory(history,MODES,external,H);assert.equal(history._covbHist.undo.length,64);assert.ok(history._covbHist.undo.every(x=>x===null));
console.log('PASS key movement, reversed curve copying and bounded undo/redo preserve exact state');
for(const phase of ['start','complete','queued','error']) {
 const s=fresh();const e={tool_name:'add_text_animator',phase,call_id:'empty'};covbLearn(s,e);
 if(phase==='start')assert.deepEqual(plain(s._covbAnimArgs),{empty:{}});
 else assert.equal(s._covbAnim,undefined);
 const result=covbOps(s,MODES,{tool_name:'unknown',phase,call_id:'empty'},H);assert.equal(result,false);
}
assert.equal(covbOps(fresh('absent'),MODES,event('unknown',{},null),H),false);
const same=fresh();covbOps(same,MODES,event('move_key',{property:'custom_value',time:1},{},{clip:{'props.custom_value.keys':[[{t:1,v:2}],[{t:1,v:2}]]}}),H);
assert.deepEqual(plain(blocks(same)),[['Property','custom value'],['Key time','1 s'],['Value','2'],['Keys','1']]);
const changed=fresh();changed.views.graph={};const inspectorKey=Object.entries(MODES.graph.panels).find(([,p])=>p.kind==='kv')[0];changed.views.graph[inspectorKey]={blocks:[{k:'persistent',v:'stay'},{k:'prior',_covb:true},{k:'transient',_live:true}]};
covbOps(changed,MODES,event('apply_motion_preset',{prompt:'enter',intensity:0.5},{applied:[{}]}, {clip:{ignored:true,'props.y.keys':[[{t:0,v:2}],[{t:1,v:3}]],'props.x.keys':[[],[{t:1,v:1}]]}}),H);
assert.ok(blocks(changed).some(r=>r[0]==='Intensity'&&r[1]==='0.5'));
assert.ok(blocks(changed).some(r=>r[0]==='Channels'&&r[1]==='x, y'));
const mismatch=fresh();mismatch._covbCurves=realm({'src|x':{keys:[[0,0],[1,1]],more:0}});
covbOps(mismatch,MODES,event('copy_curve',{from_clip:'src',from_property:'x',to_clip:'dst',to_property:'y'},{keys:3}),H);
assert.equal(mismatch._covbCurves['dst|y'],undefined);
for(const [args,needle] of [[{time_offset:-1},['Time offset','-1 s']],[{value_offset:1},['Value','+1']],[{value_scale:2},['Value','×2']],[{reverse:true},['Flip','reversed']],[{invert:true},['Flip','inverted']]]) {
 const s=fresh();covbOps(s,MODES,event('copy_curve',{from_property:'x',to_property:'y',...args}),H);assert.ok(blocks(s).some(r=>r[0]===needle[0]&&r[1]===needle[1]));
}
const anim=fresh();covbOps(anim,MODES,event('set_animator_keyframe',{prop:'weight',time:1,value:3},{},{clip:{'text_fx.animators[a].weight.keys':[[{t:1,v:1}],[{t:1,v:3}]]}}),H);
assert.ok(blocks(anim).some(r=>r[0]==='Value'&&r[1]==='3'&&r[2]==='1'));
const extracted=fresh();covbOps(extracted,MODES,event('motion_extract',{target_clip_id:'c1',invert:true},{curves:[[1,2,1,0],'unknown']}),H);
assert.ok(blocks(extracted).some(r=>r[0]==='Applied to'&&r[1]==='clip (inverted)'));
const bounded=fresh();covbOps(bounded,MODES,event('bake_property',{property:'x',camera:true},{max_error:0.1},{clip:{'props.x.keys':[[{t:0,v:1}],[{t:0,v:1}]]}}),H);
assert.ok(blocks(bounded).some(r=>r[0]==='Max error'&&r[1]==='0.1'));
const smoothing=fresh();covbOps(smoothing,MODES,event('smooth_curve',{property:'x'}, {},{clip:{'props.x.keys':[[{t:0,v:1}],[{t:0,v:2},{t:1,v:3}]]}}),H);
assert.ok(blocks(smoothing).some(r=>r[0]==='Max change'&&r[1]==='1 @ 0 s'));
const large=fresh();const dense=Array.from({length:65},(_,i)=>({t:i,v:i}));covbOps(large,MODES,event('smooth_curve',{property:'x'}, {},{clip:{'props.x.keys':[dense,dense]}}),H);
assert.ok(large.views.graph.graph.curves.every(c=>c.keys.length===0));assert.equal(large.views.graph.graph.curves[0].pts.length,65);
for(const tool of ['move_key','remove_motion_path','raycast_mesh_raw','compile_scene3d_descriptor']) {
 const s=fresh('absent');assert.equal(covbOps(s,MODES,event(tool),H),tool==='remove_motion_path',tool);
}
const layout=fresh();covbOps(layout,MODES,event('solve_constraints',{constraints:[{items:['box']}]},{positions:[{id:'box',x:64,y:36,w:128,h:72}],violated:[{constraint:0}],satisfied:[]}),H);
assert.equal(layout.views.graph.viewer.overlay[0].em,true);assert.deepEqual(plain(layout.views.graph.viewer.overlay[0].rect),[0.1,0.1,0.2,0.2]);
assert.ok(blocks(layout).some(r=>r[0]==='Violated'&&r[1]==='1'));
const text=fresh();covbOps(text,MODES,event('compute_text_bounds',{}, {font:{file:'/fonts/Name.otf',substituted:false}}),H);assert.deepEqual(plain(blocks(text)),[['Font','Name']]);
const ray=fresh('modelling');for(let i=0;i<10;i++)covbOps(ray,MODES,event('raycast_mesh_raw'),H);
const stack=Object.values(ray.views.modelling).find(v=>v.nodes);assert.equal(stack.nodes.length,7);assert.equal(stack.nodes.at(-1).name,'raycast · miss');assert.equal(stack.nodes[0].em,false);
const scene=fresh('scene');
// The scene compile mode is taken from the actual mode registry below.
const sceneMode=Object.keys(MODES).find(k=>Object.values(MODES[k].panels).some(p=>p.kind==='tree'));
scene.mode=sceneMode;
for(let i=0;i<4;i++)covbOps(scene,MODES,event('compile_scene3d_descriptor',{scene:{sources:[{kind:'primitive',primitive:{kind:'plane'}}],nodes:[{source:0},{source:99}]}},{draws:[{},{}],stats:{sources:2}}),H);
const tree=Object.values(scene.views[sceneMode]).find(v=>v.items);assert.ok(tree.items.some(i=>i.name==='plane 1'&&i.icon==='plane'));assert.ok(tree.items.some(i=>i.name==='draw 2'));
const managed=fresh();covbOps(managed,MODES,event('add_managed_linear_effect',{}, {},{clip:{effects:[[{kind:'blur'}],[]]}}),H);assert.ok(blocks(managed).some(r=>r[0]==='Effects'&&r[2]==='blur'));
const match=fresh();covbOps(match,MODES,event('preview_subject_match',{source_regions:{skin:'unknown'},ref_regions:{lighting:'unknown'}},{source_stats:[{lab_mean:[1]}],result_stats:[{lab_mean:[2]}]}),H);
assert.deepEqual(plain(blocks(match)),[['Skin','mask → —'],['Skin L*','2','1'],['Lighting','— → mask'],['Preview','before / after']]);
console.log('PASS curve boundaries, malformed measurements and scene defaults preserve precise visible output');
for(const times of [[-Number.MAX_VALUE,Number.MAX_VALUE],[Number.MAX_VALUE,Number.MAX_VALUE],[-Number.MAX_VALUE,0],[0,Number.MAX_VALUE]]) {
 const s=fresh();const keys=times.map((t,i)=>({t,v:i}));
 covbOps(s,MODES,event('smooth_curve',{property:'x'}, {},{clip:{'props.x.keys':[[],keys]}}),H);
 assert.equal(s.views.graph.graph.ruler,undefined,'unrepresentable time domain must not create a NaN ruler');
 assert.deepEqual(plain(s.views.graph.graph.curves),[]);
 assert.deepEqual(plain(s.views.graph.graph.lanes),[]);
}
const diff=fresh();
covbOps(diff,MODES,event('motion_diff',{}, {frames:3,channels:{scale:{rms_diff:0.1},dx_px:{rms_diff:1}}}),H);
assert.equal(diff.views.graph.graph.playhead,null);
assert.deepEqual(plain(blocks(diff)),[['Frames','3'],['Scale RMS','10% · r 0'],['X RMS','1 px · r 0']]);
covbOps(diff,MODES,event('motion_diff',{}, {worst_translation_frame:2}),H);
assert.deepEqual(plain(blocks(diff)),[['Worst frame','2']]);
const noCanvas=fresh();delete noCanvas.session;
covbOps(noCanvas,MODES,event('solve_constraints',{}, {violated:[0]}),H);
assert.deepEqual(plain(blocks(noCanvas)),[['Items','0'],['Violated','1']]);
for(const constraints of [[{items:['box']}],[{}]]) {
 const s=fresh();covbOps(s,MODES,event('solve_constraints',{constraints},{violated:[0],positions:[{id:'box',x:0,y:0,w:1,h:1}]}),H);
 assert.equal(s.views.graph.viewer.overlay[0].em,!!constraints[0].items);
}
const fullTree=fresh(sceneMode);fullTree.views[sceneMode]={};
const treeKey=Object.entries(MODES[sceneMode].panels).find(([,p])=>p.kind==='tree')[0];
fullTree.views[sceneMode][treeKey]={items:Array.from({length:16},(_,i)=>({name:`old ${i}`,sel:true}))};
covbOps(fullTree,MODES,event('compile_scene3d_descriptor',{scene:{sources:[],nodes:[{}]}},{draws:[{}]}),H);
assert.equal(fullTree.views[sceneMode][treeKey].items.length,14);
assert.equal(fullTree.views[sceneMode][treeKey].items[0].name,'old 3');
assert.equal(fullTree.views[sceneMode][treeKey].items[0].sel,false);
const unnamed=fresh();covbOps(unnamed,MODES,event('add_managed_linear_effect',{}, {},{clip:{effects:[[null,{}],[]]}}),H);
assert.ok(blocks(unnamed).some(r=>r[0]==='Effects'&&r[2]==='effect, effect'));
const missingHistory=fresh();missingHistory.liveTL=realm({});
const mutation=event('move_key');mutation.payload.mutated=true;
covbHistory(missingHistory,MODES,mutation,H);delete missingHistory.views.graph;
covbHistory(missingHistory,MODES,event('undo',{}, {undone:1}),H);
assert.deepEqual(plain(missingHistory.views.graph),{});
for(const ev of [{tool_name:'undo',phase:'complete'},event('redo',{},null)]) {
 covbHistory(missingHistory,MODES,ev,H);
 assert.equal(missingHistory._covbHist.redo.length,1);
}
console.log('PASS missing comparison metrics, bounded scene trees and sparse history restore remain deterministic');
