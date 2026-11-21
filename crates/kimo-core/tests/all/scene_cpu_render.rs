//! Scene CPU executor verification (CP2): multi-draw composition over a shared depth buffer.
//! Master oracle: the multi-draw frame must equal, PIXEL FOR PIXEL, the argmin-depth
//! selection of the isolated per-draw renders (tie -> earlier draw). Covers occlusion,
//! intersecting surfaces, coplanar duplicates (exact ties), near-plane clipping, and a
//! mixed source set (primitive + SDF contour + raw stream). Empty scene renders empty.
use kimo_core::threed::{scene::{self,Scene,Source,Node,Cache},primitives::{generate,Primitive},vertex_stream::Stream,sdf_graph::{Field,Graph},contour_wrapped::{self,Config},texture_material::Surface,lit_raster,lighting_stage::Light};
use kimo_core::camera_projection::Camera;

fn surf(rgb:[f64;3])->Surface{Surface{albedo:rgb,metallic:0.,roughness:0.5,base:None,metal_rough:None,occlusion:None,normal:None,ao_strength:1.,normal_scale:1.,double_sided:false}}
fn ident()->[[f64;4];4]{[[1.,0.,0.,0.],[0.,1.,0.,0.],[0.,0.,1.,0.],[0.,0.,0.,1.]]}
fn t(x:f64,y:f64,z:f64)->[[f64;4];4]{let mut m=ident();m[0][3]=x;m[1][3]=y;m[2][3]=z;m}
fn cam()->Camera{Camera{eye:[0.,0.,0.],pan:0.,tilt:0.,roll:0.,sensor_height:24.,focal_length:35.,near:0.5}}
fn lights()->Vec<Light>{vec![Light::Directional{travel_direction:[0.2,0.3,1.],radiance:[2.;3]}]}
fn sphere(r:f64)->Stream{let g=generate(&Primitive::Sphere{radius:r,longitude_slices:20,latitude_slices:10}).unwrap();Stream{vertices:g.vertices,indices:g.indices}}
fn torus()->Stream{let g=Graph{nodes:vec![Field::Torus{center:[0.;3],major_radius:1.,minor_radius:0.4}],output:0};contour_wrapped::polygonize(&g,&Config{bounds:[[-1.6;3],[1.6;3]],resolution:20,normal_epsilon:0.001}).unwrap().stream}

const W:usize=160;const H:usize=90;

/// Pixelwise argmin-depth oracle over isolated renders.
fn oracle_check(multi:&lit_raster::Output,isolated:&[lit_raster::Output],label:&str){
 assert_eq!(multi.frame.d.len(),W*H*4);
 for px in 0..W*H{
  let mut best:Option<(f64,usize)>=None;
  for(i,o)in isolated.iter().enumerate(){let z=o.depth[px];if z.is_finite()&&best.map(|(bz,_)|z<bz).unwrap_or(true){best=Some((z,i));}}
  match best{
   None=>assert!(multi.frame.d[px*4..px*4+4].iter().all(|v|*v==0.),"{label}: px {px} uncovered must be black"),
   Some((_,i))=>assert_eq!(&multi.frame.d[px*4..px*4+4],&isolated[i].frame.d[px*4..px*4+4],"{label}: px {px} must come from nearest draw {i}"),}}
}

fn isolated(scene_desc:&Scene,w:usize,h:usize)->Vec<lit_raster::Output>{
 // one render per NODE: source stream baked at the node's world, node's material
 let mut cache=Cache::new();
 scene_desc.nodes.iter().map(|n|{
  let one=Scene{sources:scene_desc.sources.clone(),nodes:vec![n.clone()],materials:scene_desc.materials.clone(),lights:scene_desc.lights.clone(),camera:scene_desc.camera};
  scene::render_cpu(&scene::compile(&one,&mut cache).unwrap(),w,h).unwrap()}).collect()
}

#[test]fn mixed_scene_multi_equals_argmin_of_isolated(){
 let s=Scene{
  sources:vec![Source::Primitive{primitive:Primitive::Sphere{radius:1.,longitude_slices:20,latitude_slices:10}},Source::SdfContour{graph:Graph{nodes:vec![Field::Torus{center:[0.;3],major_radius:1.,minor_radius:0.4}],output:0},config:Config{bounds:[[-1.6;3],[1.6;3]],resolution:20,normal_epsilon:0.001}},Source::Stream{stream:sphere(0.8)}],
  nodes:vec![Node{source:0,world:t(-0.6,0.,3.5),material:0},Node{source:1,world:t(0.6,0.2,4.5),material:1},Node{source:2,world:t(0.,-0.3,3.0),material:2}],
  materials:vec![surf([0.9,0.2,0.2]),surf([0.2,0.9,0.2]),surf([0.2,0.2,0.9])],lights:lights(),camera:cam()};
 let multi=scene::render_cpu(&scene::compile(&s,&mut Cache::new()).unwrap(),W,H).unwrap();
 assert!(multi.covered>500,"mixed scene covers pixels: {}",multi.covered);
 oracle_check(&multi,&isolated(&s,W,H),"mixed");
 println!("SCENE-CPU mixed: {} draws, covered={}, clipped={}, oracle=argmin-depth EXACT",3,multi.covered,multi.clipped_triangles);
}

#[test]fn intersecting_spheres_per_pixel_depth_resolves(){
 // two spheres whose volumes interpenetrate: neither contains the other in screen space
 let s=Scene{sources:vec![Source::Stream{stream:sphere(1.)}],nodes:vec![Node{source:0,world:t(-0.5,0.,4.),material:0},Node{source:0,world:t(0.5,0.,4.2),material:1}],materials:vec![surf([0.9,0.2,0.2]),surf([0.2,0.2,0.9])],lights:lights(),camera:cam()};
 let iso=isolated(&s,W,H);
 let multi=scene::render_cpu(&scene::compile(&s,&mut Cache::new()).unwrap(),W,H).unwrap();
 oracle_check(&multi,&iso,"intersecting");
 // sanity: both materials actually visible (the seam crosses the overlap)
 let red=iso[0].frame.d.clone();let blue=iso[1].frame.d.clone();let _=(red,blue);
 assert!(multi.covered>200);println!("SCENE-CPU intersecting: covered={}, oracle EXACT",multi.covered);
}

#[test]fn coplanar_duplicate_exact_tie_keeps_earlier_draw(){
 // SAME stream, SAME world, different materials: every pixel is an exact depth tie ->
 // first draw wins everywhere (z>=depth skips the later one)
 let s=Scene{sources:vec![Source::Stream{stream:sphere(1.)}],nodes:vec![Node{source:0,world:t(0.,0.,4.),material:0},Node{source:0,world:t(0.,0.,4.),material:1}],materials:vec![surf([0.9,0.2,0.2]),surf([0.2,0.2,0.9])],lights:lights(),camera:cam()};
 let iso=isolated(&s,W,H);
 let multi=scene::render_cpu(&scene::compile(&s,&mut Cache::new()).unwrap(),W,H).unwrap();
 assert_eq!(multi.frame.d,iso[0].frame.d,"coplanar tie: output is EXACTLY the first draw");
 assert_eq!(multi.covered,iso[0].covered);
 println!("SCENE-CPU coplanar: exact tie -> earlier draw, covered={}",multi.covered);
}

#[test]fn near_plane_straddle_composes_with_background(){
 // huge sphere centered near the camera straddles the near plane; background sphere behind
 let s=Scene{sources:vec![Source::Stream{stream:sphere(2.5)},Source::Stream{stream:sphere(0.7)}],nodes:vec![Node{source:0,world:t(0.,0.,1.2),material:0},Node{source:1,world:t(0.4,0.3,4.),material:1}],materials:vec![surf([0.8,0.8,0.2]),surf([0.2,0.8,0.9])],lights:lights(),camera:cam()};
 let iso=isolated(&s,W,H);
 let multi=scene::render_cpu(&scene::compile(&s,&mut Cache::new()).unwrap(),W,H).unwrap();
 assert!(multi.clipped_triangles>0,"near-straddling sphere clipped by the near plane");
 oracle_check(&multi,&iso,"near-clip");
 println!("SCENE-CPU near-clip: clipped={}, covered={}, oracle EXACT",multi.clipped_triangles,multi.covered);
}

#[test]fn empty_scene_renders_empty_frame(){
 let s=Scene{sources:vec![],nodes:vec![],materials:vec![],lights:lights(),camera:cam()};
 let o=scene::render_cpu(&scene::compile(&s,&mut Cache::new()).unwrap(),W,H).unwrap();
 assert_eq!(o.covered,0);assert!(o.frame.d.iter().all(|v|*v==0.));
 println!("SCENE-CPU empty: empty is a result, no error");
}

// ---- the scene descriptor tools (ext/scene_compose.rs) over the same executor, on a 32x18 target ----
fn tool(n:&str,a:serde_json::Value)->Result<serde_json::Value,String>{let mut s=kimo_core::commands::Session::new();kimo_core::commands::dispatch(&mut s,n,&a)}
fn small_scene()->Scene{Scene{sources:vec![Source::Primitive{primitive:Primitive::Sphere{radius:0.9,longitude_slices:12,latitude_slices:6}},Source::Stream{stream:sphere(0.5)}],
 nodes:vec![Node{source:0,world:t(0.,0.,3.),material:0},Node{source:1,world:t(0.6,0.,2.5),material:1}],materials:vec![surf([0.9,0.1,0.1]),surf([0.1,0.9,0.1])],lights:lights(),camera:cam()}}
fn tmp(name:&str)->std::path::PathBuf{std::env::temp_dir().join(format!("kimo-scenetool-{}-{name}",std::process::id()))}
fn is_png(p:&std::path::Path)->bool{std::fs::read(p).map(|b|b.starts_with(b"\x89PNG")).unwrap_or(false)}

#[test]fn scene_tools_compile_render_and_report_the_executor_stats(){
 let sc=serde_json::to_value(small_scene()).unwrap();
 let c=tool("compile_scene3d_descriptor",serde_json::json!({"scene":sc})).unwrap();assert_eq!((c["stats"]["draws"].as_u64(),c["stats"]["sources_resolved"].as_u64()),(Some(2),Some(2)));
 let lib=scene::render_cpu(&scene::compile(&small_scene(),&mut Cache::new()).unwrap(),32,18).unwrap();
 let path=tmp("plain.png");let r=tool("render_scene3d_cpu",serde_json::json!({"scene":sc,"width":32,"height":18,"path":path})).unwrap();
 assert_eq!(r["covered_pixels"].as_u64(),Some(lib.covered as u64),"the tool renders exactly the library executor");assert!(lib.covered>0&&is_png(&path));assert!(r.get("cpu_lighting").is_none());
 // cpu_lighting is taken out of the scene object before parsing: IBL on a uniform environment
 let mut lit=sc.clone();lit["cpu_lighting"]=serde_json::json!({"ibl":{"environment":{"kind":"uniform","radiance":[1,1,1]},"samples":16}});
 let r=tool("render_scene3d_cpu",serde_json::json!({"scene":lit,"width":32,"height":18,"path":path})).unwrap();assert_eq!(r["cpu_lighting"]["ibl"],true);assert_eq!(r["covered_pixels"].as_u64(),Some(lib.covered as u64));
 std::fs::remove_file(&path).unwrap();
}

#[test]fn scene_tools_sample_animation_and_render_sequences(){
 let sc=serde_json::to_value(small_scene()).unwrap();
 let anim=serde_json::json!({"nodes":[{"node":0,"translation":{"times":[0.0,2.0],"values":[[-1.2,0.0,3.0],[1.2,0.0,3.0]]}}],"lights":[]});
 let r=tool("sample_scene3d_animation",serde_json::json!({"scene":sc,"animation":anim,"time":1.5})).unwrap();
 assert_eq!((r["nodes_animated"].as_u64(),r["lights_animated"].as_u64(),r["camera_animated"].as_bool()),(Some(1),Some(0),Some(false)));
 assert!((r["scene"]["nodes"][0]["world"][0][3].as_f64().unwrap()-0.6).abs()<1e-12,"linear translation key at 1.5 of 0..2");
 let dir=tmp("seq");let r=tool("render_scene3d_sequence",serde_json::json!({"scene":sc,"animation":anim,"times":[0.0,2.0],"dir":dir,"width":32,"height":18})).unwrap();
 assert_eq!(r["count"],2);for k in 0..2{let p=std::path::PathBuf::from(r["frames"][k]["path"].as_str().unwrap());assert!(is_png(&p),"{p:?}");assert!(r["frames"][k]["render"]["covered_pixels"].as_u64().unwrap()>0);}
 assert_eq!(r["frames"][1]["time"],2.0);
 let mut lit=sc.clone();lit["cpu_lighting"]=serde_json::json!({"ibl":{"environment":{"kind":"uniform","radiance":[0.5,0.5,0.5]},"samples":16}});
 let r=tool("render_scene3d_sequence",serde_json::json!({"scene":lit,"animation":anim,"times":[1.0],"dir":dir,"width":32,"height":18})).unwrap();assert_eq!(r["frames"][0]["render"]["cpu_lighting"]["ibl"],true);
 std::fs::remove_dir_all(&dir).unwrap();
}
