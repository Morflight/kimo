import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {dirname, resolve} from 'node:path';
import vm from 'node:vm';
const directory = fileURLToPath(new URL('../ui/', import.meta.url));
const context = vm.createContext({performance:{now:()=>1000}}), modules = new Map();
function get(filename) {
  if (!modules.has(filename)) modules.set(filename,new vm.SourceTextModule(readFileSync(filename,'utf8')+(filename===resolve(directory,'project_ops.js')?'\nexport {working,nits,bytes,panel,pipeRows,setPipe,histPush,diffLines,batchTable,mediaFoot,pipeApply,tlApply,layerClips,metaOps,qualityLine,side};':''),{identifier:filename,context}));
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
const {space,pipelineOps,cPush,qaOps,projectOps,cSnap,cRestore,working,nits,bytes,panel,pipeRows,setPipe,histPush,diffLines,batchTable,mediaFoot,pipeApply,tlApply,layerClips,metaOps,qualityLine,side}=get(resolve(directory,'project_ops.js')).namespace;
const plain=x=>JSON.parse(JSON.stringify(x));
for(const [g,t,expected] of [[3,0,'ACEScg'],[3,3,'ACEScct'],[0,1,'Rec.709 sRGB'],[0,8,'Rec.709 BT.709'],
  [1,6,'Rec.2100 PQ'],[1,7,'Rec.2100 HLG'],[5,4,'ARRI LogC3'],[6,5,'S-Log3'],[7,5,'S-Log3 Cine'],
  [2,2,'P3-D65 gamma 2.4'],[null,null,null],[99,99,null],[99,1,'sRGB'],[2,null,'P3-D65']]) {
  assert.equal(space(g,t),expected,`${g}/${t}`);
}
const state={mode:'color',views:{}};
const event=(tool,phase='complete',args={},result={})=>({tool_name:tool,phase,call_id:'test',payload:{args,result}});
assert.equal(pipelineOps(state,MODES,event('set_color_pipeline','queued')),false);
assert.equal(pipelineOps(state,MODES,event('set_color_pipeline','start',{settings:{input_gamut:0,input_transfer:8,working_gamut:3,output_gamut:1,output_transfer:6}})),false);
assert.equal(pipelineOps(state,MODES,event('set_color_pipeline')),true);
assert.equal(state._pipe.input,'Rec.709 BT.709');
assert.equal(state._pipe.working,'ACEScg');
assert.equal(state._pipe.output,'Rec.2100 PQ');
assert.deepEqual(plain(state._covArgs),{});
const before=plain(state);
assert.equal(pipelineOps(state,MODES,event('set_hdr_mastering','error',{metadata:{max_nits:999}})),false);
assert.deepEqual(plain(state),before);
assert.equal(pipelineOps(state,MODES,event('set_color_pipeline','complete',{settings:{enabled:false}})),true);
assert.equal(state._pipe.working,'off (display-referred)');
assert.equal(pipelineOps(state,MODES,event('set_color_pipeline','complete',{settings:{working_gamut:99}})),true);
assert.equal(state._pipe.working,'gamut 99 linear');
assert.equal(pipelineOps(state,MODES,event('set_clip_input_color','complete',{clip_id:'@Hero.mp4',gamut:3,transfer:0})),true);
assert.equal(state._pipe.clips.Hero,'ACEScg');
assert.equal(pipelineOps(state,MODES,event('set_clip_input_color','complete',{clip_id:'@Hero.mp4',gamut:3,transfer:3})),true);
assert.equal(state._pipe.clips.Hero,'ACEScct');
assert.equal(pipelineOps(state,MODES,event('set_clip_input_color')),false);
assert.equal(pipelineOps({views:{}},MODES,event('set_hdr_mastering','complete',{clear:true})),false);
pipelineOps(state,MODES,event('set_hdr_mastering','complete',{metadata:{max_cll:1000,max_fall:400,max_nits:1200,min_nits:0.0014}}));
assert.deepEqual(plain(state._pipe.hdr),{max_cll:'1000 nits',max_fall:'400 nits',max_nits:'1200 nits',min_nits:'0.001 nits'});
pipelineOps(state,MODES,event('set_hdr_mastering','complete',{clear:true}));
assert.deepEqual(plain(state._pipe.hdr),{max_cll:'cleared',max_fall:'cleared',max_nits:'cleared',min_nits:'cleared'});
const error=event('set_ocio_view_delivery','error',{selection:{view:'AgX'}});
error.payload.error='native-ocio unavailable';
assert.equal(pipelineOps(state,MODES,error),true);
assert.equal(state._pipe.view,'✗ AgX');
assert.equal(state._pipe._viewErr,true);
assert.match(error._hudErr,/engine-disabled/);
assert.equal(pipelineOps(state,MODES,event('set_ocio_view_delivery','complete',{selection:{display:'Monitor',output_gamut:0,output_transfer:1}})),true);
assert.equal(state._pipe.view,'Monitor');
assert.equal(state._pipe._viewErr,false);
assert.equal(state._pipe.output,'Rec.709 sRGB');
assert.equal(pipelineOps(state,MODES,event('set_ocio_view_delivery')),false);
assert.equal(pipelineOps(state,MODES,event('unknown')),false);
assert.equal(pipelineOps({views:{}},{},event('set_color_pipeline','complete',{settings:{enabled:false}})),true);
console.log('PASS color pipeline defaults, refused calls, clip overrides, HDR clearing and OCIO recovery');
const history={};
cPush(history,{before:true},{after:true},'trim_clip',{});
assert.deepEqual(plain(history._covHist),{undo:[],redo:[]});
history._covHist.undo=[null];
cPush(history,'before','after','trim_clip',{});
assert.deepEqual(plain(history._covHist.undo),[{tool:'trim_clip',c:true,before:'before',after:'after'}]);
cPush(history,'ignored','ignored','move_clip',{});
assert.equal(history._covHist.undo[0].tool,'trim_clip');
cPush(history,'before','after','load_project',{undo_steps:100});
assert.equal(history._covHist.undo.length,48);
assert.ok(history._covHist.undo.slice(0,47).every(x=>x===null));
assert.equal(history._covHist.undo[47].tool,'load_project');
assert.deepEqual(plain(history._covHist.redo),[]);
cPush(history,null,null,'load_project',null);
assert.equal(history._covHist.undo.length,1);
console.log('PASS history associates only the pending slot and bounds unseen loaded steps');
const fresh=(mode='graph')=>({mode,explicitMode:true,views:{},liveTL:{tracks:[{id:'v1',label:'Video',clips:[{id:'c1',label:'Hero.mp4',s:0,e:10}]}]}});
const qa=(tool,args={},result={},payload={})=>{const e=event(tool,'complete',args,result);Object.assign(e.payload,payload);const ok=qaOps(fresh(),MODES,e);return {e,ok};};
for(const tool of ['diff_frames','compare_ranges','flash_series','audio_spectrum','cache_set_budget','check_code_clip_determinism','unknown'])assert.equal(qa(tool).ok,false,tool);
for(const tool of ['lint_frame','cache_clear'])assert.equal(qa(tool).ok,true,tool);
for(const phase of ['start','queued','error']) {const s=fresh(), e={tool_name:'diff_frames',phase};assert.equal(qaOps(s,MODES,e),false);if(phase==='start')assert.deepEqual(plain(s._covArgs),{undefined:{}});}
assert.equal(qaOps(fresh(),MODES,{tool_name:'unknown',phase:'complete'}),false);
for(const [delta,what] of [[{bit_identical:true},'none'],[{changed_percent:0.123},'0.12%'],[{changed_percent:12.34},'12.3%']]) {
 const {e,ok}=qa('diff_frames',{}, {delta});assert.equal(ok,true);assert.equal(e._railArgs.changed,what);
}
for(const [summary,expected] of [[{psnr_db:{mean:null}},'PSNR none'],[{ssim:{mean:null}},'SSIM none'],[{ms_ssim:{mean:null}},'MS-SSIM none'],[{},'MS-SSIM null']]) {
 const {e,ok}=qa('compare_ranges',{}, {points:1,summary});assert.equal(ok,true);assert.equal(e._railArgs['1 sample'],expected);
}
for(const [stats,expected] of [[{mean:null},'identical'],[{mean:20,min:10,p5:12},'mean 20.00 dB, min 10.00 dB, p5 12.00 dB'],[{mean:20,min:10,p5:12,min_at_s:1},'mean 20.00 dB, min 10.00 dB at 1s, p5 12.00 dB']]) {
 const {e}=qa('compare_ranges',{}, {points:1,identical_count:1,summary:{psnr_db:stats}});assert.ok(e._crRows[1][1].startsWith(expected));
}
for(const [args,expected] of [[{},'frame'],[{time:1},'1s'],[{times:[1,2]},'2 times']]) {
 const {e}=qa('lint_frame',args,{summary:{counts_by_code:{unknown_code:2,zero:0}}});assert.deepEqual(plain(e._readRows),[['Frame',expected],['Findings','unknown code ×2']]);
}
for(const quality of [{},{max_crushed_frac:0.1},{max_crushed_frac:0.1,max_clipped_frac:0.2},{max_band_plateau_px:2},{min_luma_levels:16},{identical_to_prev:0}]) {
 const {e}=qa('lint_frame',{}, {summary:{quality}});
 const extra=[];if(quality.max_clipped_frac!==undefined)extra.push(['Crushed / clipped','max 10% / 20%','stack']);
 if(quality.max_band_plateau_px!==undefined)extra.push(['Banding / frozen','plateau 2 px','stack']);
 if(quality.min_luma_levels!==undefined)extra.push(['Banding / frozen','16 levels','stack']);
 if(quality.identical_to_prev!==undefined)extra.push(['Banding / frozen','0 same as prev','stack']);
 assert.deepEqual(plain(e._readRows.slice(2)),extra);
}
for(const [payload,expected] of [[{},'not in feed'],[{lum_stamps:[null,{},'…+2',{t:NaN,dir:1},{t:1,dir:0}]},'none'],[{lum_stamps:[{t:1,dir:-1},{t:2,dir:1}]},'↓1s ↑2s']]) {
 const {e}=qa('flash_series',{}, {samples:1},payload);assert.equal(e._flashStamps,expected);assert.equal(e._readRows[0][1],'1 frame');
}
for(const [result,rule] of [[{},null],[{worst_window_flashes:0},'0 · red –'],[{worst_red_window_flashes:2},'– · red 2'],[{worst_window_flashes:1,worst_window_start:2,windows_over_limit:3,flash_area_pct_max:4},'1 @ 2s, 3>3, 4% · red –']]) {
 const {e}=qa('flash_series',{}, {samples:1,flash_limit:3,...result});assert.equal(e._flashRule,rule);
}
for(const [band,expected] of [[{lo_hz:10,hi_hz:20,dbfs:0},'10–20 Hz, 0.0 dBFS'],[{lo_hz:1000,hi_hz:2000,dbfs:-2},'1–2 kHz, −2.0 dBFS'],[{lo_hz:500,hi_hz:2000,dbfs:1},'500 Hz–2 kHz, 1.0 dBFS'],[{},'none']]) {
 const {e}=qa('audio_spectrum',{}, {fft_size:1024,loudest_band:band});assert.equal(e._specRows[2][1],expected);assert.equal(e._specRows[1][1],'silent');
}
const cache=fresh();for(const [cap,expected] of [[0,'off'],[1024,'off → 1 KB'],[1024,'1 KB'],[0,'1 KB → off']]) {
 const e=event('cache_set_budget','complete',{}, {cap_bytes:cap});assert.equal(qaOps(cache,MODES,e),true);assert.equal(e._railArgs.budget,expected);
}
assert.deepEqual(plain(qa('cache_clear').e._readRows),[['Freed','0 B'],['Frames','0']]);
assert.equal(qa('check_code_clip_determinism',{}, {deterministic:true}).e._readRows[0][1],'frame');
console.log('PASS sparse quality measurements preserve missing values, units, counts and cache transitions');
const project=(s,tool,args={},result={},phase='complete',payload={})=>{const e=event(tool,phase,args,result);Object.assign(e.payload,payload);return {ok:projectOps(s,MODES,e),e};};
const byKind=(s,mode,kind)=>s.views[mode]?.[Object.entries(MODES[mode].panels).find(([,p])=>p.kind===kind)[0]];
for(const tool of ['checkpoint','restore','template_define','template_delete','set_meta','unknown'])assert.equal(project(fresh(),tool).ok,false,tool);
for(const tool of ['checkpoint_delete','undo','redo','save_project','load_project','project_diff','batch','recipe_apply','template_apply'])assert.equal(project(fresh(),tool).ok,true,tool);
for(const phase of ['start','queued','error'])assert.equal(projectOps(fresh(),MODES,{tool_name:'batch',phase}),false);
assert.equal(projectOps(fresh(),MODES,{tool_name:'unknown',phase:'complete'}),false);
const checkpoints=fresh();for(let i=0;i<7;i++)project(checkpoints,'checkpoint',{name:`cp${i}`});
assert.deepEqual(plain(byKind(checkpoints,'batch','history').items.map(x=>x.id)),['cp6','cp5','cp4','cp3','cp2']);
project(checkpoints,'checkpoint',{name:'cp6'});assert.equal(byKind(checkpoints,'batch','history').items[0].name,'checkpoint · replaced');
project(checkpoints,'checkpoint_delete',{name:'cp6'});assert.equal(byKind(checkpoints,'batch','history').items[1].sel,true);
const solitary=fresh();project(solitary,'checkpoint',{name:'only'});project(solitary,'checkpoint_delete',{name:'only'});assert.equal(byKind(solitary,'batch','history').items[0].dim,true);
const mixed=fresh();project(mixed,'save_project',{path:'first'});for(let i=0;i<6;i++)project(mixed,'checkpoint',{name:`cp${i}`});assert.equal(byKind(mixed,'batch','history').items.length,5);assert.equal(byKind(mixed,'batch','history').items.some(x=>x.id==='first'),false);
const old=byKind(mixed,'batch','history').items;old[0]._t=-120000;old[1]._t=-7200000;project(mixed,'undo',{steps:2});assert.equal(old[0].ago,'2m');assert.equal(old[1].ago,'2h');
for(const dependencies of [{},{total:2,missing:2,changed:1,not_ok:[{}, {},{resolved:'/gone.png',status:'changed'},null]}]) {
 const s=fresh();project(s,'load_project',{}, {dependencies});const media=byKind(s,'batch','media');assert.match(media.foot,/load_project:/);
 if(dependencies.not_ok)assert.deepEqual(plain(media.items),[{name:'file',note:'missing ×2',em:true},{name:'gone.png',note:'changed',em:true}]);
}
for(const [args,result,expected] of [
 [{},{},[['~','no named changes']]],
 [{a:'undo:2'},{identical:true},[['~','identical']]],
 [{a:'checkpoint:cp'},{added:['named',{},null],removed:['gone'],changed:[{id:'c1'}]},[['+','named'],['-','gone'],['~','Hero.mp4']]],
 [{a:'file:/project.kimo'},{tracks:{added:['video'],removed:['audio'],renamed:[null,{after:'new'}]}},[['~','track unnamed -> new'],['+','track video'],['-','track audio']]],
 ]) {const s=fresh();project(s,'project_diff',args,result);assert.deepEqual(plain(byKind(s,'batch','diffbatch').diff),expected);}
for(const [args,result,text] of [[{name:'plain'}, {},'template plain · 0 ops'],[{library_path:'/lib/demo'},{replaced:true},'template demo · 0 ops'],[{name:'with',params:[null,{name:'x'}],ops:[{}]}, {},'template with (x) · 1 op']]) {
 const s=fresh();project(s,'template_define',args,result);assert.equal(byKind(s,'batch','diffbatch').diff[0][1],text);
}
const batches=fresh();project(batches,'batch',{ops:[null,{tool:'trim_clip',args:{name:'Named'}},{args:{track_id:'Video'}}]}, {dry_run:true});
assert.deepEqual(plain(byKind(batches,'batch','diffbatch').batch.rows),[['op 1','','dry run'],['Named','trim_clip','dry run'],['Video','','dry run']]);
project(batches,'batch',{ops:[{tool:'move_clip',args:{clip_id:'c1',to:2}},{tool:'trim_clip',args:{clip_id:'missing'}}]}, {results:[{index:0},{index:1},{index:3},{}],warnings:[null]});
assert.equal(batches.liveTL.tracks[0].clips[0].s,2);assert.equal(batches.liveTL.tracks[0].clips[0].e,12);
for(const tool of ['recipe_apply','template_apply'])for(const dry_run of [false,true]) {
 const s=fresh();project(s,tool,{}, {dry_run});const table=byKind(s,'batch','diffbatch').batch;
 assert.equal(table.rows[0][2],dry_run?'dry run':'done');
}
const recipe=fresh();project(recipe,'recipe_apply',{params:{title:'Title'}},{roles:{'track/title':1},created:{clip_ids:['title1','title2']}},'complete',{track:'v1',s:1,e:3});
assert.equal(recipe.liveTL.tracks[0].clips[1].label,'recipe · Title');assert.deepEqual(plain(recipe.liveTL.tracks[0].clips[1].fx),['2 layers']);
const template=fresh();project(template,'template_apply',{}, {args:{unused:4,text:'Hello'},created:{clip_ids:['c1']}},'complete',{track:'v1',s:1,e:2});
assert.equal(template.liveTL.tracks[0].clips[1].label,'Hello');
const meta=fresh();meta.liveTL.tracks.push({id:'empty',name:'Long track name'});meta.liveTL.tracks[0].clips[0].name='Full clip name';
assert.equal(project(meta,'set_meta',{target:'track',id:'Long track name',name:'New track'}).ok,true);assert.equal(meta.liveTL.tracks[1].label,'New track');
assert.equal(project(meta,'set_meta',{id:'Full clip name',name:null,role:null,add_tags:['A'],remove_tags:['B']}).ok,true);
const metaRows=byKind(meta,'edit','kv').blocks.filter(x=>x.k).map(x=>[x.k,x.v]);assert.deepEqual(plain(metaRows),[['Name','cleared'],['Role','cleared'],['Tags','+A -B']]);
project(meta,'set_meta',{id:'c1',tags:[],name:'Hero.mp4'});assert.equal(byKind(meta,'edit','kv').blocks.at(-1).v,'none');
for(const target of ['clip','track'])assert.equal(project(fresh(),'set_meta',{target,id:'missing'}).ok,false);
const startMeta=project(fresh(),'set_meta',{target:'track',id:'missing',name:'n'}, {},'start').e;assert.deepEqual(plain(startMeta.payload.args),{target:'track',name:'n'});
console.log('PASS project defaults, bounded checkpoints, sparse dependencies, batches and metadata');
assert.equal(working(null),null);assert.equal(nits(null),null);assert.equal(bytes(null),null);
assert.equal(panel(fresh(),{color:{panels:{}}},'color','kv'),null);
const partial=fresh();partial._pipe={_ch:{input:{from:'old'}}};pipeRows(partial,MODES);assert.equal(byKind(partial,'color','kv').blocks[0].v,'—');
delete partial._pipe._ch;pipeRows(partial,MODES);assert.equal(byKind(partial,'color','kv').blocks[0].v,'—');
const override={};setPipe(override,'clip:Hero','ACEScg');assert.equal(override.clips.Hero,'ACEScg');
for(const e of [{tool_name:'unknown',phase:'start'},{tool_name:'unknown',phase:'complete'}])assert.equal(pipelineOps(fresh(),MODES,e),false);
const sparsePipe=fresh();pipelineOps(sparsePipe,MODES,event('set_color_pipeline'));assert.equal(sparsePipe._pipe.input,null);
pipelineOps(sparsePipe,MODES,event('set_color_pipeline','complete',{settings:{mastering:{max_cll:100}}}));assert.equal(sparsePipe._pipe.hdr.max_cll,'100 nits');
const clipPipe=fresh();pipelineOps(clipPipe,MODES,event('set_clip_input_color','complete',{clip_id:'c1',gamut:0,transfer:1}));assert.equal(clipPipe._pipe.clips.Hero,'Rec.709 sRGB');
const emptyPipe=fresh();emptyPipe._pipe={};pipelineOps(emptyPipe,MODES,event('set_clip_input_color','complete',{gamut:0,transfer:1}));assert.equal(emptyPipe._pipe.clips['clip input'],'Rec.709 sRGB');
const noError=fresh();noError._pipe={};const denied=event('set_ocio_view_delivery','error',{selection:{view:'View'}});pipelineOps(noError,MODES,denied);assert.equal(denied._hudErr,undefined);
for(const [g,t,out] of [[1,6,'Rec.2100 PQ'],[3,1,'sRGB']]){pipelineOps(noError,MODES,event('set_ocio_view_delivery','complete',{selection:{view:'View',output_gamut:g,output_transfer:t}}));assert.equal(noError._pipe.output,out);}
for(const helper of [()=>histPush(fresh(),{},{}),()=>diffLines(fresh(),{},'',[]),()=>batchTable(fresh(),{},'','',[]),()=>mediaFoot(fresh(),{},'')])assert.equal(helper(),false);
const noModes=fresh();assert.equal(projectOps(noModes,{},event('checkpoint_delete')),false);
project(fresh(),'restore',{name:'unknown'});
const oneBack=fresh();project(oneBack,'project_diff',{a:'undo:1',b:'named revision'});assert.equal(byKind(oneBack,'batch','diffbatch').titleSmall,'1 step back to named revision');
const appended=fresh();const diffPanel=panel(appended,MODES,'batch','diffbatch');diffPanel._ctx='same';diffLines(appended,MODES,'same',[['+','one']]);assert.deepEqual(plain(diffPanel.diff),[['+','one']]);
const dry=fresh();project(dry,'recipe_apply',{}, {dry_run:true,roles:{title:'id'}});assert.equal(byKind(dry,'batch','diffbatch').batch.rows[0][2],'dry run');
project(dry,'template_apply',{}, {dry_run:true,created:[null,null]});assert.deepEqual(plain(byKind(dry,'batch','diffbatch').batch.rows),[['clip 1','template','dry run'],['clip 2','template','dry run']]);
const noTracks=fresh();delete noTracks.liveTL.tracks;for(const target of ['clip','track'])assert.equal(project(noTracks,'set_meta',{target,id:'missing'}).ok,false);
const noClips=fresh();noClips.liveTL.tracks=[{}];assert.equal(project(noClips,'set_meta',{id:'missing'}).ok,false);
assert.equal(metaOps(fresh(),{}, {id:'c1',name:'Renamed'},{}),true);
for(const [q,line] of [[{},null],[{psnr_db:null,identical:true},'PSNR identical'],[{psnr_db:null,ssim:NaN,ms_ssim:null},'PSNR none · SSIM none · MS-SSIM none']])assert.equal(qualityLine(q),line);
assert.equal(side(fresh(),MODES,{source:'clip:c1',frame:2}),'Hero.mp4 f2');assert.equal(side(fresh(),MODES,{source:'clip:c999'}),'clip');
assert.equal(qa('audio_spectrum',{clip_id:'c1'},{fft_size:1024}).e._specRows[0][1],'Hero.mp4');
assert.equal(qa('cache_set_budget',{megabytes:2}).e._railArgs.budget,'2 MB');
assert.equal(qa('check_code_clip_determinism',{time:1},{deterministic:true}).e._readRows[0][1],'1s');
const apply=fresh();apply.liveTL={};tlApply(apply,MODES,{tracks:[{id:'empty'}],markers:[],mc:{}},1000);assert.deepEqual(plain(apply.liveTL.tracks),[{id:'empty',_flash:1000}]);
tlApply(apply,MODES,null,1000);assert.equal(apply.liveTL.tracks.length,1);
const noPanels=fresh();cRestore(noPanels,{}, {tl:{tracks:[],markers:[],mc:{}},pipe:null,v:[]});assert.deepEqual(plain(noPanels.liveTL.tracks),[]);
const pipe=fresh();pipeApply(pipe,MODES,{input:'A',clips:{Hero:'B'},hdr:{max_cll:'100 nits'}});assert.equal(pipe._pipe.input,'A');
pipeApply(pipe,MODES,{input:'A',clips:{Hero:'B'},hdr:{max_cll:'100 nits'}});assert.deepEqual(plain(pipe._pipe._ch),{});
pipeApply(pipe,MODES,{input:'C',clips:{Hero:'D'},hdr:{max_cll:'200 nits'}});assert.equal(pipe._pipe.clips.Hero,'D');
pipeApply(pipe,MODES,null);assert.equal(pipe._pipe,null);assert.deepEqual(plain(byKind(pipe,'color','kv').blocks),[]);
pipeApply(pipe,MODES,null);assert.equal(pipe._pipe,null);
const layer=fresh();layerClips(layer,MODES,{track:'v1',s:null,e:2},['one'],'x');assert.equal(layer.liveTL.tracks[0].clips.length,1);
delete layer.liveTL.tracks[0].clips;layerClips(layer,MODES,{track:'v1',s:0,e:2},['one'],'x');assert.equal(layer.liveTL.tracks[0].clips[0].label,'x');
console.log('PASS private formatting and state restoration keep exact sparse-state behavior');
const rawRevision=fresh();project(rawRevision,'project_diff',{a:'c999'});assert.equal(byKind(rawRevision,'batch','diffbatch').titleSmall,'revision to now');
assert.equal(side(fresh(),MODES,{source:'file:/media/shot.mp4'}),'shot.mp4');
assert.equal(qa('audio_spectrum',{clip_id:'c999'},{fft_size:1024}).e._specRows[0][1],'clip');
const fallbackPipe=fresh();pipeApply(fallbackPipe,MODES,{toJSON:()=>null});assert.deepEqual(plain(fallbackPipe._pipe),{input:null,working:null,view:null,output:null,_ch:{}});
const oldPipe=fresh();oldPipe._pipe={};pipeApply(oldPipe,MODES,{clips:{Hero:'ACEScg'},hdr:{max_cll:'100 nits'}});assert.equal(oldPipe._pipe.clips.Hero,'ACEScg');
cRestore(fresh(),MODES,{tl:null,pipe:null,v:[]});
for(const tool of ['set_color_pipeline','set_clip_input_color','set_hdr_mastering']) {
 const s=fresh(), before=plain(s);assert.equal(pipelineOps(s,MODES,event(tool,'error',{settings:{enabled:false},gamut:0,transfer:1,metadata:{max_cll:1}})),false);assert.deepEqual(plain(s),before);
}
const thumbnail=fresh();thumbnail.views.batch={viewer:{src:'fixture://frame'}};project(thumbnail,'checkpoint',{name:'framed'});assert.equal(byKind(thumbnail,'batch','history').items[0].src,'fixture://frame');
assert.equal(side(fresh(),MODES,{source:'named revision'}),'named revision');
const qualityResult=qa('diff_frames',{a:{source:'snapshot:checkpoint:A'},b:{source:'file:/B.png'}},{delta:{changed_percent:1},quality:{psnr_db:20,ssim:0.9,ms_ssim:0.8}}).e;
assert.equal(qualityResult._readRows[2][1],'PSNR 20.00 dB · SSIM 0.9000 · MS-SSIM 0.8000');
const flashingResult=qa('flash_series',{}, {samples:10,range:{start:0,end:1},totals:{max_changed_area_fraction:0.25},flash_limit:3,worst_window_flashes:4,area_basis:'field',worst_field_rect:[1,2,3,4]}, {lum_stamps:Array.from({length:9},(_,i)=>({t:i/10,dir:1})),lum_stamps_total:12}).e;
assert.equal(flashingResult._flashStamps,'↑0s ↑0.1s ↑0.2s ↑0.3s ↑0.4s ↑0.5s ↑0.6s ↑0.7s +4 more');
assert.equal(flashingResult._readRows[0][1],'0s–1s');assert.match(flashingResult._readRows[1][1],/area 25%/);assert.match(flashingResult._readRows[3][1],/field 1,2 3×4/);
for(const rect of [null,[],[1,2,3,NaN]]) {const e=qa('flash_series',{}, {samples:1,flash_limit:3,worst_window_flashes:1,area_basis:'field',worst_field_rect:rect}).e;assert.doesNotMatch(e._readRows.at(-1)[1],/field/);}
const spectrum=qa('audio_spectrum',{}, {fft_size:1024,range:{start:0,end:1},peak_hz:2000,peak_dbfs:-1,total_dbfs:-2}).e;
assert.deepEqual(plain(spectrum._specRows),[['Range','mix 0s–1s','wrap'],['Peak','2 kHz, −1.0 dBFS','wrap'],['Loudest band','none','wrap'],['Total','−2.0 dBFS','wrap']]);
assert.equal(qa('compare_ranges',{a:{source:'clip:c1'},b:{source:'composite'}},{points:1}).ok,true);
const withoutTracks=fresh();delete withoutTracks.liveTL.tracks;cSnap(withoutTracks,MODES);assert.equal(withoutTracks.liveTL.tracks,undefined);
