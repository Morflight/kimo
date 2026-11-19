import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {dirname,resolve} from 'node:path';
import vm from 'node:vm';
const directory=fileURLToPath(new URL('../ui/',import.meta.url)), modules=new Map(), images=[], timers=[];
const mainEvents=new Map();const element={addEventListener:(name,fn)=>mainEvents.set(name,fn)};
const document={visibilityState:'hidden',getElementById:()=>element,createElement:()=>({}),addEventListener(){},fonts:{ready:new Promise(()=>{})}};
const window={addEventListener(){}};
const context=vm.createContext({document,window,performance:{now:()=>1000},requestAnimationFrame:()=>1,cancelAnimationFrame(){},setTimeout:fn=>{timers.push(fn);return timers.length;},clearTimeout(){},Image:class {constructor(){images.push(this);}}});
function get(filename){if(!modules.has(filename))modules.set(filename,new vm.SourceTextModule(readFileSync(filename,'utf8')+(filename===resolve(directory,'app.js')?'\nexport {enqueue,queue,setHidden,handover,statsLine,fitPicked,syncAttrs,adoptImgs,keepFrame,onWheel,renderPanel,pans,buildLayout,nodes,render,stepSprings,springs};':''),{identifier:filename,context}));return modules.get(filename);}
const router=get(resolve(directory,'router.js'));await router.link((name,parent)=>get(resolve(dirname(parent.identifier),name)));await router.evaluate();
const app=get(resolve(directory,'app.js'));await app.link((name,parent)=>get(resolve(dirname(parent.identifier),name)));await app.evaluate();
const {state,apply,enqueue,queue,setHidden,metrics,QMAX,handover,statsLine,fitPicked,syncAttrs,adoptImgs,keepFrame,onWheel,renderPanel,pans,buildLayout,nodes,render,stepSprings,springs}=app.namespace;
const plain=x=>JSON.parse(JSON.stringify(x));
for(const invalid of [null,undefined,1,'bad'])apply(invalid);
apply({type:'unknown'});apply({type:'view'});
apply({type:'view',data:{nested:{keep:1,change:{old:2}},scalar:1,list:[1]}});
apply({type:'view',data:{nested:{change:{new:3}},scalar:{value:2},list:{value:3}}});
assert.deepEqual(plain(state.views.edit),{nested:{keep:1,change:{old:2,new:3}},scalar:{value:2},list:{value:3}});
for(let i=0;i<402;i++)apply({type:'tool_start',id:i,name:'fixture',args:{i}});
assert.equal(state.calls.length,400);assert.equal(state.calls[0].id,2);
apply({type:'tool_start',id:401,name:'replacement'});assert.equal(state.calls.length,400);assert.equal(state.calls.at(-1).name,'replacement');
apply({type:'tool_end',result:'last result'});assert.equal(state.calls.at(-1).result,'last result');assert.equal(state.calls.at(-1).dur_ms,0);
apply({type:'tool_end',id:'lost-start',name:'recovered',result:'done'});assert.equal(state.calls.length,400);assert.equal(state.calls.at(-1).name,'recovered');
apply({type:'session',reset:true});apply({type:'tool_end'});assert.deepEqual(plain(state.calls),[]);
apply({type:'tool_start',name:'one'});apply({type:'tool_start',name:'two'});assert.equal(state.calls.length,2);
apply({type:'project_changed',project:'  Renamed  ',rev:3,width:640,height:360});assert.equal(state.session.project,'Renamed');assert.equal(state.preview.want,3);
apply({type:'frame_rendered',src:'fixture://frame',rev:3,tc:'00:00',res:'640×360',tag:'test',left:'L',right:'R',frame:0});
assert.equal(state.views.edit.viewer.src,'fixture://frame');assert.equal(state.views.edit.viewer.frame,0);
state.views.graph.viewer={src:'fixture://scenario'};
apply({type:'frame_rendered',src:'fixture://new',rev:4});assert.equal(state.views.graph.viewer.src,'fixture://scenario');
apply({type:'project_changed',rev:5});apply({type:'frame_rendered',src:'fixture://legacy'});assert.equal(state.preview.shown,5);assert.equal(state.views.edit.viewer.stale,undefined);
state._gg={clip:{nodes:{n1:{fx:'color_grade'}}},empty:{}};
apply({type:'frame_skipped',rev:6,reason:' n1 failed for clip9 at sample 42 '});
assert.equal(state.preview.refused,'· color grade failed for at sample 42');assert.equal(state.views.edit.viewer.refused,state.preview.refused);
state.preview.refusedSrc=null;apply({type:'project_changed',rev:7});assert.equal(state.views.edit.viewer.refusedSrc,'engine');
for(const reason of ['',null,'   '])apply({type:'frame_skipped',reason});assert.equal(state.preview.want,7);
const before=metrics.staleFrames;apply({type:'frame_rendered',src:'fixture://old',rev:1});assert.equal(metrics.staleFrames,before+1);assert.equal(state.views.edit.viewer.src,'fixture://legacy');
state._scopeCalls={scope:'waveform'};
for(const [data,expected] of [[{png_b64:'png'},'data:image/png;base64,png'],[{jpg_b64:'jpg'},'data:image/jpeg;base64,jpg'],[{src:'fixture://scope'},'fixture://scope'],[{},'fixture://scope']]) {
 apply({type:'frame_rendered',call_id:'scope',...data});
 const scopes=Object.values(state.views).flatMap(v=>Object.values(v)).filter(v=>v.img?.kind==='waveform');assert.ok(scopes.length);assert.ok(scopes.every(v=>v.img.src===expected));
}
apply({type:'frame_rendered',png_b64:'first'});apply({type:'frame_rendered',png_b64:'second'});assert.equal(images.length,2);images[0].onload();
assert.equal(Object.values(state.views).flatMap(v=>Object.values(v)).some(v=>v.parade),false,'superseded image decoding must not publish a parade');
setHidden(false);queue.length=0;
enqueue({type:'tool_progress',id:'p',progress:1});enqueue({type:'tool_progress',id:'p',progress:2});assert.equal(queue.length,1);assert.equal(queue[0].progress,2);
const dropped=metrics.dropped;for(let i=0;i<QMAX;i++)enqueue({type:'stats',index:i});assert.equal(queue.length,QMAX);assert.equal(metrics.dropped,dropped+1);assert.equal(queue[0].index,0);
enqueue({type:'stats',index:QMAX});assert.equal(queue.length,QMAX);assert.equal(queue[0].index,1);assert.equal(metrics.dropped,dropped+2);
setHidden(true);
console.log('PASS reducer merges, bounded call/ingestion queues, missing starts, stale frames and scope isolation');
apply({type:'session',reset:true});apply({type:'frame_skipped',reason:'codec H264 unavailable'});assert.equal(state.preview.revs,true);assert.equal(state.preview.refused,'codec H264 unavailable');
function animated(pics=[]) {const x={style:{},isConnected:true,getElementsByTagName:()=>pics,animations:[],animate(frames,options){const a={frames,options};this.animations.push(a);return a;}};return x;}
const fading=animated([{complete:false,naturalWidth:0,decode:()=>Promise.reject(Error('decode'))}]);
handover([fading],[]);assert.equal(fading.style.opacity,'0');await new Promise(setImmediate);assert.equal(fading.animations.length,1);fading._enter.onfinish();assert.equal(fading._enter,null);
const obsolete=animated([{complete:false}]);handover([obsolete],[]);handover([],[]);await new Promise(setImmediate);assert.equal(obsolete.animations.length,0);
const disconnected=animated();disconnected.isConnected=false;handover([disconnected],[]);assert.equal(disconnected.animations.length,0);
const entering=animated();entering._enter={};handover([entering],[]);assert.equal(entering.animations.length,0);
class Node {
 constructor(cls=''){this.className=cls;this.children=[];this.style={};this.parentNode=null;this.textContent='';this.attrs={};}
 get classList(){return {contains:c=>this.className.split(' ').includes(c),remove:c=>{this.className=this.className.split(' ').filter(x=>x!==c).join(' ');}};}
 appendChild(n){n.parentNode=this;this.children.push(n);}
 replaceWith(n){const p=this.parentNode,i=p.children.indexOf(this);p.children[i]=n;n.parentNode=p;this.parentNode=null;}
 remove(){if(this.parentNode){const p=this.parentNode;p.children.splice(p.children.indexOf(this),1);this.parentNode=null;}}
 get lastElementChild(){return this.children.at(-1);}
 get previousElementSibling(){return this.parentNode?this.parentNode.children[this.parentNode.children.indexOf(this)-1]:null;}
 getBoundingClientRect(){return {bottom:this.parentNode?(this.parentNode.children.indexOf(this)+1)*20:100};}
 set innerHTML(value){this.html=value;this.children=[new Node(),new Node()];for(const c of this.children)c.parentNode=this;}
 get attributes(){return Object.entries(this.attrs).map(([name,value])=>({name,value}));}
 hasAttribute(k){return k in this.attrs;}
 getAttributeNames(){return Object.keys(this.attrs);}
 getAttribute(k){return this.attrs[k]??null;}
 setAttribute(k,v){this.attrs[k]=v;}
 removeAttribute(k){delete this.attrs[k];}
}
document.createElement=()=>new Node();
const stats=new Node();statsLine(stats,{text:'first'});assert.equal(stats._st.className,'scopestats');const originalStats=stats._st;
statsLine(stats,{text:'second',_flash:1000});assert.notEqual(stats._st,originalStats);assert.equal(originalStats.parentNode,null);
const flashingStats=stats._st;statsLine(stats,{text:'third',_flash:999});assert.notEqual(stats._st,flashingStats);assert.equal(stats._st.textContent,'third');
timers.at(-1)();assert.equal(stats._st.classList.contains('flash'),false);
statsLine(stats,null);assert.equal(stats._st,null);assert.equal(stats.children.length,0);
const stale=new Node('flash');stale._ft=1;stats._st=stale;statsLine(stats,{text:'replacement'});assert.equal(stale._ft,0);assert.equal(stats._st.textContent,'replacement');
fitPicked({querySelector:()=>null});const small=new Node();small.appendChild(new Node('row'));fitPicked({querySelector:()=>small,getBoundingClientRect:()=>({bottom:100})});assert.equal(small.children.length,1);
const overflowing=new Node();for(const cls of ['row','row','row','sec','row','row'])overflowing.appendChild(new Node(cls));
fitPicked({querySelector:()=>overflowing,getBoundingClientRect:()=>({bottom:90})});assert.equal(overflowing.children.at(-1).className,'row dim');assert.equal(overflowing.children.some(n=>n.className==='sec'),false);
const from=new Node(),to=new Node();from.attrs={src:'one',title:'new'};to.attrs={src:'old',extra:'remove'};syncAttrs(from,to);assert.deepEqual(to.attrs,{src:'one',title:'new'});
const oldImage=new Node(),newImage=new Node(),parent=new Node();oldImage.attrs={src:'same',title:'old'};newImage.attrs={src:'same',title:'new'};parent.appendChild(newImage);
adoptImgs({getElementsByTagName:()=>parent.children},[oldImage]);assert.equal(parent.children[0],oldImage);assert.equal(oldImage.attrs.title,'new');
keepFrame({querySelector:()=>null},null,null);
console.log('PASS animation cancellation, stale stats timers, picked overflow and decoded-image reuse');
const {MODES}=get(resolve(directory,'modes.js')).namespace;
const wheelElement={dataset:{panel:'tl'},clientWidth:600,getBoundingClientRect:()=>({left:0})};let prevented=0;
const wheel=opts=>onWheel({target:{closest:()=>wheelElement},deltaX:0,deltaY:0,clientX:100,preventDefault:()=>prevented++,...opts});
onWheel({target:{}});wheelElement.dataset.panel='unknown';wheel({});wheelElement.dataset.panel='tl';state.mode='edit';state.views.edit={};wheel({});state.views.edit.tl={};wheel({});
state.views.edit.tl={tracks:[{id:'one'}]};wheel({ctrlKey:true,deltaY:100});assert.equal(prevented,0);
pans['edit:tl'].kmax=0;wheel({deltaY:100});assert.equal(prevented,0);
MODES.fixture={panels:{tl:{kind:'timeline'}}};state.mode='fixture';state.views.fixture={tl:{tracks:[{id:'one'}]}};wheel({ctrlKey:true,deltaY:-100});assert.ok(pans['fixture:tl'].z>1);assert.equal(prevented,1);
renderPanel('unknown');
state.mode='missing';assert.doesNotThrow(()=>wheel({}),'stale mode must not throw during input');assert.doesNotThrow(()=>renderPanel('tl'));

MODES.fixture={};state.mode='fixture';assert.doesNotThrow(()=>wheel({}));assert.doesNotThrow(()=>renderPanel('tl'));
const style=()=>({setProperty(k,v){this[k]=v;},removeProperty(k){delete this[k];}});
Object.assign(element,{style:style(),children:[]});document.body={};document.documentElement={style:style()};context.innerWidth=1376;context.innerHeight=768;
MODES.fixture={rects:{},panels:{missing:{kind:'kv'}}};state.mode='fixture';buildLayout();assert.equal(element.style['--top'],'46px');assert.equal(element.style['--foot'],'0px');assert.deepEqual(Object.keys(nodes),[]);
MODES.fixture={rects:{viewer:[0,46,1376,768],insp:[0,46,1376,768]},panels:{}};context.innerWidth=100;context.innerHeight=100;buildLayout();assert.match(element.style.transform,/scale/);
const ui=new Map();document.getElementById=id=>{if(!ui.has(id)){const n=new Node();n.style=style();n.classList.toggle=()=>{};ui.set(id,n);}return ui.get(id);};
// Header nodes need only class toggling; the inspector below uses real panel renderers.
for(const id of ['livepill']){const n=document.getElementById(id);Object.defineProperty(n,'classList',{value:{toggle(){}}});}
Node.prototype.querySelector=()=>null;Node.prototype.querySelectorAll=()=>[];Node.prototype.getElementsByTagName=()=>[];
MODES.fixture={rects:{},panels:{kv:{kind:'kv',title:'Details'},unknown:{kind:'not-a-renderer'},missing:{kind:'kv'}}};buildLayout();nodes.kv=new Node();nodes.kv.clientWidth=400;nodes.kv.clientHeight=300;nodes.unknown=new Node();
state.views.fixture={};state.calls=[{id:'picked',name:'running-call',status:'running',args:{x:1},t:1000}];state.pick='picked';state.activity=null;state.explicitActivity=false;render();assert.match(nodes.kv.html,/running-call/);assert.equal(document.getElementById('pill-act').textContent,'fixture');
MODES.fixture.panels={unknown:{kind:'not-a-renderer'}};render();assert.equal(nodes.kv.html.includes('running-call'),true);
MODES.fixture.panels={scope:{kind:'scopes'}};nodes.scope=new Node();nodes.scope.clientWidth=600;nodes.scope.clientHeight=300;state.views.fixture={scope:{stats:{text:'Measured'}}};renderPanel('scope');assert.equal(nodes.scope._st.textContent,'Measured');
state.views.fixture.scope.stats=null;renderPanel('scope');assert.equal(nodes.scope._st,null);
springs.push({el:{style:{},dataset:{}},D:10,W:10,H:10,d:[0,0,1,1]});assert.equal(stepSprings(0),false);assert.equal(springs.length,0);
mainEvents.get('click')({target:{}});mainEvents.get('click')({target:{closest:()=>({dataset:{call:'absent'}})}});assert.equal(state.pick,null);
state.calls=[{id:3}];mainEvents.get('click')({target:{closest:()=>({dataset:{call:'3'}})}});assert.equal(state.pick,3);mainEvents.get('click')({target:{closest:()=>({dataset:{call:'3'}})}});assert.equal(state.pick,null);
console.log('PASS sparse layouts, missing renderer, running-call pick, scope refresh and call deselection');
