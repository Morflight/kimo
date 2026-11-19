import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {dirname,resolve} from 'node:path';
import vm from 'node:vm';
const directory=fileURLToPath(new URL('../ui/',import.meta.url)), modules=new Map();
const context=vm.createContext({performance:{now:()=>1000}});
function get(filename){if(!modules.has(filename))modules.set(filename,new vm.SourceTextModule(readFileSync(filename,'utf8')+(filename===resolve(directory,'router.js')?'\nexport {gradeOps,graphicsOps,maskOps,sceneOps,editOps2};':''),{identifier:filename,context}));return modules.get(filename);}
const root=get(resolve(directory,'router.js'));await root.link((name,parent)=>get(resolve(dirname(parent.identifier),name)));await root.evaluate();
const {gradeOps,graphicsOps,maskOps,sceneOps,editOps2}=root.namespace;
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
