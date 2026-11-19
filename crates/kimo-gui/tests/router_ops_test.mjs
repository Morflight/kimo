import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {dirname,resolve} from 'node:path';
import vm from 'node:vm';
const directory=fileURLToPath(new URL('../ui/',import.meta.url)), modules=new Map();
const context=vm.createContext({performance:{now:()=>1000}});
function get(filename){if(!modules.has(filename))modules.set(filename,new vm.SourceTextModule(readFileSync(filename,'utf8')+(filename===resolve(directory,'router.js')?'\nexport {gradeOps,graphicsOps,maskOps,sceneOps,editOps2,timelineOps,timelineOps1,exportOps,inspectOps,inspectorDiff,callDiff};':''),{identifier:filename,context}));return modules.get(filename);}
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
