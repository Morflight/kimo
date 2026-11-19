import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {dirname,resolve} from 'node:path';
import vm from 'node:vm';
const directory=fileURLToPath(new URL('../ui/',import.meta.url)), modules=new Map();
const context=vm.createContext({performance:{now:()=>1000}});
function get(filename){if(!modules.has(filename))modules.set(filename,new vm.SourceTextModule(readFileSync(filename,'utf8')+(filename===resolve(directory,'router.js')?'\nexport {gradeOps,graphicsOps,maskOps,sceneOps,editOps2,timelineOps,timelineOps1,exportOps,inspectOps,inspectorDiff,callDiff,audioOps,ensureTrack,insertClip,midPath,shortPath,fmtBytes,curveAt,easeOf,bezOf,cubicY,keyChange,keyGraph,xfOps,summary,railArgs,assetNames,splitName,clipLabel,peek,callSection,COVB_H};':''),{identifier:filename,context}));return modules.get(filename);}
const root=get(resolve(directory,'router.js'));await root.link((name,parent)=>get(resolve(dirname(parent.identifier),name)));await root.evaluate();
const {gradeOps,graphicsOps,maskOps,sceneOps,editOps2,timelineOps,timelineOps1,exportOps,inspectOps}=root.namespace;
const modes=get(resolve(directory,'modes.js'));if(modes.status==='unlinked')await modes.link(()=>{});await modes.evaluate();const {MODES}=modes.namespace;
const plain=x=>JSON.parse(JSON.stringify(x));let serial=0;
const fresh=mode=>({mode,explicitMode:true,views:{},session:{width:1000,height:500}});
const event=(tool,args={},phase='complete',result={})=>({tool_name:tool,phase,call_id:'op'+serial++,payload:{args,result}});
const s=fresh('graphics');
function gfx(tool,args={},result={}) {const e=event(tool,args,'complete',result);assert.equal(graphicsOps(s,MODES,e),true,tool);assert.equal(e._kvDone,true);return s.views.graphics.viewer.overlay?.at(-1);}
assert.equal(graphicsOps(s,MODES,event('add_hud_frame',{},'progress')),false);
const start=event('add_hud_frame',{label:'Cached'},'start');assert.equal(graphicsOps(s,MODES,start),false);assert.equal(graphicsOps(s,MODES,{...start,phase:'complete',payload:{}}),true);assert.equal(s.views.graphics.viewer.overlay.at(-1).label,'Cached');assert.equal(s._gfxArgs[start.call_id],undefined);
let o=gfx('add_detection_box',{x:.1,y:.2,w:.3,h:.4,label:'Person',confidence:.875,stroke_width:2,color:[1,0,128],opacity:.5});assert.deepEqual(plain(o.rect),[.1,.2,.3,.4]);assert.equal(o.label,'Person 0.88');
o=gfx('add_box_expand',{from:[0,0,.1,.1]});assert.deepEqual(plain(o.to),[0,0,1,1]);
o=gfx('add_box_expand',{from:[0,0,.1,.1],to:[.2,.2,.4,.4],label:'Expand'});assert.deepEqual(plain(o.to),[.2,.2,.4,.4]);
o=gfx('add_hud_frame',{});assert.equal(o.label,'BOXES');
o=gfx('add_counter',{from:2,to:4,x:100,y:-50,prefix:'$',suffix:'!',decimals:1});assert.equal(o.text,'$2.0! → $4.0!');assert.deepEqual(plain(o.at),[.6,.4]);
o=gfx('add_prompt_box',{text:'a'.repeat(60),y:9999});assert.equal(o.text.length,48);assert.equal(o.at[1],.92);
o=gfx('add_prompt_box',{text:'Prompt',y:-9999});assert.equal(o.at[1],.08);
o=gfx('add_shape',{shape:'ellipse',width:100,height:50});assert.deepEqual(plain(o.rect),[.45,.45,.1,.1]);assert.equal(o.shape,'ellipse');
o=gfx('add_shape',{});assert.equal(o.shape,'rect');assert.deepEqual(plain(o.rect),[.4,.3,.2,.4]);
for(const tool of ['add_path_shape','set_path','add_path_key']) {o=gfx(tool,{points:[[100,50],{x:200,y:100},null,{x:'bad',y:0}],closed:true});assert.deepEqual(plain(o.path),[[.1,.1],[.2,.2]]);assert.equal(o.closed,true);}
o=gfx('add_path_shape',{points:[[0,0],[1,1]]});assert.deepEqual(plain(o.path),[[0,0],[1,1]]);
o=gfx('add_glitch_confetti',{squares:100,seed:9});assert.equal(o.squares,60);assert.equal(o.seed,9);
o=gfx('add_glitch_confetti',{});assert.equal(o.squares,24);assert.equal(o.seed,1);
for(const tool of ['set_layer_style','add_shape_modifier','add_modifier']) {gfx(tool,{clip_id:'Hero',kind:'outline',params:{width:2,opacity:.5,bad:'invalid'}});gfx(tool,{clip_id:'Hero',kind:'outline',params:{width:3}});gfx(tool,{kind:'glow'});}
for(const tool of ['remove_layer_style','remove_shape_modifier','remove_modifier','clear_path_keys'])gfx(tool,{effect_id:'FX'});
for(let i=0;i<12;i++)gfx('add_hud_frame',{label:'Layer '+i,stroke_color:i%2?'red':[255,0,0],opacity:50},{id:'layer-name-'+i});
const inspector=Object.values(s.views.graphics).find(v=>v.blocks);const list=inspector.blocks.find(b=>b._layers);assert.equal(list.list.length,8);assert.equal(list.list.at(-1),'layer-name-11');
for(const [tool,args] of [['add_detection_box',{x:0}],['add_box_expand',{from:[0,1]}],['add_counter',{}],['add_prompt_box',{}],['add_path_shape',{points:[[0,0],null]}],['unrecognized',{}]])assert.equal(graphicsOps(fresh('graphics'),MODES,event(tool,args)),false);
const noSize=fresh('graphics');noSize.session={};assert.equal(graphicsOps(noSize,MODES,event('add_shape')),false);assert.equal(graphicsOps(noSize,MODES,event('add_path_shape',{points:[[100,50],[200,100]]})),false);
const insert=event('add_hud_frame',{track_id:'track1',start:2,duration:3,label:'HUD'},'complete',{clip_id:'clip42'});graphicsOps(s,MODES,insert);assert.equal(insert.payload.s,2);assert.equal(insert.payload.e,5);assert.equal(insert.payload.label,'HUD');
console.log('PASS graphics cached args, normalized geometry, style updates, bounded layers and invalid input');
assert.equal(list.sel,list.list.length-1,'newest layer remains selected after trimming history');
const grade=fresh('grading');
function grading(tool,args={},result={}) {const e=event(tool,args,'complete',result);assert.equal(gradeOps(grade,MODES,e),true,tool);return Object.values(grade.views.grading).find(v=>v.blocks);}
assert.equal(gradeOps(grade,MODES,event('grade',{},'start')),false);
let kv=grading('grade',{lift:'+0.1',gamma:'-0.1',gain:1.2,clip_id:'Hero'});let wheels=Object.values(grade.views.grading).find(v=>v.wheels);assert.deepEqual(plain(wheels._g.lift),{r:.1,g:.1,b:.1});assert.equal(wheels._g.gamma.r,.9);assert.equal(wheels._g.gain.r,1.2);
grading('grade',{lift_r:.2,gamma_b:1,gain_g:1});assert.equal(wheels._g.lift.r,.2);assert.ok(wheels.wheels.every(w=>w.dot.every(Number.isFinite)));
for(const kind of ['temperature','tint','contrast','saturation','exposure']) {kv=grading('add_effect',{kind,params:{amount:.5}});assert.equal(kv.blocks.find(b=>b.k===kind[0].toUpperCase()+kind.slice(1)).v,'0.5');}
kv=grading('add_effect',{kind:'lens_flare',params:{intensity:.8},strings:{source_clip:'Hero',source_light:'Key',occluder:'Wall'}});assert.equal(kv.blocks.find(b=>b.k==='lens_flare source_clip').v,'Hero');
kv=grading('add_effect',{kind:'vignette',strings:{source_clip:'Hero',source_mask:'Mask'}});assert.equal(kv.blocks.find(b=>b.k==='vignette source_mask').v,'Mask');
for(const args of [{node_id:'n1',kind:'grade'},{node:{id:'n2',kind:'exposure'}},{node:{node_id:'n2',effect:'contrast'}},{kind:'saturation'}])grading('set_grade_node',args);
kv=grading('set_grade_output',{node_id:'n2'});assert.equal(kv._nodes.find(n=>n.id==='n2').out,true);
grading('set_grade_output',{});assert.equal(kv._nodes.find(n=>n.id==='input').out,true);
grading('generate_lut_node',{});assert.ok(kv._nodes.some(n=>n.id==='lut'));
grading('generate_lut_node',{node_id:'lut2'});assert.ok(kv._nodes.some(n=>n.id==='lut2'));
grading('set_node_param',{node_id:'n1',parameter:'exposure',property:{base:1.25}});assert.equal(kv.blocks.find(b=>b.k==='exposure').v,'1.25');
grading('set_node_param',{node_id:'n1',parameter:'exposure',property:{value:2}});assert.equal(kv.blocks.find(b=>b.k==='exposure').v,'2');
grading('set_node_param',{parameter:'missing'});assert.equal(kv.blocks.find(b=>b.k==='missing').v,'');
for(const mask of [{kind:'ellipse'},{type:'rect'},{},null]) {grading('set_node_mask',{node_id:'n1',mask});assert.equal(kv.blocks.find(b=>b.k==='Mask').v,mask?(mask.kind??mask.type??'on'):'off');}
for(const [tool,args,expected] of [['connect_nodes',{from:'n1',to:'n2'},'n1 → n2'],['disconnect_nodes',{source:'n1',target:'n2'},'n1 × n2'],['connect_nodes',{src:'n2',dst:'n3'},'n2 → n3']]){grading(tool,args);assert.equal(kv.blocks.find(b=>b.k==='Edge').v,expected);}
grading('connect_nodes',{});grading('create_shared_grade_graph',{id:'shared'});assert.ok(kv._nodes.some(n=>n.id==='shared:shared'));
grading('remove_shared_grade_graph',{id:'shared'});grading('remove_shared_grade_graph',{id:'missing'});assert.ok(kv._nodes.every(n=>n.id!=='shared:shared'));
grading('bind_shared_grade_graphs',{graphs:['a','b']});assert.equal(kv.blocks.find(b=>b.k==='Shared').v,'a, b');grading('bind_shared_grade_graphs',{graphs:[]});assert.equal(kv.blocks.find(b=>b.k==='Shared').v,'none');
grading('apply_asc_cdl',{path:'C:\\looks\\shot.cdl'});assert.equal(kv.blocks.find(b=>b.k==='CDL').v,'shot.cdl');
grading('set_color_curve',{channel:'r',points:[[0,0],[1,1],null,['bad',1]]});assert.deepEqual(plain(kv._curve.pts),[[0,0],[1,1]]);
grading('set_color_curve',{channel:'hue_sat',points:[[0,1]]});assert.equal(kv.blocks.find(b=>b.k==='Hue vs Sat').v,'1 pts');
grading('set_color_wheels',{lift:[0,1,'bad',3],gamma:[1,1,1,1]});assert.match(kv.blocks.find(b=>b.k==='Wheels').v,/Lif 0\/1\/\?\/3/);
grading('set_color_wheels',{space:'ACES'});assert.equal(kv.blocks.find(b=>b.k==='Wheels').v,'ACES');grading('set_color_wheels');assert.equal(kv.blocks.find(b=>b.k==='Wheels').v,'neutral');
grading('set_grade_scopes',{scopes:{groups:[1,2],graphs:{a:{}}}});assert.equal(kv.blocks.find(b=>b.k==='Scopes').v,'2 groups · 1 graphs');grading('set_grade_scopes',{scopes:{}});assert.equal(kv.blocks.find(b=>b.k==='Scopes').v,'set');
grading('render_scope',{scope:'vectorscope',data:true,targets_75:true});assert.equal(kv.blocks.find(b=>b.k==='Scope').v,'vectorscope · data · 75% targets');
grading('add_acescct_primaries',{params:{lift:.2,gamma:1,gain:1,offset:.1}});assert.equal(kv.blocks.find(b=>b.k==='Offset').v,'0.1');
grading('remove_grade_node',{node_id:'n2'});grading('remove_grade_node',{node_id:'absent'});assert.ok(kv._nodes.every(n=>n.id!=='n2'));
grading('pick_white_balance',{x:.1,y:.2,w:.3,h:.4});assert.deepEqual(plain(grade.views.grading.viewer.overlay.at(-1).rect),[.1,.2,.3,.4]);
grading('pick_key_color');assert.deepEqual(plain(grade.views.grading.viewer.overlay.at(-1).rect),[0,0,.15,.15]);
grading('match_grade',{source_region:[0,0,.2,.2],ref_region:[.5,.5,.1,.1],ref_clip_id:'Reference'});assert.equal(grade.views.grading.viewer.overlay.at(-1).label,'ref · Reference');
grading('match_color',{source_region:[1],ref_region:'invalid'});assert.equal(grade.views.grading.viewer.wipe,1);
grading('add_effect',{kind:'keyer',params:{matte_model:2,despill_mode:3,view:4},strings:{aux_path:'/tmp/plate.png',garbage_mask:'m1',core_mask:'m2'}});assert.match(kv.blocks.find(b=>b.k==='Keyer').v,/clean plate plate.png.*garbage\/core.*despill min.*raw screen view/);
console.log('PASS grade masters/channels, graph lifecycle, CDL, curves, wheels, scopes and sampled regions');
const masks=fresh('roto');masks.session.fps=25;
function mask(tool,args={},result={}) {const e=event(tool,args,'complete',result);assert.equal(maskOps(masks,MODES,e),true,tool);assert.equal(e._kvDone,true);return masks.views.roto;}
let mv=mask('add_mask',{name:'subject',points:[[100,50],{x:200,y:100,fi:2},null],feather:2,expand:-3,params:{precision:1,falloff:1,hue_soft:4,sat_soft:.1}});
assert.deepEqual(plain(mv.viewer.overlay[0].path),[[.1,.1],[.2,.2]]);
let mk=Object.values(mv).find(v=>v.blocks);assert.equal(mk.blocks.find(b=>b.k==='Feather').v,'2 px');assert.equal(mk.blocks.find(b=>b.k==='Choke').v,'3');assert.equal(mk.blocks.find(b=>b.k==='Inner/outer feather').v,'1 pts');
mask('update_mask',{mask_id:'subject',kind:'ellipse',params:{cx:.5,cy:.5,w:.2,h:.4,precision:0,falloff:0},feather:4});assert.deepEqual(plain(mv.viewer.overlay[0].rect),[.4,.3,.2,.4]);assert.equal(mk.blocks.find(b=>b.k==='Feather').from,'2 px');assert.equal(mk.blocks.find(b=>b.k==='Precision').v,'standard');
mask('set_mask',{cx:.5,cy:.5,w:1,h:1});assert.equal(mv.viewer.overlay.at(-1).shape,'rect');
const cachedMask=event('set_mask_path_key',{mask_id:'subject',time:1,points:[[0,0],[1,1]]},'start');assert.equal(maskOps(masks,MODES,cachedMask),false);assert.equal(maskOps(masks,MODES,{...cachedMask,phase:'complete',payload:{}}),true);assert.equal(masks._maskArgs[cachedMask.call_id],undefined);
mask('set_mask_path_key',{mask_id:'subject',time:3,points:[[0,0],[1,1]]});mask('set_mask_path_key',{mask_id:'subject',time:1});
let kk=Object.values(mv).find(v=>v.lanes);assert.deepEqual(plain(kk.lanes[0].keys),[25,75]);assert.equal(kk.playhead,25);assert.equal(kk.ruler.step,10);
mask('track_mask',{mask_id:'subject',motion:'affine'},{frames:[25,75]});assert.deepEqual(plain(kk.lanes[0].bar[0].slice(0,2)),[25,75]);assert.match(mk.sub,/affine/);
mask('roto_propagate',{mask_id:'subject'});assert.deepEqual(plain(kk.lanes[0].bar[0].slice(0,2)),[25,75]);
mask('set_mask_prop_key',{mask_id:'subject',property:'feather',value:5,time:10});assert.equal(mk.blocks.find(b=>b.k==='Feather').v,'5 px');assert.equal(kk.ruler.step,50);
mask('set_mask_keyframe',{mask_id:'subject',property:'expand',value:2,time:.1});assert.equal(mk.blocks.find(b=>b.k==='Choke').v,'-2');
mask('set_mask_keyframe',{mask_id:'other',time:0});mask('set_mask_keyframe',{mask_id:'third',time:2});assert.equal(kk.lanes.length,4);
mask('reorder_mask',{mask_id:'third',index:0});assert.ok(kk.lanes[0].label.startsWith('third '));
mask('refine_matte',{auto_bg:true},{matte_mae:.125});assert.equal(mk.blocks.find(b=>b.k==='Bg model').v,'auto');assert.equal(mk.blocks.find(b=>b.k==='Matte MAE').v,'0.13');
mask('refine_matte',{auto_bg:false},{mae:.2});assert.equal(mk.blocks.find(b=>b.k==='Bg model').v,'manual');
masks.liveTL={tracks:[{id:'t',clips:[{id:'clip42',s:2,e:5}]}]};mask('set_track_matte',{clip_id:'clip42',mode:'luma',luma_space:'linear'});let matte=kk.lanes.find(l=>l.label.startsWith('track matte'));assert.deepEqual(plain(matte.area),[[50,1],[125,1]]);
mask('remove_track_matte');assert.ok(kk.lanes.every(l=>!l.label.startsWith('track matte')));assert.equal(maskOps(masks,MODES,event('remove_track_matte')),false);
mask('remove_mask_layer',{mask_id:'third'});assert.ok(kk.lanes.every(l=>!l.label.startsWith('third ')));
mask('remove_mask');assert.equal(kk.lanes.length,0);assert.equal(mv.viewer.overlay.length,0);assert.equal(mk.sub,'');
const emptyMask=fresh('roto');assert.equal(maskOps(emptyMask,MODES,event('set_track_matte')),true);const emptyKeys=Object.values(emptyMask.views.roto).find(v=>v.lanes);assert.equal(emptyKeys.lanes[0].label,'track matte');assert.equal(emptyKeys.ruler,undefined);
console.log('PASS mask geometry, cached path keys, frame conversion, matte spans, lane limits and removal');

const edit=fresh('edit'), timeline={tracks:[{id:'v1',label:'Video',clips:[{id:'a',label:'A',s:1,e:3},{id:'b',label:'B',s:5,e:8}]},{id:'v2',clips:[]}]};
const editCall=(tool,args={},result={},target={})=>editOps2(edit,MODES,timeline,{...event(tool,args,'complete',result),target},args);
const clip=id=>timeline.tracks.flatMap(t=>t.clips||[]).find(c=>c.id===id);
assert.equal(editCall('project_set',{width:1920,height:1080,fps:25}),true);assert.equal(edit.views.edit.viewer.res,'1920 x 1080 25p');
assert.equal(editCall('set_track',{track_id:'v1',name:'  Renamed video track  ',locked:true}),true);assert.equal(timeline.tracks[0].label,'Renamed video tr');assert.equal(timeline.tracks[0].lock,true);
editCall('set_track',{track_id:'v1',name:' ',lock:false});assert.equal(timeline.tracks[0].lock,false);
assert.equal(editCall('set_track',{track_id:'missing'}),false);assert.equal(editCall('delete_track',{track_id:'missing'}),false);
assert.equal(editCall('swap_clips',{clip_a:'b',clip_b:'a'}),true);assert.deepEqual(plain(timeline.tracks[0].clips.map(c=>[c.id,c.s,c.e])),[['b',1,4],['a',6,8]]);
assert.equal(editCall('duplicate_clip',{clip_id:'a',start:10,track_id:'v2'},{clip_id:'copy'}),true);assert.deepEqual([clip('copy').s,clip('copy').e],[10,12]);
assert.equal(editCall('swap_clips',{clip_a:'a',clip_b:'copy'}),false);
assert.equal(editCall('duplicate_clip',{clip_id:'a'}),false);assert.equal(editCall('duplicate_clip',{clip_id:'a',start:14}),true);assert.equal(clip('a_dup').e,16);
assert.equal(editCall('loop_clip',{clip_id:'copy',times:2,gap:1},{clip_ids:['loop1']}),true);assert.deepEqual([clip('loop1').s,clip('copy_loop2').s],[13,16]);
assert.equal(editCall('distribute_clips',{track_id:'v2',gap:2,start:0}),true);assert.deepEqual(plain(timeline.tracks[1].clips.map(c=>[c.s,c.e])),[[0,2],[4,6],[8,10]]);
assert.equal(editCall('distribute_clips',{track_id:'missing'}),false);
assert.equal(editCall('group_clips',{clip_ids:['a','copy','missing'],track_id:'v3'},{id:'group1'}),true);assert.deepEqual([clip('group1').s,clip('group1').e,clip('group1')._kids.length],[0,8,2]);assert.equal(clip('a'),undefined);
assert.equal(editCall('ungroup_clip',{clip_id:'group1'}),true);assert.equal(clip('group1'),undefined);assert.equal(clip('a').s,6);assert.equal(editCall('ungroup_clip',{clip_id:'a'}),false);
assert.equal(editCall('group_clips',{clip_ids:['missing']}),false);
assert.equal(editCall('add_camera_shake',{start:4,track_id:'v3',hits:[[.5],{time:5},6,null,'bad']}),true);assert.deepEqual(plain(clip('shake').keys),[{t:4.5},{t:5},{t:6}]);assert.equal(clip('shake').e,6);
assert.equal(editCall('add_camera_shake',{start:4}),false);
for(const [tool,args,expected] of [
 ['freeze_frame',{},'freeze'],['freeze_frame',{at:2},'freeze 2s'],['set_speed',{speed:2},'2×'],
 ['set_speed_ramp',{keys:[[0,1],{t:1},null],frame_blend:'optical_flow'},'ramp · flow'],
 ['set_speed_ramp',{keys:[[2,1]]},'ramp · flow'],['set_speed_ramp',{keys:[[2,1]],frame_blend:'frame_mix'},'ramp · frame mix'],
 ['set_speed_ramp',{keys:[[2,1]],frame_blend:'none'},'ramp'],
 ['set_time_remap',{keys:[[0,2],[1,3],null],frame_blend:'frame_mix'},'remap · frame mix'],
 ['set_time_remap',{},'remap · frame mix'],['set_time_remap',{frame_blend:'optical_flow'},'remap · flow'],['set_time_remap',{frame_blend:'none'},'remap'],
 ['set_blend',{mode:'screen'},'screen'],['set_blend',{mode:'multiply'},'multiply'],
 ['set_echo',{count:3.4},'echo ×3'],['set_echo',{},'echo'],
 ['set_flow_blur',{},'flow blur'],['set_flow_blur',{amount:.5,mode:'vector',shutter_phase:45,curve_points:[[0,0],[1,1]]},'flow blur 0.5 · vector · phase 45° · curve custom (2 pts)'],
 ['set_flow_blur',{mode:'vector',shutter_curve:'box'},'flow blur vector']
]){assert.equal(editCall(tool,{clip_id:'a',...args}),true);assert.ok(clip('a').fx.includes(expected),`${tool}: ${clip('a').fx}`);}
assert.equal(clip('a').fx.includes('screen'),false);
editCall('set_speed_ramp',{clip_id:'a',keys:[]});editCall('set_time_remap',{clip_id:'a',keys:[]});assert.ok(clip('a').keys.every(k=>!k._ramp&&!k._remap));assert.ok(clip('a').fx.every(f=>!/^ramp|^remap/.test(f)));
assert.equal(editCall('rate_stretch',{clip_id:'a',duration:5}),true);assert.equal(clip('a').e,11);
assert.equal(editCall('remove_audio_effect',{clip_id:'a',name:'echo'}),true);assert.ok(!clip('a').fx.includes('echo'));
for(const tool of ['set_speed','rate_stretch','set_blend','remove_audio_effect','unknown'])assert.equal(editCall(tool,{clip_id:'a'}),false);
assert.equal(editCall('unknown',{clip_id:'missing'}),false);
assert.equal(editCall('delete_track',{track_id:'v3'}),true);assert.ok(timeline.tracks.every(t=>t.id!=='v3'));
console.log('PASS timeline structural edits, duplicate/group restoration, interpolation badges, key removal and track metadata');

const tracking=fresh('tracking');
const sc=(state,tool,args={},result={},phase='complete')=>{const ev=event(tool,args,phase,result);const hit=sceneOps(state,MODES,ev);return {hit,ev,view:state.views[state.mode].viewer};};
let sr=sc(tracking,'track_matte',{seeds:[{points:[[0,0],[1,0],[1,1],null,['bad',0]]}]});assert.deepEqual(plain(sr.view.overlay.at(-1).poly),[[0,0],[1,0],[1,1]]);
const quad=[0,0,1,0,1,1,0,1];
for(const tool of ['fit_track_homography','ecc_refine_track','decompose_track_homography','corner_disagreement','lk_residual_audit','validate_planar_track']){
 sr=sc(tracking,tool,{native_quads:[quad]},{p95_px:.2,max_px:.4,ok:true});assert.equal(sr.hit,true);assert.deepEqual(plain(sr.view.overlay.at(-1).quad),[[0,0],[1,0],[1,1],[0,1]]);assert.match(sr.view.left,/0.2 px p95 · 0.4 px max · ok$/);
 sr=sc(tracking,tool,{}, {roi:quad,ok:false});assert.match(sr.view.left,/failed$/);
}
sr=sc(tracking,'undistort_clip',{k1:.2,k2:-.1,cx:200,cy:.4});assert.deepEqual(plain(sr.view.overlay.at(-1).c),[null,.4]);assert.equal(sr.view.left,'undistort · k1 0.2 · k2 -0.1');
sr=sc(tracking,'stabilize',{smooth:4,lock:true,rotation:false},{zoom:1.25});assert.equal(sr.view.overlay.at(-1).crop,.8);assert.equal(sr.view.left,'stabilize · smooth 4 · locked · no rotation · zoom 1.25×');
sr=sc(tracking,'stabilize',{method:'position_scale_rotation',smoothness:30,smoothing:'l1',framing:'synthesize_edges',rolling_shutter:2},{scale:1.1});assert.equal(sr.view.left,'warp stabilize · similarity · smooth 30% · L1 path · synth edges · RS 2 · zoom 1.1×');
sr=sc(tracking,'stabilize',{}, {engine:'warp',mode:'no_motion',method:'perspective',framing:'crop_only'});assert.equal(sr.view.left,'warp stabilize · perspective · locked · crop only');
sr=sc(tracking,'apply_camera_solve',{solve_id:'solve1',fov:false},{keyframes:4});assert.equal(sr.view.left,'camera solve → keys · solve1 · 4 keys · no fov');
sr=sc(tracking,'set_ground_plane',{solve_id:'solve1',track_ids:[1,2,3]},{scale:2,residual_rms:.2,reapplied:true});assert.equal(sr.view.left,'ground plane · solve1 · 3 points · fit 0.2 rms · scale 2 · keys re-applied');
sr=sc(tracking,'place_at_track',{kind:'plane',track_id:4,solve_id:'s'},{clip_id:'plane1'});assert.equal(sr.view.left,'plane at track · #4 · s · plane1');
sr=sc(tracking,'export_st_map',{direction:'distort'},{width:1000,height:500,clamped_pixels:3});assert.equal(sr.view.left,'ST map · distort · 1000×500 · 3 clamped');
sr=sc(tracking,'pin_layer',{keys:[{quad:[0,0,1000,0,1000,500,0,500]}]});assert.deepEqual(plain(sr.view.overlay.at(-1).quad),[[0,0],[1,0],[1,1],[0,1]]);
sr=sc(tracking,'set_corner_pin',{corners:[[10,20],[0,0],[0,0],[0,0]]});assert.deepEqual(plain(sr.view.overlay.at(-1).quad[0]),[.01,.04]);
const particles=fresh('particles');particles.session.fps=25;
sr=sc(particles,'add_particles3d',{clip_id:'spark',start:2,duration:3,emitter:{kind:'box',rate:100,seed:9,size:20},physics:{gravity:[0,-10,0],bounce:.4,drag:.1,turbulence:2,wind:[0,3,0]},sprite:{size:4,lifetime:2,count:200}});assert.equal(sr.hit,true);assert.equal(sr.ev._ptKv,true);
const part=particles.views.particles, pk=Object.values(part).find(v=>v.blocks), pl=Object.values(part).find(v=>v.rows), lanes=Object.values(part).find(v=>v.lanes);
assert.equal(pk.blocks.find(b=>b.k==='Size').v,'4');assert.equal(pk.blocks.find(b=>b.k==='Gravity').v,'-10');assert.equal(pk.blocks.find(b=>b.k==='Lifetime').v,'2 s');assert.equal(pl.rows[0].v,'100/s');assert.deepEqual(plain(lanes.lanes[0].dots),[50,125]);
sc(particles,'set_particles3d',{clip_id:'spark',start:2,duration:0,emitter:{rate_per_s:200},physics:null,sprite:[]});assert.equal(pl.rows.length,1);assert.equal(pl.rows[0].v,'200/s');assert.deepEqual(plain(lanes.lanes[0].dots),[50]);
for(let i=0;i<6;i++)sc(particles,'add_emitter',{name:'e'+i,start:i,duration:12,config:{count:50,life:2,size0:3}});
assert.equal(pl.rows.length,4);assert.equal(lanes.lanes.length,3);assert.equal(lanes.ruler.step,120);assert.equal(pl.rows.filter(r=>r.em).length,1);
sc(particles,'add_shard_burst',{start:0,duration:1,shards:99,seed:4});assert.equal(pk.sub,'shard_burst');assert.equal(pk.blocks.find(b=>b.k==='Shards').v,'99');
const light=fresh('lighting');sc(light,'set_light',{id:'key',kind:'spot',pos:[.7,.2],color:[1,.5,0],temperature:5600,intensity:2,falloff:1.5});
let lv=light.views.lighting, rig=Object.values(lv).find(v=>v.lights), lights=Object.values(lv).find(v=>v.rows&&v!==rig);assert.equal(rig.rows[0][1],'5600K');assert.equal(rig.rows[0][3],1);assert.equal(lights.rows[0].k,'key');
sc(light,'light_rig',{preset:'portrait',prefix:'studio',subject:[.5,.5]},{lights:['fill',{id:'rim'},null]});assert.ok(lights.rows.some(r=>r.k==='rim'));assert.equal(lv.viewer.overlay.at(-1).kind,'subject');
sc(light,'set_material',{name:'gold',albedo:[1,.7,'bad'],roughness:.2,metal:.8,ior:1.4,clearcoat:.3});const material=Object.values(lv).find(v=>v.blocks);assert.equal(material.sub,'gold');assert.equal(material.blocks.find(b=>b.k==='Albedo').v,'1.00  0.70  0.00');assert.equal(material.blocks.find(b=>b.k==='Metallic').v,'0.80');
sc(light,'preview_material',{material:'gold'},{path:'preview.png',mean:.25});assert.equal(lv.viewer.left,'preview · gold · mean 0.25');
sc(light,'set_relight',{clip_id:'hero'});assert.equal(lv.viewer.tag,'relight hero');sc(light,'remove_material',{name:'gold'});assert.equal(material.sub,'');
sc(light,'remove_light',{id:'key'});assert.ok(lights.rows.every(r=>r.k!=='key'));assert.ok(rig.rows.every(r=>r[0]!=='key'));assert.ok(lv.viewer.overlay.every(o=>o._kind!=='light:key'));
const scene=fresh('scene3d');sc(scene,'add_scene3d',{config:{objects:[{name:'Floor',kind:'plane'},{id:'child',parent:'Floor'},{}],lights:[{name:'fill'},{kind:'spot'}]}});const tree=Object.values(scene.views.scene3d).find(v=>v.items);assert.deepEqual(plain(tree.items.map(i=>i.name)),['Floor','child','object 3','fill','spot 2']);assert.equal(tree.items[1].d,1);assert.equal(tree.items[2].sel,true);
for(let i=0;i<15;i++)sc(scene,'add_scene3d',{config:{objects:[{name:'object'+i}]}});assert.equal(tree.items.length,14);assert.equal(tree.items.at(-1).name,'object14');
sr=sc(scene,'simulate_physics',{bodies:[{x:100,y:-50,radius_px:10,clip_id:'body'},null,{x:'bad',y:0}]});assert.deepEqual(plain(sr.view.overlay.at(-1).body),[.6,.4]);assert.equal(sr.view.overlay.at(-1).r,.01);
console.log('PASS tracking measurements, stabilization, particle sections, material/light lifecycle and scene body geometry');

const structural=fresh('edit');structural.liveTL={tracks:[{id:'v',label:'Video',clips:[{id:'a',label:'Alpha',s:0,e:4},{id:'b',label:'Beta',s:5,e:8},{id:'c',label:'Gamma',s:9,e:11}]}]};
const structuralCall=(tool,args,result={})=>{const ev=event(tool,args,'complete',result);assert.equal(timelineOps(structural,MODES,ev,'edit'),true,tool);assert.equal(ev._kvDone,true);return Object.values(structural.views.edit).find(v=>v.blocks);};
for(const [tool,args,key,value] of [
 ['set_speed',{clip_id:'a',speed:2},'Speed','2×'],['rate_stretch',{clip_id:'a',duration:3},'Duration','3 s'],
 ['loop_clip',{clip_id:'a',times:2,gap:1},'Copies','2'],['duplicate_clip',{clip_id:'a',start:20,track_id:'v'},'Start','20 s'],
 ['set_speed_ramp',{clip_id:'a',keys:[[0,1],[1,2]]},'Ramp keys','2 keys'],['set_blend',{clip_id:'a',mode:'screen'},'Mode','screen'],
 ['set_echo',{clip_id:'a',count:4},'Count','4'],['freeze_frame',{clip_id:'a',at:1},'At','1 s'],['set_flow_blur',{clip_id:'a',amount:.25},'Amount','0.25'],
 ['set_track',{track_id:'v',name:'New video'},'Name','New video'],['project_set',{width:640,height:360,fps:30},'Size','640 x 360'],
 ['add_camera_shake',{track_id:'v',start:0,duration:2,amplitude:5},'Amplitude','5']
])assert.equal(structuralCall(tool,args).blocks.find(b=>b.k===key).v,value);
structural.liveTL.tracks[0].clips=[{id:'b',label:'Beta',s:5,e:8},{id:'c',label:'Gamma',s:9,e:11}];
assert.equal(structuralCall('swap_clips',{clip_a:'b',clip_b:'c'}).blocks.find(b=>b.k==='Clips').v,'Beta, Gamma');
assert.equal(structuralCall('group_clips',{clip_ids:['b','c','absent']},{clip_id:'g'}).blocks.find(b=>b.k==='Grouped').v,'3 clips');
const noDimensions=fresh('edit');noDimensions.session={};assert.equal(editOps2(noDimensions,MODES,{tracks:[]},event('project_set'),{}),false);
const legacy={tracks:[{id:'v',clips:[{id:'c',s:0,e:5},{id:'d',s:7,e:9}]},{id:'CAP',clips:[]}]};
const legacyCall=(tool,fam,payload={},target={})=>timelineOps1(legacy,{tool_name:tool,phase:'complete',payload,target},fam);
assert.equal(legacyCall('import_media','ingest',{s:1,e:3,thumb:'image'},{track_id:'new',clip_id:'import'}),true);assert.equal(legacy.tracks.find(t=>t.id==='new').clips[0].id,'import');
assert.equal(legacyCall('trim_clip','edit',{s:2,e:4},{clip_id:'joined',track_id:'v'}),true);assert.equal(legacy.tracks[0].clips.find(c=>c.id==='joined').s,2);
legacy.tracks[0].clips=[{id:'c',s:0,e:5},{id:'d',s:7,e:9}];
assert.equal(legacyCall('delete_clip','edit',{}, {clip_id:'c'}),true);assert.equal(legacy.tracks[0].clips.find(c=>c.id==='d').s,2);
assert.equal(legacyCall('caption','text',{text:'Subtitle',s:1}),true);assert.equal(legacy.tracks[1].clips[0].e,3);
assert.equal(legacyCall('caption','text',{text:'No track',s:1},{track_id:'missing'}),false);
const ex=fresh('export'), renName=Object.keys(MODES.export.panels).find(k=>MODES.export.panels[k].kind==='renders');
const expCall=(tool,args={},phase='complete',result={})=>exportOps(ex,MODES,event(tool,args,phase,result));
assert.equal(expCall('cancel_job',{job_id:'none'}),false);
const row=['Export','fast','1s','● running','now'];row._jobId='job';row._live=true;ex.views.export[renName].rows.push(row);
assert.equal(expCall('cancel_job',{job_id:'job'},'complete',{completed_ranges:[[0,1],[2,3]]}),true);assert.equal(row[3],'✕ cancelled · 2 chunks kept');assert.equal(row._live,false);
assert.equal(expCall('resume_job',{job_id:'job'},'start'),true);assert.equal(row[3],'● resuming');assert.equal(row._live,true);
expCall('resume_job',{job_id:'job'},'complete',{path:'export.mp4'});assert.equal(row._live,false);
const inspected=fresh('edit');assert.deepEqual(plain(inspectOps(inspected,MODES,{tool_name:'get_project',phase:'progress',payload:{pct:42}},'read1')),[{type:'tool_progress',id:'read1',progress:{pct:42}}]);
grade.liveTL={tracks:[{id:'v',clips:[{id:'plate',label:'Clean plate',s:0,e:1}]}]};grading('add_effect',{kind:'keyer',strings:{ref_clip:'plate'}});assert.match(kv.blocks.find(b=>b.k==='Keyer').v,/Clean plate/);
console.log('PASS structural inspector rows, legacy event edits, cancellation/resume and read progress');
assert.equal(legacyCall('auto_duck','audio',{args:{track:'v',detect:'speech',amount_db:-8,hold:.3}}),true);assert.deepEqual(plain(legacy.tracks[0].duck),{detect:'speech',amount_db:-8,hold:.3});
assert.equal(legacyCall('auto_duck','audio',{result:{track:'v',mode:'rms',hold:.5}}),true);assert.deepEqual(plain(legacy.tracks[0].duck),{detect:'rms',amount_db:-12,hold:.5});
assert.equal(legacyCall('auto_duck','audio',{}, {track_id:'v'}),true);assert.deepEqual(plain(legacy.tracks[0].duck),{detect:'rms',amount_db:-12,hold:0});assert.equal(legacyCall('auto_duck','audio',{}, {track_id:'missing'}),false);
console.log('PASS audio duck detection, defaults, result fields and missing-track refusal');
for(const [tool,args,key,value] of [
 ['add_effect',{kind:'glow',params:{mode:1,falloff:2,iterations:3,linear:1}},'glow falloff','2'],
 ['apply_effect',{kind:'lens_flare',params:{ghosts:4,halo:.8}},'lens_flare ghosts','4'],
 ['set_effect_param',{param:'falloff',value:3},'falloff','3'],
 ['set_color_curve',{points:[[0,0],[1,1]]},'Curve m','2 pts'],
 ['render_display_scope',{targets_75:true},'Scope','75% targets'],
 ['render_scope',{data:true},'Scope','data'],
 ['add_effect',{kind:'keyer',params:{despill_mode:8,view:8}},'Keyer','keyer · model 0 · despill 8 · 8 view'],
]){const actual=grading(tool,args).blocks.find(b=>b.k===key);assert.equal(actual.v,value);}
const savedGain=plain(wheels._g.gain);assert.equal(gradeOps(grade,MODES,event('add_effect',{kind:'audio',params:{gain:7}})),false);assert.deepEqual(plain(wheels._g.gain),savedGain);
grading('grade',{kind:'custom',gain:.8});assert.equal(wheels._g.gain.r,.8);assert.ok(wheels.wheels.every(w=>w.dot.every(Number.isFinite)));
grading('add_effect',{kind:'grade',params:{lift:-.2}});assert.deepEqual(plain(wheels._g.lift),{r:-.2,g:-.2,b:-.2});
grading('add_acescct_primaries');assert.ok(kv.blocks.some(b=>b.k==='Offset'));
const cacheGrade=event('grade',{},'complete');grade._gradeArgs={[cacheGrade.call_id]:{gamma:1.1}};delete cacheGrade.payload;assert.equal(gradeOps(grade,MODES,cacheGrade),true);assert.equal(wheels._g.gamma.r,1.1);
for(const plate of ['missing','clip_1234567890abcdef']){grading('add_effect',{kind:'keyer',strings:{ref_clip:plate}});assert.match(kv.blocks.find(b=>b.k==='Keyer').v,new RegExp(plate));}
console.log('PASS additional grading effect rows, default scope/curve options, cached args and unsupported gain isolation');
for(const args of [{to:7},{from:3}]) {
 const minimal=fresh('graphics');delete minimal.session;
 assert.equal(graphicsOps(minimal,MODES,event('add_counter',args)),true);
 const o=minimal.views.graphics.viewer.overlay.at(-1);
 assert.deepEqual(plain(o.at),[.5,.5]);assert.equal(o.text,args.to?'0 → 7':'3 → 0');
}
const fallbackGfx=fresh('graphics');
assert.equal(graphicsOps(fallbackGfx,MODES,{tool_name:'add_hud_frame',phase:'start',call_id:'empty'}),false);
assert.equal(graphicsOps(fallbackGfx,MODES,{tool_name:'add_hud_frame',phase:'complete',call_id:'uncached'}),true);
assert.equal(fallbackGfx.views.graphics.viewer.overlay.at(-1).label,'BOXES');
assert.equal(graphicsOps(fallbackGfx,MODES,event('add_detection_box',{x:0,y:0,w:1,h:1})),true);
assert.equal(fallbackGfx.views.graphics.viewer.overlay.at(-1).label,'');
assert.equal(graphicsOps(fallbackGfx,MODES,event('add_prompt_box',{text:'Centered'})),true);
assert.deepEqual(plain(fallbackGfx.views.graphics.viewer.overlay.at(-1).at),[.5,.5]);
for(const args of [{clip_id:'named'},{}, {effect_id:'effect'}]) {
 assert.equal(graphicsOps(fallbackGfx,MODES,event('remove_modifier',args)),true);
 const kv=Object.values(fallbackGfx.views.graphics).find(v=>v.blocks);
 assert.equal(kv.blocks.at(-1).v,args.effect_id??args.clip_id??'');
}
assert.equal(graphicsOps(fallbackGfx,MODES,event('set_layer_style',{params:{width:4}})),true);
assert.equal(Object.values(fallbackGfx.views.graphics).find(v=>v.blocks).blocks.find(b=>b.k==='width').v,'4');
for(const [tool,result,expected] of [['add_hud_frame',{},'hud'],['add_hud_frame',{id:'named'},'named'],['unknown',{},'unknown']]) {
 const e=event(tool,{track_id:'track1',start:1},'complete',result);
 graphicsOps(fallbackGfx,MODES,e);
 assert.equal(e.payload.clip,expected);assert.equal(e.payload.label,expected);assert.equal(e.payload.e,5);
}
console.log('PASS graphics missing dimensions, scalar counter defaults, empty telemetry and fallback insertion identity');

const diffState=fresh('edit'), inspectorDiff=root.namespace.inspectorDiff;
const diff=(payload,target,tool='synthetic_edit')=>inspectorDiff(diffState,MODES,{tool_name:tool,payload,target},'edit');
assert.equal(diff(undefined,undefined),false);
const diffKv=Object.values(diffState.views.edit).find(v=>v.blocks);
diffKv.blocks=[{k:'Volume',v:'old'},{k:'Other'}];
assert.equal(diff({value:2},{param:'volume'},'normalize_loudness'),true);
assert.deepEqual(plain(diffKv.blocks[0]),{k:'Volume',v:'x2',from:'old',_flash:1000});
assert.equal(diff({value:3,from:1},{param:'volume'},'normalize_loudness'),true);
assert.equal(diffKv.blocks[0].from,'x1');
assert.equal(diff({to:'quiet',from:'loud'},{param:'volume'},'normalize_loudness'),true);
assert.equal(diffKv.blocks[0].from,'loud');assert.equal(diffKv.blocks[0].v,'quiet');
assert.equal(diff({value:7},{param:'other'}),true);
assert.equal(diffKv.blocks[1].v,'7');assert.equal(diffKv.blocks[1].from,undefined);
diffKv.blocks=[];
assert.equal(diff({param:'volume',value:2,from:1,custom:'detail'},undefined,'normalize_loudness'),true);
assert.ok(diffKv.blocks.some(b=>b.k==='Volume'&&b.v==='x2'&&b.from==='x1'));
assert.ok(diffKv.blocks.some(b=>b.k==='Custom'&&b.v==='detail'));
assert.equal(diff({param:'volume',value:'quiet',from:'loud'},undefined,'normalize_loudness'),true);
assert.ok(diffKv.blocks.some(b=>b.k==='Volume'&&b.v==='quiet'&&b.from==='loud'));
assert.equal(diff({param:'shape',value:{x:1},label:'unchanged'},undefined),true);
assert.ok(!diffKv.blocks.some(b=>b.k==='Param'));
assert.equal(diff({value:4,label:'result',custom:'shown'},undefined),true);
assert.ok(diffKv.blocks.some(b=>b.k==='Value'&&b.v==='4'));
console.log('PASS inspector updates preserve previous values, normalize volume units and omit object-valued changes');

// Readouts resolve actual clip names and never expose raw engine IDs as reference labels.
assert.equal(root.namespace.family(undefined),'other');
const referenceId='c475_s0';
for(const [label,id,expected] of [['Reference plate',referenceId,'Reference plate'],[referenceId,referenceId,null],['','named-reference','named-reference'],[undefined,'missing-reference','missing-reference']]) {
 const st=fresh('color');st.liveTL={tracks:[{id:'track',clips:label===undefined?[]:[{id,label,s:0,e:1}]}]};
 assert.equal(gradeOps(st,MODES,event('match_grade',{ref_clip_id:id},'complete',{source_lab_mean:[50,0,0],reference_lab_mean:[60,0,0]})),true);
 const kv=Object.values(st.views.color).find(v=>v.blocks),ref=kv.blocks.find(b=>b.k==='Ref');
 assert.equal(ref?.v??null,expected);assert.equal(kv.blocks.find(b=>b.k==='Src L').v,'50');
 gradeOps(st,MODES,event('add_effect',{kind:'keyer',strings:{ref_clip:id}}));
 assert.match(kv.blocks.find(b=>b.k==='Keyer').v,new RegExp('clean plate '+(label&&label!==referenceId?label:id)));
}
// A custom screen without an inspector still receives the sampled overlay and wipe.
const minimalModes={custom:{panels:{viewer:{kind:'viewer'}}}};
const custom=fresh('custom');
assert.equal(gradeOps(custom,minimalModes,event('match_color',{},'complete',{src_mean:[.1,.2,.3]})),true);
assert.equal(custom.views.custom.viewer.wipe,1);
assert.equal(gradeOps(custom,minimalModes,event('add_effect',{kind:'keyer'})),false);
assert.equal(gradeOps(fresh('color'),MODES,{tool_name:'grade',phase:'complete'}),false);
console.log('PASS grade references resolve clip names, hide engine IDs and tolerate screens without inspectors');


const changeRows=(state,payload,target)=>plain(root.namespace.callDiff(state,{payload,target}));
assert.deepEqual(changeRows({},undefined,undefined),null);
assert.deepEqual(changeRows({}, {changed:{clip:{start:[1,2],duration:[3,4]}}}),[
 {k:'Start',from:'1s',v:'2s'},{k:'Duration',from:'3s',v:'4s'}]);
assert.deepEqual(changeRows({}, {changed:{clip:{start:['bad',2],duration:[3,4]}}}),[
 {k:'Start',from:'—',v:'2s'},{k:'Duration',from:'3s',v:'4s'}]);
assert.deepEqual(changeRows({}, {changed:{clip:{keys:[null,[{t:1,v:2}]]}}}),[{k:'Value key',from:'—',v:'1s 2'}]);
assert.deepEqual(changeRows({}, {changed:{clip:{keys:[[{t:1,v:2}],null]}}}),[{k:'Value key',from:'1s 2',v:'—'}]);
assert.deepEqual(changeRows({}, {changed:{clip:{points:[[],[{x:1}]],source:[null,3]}}}),[
 {k:'Points',from:'[0]',v:'[1]'},{k:'Source',from:'—',v:'3'}]);
const namedTimeline={liveTL:{tracks:[{id:'t1',label:'Video',clips:[{id:'c1',label:'Hero',s:0,e:1}]},{id:'t2',label:'Audio',clips:[]}]}};
assert.deepEqual(changeRows(namedTimeline,{changed:{clips_added:[{clip_id:'c1'},{clip_id:null}],clips_removed:[{clip_id:'c1'},{clip_id:'c999'}],clips_moved:[{clip_id:'c1',from:'t1',to:'t2'}],tracks_removed:['t2','t999',null]}}),[
 {k:'Clip',from:'—',v:'Hero'},{k:'Clip',from:'Hero',v:'—'},{k:'Hero track',from:'Video',v:'Audio'},{k:'Track',from:'Audio',v:'—'}]);
assert.deepEqual(changeRows({}, {clip:'c9',label:'Added title',changed:{clips_added:[{clip_id:'c9'},{clip_id:'c8'}]}}),[{k:'Clip',from:'—',v:'Added title'}]);
assert.deepEqual(changeRows({}, {label:'Target title',changed:{clips_added:[{clip_id:'c9'}]}},{clip_id:'c9'}),[{k:'Clip',from:'—',v:'Target title'}]);
assert.deepEqual(changeRows({}, {clip:'c9',label:'c9',changed:{clips_added:[{clip_id:'c9'}],clips_removed:[{clip_id:'c1'}],tracks_removed:['t1']}}),null);
assert.deepEqual(changeRows({}, {changed:{clips_moved:[{clip_id:'c1',from:'Video',to:'Audio'}]}}),[{k:'Track',from:'Video',v:'Audio'}]);
console.log('PASS structural change readouts retain timing, key removal and human clip/track identities');


const audioModes={mixFixture:{panels:{inspector:{kind:'kv'},extra:{kind:'kv'},loudness:{kind:'loudness'}}}};
const mixState=fresh('mixFixture');
const eqFixture={widget:'eq',bands:[{f:100,g:0},{f:1000,g:1,sel:true},{f:500,g:2,type:'highpass'},{f:300,type:'bell'}]};
const compFixture={widget:'comp'},panFixture={k:'Pan',v:'C'};
mixState.views.mixFixture={inspector:{blocks:[eqFixture,compFixture,panFixture]},extra:{},loudness:{meters:[{}, {v:.95,peak:.99},{v:.1,peak:.9}]}};
const mix=(tool,payload)=>root.namespace.audioOps(mixState,audioModes,{tool_name:tool,payload});
assert.equal(root.namespace.fmtPan(undefined),'');assert.equal(root.namespace.fmtPan('unknown'),'unknown');
mix('eq',{args:{frequency:900,g:3,q:2}});
assert.equal(eqFixture.bands[1].f,900);assert.equal(eqFixture.bands[1].g,3);assert.equal(eqFixture.bands[1].q,2);assert.equal(eqFixture.bands[2].g,2);
mix('eq',{band:0,to:0});assert.equal(eqFixture.bands[0].g,0);assert.equal(eqFixture.bands[0].sel,true);assert.equal(eqFixture.bands[1].sel,false);
mix('eq',{});assert.equal(eqFixture.bands[0]._flash,1000);
eqFixture.bands.forEach(b=>delete b.sel);mix('eq',{});assert.equal(eqFixture.bands[0].sel,true);
mix('compress',{});assert.equal(compFixture.inDb,-10);
mix('compress',{threshold:-6,ratio:3,level:-2});assert.equal(compFixture.threshold,-6);assert.equal(compFixture.ratio,3);assert.equal(compFixture.inDb,-2);
mix('pan',{value:-.5});assert.equal(panFixture.v,'L50');assert.equal(panFixture.from,'C');
mix('pan',{to:-.5});assert.equal(panFixture.from,'C');mix('pan',{});assert.equal(panFixture.v,'L50');
for(const [payload,expected] of [[{result:{integrated_lufs:-14}},'-14.0'],[{result:{measured_lufs:-20,gain_db:6}},'-14.0'],[{result:{target_lufs:-16}},'-16.0'],[{result:{lufs:-18}},'-18.0'],[{lufs:-19},'-19.0'],[{param:'loudness',to:-21},'-21.0'],[{args:{target_lufs:-22}},'-22.0'],[{args:{lufs:-23}},'-23.0']]) {
 mix('measure_loudness',payload);assert.equal(mixState.views.mixFixture.loudness.lufs,expected);
}
const loud=mixState.views.mixFixture.loudness;
mix('normalize_loudness',{result:{measured_lufs:0,gain_db:0,sample_peak_dbfs:-1,target_lufs:-14}});
assert.equal(loud.target,'-14.0');assert.deepEqual(plain(loud.rows),[['Input','0.0 LUFS'],['Gain','0.0 dB'],['Peak','-1.0 dBFS']]);
mix('measure_loudness',{result:{loudness_range:{lra_lu:4.25}}});assert.deepEqual(plain(loud.rows[0]),['LRA','4.3 LU']);
mix('gain',{gain:100});assert.ok(loud.meters.every(m=>m.hit.lvl===1));assert.ok(loud.meters.every(m=>m.peakHold.lvl===1));
mix('gain',{gain:-100});assert.ok(loud.meters.every(m=>m.hit.lvl===0));assert.equal(loud.meters[0].hit.base,.5);assert.equal(loud.meters[2].peakHold.lvl,1);
const dryModes={dry:{panels:{inspector:{kind:'kv'}}}},dry=fresh('dry');dry.views.dry={inspector:{blocks:[{widget:'eq'}]}};
assert.equal(root.namespace.audioOps(dry,dryModes,{tool_name:'eq'}),false);
console.log('PASS EQ selection, compressor defaults, pan stability, loudness precedence and bounded meter levels');

for(const [tool,args,result,label] of [
 ['stabilize',{}, {},'stabilize'],['stabilize',{}, {engine:'warp'},'warp stabilize'],
 ['stabilize',{smoothness:0,rolling_shutter:0},{zoom:1},'warp stabilize · smooth 0%'],
 ['apply_camera_solve',{}, {},'camera solve → keys'],['apply_camera_solve',{}, {count:0},'camera solve → keys · 0 keys'],
 ['set_ground_plane',{}, {},'ground plane'],['set_ground_plane',{}, {scale:1},'ground plane'],
 ['place_at_track',{}, {},'null at track'],['export_st_map',{}, {},'ST map · undistort'],
 ['export_st_map',{}, {width:100,height:50,clamped_pixels:0},'ST map · undistort · 100×50']
]) {
 const resultView=sc(fresh('tracking'),tool,args,result);assert.equal(resultView.hit,true);assert.equal(resultView.view.left,label);
}
for(const [args,lens,center,label] of [[{k1:.1},[.1,0],[.5,.5],'undistort · k1 0.1'],[{k2:.2},[0,.2],[.5,.5],'undistort · k2 0.2']]) {
 const resultView=sc(fresh('tracking'),'undistort_clip',args);assert.deepEqual(plain(resultView.view.overlay[0].lens),lens);assert.deepEqual(plain(resultView.view.overlay[0].c),center);assert.equal(resultView.view.left,label);
}
const trackDefaults=sc(fresh('tracking'),'track_mesh',{anchors:[{corners:quad}]});assert.deepEqual(plain(trackDefaults.view.overlay[0].grid),[7,5]);
const pointDefault=sc(fresh('tracking'),'track_point',{x:0,y:0});assert.deepEqual(plain(pointDefault.view.overlay[0].pt),[0,0]);assert.equal(pointDefault.view.overlay[0].r,.12);
for(const corners of [[[0,0],[0,0],[0,0],null],[[0,0],[0,0],[0,0],['bad',0]]]) {
 const bad=sc(fresh('tracking'),'set_corner_pin',{corners});assert.equal(bad.hit,false);assert.equal(bad.view.overlay,undefined);
}
const cachedScene=fresh('tracking'),sceneStart=event('track_point',{x:.25,y:.75},'start');sceneOps(cachedScene,MODES,sceneStart);
sceneOps(cachedScene,MODES,{...sceneStart,phase:'complete',payload:{}});assert.deepEqual(plain(cachedScene.views.tracking.viewer.overlay[0].pt),[.25,.75]);
for(const [tool,args,name] of [['add_particles3d',{},'point 3D emitter'],['set_particles3d',{emitter:{kind:'box'}},'box 3D emitter'],['add_particles',{shape:'circle'},'circle emitter'],['add_emitter',{preset:'sparks'},'sparks'],['add_shard_burst',{},'shards']]) {
 const st=fresh('particles');sc(st,tool,{start:1,...args});
 const listView=Object.values(st.views.particles).find(v=>v.rows),keyView=Object.values(st.views.particles).find(v=>v.lanes);
 assert.equal(listView.rows[0].k,name);assert.equal(listView.rows[0].v,'');assert.deepEqual(plain(keyView.lanes[0].dots),[24]);
}
const materialDefault=fresh('lighting');sc(materialDefault,'set_material',{albedo:[null,.5,'bad']});
const materialKv=Object.values(materialDefault.views.lighting).find(v=>v.blocks);assert.equal(materialKv.sub,'material');assert.equal(materialKv.blocks.find(b=>b.k==='Albedo').v,'0.00  0.50  0.00');
assert.equal(sc(fresh('lighting'),'preview_material',{},{}).hit,false);
assert.equal(sc(fresh('lighting'),'preview_material',{}, {path:'/fixture/material.png'}).view.left,'preview ·');
assert.equal(sc(fresh('lighting'),'set_relight',{}).hit,false);
console.log('PASS sparse tracking results, particle defaults, cached geometry and material readouts');

// Engine timelines assign stable human labels and grow their ruler without shrinking it.
const {ensureTrack,fitRuler}=root.namespace;
const sparseTimeline={tracks:[{id:'existing',label:'Original',clips:[]}]};
assert.equal(ensureTrack(sparseTimeline,'existing'),sparseTimeline.tracks[0]);
assert.equal(ensureTrack(sparseTimeline,null),null);
for(const [id,hint,label,kind] of [['track2',undefined,'V2','v'],['3','audio','A1','a'],['Music',undefined,'Music','a'],['track4','video','V3','v']]) {
 const track=ensureTrack(sparseTimeline,id,hint);assert.equal(track.label,label);assert.equal(track.kind,kind);assert.deepEqual(plain(track.clips),[]);
}
for(const [end,maximum,step] of [[0,10,2],[20,23,5],[50,58,10],[100,115,30]]) {
 const tl={_auto:true,tracks:[{}, {clips:[{}, {e:end}]}]};fitRuler(tl);
 assert.deepEqual(plain(tl.ruler),{t0:0,t1:maximum,step,edge:true});
 tl.tracks=[];const saved=tl.ruler;fitRuler(tl);assert.equal(tl.ruler,saved,'ruler never shrinks after clip removal');
}
const emptyAuto={_auto:true};fitRuler(emptyAuto);assert.equal(emptyAuto.ruler.t1,10);
const manual={_auto:false,ruler:{t0:10,t1:20,step:1},tracks:[{clips:[{e:200}]}]};fitRuler(manual);assert.deepEqual(manual.ruler,{t0:10,t1:20,step:1});
console.log('PASS track naming and automatic ruler growth preserve existing tracks and manual ranges');

const {insertClip}=root.namespace;
const overwrite={clips:[{id:'left',s:0,e:3},{id:'covered',s:3,e:4},{id:'right',s:4,e:8},{id:'same',s:2,e:6}]};
const originalSame=overwrite.clips[3];insertClip({},overwrite,{id:'same',s:2,e:6,label:'Updated'});
assert.deepEqual(plain(overwrite.clips.map(c=>[c.id,c.s,c.e])),[['left',0,2],['same',2,6],['right',6,8]]);
assert.equal(overwrite.clips[1],originalSame);assert.equal(originalSame.label,'Updated');
const emptyTrack={};insertClip({},emptyTrack,{id:'first',s:0,e:1});assert.equal(emptyTrack.clips[0].id,'first');
for(const operation of ['set_speed_ramp','set_time_remap']) {
 const c={id:'sparse',s:2,e:5},tl={tracks:[{id:'v',clips:[c]}]};
 const call=(args,result)=>editOps2(fresh('edit'),MODES,tl,event(operation,args,'complete',result),{clip_id:'sparse',...args});
 assert.equal(call({keys:[]}),true);assert.deepEqual(plain(c.fx),[]);assert.deepEqual(plain(c.keys),[]);
 c.keys=[{t:3,_other:true}];c.fx=['keep'];
 assert.equal(call({keys:[[0,1],[1,2]]}),true);assert.ok(c.fx.includes(operation==='set_speed_ramp'?'ramp':'remap'));
 assert.equal(c.keys[0]._other,true);assert.deepEqual(plain(c.keys.slice(1).map(k=>k.t)),[2,3]);
 c.fx=[operation==='set_speed_ramp'?'ramp · frame mix':'remap · flow'];
 assert.equal(call({keys:[[2,3]]}),true);assert.equal(c.fx[0],operation==='set_speed_ramp'?'ramp · frame mix':'remap · flow');
}
const defaultsTimeline={tracks:[{id:'empty'},{id:'v',clips:[{id:'one',s:2,e:4},{id:'two',s:8,e:9}]}]};
const defaultsCall=(name,args={},result)=>editOps2(fresh('edit'),MODES,defaultsTimeline,event(name,args,'complete',result),args);
assert.equal(defaultsCall('distribute_clips',{track_id:'empty'}),false);
assert.equal(defaultsCall('distribute_clips',{track_id:'v'}),true);
assert.deepEqual(plain(defaultsTimeline.tracks[1].clips.map(c=>[c.s,c.e])),[[2,4],[4,5]]);
assert.equal(defaultsCall('loop_clip',{clip_id:'two'}),true);
assert.deepEqual(plain(defaultsTimeline.tracks[1].clips.map(c=>[c.id,c.s,c.e])),[['one',2,4],['two',4,5],['two_loop1',5,6]]);
console.log('PASS overwrite trimming, sparse time-key state, interpolation retention and default clip distribution');

// Incomplete spectator diffs must not expose unresolved engine identifiers or invent values.
const unnamedTimeline={liveTL:{tracks:[{id:'t1',label:'t1',clips:[{id:'c1',label:'c1'}]},{id:'t2',clips:[{id:'c2'}]}]}};
assert.equal(changeRows(unnamedTimeline,{changed:{clips_added:[{clip_id:'c1'},{clip_id:'c2'}],tracks_added:['t1','t2'],clips_moved:[{clip_id:'c1',from:'t1',to:'t2'}]}}),null);
assert.deepEqual(changeRows({}, {changed:{clip:{'props.keys':[[{t:1,v:3}],[{t:1,v:3},{v:4}]],'effects.params':[{},{}]}}}),null);
assert.deepEqual(changeRows({}, {changed:{clip:{'params':[[1,2],[3,4]],'effects.kind':['old','new'],'source.in_point':[1,2]}}}),[
 {k:'Params',from:'1, 2',v:'3, 4'},{k:'Kind',from:'old',v:'new'},{k:'Source in point',from:'1s',v:'2s'}]);
assert.equal(changeRows({}, {s:4,e:8,changed:{clip:{start:['invalid',4],duration:['invalid',4]}}}),null);
assert.deepEqual(changeRows({}, {s:4,e:8,changed:{clip:{duration:[2,4]}}}),[{k:'Out',from:'6s',v:'8s'}]);
assert.deepEqual(changeRows({}, {changed:'invalid',param:'opacity',from:.2,to:.5}),[{k:'Opacity',from:'20%',v:'50%'}]);
assert.equal(changeRows({}, {param:'opacity',from:.2}),null);
console.log('PASS incomplete diffs hide unresolved IDs, preserve unchanged keys and format source timing');

// Partial events arrive before argument/result hydration; unrelated tools never gain a visual effect.
for(const [handler,mode] of [[graphicsOps,'graphics'],[maskOps,'roto'],[sceneOps,'tracking'],[gradeOps,'grading']]) {
 for(const phase of ['start','progress','complete']) {
  const state=fresh(mode);
  assert.equal(handler(state,MODES,{tool_name:'unrelated_tool',phase,call_id:'incomplete'}),false,`${mode} ${phase}`);
  assert.equal(state.views[mode]?.viewer?.overlay,undefined);
 }
}
assert.equal(editOps2(fresh('edit'),MODES,{tracks:[]},{tool_name:'delete_track',phase:'complete'},{track_id:'absent'}),false);
assert.equal(editOps2(fresh('edit'),MODES,{}, {tool_name:'delete_track',phase:'complete'},{}),false);
console.log('PASS unhydrated events do not invent visual operations or missing tracks');

// Legacy spectator payloads use target IDs and in/out aliases as well as modern args.
const legacySparse={};
assert.equal(timelineOps1(legacySparse,{tool_name:'add_track'},'edit'),true);
assert.equal(legacySparse.tracks[0].id,'track1');
for(const added of [[{track_id:'t2'},{id:'t3'}],['t4']]) {
 assert.equal(timelineOps1(legacySparse,{tool_name:'add_track',payload:{changed:{tracks_added:added}}},'edit'),true);
}
assert.deepEqual(plain(legacySparse.tracks.map(t=>t.id)),['track1','t2','t3','t4']);
assert.equal(timelineOps1(legacySparse,{tool_name:'add_clip',target:{track_id:'t2'},payload:{clip:'hero',s:2}},'graphics'),true);
const legacyHero=legacySparse.tracks[1].clips[0];assert.deepEqual([legacyHero.s,legacyHero.e],[2,6]);
legacySparse.tracks[1].clips.push({id:'later',s:8,e:10});
assert.equal(timelineOps1(legacySparse,{tool_name:'split_clip',target:{clip_id:'hero'},payload:{at:4,ripple:2}},'edit'),true);
assert.deepEqual(plain(legacySparse.tracks[1].clips.map(c=>[c.s,c.e])),[[2,4],[4,6],[10,12]]);
for(const [payload,expected] of [[{result:{kind:'J',seconds:1}},{kind:'J',seconds:1}],[{kind:'L',seconds:2},{kind:'L',seconds:2}],[{args:{kind:'J',seconds:3}},{kind:'J',seconds:3}]]) {
 assert.equal(timelineOps1(legacySparse,{tool_name:'set_split_edit',target:{clip_id:'hero'},payload},'edit'),true);
 assert.deepEqual(plain(legacyHero.jl),expected);assert.deepEqual([legacyHero.s,legacyHero.e],[2,4]);
}
assert.equal(timelineOps1(legacySparse,{tool_name:'trim_clip',target:{clip_id:'hero'},payload:{in:1,out:3}},'edit'),true);
assert.deepEqual([legacyHero.s,legacyHero.e],[1,3]);
for(const payload of [{name:'glow'},{type:'blur'},{}])assert.equal(timelineOps1(legacySparse,{tool_name:'apply_effect_tint',target:{clip_id:'hero'},payload},'effects'),true);
assert.deepEqual(plain(legacyHero.fx),['glow','blur','tint']);
for(const [payload,expected] of [[{},true],[{},false],[{value:true},true],[{on:false,value:true},false]]) {
 assert.equal(timelineOps1(legacySparse,{tool_name:'solo_track',target:{track_id:'t2'},payload},'audio'),true);
 assert.equal(legacySparse.tracks[1].solo,expected);
}
assert.equal(timelineOps1(legacySparse,{tool_name:'mute_track',target:{track_id:'absent'}},'audio'),false);
for(const [payload,target,id] of [[{s:0,track:'Video'},{},'Video'],[{s:0},{},'V1']]) {
 const tl={};assert.equal(timelineOps1(tl,{tool_name:'import_media',payload,target},'ingest'),true);
 assert.equal(tl.tracks[0].id,id);assert.equal(tl.tracks[0].clips[0].e,4);
}
const sparseCaptions={tracks:[{id:'CAP'}]};
assert.equal(timelineOps1(sparseCaptions,{tool_name:'caption',payload:{text:'Hello',s:1}},'text'),true);
assert.deepEqual(plain(sparseCaptions.tracks[0].clips.map(c=>[c.label,c.s,c.e])),[['Hello',1,3]]);
console.log('PASS legacy timeline insertion, ripple splits, J/L edits, trim aliases and track solo toggles');

// Export cards reflect the specific job and reported size; incomplete progress stays indeterminate.
assert.equal(exportOps(fresh('export'),{},{}),false);
for(const [tool,args,preset,label] of [
 ['join_export',{parts:['a','b']},'join 2 parts','join_export'],['join_export',{},'join','join_export'],
 ['preview_range',{scale:.5},'preview 0.5x','preview_range'],['preview_range',{scale:1},'preview','preview_range'],
 ['collect_project',{dest_dir:'/fixtures/collected/'},'collect','collected'],['collect_project',{dest_dir:'/'},'collect','collect_project'],
 ['contact_sheet',{},'contact sheet','contact_sheet']
]) {
 const state=fresh('export');assert.equal(exportOps(state,MODES,{tool_name:tool,phase:'start',payload:{args}}),true);
 const rows=Object.values(state.views.export).find(v=>v.rows).rows;
 assert.equal(rows[0][0],label);assert.equal(rows[0][1],preset);
 assert.equal(rows[0][3],tool==='collect_project'?'● collecting':'● rendering');
}
for(const [bytes,size] of [[12,'12 B'],[1200,'1 KB'],[2e9,'2.00 GB']]) {
 const state=fresh('export');const start={tool_name:'export',call_id:'size',phase:'start',payload:{args:{path:'out.mp4',crf:20,width:1920,height:1080}}};
 exportOps(state,MODES,start);
 const view=state.views.export,rows=Object.values(view).find(v=>v.rows).rows,vw=view.viewer;
 assert.equal(exportOps(state,MODES,{...start,phase:'progress',payload:{}}),false);assert.equal(vw.progress.pct,null);
 exportOps(state,MODES,{...start,phase:'complete',payload:{result:{bytes,job_id:'job'}}});
 assert.equal(rows[0][3],`✓ done · ${size}`);assert.equal(rows[0]._jobId,'job');assert.equal(vw.progress.right,'');
 exportOps(state,MODES,{...start,phase:'start',payload:{args:{width:1280,height:720,crf:18}}});
 const blocks=Object.values(view).find(v=>v.blocks).blocks;
 assert.deepEqual(plain(blocks.find(b=>b.k==='Resolution')).from,'1920x1080');assert.equal(blocks.find(b=>b.k==='Resolution').v,'1280x720');
 assert.equal(blocks.find(b=>b.k==='Quality').from,'CRF 20');
 exportOps(state,MODES,{...start,phase:'progress',payload:{current_frame:3,total_frames:12}});assert.equal(vw.progress.pct,25);
 exportOps(state,MODES,{...start,phase:'error',payload:{}});assert.equal(rows[0][3],'✕ failed');assert.equal(vw.progress.halt,true);
}
const cancelled=fresh('export');exportOps(cancelled,MODES,{tool_name:'export',call_id:'cancelled',phase:'start'});
exportOps(cancelled,MODES,{tool_name:'cancel_job',phase:'complete'});
assert.equal(Object.values(cancelled.views.export).find(v=>v.rows).rows[0][3],'✕ cancelled');assert.equal(cancelled.views.export.viewer.progress.right,'cancelled');
assert.equal(exportOps(cancelled,MODES,{tool_name:'resume_job',phase:'start',call_id:'resumed',payload:{args:{job_id:'missing'}}}),false);
console.log('PASS export job presets, size units, resolution changes, frame progress and failed/cancelled status');

const {midPath,shortPath,fmtBytes}=root.namespace;
assert.equal(midPath('/very-long-root-folder-name/output.mp4'),'…/output.mp4');
assert.equal(midPath('abcdefghijklmnopqrstuvwxyz',10),'abcde…wxyz');
assert.equal(shortPath('/home/fixture/a-very-long-component/b/c/d/very-long-name.mp4'),'…/d/very-long-name.mp4');
assert.equal(shortPath('/home/fixture/a-very-long-component/b/c/d/very-long-name.mp4',30),'…/c/d/very-long-name.mp4');
assert.equal(shortPath('a-long-filename.mp4',5),'a-long-filename.mp4');
for(const [bytes,text] of [[0,'0 B'],[999,'999 B'],[1000,'1 KB'],[1e6,'1.0 MB'],[1e9,'1.00 GB']])assert.equal(fmtBytes(bytes),text);
for(const [pan,text] of [[0,'C'],[-25,'L25'],[25,'R25']])assert.equal(root.namespace.fmtPan(pan),text);
const codeState=fresh('edit');codeState.liveTL={tracks:[{id:'v',label:'Code',clips:[{id:'code',label:'Chart',s:0,e:5}]}]};
for(const [args,value] of [[{name:'height',value:2},'height = 2'],[{name:'height',value:3,time:1.25},'height = 3 @ 1.25 s']]) {
 const ev=event('set_code_clip_param',{clip_id:'code',...args});
 assert.equal(timelineOps(codeState,MODES,ev,'edit'),true);
 const inspector=Object.values(codeState.views.edit).find(v=>v.blocks);
 assert.equal(inspector.blocks.find(b=>b.k==='Param').v,value);assert.equal(inspector.sub,'Chart');
 assert.equal(codeState.liveTL.tracks[0].clips[0]._flash,1000);
}
for(const args of [{clip_id:'missing',name:'height',value:2},{clip_id:'code',value:2},{clip_id:'code',name:'height',value:'bad'}])assert.equal(editOps2(codeState,MODES,codeState.liveTL,event('set_code_clip_param',args),args),false);
console.log('PASS compact paths, size/pan boundaries and constant/keyed code-clip parameter feedback');

// Sparse timing events must preserve unrelated animation and the chosen interpolation.
for(const [tool,prefix,marker] of [['set_speed_ramp','ramp','_ramp'],['set_time_remap','remap','_remap']]) {
 for(const [blend,label] of [['frame_mix','frame mix'],['optical_flow','flow']]) {
  const state=fresh('edit'),clip={id:'timing',s:2,e:8,fx:[`${prefix} · ${label}`],keys:[{t:3,_other:true}]};
  const tl={tracks:[{id:'v',clips:[clip]}]};
  const args={clip_id:'timing',keys:[[0,1],[2,3],null,['bad',0]]};
  assert.equal(editOps2(state,MODES,tl,event(tool,args),args),true);
  assert.deepEqual(plain(clip.fx),[`${prefix} · ${label}`]);
  assert.deepEqual(plain(clip.keys),[{t:3,_other:true},{t:2,[marker]:true},{t:4,[marker]:true}]);
  const clear={clip_id:'timing',keys:[]};
  assert.equal(editOps2(state,MODES,tl,event(tool,clear),clear),true);
  assert.deepEqual(plain(clip.fx),[]);assert.deepEqual(plain(clip.keys),[{t:3,_other:true}]);
 }
 const state=fresh('edit'),clip={id:'timing',s:0,e:5},tl={tracks:[{id:'v',clips:[clip]}]},args={clip_id:'timing',keys:[]};
 assert.equal(editOps2(state,MODES,tl,event(tool,args),args),true);
 assert.deepEqual(plain(clip.fx),[]);assert.deepEqual(plain(clip.keys),[]);
}
const sparseFx={id:'effects',s:0,e:1},sparseFxTL={tracks:[{id:'v',clips:[sparseFx]}]};
assert.equal(editOps2(fresh('edit'),MODES,sparseFxTL,event('remove_audio_effect'),{clip_id:'effects',effect_id:'missing'}),false);
assert.equal(sparseFx.fx,undefined);
console.log('PASS timing interpolation survives re-keying; clearing preserves unrelated keys and sparse clips');

// Roto events can arrive on layouts without inspector/key panels; geometry remains usable.
const geometryOnly={roto:{panels:{viewer:{kind:'viewer'}}}};
const geometryState=fresh('roto');
assert.equal(maskOps(geometryState,geometryOnly,event('add_mask',{points:[[0,0],[1,1]],feather:3})),true);
assert.deepEqual(plain(geometryState.views.roto.viewer.overlay[0].path),[[0,0],[1,1]]);
assert.equal(maskOps(geometryState,geometryOnly,event('set_mask_path_key',{time:1})),false);
assert.equal(maskOps(geometryState,geometryOnly,event('remove_track_matte')),false);
assert.deepEqual(Object.keys(geometryState.views.roto),['viewer']);
const sparseMask=fresh('roto');sparseMask.session={fps:30};
assert.equal(maskOps(sparseMask,MODES,event('add_mask',{points:[[200,300],[400,500]]})),false);
assert.equal(sparseMask.views.roto.viewer.overlay,undefined,'pixel points without dimensions cannot be normalized');
assert.equal(maskOps(sparseMask,MODES,event('set_mask_keyframe',{mask_id:'subject'})),true);
const sparseMaskKeys=Object.values(sparseMask.views.roto).find(v=>v.lanes);
assert.deepEqual(plain(sparseMaskKeys.lanes[0].keys),[]);assert.equal(sparseMaskKeys.ruler,undefined);
assert.equal(maskOps(sparseMask,MODES,event('track_mask',{mask_id:'subject'})),false);
assert.equal(maskOps(sparseMask,MODES,event('roto_propagate',{mask_id:'subject'})),false);
assert.equal(maskOps(sparseMask,MODES,event('reorder_mask',{mask_id:'missing',index:0})),false);
assert.equal(maskOps(sparseMask,MODES,event('refine_matte',{clip_id:'subject',auto_bg:false},'complete',{mae:.3})),true);
const sparseMatteRows=Object.values(sparseMask.views.roto).find(v=>v.blocks);
assert.equal(sparseMatteRows.blocks.find(b=>b.k==='Matte MAE').v,'0.30');
console.log('PASS sparse roto layouts retain geometry without inventing dimensions, key times or tracked spans');

// Keyer references display the timeline's label and preserve unknown numeric modes.
for(const [ref,label] of [['clip42','Clean plate'],['unresolved','unresolved']]) {
 const state=fresh('grading');state.liveTL={tracks:[{id:'v',clips:[{id:'clip42',label:'Clean plate',s:0,e:1}]}]};
 assert.equal(gradeOps(state,MODES,event('add_effect',{kind:'keyer',strings:{ref_clip:ref},params:{despill_mode:9,view:9}})),true);
 const blocks=Object.values(state.views.grading).find(v=>v.blocks).blocks;
 assert.equal(blocks.find(b=>b.k==='Keyer').v,`keyer · model 0 · clean plate ${label} · despill 9 · 9 view`);
}
const keyerViewer=fresh('grading');
assert.equal(gradeOps(keyerViewer,{grading:{panels:{viewer:{kind:'viewer'}}}},event('add_effect',{kind:'keyer'})),false);
assert.deepEqual(Object.keys(keyerViewer.views.grading),['viewer']);
console.log('PASS keyer timeline names, unresolved references and forward-compatible numeric modes');

// Completion metadata remains useful when optional tracking details are absent.
for(const [tool,args,result,text] of [
 ['stabilize',{}, {},'stabilize'],
 ['stabilize',{}, {engine:'warp'},'warp stabilize'],
 ['stabilize',{}, {engine:'warp',method:'position',smoothness:20,framing:'stabilize_only',rolling_shutter:0,zoom:1},'warp stabilize · position · smooth 20% · stabilize only'],
 ['apply_camera_solve',{}, {keyframes:3},'camera solve → keys · 3 keys'],
 ['apply_camera_solve',{}, {count:4},'camera solve → keys · 4 keys'],
 ['apply_camera_solve',{}, {},'camera solve → keys'],
 ['set_ground_plane',{}, {scale:1},'ground plane'],
 ['place_at_track',{}, {},'null at track'],
 ['export_st_map',{}, {},'ST map · undistort'],
 ['validate_planar_track',{}, {ok:false},'validate failed'],
 ['fit_track_homography',{}, {ok:true},'fit ok']
]) {
 const state=fresh('tracking');assert.equal(sceneOps(state,MODES,event(tool,args,'complete',result)),true);
 assert.equal(state.views.tracking.viewer.left,text,tool);
}
for(const tool of ['track_planar','track_mesh','track_features','track_point','track_matte','pin_layer','set_corner_pin']) {
 const state=fresh('tracking');assert.equal(sceneOps(state,MODES,event(tool,{})),false,tool);
 assert.equal(state.views.tracking.viewer.overlay,undefined);
}
console.log('PASS sparse tracking completions retain reported metadata and never invent tracking geometry');

// Keyframe presentation preserves sparse lists and evaluates degenerate CSS slopes.
const {curveAt,easeOf,bezOf,cubicY,keyChange,keyGraph,evalKeys,xfOps}=root.namespace;
assert.equal(curveAt([],2),null);
for(const [time,value] of [[-1,2],[0,2],[1,4],[2,6],[3,6]])assert.equal(curveAt([{t:2,v:6},{t:0,v:2},{t:1,v:'bad'}],time),value);
assert.equal(easeOf({}),'linear');assert.equal(easeOf({bez:[0,.2,1,.8]}),'bezier 0 .2 1 .8');
assert.equal(bezOf({bez:[0,0,1,'invalid']}),null);assert.deepEqual(plain(bezOf({ease:'EASE_IN'})),[.42,0,1,1]);
for(const x of [.0001,.01,.3,.9]) {const t=Math.cbrt(x);assert.ok(Math.abs(cubicY([0,0,0,1],x)-(3*t*t-2*t*t*t))<1e-5);}
for(const ease of ['hold','step','constant'])assert.deepEqual(plain(evalKeys([{t:0,v:2,ease},{t:2,v:4}]).pts),[[0,2],[2,2],[2,4]]);
assert.deepEqual(plain(evalKeys([{t:0,v:2,bez:[0,0,1,1]},{t:0,v:3}]).handles),[]);
const keyEvent=(from,to,extra={})=>({tool_name:'set_keyframe',phase:'complete',payload:{from,to,...extra}});
for(const [from,to] of [[[],[]],[[{t:0,v:2}],['invalid']],[[{t:'bad',v:2}],[{t:0,v:2}]]]) {const ev=keyEvent(from,to);assert.equal(keyChange(ev),ev);}
let changed=keyChange(keyEvent([{t:0,v:1}],[{t:0,v:1}]));assert.deepEqual(plain(changed.payload.key),{label:'value keys',from:'1',to:'1',t:null});
changed=keyChange(keyEvent([],[{t:0,v:1}],{param:'gain'}));assert.equal(changed.payload.key.from,'none');assert.equal(changed.payload.key.label,'gain @0s');
changed=keyChange(keyEvent([{t:0,v:1}],[],{args:{property:'x',t:0}}));assert.equal(changed.payload.key.to,'none');assert.equal(changed.payload.key.label,'x @0s');
changed=keyChange(keyEvent([{t:0,v:1},{t:2,v:3}],[{t:0,v:1},{t:1,v:4},{t:2,v:3}],{args:{time:0}}));assert.equal(changed.payload.key.t,1);assert.equal(changed.payload.key.from,'2');
changed=keyChange(keyEvent([{t:0,v:1,ease:'linear'}],[{t:0,v:1,bez:[0,0,1,1]}],{args:{property:'scale',time:0}}));assert.equal(changed.payload.key.ease,true);assert.equal(changed.payload.key.to,'bezier 0 0 1 1');
const graphModes={motion:{panels:{graph:{kind:'curves'}}}};
assert.equal(keyGraph(fresh('motion'),graphModes,{}),false);
assert.equal(keyGraph(fresh('absent'),{}, {payload:{to_keys:[]}}),false);
for(const payload of [{to_keys:[]},{to_keys:[{t:2,v:5}],from_keys:[{t:2,v:5}],param:'custom_value'},{to_keys:[{t:0,v:1},{t:2,v:2}],key:{t:0}}]) {
 const state=fresh('motion');assert.equal(keyGraph(state,graphModes,{payload,call_id:'graph'}),true);
 const graph=state.views.motion.graph;assert.ok(Number.isFinite(graph.ruler.step));assert.ok(graph.ruler.t1>graph.ruler.t0);
 assert.equal(graph.curves.length,payload.to_keys.length?(payload.from_keys?2:1):0);
 if(payload.param)assert.equal(graph.lanes[0],'custom value');else assert.equal(graph.lanes[0],'value');
 assert.equal(graph.playhead,payload.key?.t??null);
 for(const curve of graph.curves)assert.ok(curve.pts.every(point=>point.every(Number.isFinite)));
}
const transform=fresh('graph');
for(const [tool,args,expected] of [
 ['add_null',{x:2,y:3},'null'],['set_parent',{parent:'missing'},'parent → missing'],['set_parent',{},'unparented'],
 ['set_expression',{expression:'wiggle',freq:2,amp:3},'wiggle(freq 2, amp 3)'],['set_expression',{},'set_expression'],
 ['set_motion_blur',{angle:180,samples:0,shutter_phase:30,curve_points:[[0,0],[1,1]]},'shutter 180°'],
 ['set_keyframe_bezier',{handles:[0,0,1,1],at:2},'ease [0, 0, 1, 1]'],
 ['set_keyframes',{keys:[null,{t:2},{time:3},{}]},'2 keys']
]) {xfOps(transform,MODES,event(tool,args));assert.equal(transform.gizmo.read[0],expected,tool);}
xfOps(transform,MODES,{tool_name:'set_property',phase:'complete',payload:{param:'rot_x',value:30}});assert.equal(transform.gizmo.next.rot_x,30);assert.equal(transform.gizmo.three,true);
console.log('PASS sparse key changes, hold curves, degenerate Bezier slopes, graph bounds and transform aliases');

{
// Lighting and scene lists also accept partial, forward-compatible engine records.
const minimalScene={scene3d:{panels:{viewer:{kind:'viewer'},tree:{kind:'tree'},info:{kind:'kv'},keys:{kind:'keys'}}},lighting:{panels:{viewer:{kind:'viewer'},list:{kind:'list'},rig:{kind:'lightrig'}}}};
const lights=fresh('lighting');
for(const args of [{kind:'point'},{kind:'point',intensity:0,pos:['bad',.5],color:[1,0]},{id:'fill',kelvin:4500,pos:[.25,.5],color:[1,.5,0]},{id:'point',pos:[.75,.5]}])assert.equal(sceneOps(lights,minimalScene,event('set_light',args)),true);
assert.equal(lights.views.lighting.list.rows.find(r=>r._id==='point').v,'');
assert.equal(lights.views.lighting.rig.rows.find(r=>r[0]==='point')[3],0);
assert.equal(lights.views.lighting.list.rows.find(r=>r._id==='fill').v,'4500K');
assert.equal(lights.views.scene3d.tree.items.filter(x=>x.name==='point').length,1);
assert.equal(lights.views.scene3d.tree.items.find(x=>x.name==='fill').icon,'fill');
assert.equal(sceneOps(lights,minimalScene,event('set_light',{})),false);
for(const [args,result,ids] of [[{preset:'three_point'}, {},['rig_key','rig_fill','rig_rim']],[{preset:'portrait',subject:['bad',.5]},{lights:[null,{id:'custom'},'string-light']},['custom','string-light']]]) {
 const state=fresh('lighting');assert.equal(sceneOps(state,minimalScene,event('light_rig',args,'complete',result)),true);
 assert.deepEqual(plain(state.views.lighting.list.rows.map(r=>r.k)),ids);assert.equal(state.views.lighting.viewer.overlay,undefined);
}
const isolatedLight=fresh('lighting');assert.equal(sceneOps(isolatedLight,{lighting:{panels:{viewer:{kind:'viewer'}}}},event('set_light',{kind:'point'})),true);assert.equal(isolatedLight.views.scene3d,undefined);
const camera=fresh('scene3d');
for(const args of [{cam_x:4},{cam_roll:15},{}])assert.equal(sceneOps(camera,minimalScene,event('set_camera',args)),true);
assert.equal(camera.views.scene3d.info.blocks.find(r=>r.k==='Position').v,'X 4');
assert.equal(camera.views.scene3d.info.blocks.find(r=>r.k==='Rotation').v,'0, 0, 15');
for(const args of [{property:'cam_x',value:5},{property:'cam_y',value:6,time:1},{property:'cam_x',value:7,time:2},{property:'cam_z',value:'invalid'}])assert.equal(sceneOps(camera,minimalScene,event('set_camera_keyframe',args)),true);
assert.equal(camera.camKeys.length,3);assert.equal(camera.views.scene3d.keys.lanes.length,2);assert.equal(camera.views.scene3d.keys.playhead,2);
assert.equal(camera.views.scene3d.tree.items.filter(x=>x.icon==='cam').length,1);
for(const config of [{},{objects:'invalid',lights:[{}, {kind:'fill'}]},{objects:[{kind:'plane'}],lights:'invalid'}])assert.equal(sceneOps(camera,minimalScene,event('add_scene3d',{config})),Object.keys(config).length>0);
assert.ok(camera.views.scene3d.tree.items.some(x=>x.name==='light 1'));assert.ok(camera.views.scene3d.tree.items.some(x=>x.name==='fill 2'));
const mesh=fresh('modelling');
for(const [tool,args,name] of [['mesh_boolean',{},'boolean diff'],['mesh_boolean',{operation:'union'},'boolean union'],['generate_mesh_primitive',{primitive:'cube'},'cube'],['generate_mesh_primitive',{kind:'sphere'},'sphere'],['generate_mesh_primitive',{},'primitive'],['mesh_custom',{},'custom']]) {
 assert.equal(sceneOps(mesh,MODES,event(tool,args)),true);assert.equal(Object.values(mesh.views.modelling).find(v=>v.nodes).nodes.at(-1).name,name);
}
const noMaterial=fresh('lighting');noMaterial.views.lighting={info:{sub:'missing'}};
assert.equal(sceneOps(noMaterial,{lighting:{panels:{info:{kind:'kv'}}}},event('remove_material',{name:'missing'})),true);
assert.equal(noMaterial.views.lighting.info.sub,'');assert.deepEqual(plain(noMaterial.views.lighting.info.blocks),[{sec:'remove_material',_live:true},{k:'Name',v:'missing',_live:true,_flash:1000}]);
console.log('PASS sparse lighting, scene/camera records, default mesh names and material removal');
}

{
 const {summary,railArgs,assetNames,splitName,clipLabel,readCard,peek,route}=root.namespace;
 assert.equal(summary(null),'');assert.equal(summary(''),'');assert.equal(summary([1]),'1 item');
 assert.equal(summary(42),'42');assert.equal(summary('x'.repeat(40)),'x'.repeat(35)+'…');
 assert.equal(summary({ok:true,path:'/private',absent:null,build:'hash',samples:[1,2],gain:2}),'samples ×2, gain 2');
 for(const arg of [null,undefined,42,'text',[1]])assert.equal(railArgs({},arg),arg);
 const raw={clip_id:'clip123',asset_id:'asset123',track_id:'track123',gain:2};
 assert.deepEqual(plain(railArgs({},raw)),{gain:2});assert.equal(raw.clip_id,'clip123');
 assert.deepEqual(plain(railArgs({liveTL:{tracks:[]},assets:{}},raw)),{gain:2});
 assert.equal(splitName({},null),null);assert.equal(splitName({},''),'');
 assert.equal(clipLabel({}, {label:'V1'},'ingest'),'V1 · Clip 01');
 const state=fresh('edit');state.liveTL={tracks:[{id:'t1',label:'Video'}]};
 peek(state,MODES,{track_id:'t1',param:'gain'});assert.equal(state.liveTL.tracks[0]._peek,1000);
 peek(state,MODES,{track_id:'missing'});assert.equal(state.liveTL.tracks.length,1);
 assert.equal(readCard(state,MODES,'get_x',[]),false);assert.equal(readCard(fresh('unknown'),MODES,'get_x',[['X','1']]),false);
 const noPayload={tool_name:'import_media',call_id:'unpopulated',phase:'complete'};assert.equal(assetNames(noPayload,{}),noPayload);
 for(const bad of [null,{type:'tick'},{phase:'start'}])assert.equal(route(bad,state,MODES)[0],bad);
 let output=route({tool_name:'unknown_owned_tool',phase:'start',ts:42,ts_ms:100},state,MODES);
 assert.equal(output[0].id,'unknown_owned_tool@42');assert.equal(output[0].args,undefined);
 output=route({tool_name:'unknown_owned_tool',phase:'complete',ts:42,ts_ms:145},state,MODES);
 assert.equal(output[0].dur_ms,45);assert.equal(output[0].status,'ok');
 assert.equal(route({tool_name:'unknown_owned_tool',phase:'complete',duration_ms:7},state,MODES)[0].dur_ms,7);
 assert.equal(route({tool_name:'unknown_owned_tool',phase:'start'},state,MODES)[0].id,'unknown_owned_tool@');
 output=route({tool_name:'unknown_owned_tool',phase:'progress',payload:{done:3,total:4}},state,MODES);
 assert.deepEqual(plain(output[0].progress),{done:3,total:4});
 assert.equal(route({tool_name:'unknown_owned_tool',phase:'progress'},state,MODES)[0].progress.pct,null);
 output=route({tool_name:'normalize_loudness',phase:'complete',target:{param:'volume'},payload:{from:1,to:2.5}},fresh('mixing'),MODES);
 assert.equal(output[0].args.volume,'x1 → x2.5');
 const colors=fresh('color');route({tool_name:'false_color',phase:'complete'},colors,MODES);assert.equal(colors.views.color.viewer.falsecolor.split,.5);
 route({tool_name:'false_color',phase:'complete',payload:{split:.25}},colors,MODES);assert.equal(colors.views.color.viewer.falsecolor.split,.25);
 route({tool_name:'render_scope',call_id:'scope',phase:'start'},colors,MODES);assert.equal(colors._scopeCalls.scope,'scope');
 route({tool_name:'render_scope',call_id:'scope',phase:'complete'},colors,MODES);assert.equal(colors._scopeCalls.scope,'scope');
 const reading=fresh('edit');
 output=inspectOps(reading,MODES,{tool_name:'get_owned',phase:'error',duration_ms:12},'failed');assert.equal(output[0].status,'error');assert.equal(output[0].dur_ms,12);
 output=inspectOps(reading,MODES,{tool_name:'get_owned',phase:'complete'},'empty');assert.equal(output[0].status,'ok');
 console.log('PASS sparse read replies, private-ID redaction, track peeks, fallback call IDs, timing and landed volume');
}
{
 const st=fresh('scene3d');
 sc(st,'add_scene3d',{config:{objects:[{name:'Subject'}],lights:[{name:'Fill'}]}});
 sc(st,'set_camera',{cam_fov:40});
 for(let i=0;i<18;i++)sc(st,'set_light',{id:'Owned '+i,intensity:i});
 const tree=Object.values(st.views.scene3d).find(v=>v.items);
 assert.equal(tree.items.length,14);assert.equal(tree.items[0].icon,'cam');assert.deepEqual(plain(tree.items.slice(1).map(x=>x.name)),Array.from({length:13},(_,i)=>'Owned '+i));
 for(let i=0;i<30;i++)sc(st,'set_camera_key',{property:'cam_pan',value:i,time:i});
 assert.equal(st.camKeys.length,24);assert.equal(st.camKeys[0].t,6);assert.equal(st.camKeys.at(-1).t,29);
 const model=fresh('modelling');
 for(let i=0;i<10;i++)sc(model,'generate_mesh_primitive',{primitive:{kind:'sphere'+i}});
 const stack=Object.values(model.views.modelling).find(v=>v.nodes);
 assert.deepEqual(plain(stack.nodes.map(n=>n.name)),Array.from({length:7},(_,i)=>'sphere'+(i+3)));
 assert.equal(stack.nodes.filter(n=>n.em).length,1);
 const tr=fresh('tracking');
 sc(tr,'solve_camera',{}, {frame_rms:[.2,.7,1.2],solves:[{rms_px:.7}]});
 const keys=Object.values(tr.views.tracking).find(v=>v.graph);
 assert.deepEqual(plain(keys.graph.pts),[.2,.7,1.2]);assert.equal(keys.graph.max,2);assert.equal(tr.views.tracking.viewer.left,'solve 0.70 px RMS');
 keys.graph.thr=.5;
 sc(tr,'track_points',{}, {frame_rms:[1],residuals:['invalid',2],residuals_px:[.1,.2],solves:[]});
 assert.deepEqual(plain(keys.graph.pts),[.1,.2]);assert.equal(keys.graph.thr,.5);assert.equal(keys.graph.max,1);
 const pt=fresh('particles');sc(pt,'particle_simulate_raw',{steps:3});sc(pt,'particle_simulate_raw',{steps:7});
 const rows=Object.values(pt.views.particles).find(v=>v.rows).rows;
 assert.equal(rows.filter(r=>r._id==='raw').length,1);assert.equal(rows.find(r=>r._id==='raw').v,'7 steps');
 sc(pt,'add_emitter',{});assert.ok(Object.values(pt.views.particles).find(v=>v.rows).rows.some(r=>r.k==='emitter'&&r.v===''));
 const ph=sc(fresh('scene3d'),'simulate_physics',{bodies:[{x:0,y:0,clip_id:'body'}]});
 assert.equal(ph.view.overlay.at(-1).r,.02);
 console.log('PASS bounded scene history, camera keys and mesh stacks, tracking residual validation and raw simulation updates');
}
{
 // Forward-compatible layouts can retain viewer telemetry before their panels arrive.
 const unknown=fresh('future-layout');
 for(const [tool,args,result] of [
  ['add_emitter',{},{}],['set_light',{id:'light',pos:[.2,.3]},{}],
  ['set_material',{name:'owned',roughness:.5},{}],['mesh_subdivide',{},{}],
  ['set_camera',{cam_x:1},{}],['add_scene3d',{config:{objects:[{name:'owned'}]}},{}],
  ['track_point',{x:.2,y:.3},{frame_rms:[.1,.2]}],
 ]){const e=event(tool,args,'complete',result);sceneOps(unknown,MODES,e);assert.ok(unknown.views['future-layout'].viewer);}
 assert.ok(unknown.views['future-layout'].viewer.overlay.some(o=>o.cam?.cam_x===1));
 assert.ok(unknown.views['future-layout'].viewer.overlay.some(o=>o.pt?.[0]===.2));
 assert.equal(Object.values(unknown.views['future-layout']).some(v=>v.rows||v.blocks||v.nodes||v.lanes),false);
 const freshMask=fresh('roto');maskOps(freshMask,MODES,event('refine_matte',{clip_id:'owned'}));
 assert.equal(Object.values(freshMask.views.roto).find(v=>v.blocks).sub,'refine matte');
 for(const tool of ['track_mask','roto_propagate','remove_track_matte','remove_mask'])maskOps(fresh('roto'),MODES,event(tool));
 const feather=fresh('roto');maskOps(feather,MODES,event('add_mask',{points:[{x:0,y:0,fo:2},{x:1,y:1,fi:0}]}));
 assert.equal(Object.values(feather.views.roto).find(v=>v.blocks).blocks.find(b=>b.k==='Inner/outer feather').v,'1 pts');
 const spanning=fresh('roto');maskOps(spanning,MODES,event('set_mask_path_key',{time:0}));maskOps(spanning,MODES,event('set_mask_path_key',{time:4}));
 const sk=Object.values(spanning.views.roto).find(v=>v.lanes),bounds={...sk.ruler};
 maskOps(spanning,MODES,event('set_track_matte'));
 const area=sk.lanes.find(l=>l.label==='track matte').area;
 assert.equal(area[0][0],bounds.t0+(bounds.t1-bounds.t0)*.06);assert.equal(area[1][0],bounds.t1-(bounds.t1-bounds.t0)*.06);
 const removed=fresh('roto');removed.views.roto={viewer:{overlay:[{unrelated:true}]}};maskOps(removed,MODES,event('remove_mask'));assert.deepEqual(plain(removed.views.roto.viewer.overlay),[{unrelated:true}]);
 console.log('PASS future layouts retain geometry, absent masks remain harmless and matte spans use existing key bounds');
}

{
 const {route,callSection,xfOps,COVB_H}=root.namespace;
 const owned=fresh('captions');owned.liveTL={tracks:[{id:'t',clips:[{id:'clip1',label:'Owned title'}]}]};
 assert.equal(callSection(owned,MODES,event('set_text_weight',{clip_id:'clip1',weight:600})),true);
 assert.deepEqual(plain(owned.liveTL.tracks[0].clips[0].fx),['w600']);
 assert.equal(callSection(owned,MODES,event('set_text_weight',{clip_id:'clip1',weight:700})),true);
 const kv=Object.values(owned.views.captions).find(v=>v.blocks);
 assert.equal(kv.blocks.filter(b=>b.sec==='set_text_weight').length,1);assert.equal(kv.blocks.find(b=>b.k==='Weight').v,'700');
 const sparse=fresh('edit');
 xfOps(sparse,MODES,{tool_name:'set_transform',phase:'start'});
 xfOps(sparse,MODES,event('set_property',{property:'x',value:12}));assert.equal(sparse.xform._.x,12);
 xfOps(sparse,MODES,event('set_motion_blur',{samples:8,shutter_curve:'custom'}));assert.ok(sparse.gizmo.read.includes('samples 8'));assert.ok(sparse.gizmo.read.includes('curve custom'));
 xfOps(sparse,MODES,event('set_motion_blur',{}));
 assert.equal(COVB_H.gamut('unknown-owned-space'),'linear');assert.equal(COVB_H.clipObj({},'clip1'),null);assert.equal(COVB_H.clipObj({liveTL:{tracks:[]}},'clip1'),null);
 assert.equal(graphicsOps(fresh('future-layout'),MODES,event('add_hud_frame')),true);
 const graphic=fresh('graphics');route(event('add_hud_frame'),graphic,MODES);assert.equal(graphic.views.graphics.viewer.overlay.at(-1).label,'BOXES');
 for(const tool of ['new_project','set_track'])route({tool_name:tool,call_id:tool,phase:'start'},fresh('edit'),MODES);
 const color=fresh('grading');route(event('render_display_scope',{targets_75:true},'start'),color,MODES);route(event('grade',{},'progress',{ }),color,MODES);
 const explicit=fresh('grading');route({tool_name:'grade',phase:'progress',payload:{pct:25}},explicit,MODES);assert.equal(explicit.views.grading.viewer.wipe,.25);
 const exported=fresh('export');const start=event('export',{},'start');route(start,exported,MODES);
 const failed=route({...start,phase:'error',payload:{error:'owned failure'}},exported,MODES);assert.equal(failed[0].status,'error');
 assert.equal(exported.views.export.viewer.progress.halt,true);
 const t={tracks:[{id:'t',clips:[{id:'c',s:0,e:1}]}]},s=fresh('edit');
 editOps2(s,MODES,t,event('project_set',{width:640,height:360}),{width:640,height:360});assert.equal(s.views.edit.viewer.res,'640 x 360');
 for(const [tool,args] of [['set_speed_ramp',{}],['set_speed_ramp',{keys:[[0,1]]}],['set_time_remap',{keys:[[0,1]]}],['set_flow_blur',{mode:'vector',shutter_curve:'custom'}]]){
  delete t.tracks[0].clips[0].fx;delete t.tracks[0].clips[0].keys;
  assert.equal(editOps2(s,MODES,t,event(tool,{clip_id:'c',...args}),{clip_id:'c',...args}),true);
 }
 assert.ok(t.tracks[0].clips[0].fx.includes('flow blur vector · curve custom'));
 console.log('PASS replaceable text summaries, sparse transforms, explicit scope progress and failed export feedback');
}
{
 assert.deepEqual(plain(root.namespace.callDiff({}, {payload:{changed:{clips_added:[{clip_id:'Owned human title'}]}}})),[{k:'Clip',from:'—',v:'Owned human title'}]);
 mix('eq',{args:{frequency:110,g:4}});assert.equal(eqFixture.bands[0].f,110);assert.equal(eqFixture.bands[0].g,4);assert.equal(eqFixture.bands[1].g,3);
 const state=fresh('edit'),saved={tracks:[{id:'preserved',clips:[]}]};
 const other=Object.entries(MODES).filter(([name])=>name!=='edit').flatMap(([name,m])=>Object.entries(m.panels).filter(([,p])=>p.kind==='timeline').map(([key])=>[name,key]))[0];
 state.views[other[0]]={[other[1]]:saved};root.namespace.timelineView(state,MODES);assert.equal(state.views[other[0]][other[1]],saved);
 const ex=fresh('export');exportOps(ex,MODES,event('export',{},'start'));
 ex.views.export.viewer.progress={pct:42};exportOps(ex,MODES,event('resume_job',{},'start'));
 assert.equal(ex.views.export.viewer.progress.pct,42);
 exportOps(ex,MODES,event('cancel_job',{},'complete'));
 assert.equal(Object.values(ex.views.export).find(v=>v.rows).rows[0][3],'✕ cancelled');
 const material=fresh('lighting');assert.equal(sceneOps(material,MODES,event('remove_material',{name:'missing'})),true);
 assert.equal(Object.values(material.views.lighting).find(v=>v.blocks)?.sub,undefined);
 const lights=fresh('lighting');sceneOps(lights,MODES,event('remove_light',{id:'unknown'}));assert.deepEqual(plain(lights.views.lighting.viewer.overlay),[]);
 const rigEntry=Object.entries(MODES.lighting.panels).find(([,p])=>p.kind==='lightrig')[0];
 lights.views.lighting[rigEntry]={lights:[{name:'short',_id:'owned'},{name:'kept',_id:'other'}]};
 sceneOps(lights,MODES,event('remove_light',{id:'owned'}));assert.deepEqual(plain(lights.views.lighting[rigEntry].lights),[{name:'kept',_id:'other'}]);
 const unknown=fresh('future-layout');sceneOps(unknown,MODES,event('particle_simulate_raw',{steps:5}));assert.equal(Object.values(unknown.views['future-layout']).some(v=>v.rows),false);
 console.log('PASS human clip names, nearest EQ band, preserved populated timelines, resumed progress and sparse light removal');
}
