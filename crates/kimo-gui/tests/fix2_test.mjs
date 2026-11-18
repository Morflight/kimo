import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {dirname, resolve} from 'node:path';
import vm from 'node:vm';
const directory = fileURLToPath(new URL('../ui/', import.meta.url));
const context = vm.createContext({performance:{now:()=>1000}}), modules = new Map();
function get(filename) {
  if (!modules.has(filename)) {
    // Expose two pure private helpers after the complete, unchanged source.
    // Production functions keep their original filename and character offsets.
    const access = filename === resolve(directory,'fix2.js') ? '\nexport { pxs as testPixels, outlinerEntry as testOutliner };' : '';
    modules.set(filename,new vm.SourceTextModule(readFileSync(filename,'utf8') + access,{identifier:filename,context}));
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
const {fix2Pre,fix2Post,gatedFeatures}=get(resolve(directory,'fix2.js')).namespace;
const plain=x=>JSON.parse(JSON.stringify(x));
const fresh=(mode='tracking')=>({mode,explicitMode:true,views:{},preview:{},session:{width:640,height:360}});
const ev=(tool,result={},args={},changed,phase='complete')=>({tool_name:tool,phase,call_id:'fixture',payload:{result,args,changed}});
const view=s=>s.views.tracking.viewer;
assert.deepEqual(plain(gatedFeatures(null)),[]);
assert.deepEqual(plain(gatedFeatures('mesh-import missing')),[]);
assert.deepEqual(plain(gatedFeatures('requires mesh-import and gpu3d features')),['mesh-import','gpu3d']);
const refusal=fresh(), error=ev('generate_mesh_primitive',{}, {},undefined,'error');error.payload.error='requires mesh-import';
fix2Pre(refusal,MODES,error);assert.equal(fix2Post(refusal,MODES,error),true);
assert.equal(view(refusal).tags[0].text,'feature not compiled: mesh-import');
fix2Pre(refusal,MODES,ev('next',{}, {},undefined,'start'));assert.deepEqual(plain(view(refusal).tags),[]);
for (const [tool,result,text] of [
 ['track_features',{features:4,survived_all_frames:3,frames:9},'features · 4 found · 3 survived · 9 frames'],
 ['fit_track_homography',{summary:{fit_pooled_frame_median_px:0.001,fit_worst_frame_p95_px:2,held_out_features:1}},'fit · median 1.0e-3 px · worst p95 2 px · 1 held out'],
 ['decompose_track_homography',{point_px:[320,180],summary:{rotation_deg:{last:1.2345},scale_major:{last:2},orientation_flip_frames:[]}},'decompose · rot 1.235° · scale 2 · 0 flips'],
 ['ecc_refine_track',{refined:3,corner_move_px:{max:2},failed_frames:[1]},'ecc · 3/? refined · corner move max 2 px · 1 failed'],
 ['corner_disagreement',{per_corner:[{corner:0,max:1},null,{}],worst_frames:[{frame:7}]},'corners · max 1 px · worst frame 7'],
 ['lk_residual_audit',{features_detected:4,median_of_frame_medians_px:0,worst_p95_px:1},'lk audit · 4 features · median 0 px · worst p95 1 px'],
 ['track_planar_anchored',{frames:3,gaps:[null,{needs_anchor:true,suggest_anchor_frame:2,forward_closure_px:1}]},'anchored · 3 frames · 1 gap needs an anchor · try frame 2 · closure 1 px'],
 ['validate_planar_track',{flagged_frames:1,disagreement_px:{max:2},untested_frames:3},'validate · 1/? flagged · max 2 px · 3 untested'],
 ['track_feature_residuals',{frame:2,compared:1,set:'heldout',largest:[{tracked:[320,180],predicted:[288,162]},{}],length_stats_px:{max:1}},'residuals · frame 2 · 1 held out · max 1 px · vectors ×20'],
 ['track_jitter_metrics',{fit:{accel_px:{p95:1},step_px:{max:2},per_corner:[{corner:0,accel_p95:2}]}},'jitter · accel p95 1 px · step max 2 px'],
 ['track_long_gap',{analysis_size:[320,180],samples:[{state:'visible',bbox:{x:32,y:18,w:64,h:36}},{state:'lost',bbox:{x:0,y:0,w:32,h:18}},'…+4']},'long gap · 6 samples · 1/2 listed visible · 1 lost'],
]) {
 const s=fresh();assert.equal(fix2Post(s,MODES,ev(tool,result,{corners:[0,0,1,0,1,1,0,1]})),true,tool);
 assert.equal(view(s).tags[0].text,text,tool);
 assert.equal(view(s).overlay[0]._kind,'b2track');
 if(tool==='track_feature_residuals')assert.deepEqual(plain(view(s).overlay[0].pts),[{p:[0.5,0.5],v:[0.9999999999999998,0.9999999999999998]}]);
 if(tool==='track_long_gap')assert.deepEqual(plain(view(s).overlay[0].boxes),[[0.1,0.1,0.2,0.2],[0,0,0.1,0.1]]);
 const sparse=fresh();assert.equal(fix2Post(sparse,MODES,ev(tool)),true);assert.ok(view(sparse).tags[0].text.length>0);
}
console.log('PASS tracking overlays retain measured units, normalized positions and explicit missing values');
for(const [tool,args,result,row] of [
 ['generate_mesh_primitive',{primitive:{kind:'cube'}},{geometry:{vertices:[1,2,'…+3'],indices:[]}},['Mesh','cube · 5 v · 0 t']],
 ['mesh_boolean',{op:'union'},{inspection:{vertices:8,triangles:12,volume:1.25}},['Mesh','boolean union · 8 v · 12 t · vol 1.25']],
 ['mesh_extrude_path',{}, {geometry:{vertices:[],indices:[]}},['Mesh','extrude path · 0 v · 0 t']],
 ['mesh_lathe',{segments:4},{},['Mesh','lathe · 4 seg']],
 ['mesh_subdivide',{}, {inspection:{final_vertices:9,final_triangles:8}},['Mesh','subdivided · 9 v · 8 t']],
 ['derive_vertex_tangents',{}, {vertices:[1],indices:[1]},['Mesh','tangents · 1 v · 1 t']],
 ['contour_sdf_graph',{}, {resolution:8,stream:{vertices:[1]}},['Mesh','SDF contour · res 8 · 1 v']],
 ['remove_material',{name:'Gold'},{},['Material','Gold removed']],
 ['apply_camera_solve',{}, {keys:2,span:[1,3]},['Camera','solve applied · 2 keys · 2 s']],
 ['sample_scene3d_animation',{}, {nodes_animated:1,camera_animated:false,lights_animated:2},['Animation','1 node · camera still · 2 lights']],
 ['inspect_mesh_buffers',{}, {vertices:4,triangles:2},['Mesh','buffers · 4 v · 2 t']],
 ['inspect_mesh_primitive',{geometry:{primitive:{kind:'sphere'}}},{vertex_count:12,triangle_count:8,stride_bytes:32},['Mesh','sphere · 12 v · 8 t · 32 B/v']],
 ['particle_simulate_raw',{}, {states:[null,{position:[0,1.25]},'…+2']},['Particles','4 states · y 1.25']],
]) {
 const s=fresh();assert.equal(fix2Post(s,MODES,ev(tool,result,args)),true,tool);assert.deepEqual(plain(s._outl[0]),row,tool);
 for(let i=0;i<5;i++)fix2Post(s,MODES,ev(tool,result,args));assert.equal(s._outl.length,4);
}
console.log('PASS scene outliner reports geometry counts and caps history at four entries');
const graph=fresh('color');
function change(changed,args={clip_id:'clip1'}) {fix2Post(graph,MODES,ev('set_grade_node',{},args,{clip:changed}));}
change({'grade_graph.nodes.a':[null,{kind:'grade',inputs:[],effect:{kind:'color_grade'}}],'grade_graph.output':['input','a']});
assert.equal(graph.preview.refused,'incomplete reachable node · color grade');assert.equal(graph.preview.refusedSrc,'inferred');
change({'grade_graph.nodes.a.inputs':[[],['input']]});assert.equal(graph.preview.refused,null);
change({'grade_graph.nodes.a.kind':['grade','parallel']});assert.equal(graph.preview.refused,'incomplete reachable node · color grade');
change({'grade_graph.nodes.a.effect.kind':['color_grade',null],'grade_graph.nodes.a.inputs':[[],['input','input']]});assert.equal(graph.preview.refused,null);
change({'grade_graph.nodes.a':[{},null]});assert.deepEqual(plain(graph._gg.clip1.nodes),{});
change({'grade_graph':[{},null]});assert.equal(graph._gg.clip1.output,'input');
const mask=fresh();fix2Post(mask,MODES,ev('roto_propagate',{keys:2,frames_filled:3,mean_snap_px:0.1},{},{clip:{'mask.path_keys':[[],[{points:[{x:0,y:0},{x:1,y:0},{x:1,y:1},null]}]]}}));
assert.deepEqual(plain(view(mask).overlay[0].path),[[0,0],[1,0],[1,1]]);assert.equal(view(mask).overlay[0].closed,true);
assert.equal(view(mask).tags[0].text,'mask propagated · 2 keys · 3 frames filled · snap 0.1 px');
fix2Post(mask,MODES,ev('roto_propagate'));assert.deepEqual(plain(view(mask).overlay),[]);
console.log('PASS graph completion clears refusal and mask propagation replaces its prior overlay');
const media=fresh('batch');
fix2Post(media,MODES,ev('relink_media',{applied:false,would_relink:2,not_found:1,ambiguous:1,items:[{old_path:'/private/old.mov',new_path:'/new/found.mov'},null,{}]}));
assert.deepEqual(plain(media.views.batch.media.items.map(x=>[x.name,x.note])),[['relink · dry run','2 would relink · 1 not found · 1 ambiguous'],['old.mov','found.mov'],['media','']]);
assert.equal(media.views.batch.media.foot,'relink dry run · 2 would relink');
fix2Post(media,MODES,ev('relink_media',{no_history:true}));assert.equal(media.views.batch.media.foot,'relink applied · nothing missing');
const seq=fresh('export');
fix2Post(seq,MODES,ev('render_scene3d_sequence',{count:1,dir:'/home/private/exports/frames',frames:[{render:{backend:'cpu'}}]},{width:320,height:180}));
assert.deepEqual(plain(seq.views.export.renders.rows[0]),['frames','scene3d 320x180 cpu','1 frame','✓ done','now']);
assert.equal(seq.views.export.renders.rows[0]._path,'~/exports/frames');
fix2Post(seq,MODES,ev('render_scene3d_sequence',{}, {height:180},undefined,'error'));
assert.equal(seq.views.export.renders.rows.length,1);
assert.deepEqual(plain(seq.views.export.renders.rows[0]),['scene3d sequence','scene3d 180 px','-','✕ failed','now']);
const template=fresh('batch');
assert.equal(fix2Post(template,MODES,ev('template_describe',{name:'Opening',ops:[{tool:'add_clip'},{tool:'add_clip'},null],params:[{name:'title',default:'Hello'},{name:'size'}]})),true);
const diff=Object.values(template.views.batch).find(x=>x._ctx==='templates');
assert.deepEqual(plain(diff.diff),[['=','template Opening · 2 ops · add_clip · title = Hello, size']]);
assert.equal(fix2Post(fresh(),MODES,ev('template_describe',null)),false);
assert.equal(fix2Post(fresh(),MODES,ev('unknown')),false);
const deferred=fresh();fix2Post(deferred,MODES,ev('track_features',{}, {corners:[0,0,1,0,1,1,0,1]},undefined,'start'));
fix2Post(deferred,MODES,ev('track_features',{features:1}));
assert.deepEqual(plain(view(deferred).overlay[0].quad),[[0,0],[1,0],[1,1],[0,1]]);
assert.deepEqual(plain(deferred._b2Args),{});
console.log('PASS media relinks, sequence failures and template descriptions preserve their visible outcomes');
for(const [tool,row] of [
 ['generate_mesh_primitive',['Mesh','primitive']],['mesh_boolean',['Mesh','boolean']],
 ['mesh_extrude_path',['Mesh','extrude path']],['mesh_lathe',['Mesh','lathe']],
 ['mesh_subdivide',['Mesh','subdivided']],['derive_vertex_tangents',['Mesh','tangents']],
 ['contour_sdf_graph',['Mesh','SDF contour']],['remove_material',['Material','removed']],
 ['apply_camera_solve',['Camera','solve applied']],['inspect_mesh_buffers',['Mesh','buffers']],
 ['inspect_mesh_primitive',['Mesh','primitive']],['particle_simulate_raw',['Particles','0 states']],
]) {
 const s=fresh();assert.equal(fix2Post(s,MODES,{tool_name:tool,phase:'complete'}),true);assert.deepEqual(plain(s._outl[0]),row);
}
for(const [args,result,expected] of [
 [{},{geometry:{primitive:{kind:'cone'}}},'cone'], [{},{primitive:{kind:'torus'}},'torus'],
 [{primitive:{kind:'obj1'}},{},'primitive'],
]) {const s=fresh();fix2Post(s,MODES,ev('generate_mesh_primitive',result,args));assert.equal(s._outl[0][1],expected);}
for(const [result,expected] of [[{nodes_animated:2,camera_animated:true},'2 nodes · camera moves'],[{camera_animated:false},'camera still']]) {
 const s=fresh();assert.equal(fix2Post(s,MODES,ev('sample_scene3d_animation',result)),true);assert.equal(s._outl[0][1],expected);
}
assert.equal(fix2Post(fresh(),MODES,ev('sample_scene3d_animation')),false);
const px=fresh();delete px.session;
fix2Post(px,MODES,ev('track_features',{corners0_px:[0,0,640,0,640,360,0,360]}));
assert.deepEqual(plain(view(px).overlay[0].quad),[[0,0],[1,0],[1,1],[0,1]]);
const anchored=fresh();fix2Post(anchored,MODES,ev('track_features',{}, {anchors:[{corners:[0,0,1,0,1,1,0,1]}]}));
assert.deepEqual(plain(view(anchored).overlay[0].quad),[[0,0],[1,0],[1,1],[0,1]]);
const gap=fresh();fix2Post(gap,MODES,ev('track_long_gap',{analysis_size:[0,0],samples:[{state:'seed',bbox:{x:64,y:36,w:128,h:72}}]}));
assert.deepEqual(plain(view(gap).overlay[0].boxes),[[0.1,0.1,0.2,0.2]]);
const residual=fresh();fix2Post(residual,MODES,ev('track_feature_residuals',{compared:1,largest:[{p0:[64,36]}]}));
assert.deepEqual(plain(view(residual).overlay[0].pts),[{p:[0.1,0.1],v:null}]);assert.equal(view(residual).tags[0].text,'residuals · 1 compared · vectors ×20');
const empty=fresh();fix2Post(empty,MODES,{tool_name:'track_features',phase:'complete'});assert.equal(view(empty).tags[0].text,'features');
for(const tool of ['track_features','roto_propagate','relink_media','render_scene3d_sequence','template_describe']) {
 assert.equal(fix2Post(fresh('absent'),{},ev(tool)),false,`${tool} missing panel`);
}
const noPanels={tracking:{panels:{}},export:{panels:{}}};
assert.equal(fix2Post(fresh(),noPanels,ev('track_features')),false);
assert.equal(fix2Post(fresh(),noPanels,ev('render_scene3d_sequence')),false);
const fallback=fresh('absent');fallback.explicitMode=false;fix2Post(fallback,MODES,ev('track_features'));assert.equal(view(fallback).tags[0].text,'features');
const gated=fresh(), gatedEv=ev('unknown',{}, {},undefined,'error');gatedEv._gated='feature not compiled: gpu3d';
assert.equal(fix2Post(gated,{tracking:{panels:{info:{kind:'kv'}}}},gatedEv),true);
assert.deepEqual(plain(gated.views.tracking.info.read.rows),[['Refused','feature not compiled: gpu3d']]);
fix2Pre({},MODES,{phase:'start'});fix2Pre({views:{empty:null}},MODES,{phase:'start'});fix2Pre({},MODES,{phase:'error'});
const ignored=fresh();fix2Post(ignored,MODES,ev('unknown',{}, {},{clip:{'grade_graph.output':['input','x']}}));assert.equal(ignored._gg,undefined);
const sparseGraph=fresh('color');
fix2Post(sparseGraph,MODES,{tool_name:'set_grade_node',phase:'complete',target:{clip_id:'clip2'},payload:{changed:{clip:{'grade_graph.nodes.x.kind':[null,'grade'],'grade_graph.output':['input','x']}}}});
assert.equal(sparseGraph.preview.refused,'incomplete reachable node · grade');
fix2Post(sparseGraph,MODES,ev('set_grade_node',{}, {clip_id:'clip2'},{clip:{'grade_graph.nodes.x.inputs':[[],7],'grade_graph.nodes.x.kind':['grade',null]}}));
assert.equal(sparseGraph.preview.refused,'incomplete reachable node · mixer');
const many=fresh('export');for(let i=0;i<8;i++){const e=ev('render_scene3d_sequence',{frames:[{},{}]});e.call_id=String(i);fix2Post(many,MODES,e);}
assert.equal(many.views.export.renders.rows.length,6);assert.equal(many.views.export.renders.rows[0][2],'2 frames');
const simple=fresh();assert.equal(fix2Post(simple,MODES,ev('template_describe',{op_count:1})),true);
const read=Object.values(simple.views.tracking).find(x=>x.read).read;
assert.deepEqual(plain(read.rows),[['Template','template'],['Ops','1 op'],['Params','none']]);
console.log('PASS optional payloads, missing panels, tracker fallback and unnamed graph nodes stay explicit');

const {testPixels,testOutliner}=get(resolve(directory,'fix2.js')).namespace;
assert.equal(testPixels(null),null);assert.equal(testPixels(NaN),null);
assert.equal(testPixels(0),'0 px');assert.equal(testPixels(0.001),'1.0e-3 px');
assert.equal(testOutliner('unknown',{},{}),null);
console.log('PASS pixel formatting rejects non-measurements and unknown outliner tools have no entry');
const chips=fresh();chips.views.tracking={viewer:{tags:[{text:'kept'},{text:'positioned',pos:[1,1]}]}};
fix2Post(chips,MODES,ev('track_planar_anchored',{gaps:[{needs_anchor:false}]}));
assert.equal(view(chips).tags.at(-1).y,44);assert.equal(view(chips).tags.at(-1).text,'anchored · 0 gaps need an anchor · closure 0 px');
const camera=fresh();fix2Post(camera,MODES,ev('apply_camera_solve',{span:[null,3]}));assert.equal(camera._outl[0][1],'solve applied · 3 s');
const malformedMask=fresh();fix2Post(malformedMask,MODES,{tool_name:'roto_propagate',phase:'complete'});
assert.equal(view(malformedMask).tags[0].text,'mask propagated');
fix2Post(malformedMask,MODES,ev('roto_propagate',null,{}, {clip:{unrelated:[1,2],path_keys:[[],3]}}));assert.deepEqual(plain(view(malformedMask).overlay),[]);
const g2=fresh('color');g2._gg={clip1:{nodes:{a:{kind:'grade'}},output:'a'}};
fix2Post(g2,MODES,ev('graph',{}, {clip_id:'clip1'},{clip:{'grade_graph.extra':true}}));assert.equal(g2.preview.refused,'incomplete reachable node · grade');
fix2Post(g2,MODES,ev('graph',{}, {clip_id:'clip1'},{clip:{'grade_graph.nodes.a':[null,{kind:'grade'}]}}));assert.deepEqual(plain(g2._gg.clip1.nodes.a.inputs),[]);
fix2Post(g2,MODES,ev('graph',{}, {clip_id:'clip1'},undefined,'start'));
fix2Post(g2,MODES,{tool_name:'graph',phase:'complete',call_id:'fixture',payload:{changed:{clip:{'grade_graph.output':['a','input']}}}});assert.equal(g2.preview.refused,null);
fix2Post(fresh(),MODES,{tool_name:'graph',phase:'complete',payload:{changed:{clip:{'grade_graph.output':['a','input']}}}});
for(const tool of ['relink_media','render_scene3d_sequence']) {
 const s=fresh('batch');assert.equal(fix2Post(s,MODES,{tool_name:tool,phase:'start'}),false);
 assert.equal(fix2Post(s,MODES,{tool_name:tool,phase:'complete'}),true);
 const value=tool==='relink_media'?s.views.batch.media.foot:s.views.export.renders.rows[0][3];
 assert.equal(value,tool==='relink_media'?'relink applied · 0 relinked':'✓ done');
 assert.equal(fix2Post(fresh('batch'),MODES,ev(tool,null)),true);
}
const partialTemplate=fresh('batch');partialTemplate.views.batch={diff:{_ctx:'templates'}};
// Use the actual mode's diff panel key when retaining a partially initialized view.
const diffKey=Object.entries(MODES.batch.panels).find(([,p])=>p.kind==='diffbatch')[0];partialTemplate.views.batch[diffKey]={_ctx:'templates'};
fix2Post(partialTemplate,MODES,ev('template_describe',{ops:[{}]}));
assert.deepEqual(plain(partialTemplate.views.batch[diffKey].diff),[['=','template template · 1 op ·  · none']]);
assert.equal(fix2Post(fresh(),MODES,{tool_name:'template_describe',phase:'complete'}),false);
console.log('PASS partial views retain chips, default counts and cached call arguments without stale graph refusals');
for(const tool of ['relink_media','render_scene3d_sequence']) {
 const s=fresh('batch');assert.equal(fix2Post(s,MODES,{tool_name:tool,phase:'complete'}),true);
 assert.equal(tool==='relink_media'?s.views.batch.media.foot:s.views.export.renders.rows[0][3],tool==='relink_media'?'relink applied · 0 relinked':'✓ done');
}
