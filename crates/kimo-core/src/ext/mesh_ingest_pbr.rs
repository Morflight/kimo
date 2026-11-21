use super::kit::*;
fn read<T:serde::de::DeserializeOwned>(a:&Value,k:&str)->Result<T,String>{serde_json::from_value(a.get(k).ok_or_else(||format!("{k} required"))?.clone()).map_err(|e|e.to_string())}
pub fn commands()->Vec<Command>{vec![
 Command{name:"ingest_vertex_stream",description:"Crate-backed OBJ/glTF2/GLB geometry buffers into48-byte interleaved position/normal/UV/tangent. Explicit derive_tangents required when absent; split per-triangle UV tangents, not MikkTSpace. Caller supplies external buffer bytes, no URI fetch. Normals/UV required; geometry-only metadata reports transforms/materials not applied.",mutates:false,schema:obj(vec![("format",s("obj|gltf")),("text",s("OBJ")),("bytes",json!({"type":"array"})),("buffers",json!({"type":"array"})),("derive_tangents",json!({"type":"boolean"}))],&["format","derive_tangents"]),run:|_,a|{
 #[cfg(feature="mesh-import")]{let derive=read(a,"derive_tangents")?;let out=match str_arg(a,"format")?{"obj"=>crate::threed::ingest::obj(str_arg(a,"text")?,derive)?,"gltf"=>crate::threed::ingest::gltf(&read::<Vec<u8>>(a,"bytes")?,&read::<Vec<Vec<u8>>>(a,"buffers")?,derive)?,_=>return Err("format obj|gltf".into())};Ok(json!(out))}
 #[cfg(not(feature="mesh-import"))]{let _=a;Err("mesh-import feature not compiled".into())}
 }},
 Command{name:"inspect_vertex_stream",description:"Validate arbitrary interleaved vertex stream and return layout, counts, bounds, UV range and tangent sanity. Imported UV outside0..1 is allowed, wrapping caller-owned. No hidden repair.",mutates:false,schema:obj(vec![("stream",json!({"type":"object"}))],&["stream"]),run:|_,a|{let s:crate::threed::vertex_stream::Stream=read(a,"stream")?;s.inspect()}},
 Command{name:"derive_vertex_tangents",description:"Explicit triangle-split UV tangent generator. Returns new editable stream; no MikkTSpace or normal-map parity claim. Reject singular UV triangles.",mutates:false,schema:obj(vec![("stream",json!({"type":"object"}))],&["stream"]),run:|_,a|Ok(json!(crate::threed::vertex_stream::derive_tangents(read(a,"stream")?)?))},
 Command{name:"sample_direct_lighting",description:"Inspect raw GGX metallic-roughness direct lighting at world point/normal/view, directional radiance or inverse-square point intensity. Linear HDR result; no ambient/shadow/IBL preset.",mutates:false,schema:obj(vec![("material",json!({"type":"object"})),("lights",json!({"type":"array"})),("point",json!({"type":"array"})),("normal",json!({"type":"array"})),("eye",json!({"type":"array"}))],&["material","lights","point","normal","eye"]),run:|_,a|Ok(json!({"linear_radiance":crate::threed::lighting_stage::shade(&read(a,"material")?,read(a,"point")?,read(a,"normal")?,read(a,"eye")?,&read::<Vec<crate::threed::lighting_stage::Light>>(a,"lights")?)?}))},
 Command{name:"export_vertex_stream_obj",description:"Wavefront OBJ export of a validated vertex stream: f32 shortest-round-trip decimal positions/normals/UVs, single-indexed triangular faces. No materials, tangents (OBJ has no tangent field), or groups. Empty/invalid stream is a typed error, not an empty file. Re-ingest via ingest_vertex_stream derives tangents and so requires non-singular UVs; contour streams carry placeholder UV=(0,0) (no parameterization).",mutates:false,schema:obj(vec![("stream",json!({"type":"object"})),("path",s("OBJ"))],&["stream","path"]),run:|_,a|{let path=str_arg(a,"path")?;if !path.to_lowercase().ends_with(".obj"){return Err("OBJ path required".into());}let text=crate::threed::obj_export::to_obj(&read(a,"stream")?)?;let bytes=text.len();std::fs::write(path,&text).map_err(|e|e.to_string())?;Ok(json!({"path":path,"bytes":bytes}))}},
Command{name:"export_vertex_stream_gltf",description:"glTF2 GLB export of a validated vertex stream. Default: POSITION/NORMAL/TEXCOORD_0/TANGENT f32 little-endian plus u32 indices in one binary chunk - full 48-byte vertex fidelity, tangents preserved (unlike OBJ), round-trip bit-exact on every attribute, including contour streams with placeholder UV=(0,0). spec_minimal:true writes the strict glTF2 minimum (POSITION/NORMAL/indices required; TEXCOORD_0/TANGENT optional) - omits TEXCOORD_0/TANGENT only when UV is the (0,0) placeholder; real UVs keep all attributes. Spec-minimal re-ingests bit-exact on position/normal/uv/indices with tangents re-derived to a valid orthonormal frame (not bit-identical). Single mesh/node/scene, no materials or skins. Empty/invalid stream is a typed error.",mutates:false,schema:obj(vec![("stream",json!({"type":"object"})),("path",s("GLB")),("spec_minimal",json!({"type":"boolean","default":false}))],&["stream","path"]),run:|_,a|{let path=str_arg(a,"path")?;if !path.to_lowercase().ends_with(".glb"){return Err("GLB path required".into());}let minimal=a.get("spec_minimal").and_then(|v|v.as_bool()).unwrap_or(false);let bytes=crate::threed::gltf_export::to_glb_opts(&read(a,"stream")?,minimal)?;let len=bytes.len();std::fs::write(path,&bytes).map_err(|e|e.to_string())?;Ok(json!({"path":path,"bytes":len,"spec_minimal":minimal}))}},
  Command{name:"render_vertex_stream_pbr",description:"CPU camera-v2 direct GGX/Lambert rendering of raw stream/material/lights to PNG. Frame tone-mapped with the ACES fitted curve (exposure 1.0), then deterministic TPDF-dithered (+/-1 LSB, fixed seed) before 8-bit PNG quantization to remove mid-gradient terraces. Display-referred correctness proof only. No scene preset, textures, shadows or specular IBL.",mutates:false,schema:obj(vec![("stream",json!({"type":"object"})),("camera",json!({"type":"object"})),("material",json!({"type":"object"})),("lights",json!({"type":"array"})),("width",json!({"type":"integer"})),("height",json!({"type":"integer"})),("path",s("PNG"))],&["stream","camera","material","lights","width","height","path"]),run:|_,a|{let path=str_arg(a,"path")?;if !path.to_lowercase().ends_with(".png"){return Err("PNG required".into());}let out=crate::threed::lit_raster::render(&read(a,"stream")?,&read(a,"camera")?,read(a,"width")?,read(a,"height")?,&read(a,"material")?,&read::<Vec<crate::threed::lighting_stage::Light>>(a,"lights")?)?;crate::render::write_image_tpdf(&out.frame,path)?;Ok(json!({"path":path,"covered_pixels":out.covered,"clipped_triangles":out.clipped_triangles,"backend":"cpu_direct_pbr","png_encoding":"ACES fitted tone map (exposure 1.0), TPDF-dithered 8-bit, display-referred","depth_covered":out.depth.iter().filter(|v|v.is_finite()).count()}))}},
]}

#[cfg(test)]
mod tests {
 use super::*;
 fn run(name:&str,a:Value)->Result<Value,String>{let c=commands().into_iter().find(|c|c.name==name).unwrap();(c.run)(&mut Session::new(),&a)}
 /// One right triangle in z=0 facing +z, UV equal to x/y, tangent along +x.
 fn tri()->Value{let v=|p:[f32;3]|json!({"position":p,"normal":[0.,0.,1.],"uv":[p[0],p[1]],"tangent":[1.,0.,0.,1.]});json!({"vertices":[v([0.,0.,0.]),v([1.,0.,0.]),v([0.,1.,0.])],"indices":[[0,1,2]]})}
 fn temp(name:&str)->std::path::PathBuf{let d=std::env::temp_dir().join(format!("kimo-ingest-pbr-{}-{name}",std::process::id()));std::fs::create_dir_all(&d).unwrap();d}
 #[cfg(not(feature="mesh-import"))]
 #[test]fn ingest_reports_the_missing_feature(){assert_eq!(run("ingest_vertex_stream",json!({"format":"obj","text":"","derive_tangents":true})).unwrap_err(),"mesh-import feature not compiled");}
 #[test]fn inspect_reports_counts_bounds_and_uv_range(){
  let r=run("inspect_vertex_stream",json!({"stream":tri()})).unwrap();
  assert_eq!((r["vertex_count"].as_u64(),r["triangle_count"].as_u64()),(Some(3),Some(1)));
  assert_eq!(r["bounds"],json!([[0.,0.,0.],[1.,1.,0.]]));assert_eq!(r["uv_bounds"],json!([[0.,0.],[1.,1.]]));
  assert_eq!(run("inspect_vertex_stream",json!({})).unwrap_err(),"stream required");
  let mut bad=tri();bad["indices"]=json!([[0,1,7]]);
  assert_eq!(run("inspect_vertex_stream",json!({"stream":bad})).unwrap_err(),"stream index out of bounds");
 }
 #[test]fn derive_tangents_follows_the_uv_u_axis_and_refuses_singular_uvs(){
  let mut s=tri();for v in s["vertices"].as_array_mut().unwrap(){v["tangent"]=json!([0.,1.,0.,1.]);}
  let r=run("derive_vertex_tangents",json!({"stream":s})).unwrap();
  for v in r["vertices"].as_array().unwrap(){assert_eq!(v["tangent"],json!([1.,0.,0.,1.]),"u grows along +x");}
  let mut flat=tri();for v in flat["vertices"].as_array_mut().unwrap(){v["uv"]=json!([0.,0.]);}
  assert_eq!(run("derive_vertex_tangents",json!({"stream":flat})).unwrap_err(),"degenerate UV triangle; cannot derive tangent");
 }
 #[test]fn direct_lighting_sample_is_zero_for_a_coincident_point_light(){
  let m=json!({"albedo":[0.5,0.5,0.5],"metallic":0.,"roughness":0.5});
  let at=json!([{"kind":"point","position":[0.,0.,0.],"intensity":[1.,1.,1.],"min_distance":0.01}]);
  let r=run("sample_direct_lighting",json!({"material":m,"lights":at,"point":[0.,0.,0.],"normal":[0.,0.,1.],"eye":[0.,0.,4.]})).unwrap();
  assert_eq!(r["linear_radiance"],json!([0.,0.,0.]));
  // a point light 2 above the surface, inverse square: twice as far gives a quarter of the radiance
  let lit=|z:f64|run("sample_direct_lighting",json!({"material":m,"lights":[{"kind":"point","position":[0.,0.,z],"intensity":[1.,1.,1.],"min_distance":0.01}],"point":[0.,0.,0.],"normal":[0.,0.,1.],"eye":[0.,0.,4.]})).unwrap()["linear_radiance"][0].as_f64().unwrap();
  assert!((lit(1.)/lit(2.)-4.).abs()<1e-12);
  let neg=json!([{"kind":"directional","travel_direction":[0.,0.,-1.],"radiance":[-1.,0.,0.]}]);
  assert_eq!(run("sample_direct_lighting",json!({"material":m,"lights":neg,"point":[0.,0.,0.],"normal":[0.,0.,1.],"eye":[0.,0.,4.]})).unwrap_err(),"bounded finite light values required");
  assert_eq!(run("sample_direct_lighting",json!({"material":m,"point":[0.,0.,0.],"normal":[0.,0.,1.],"eye":[0.,0.,4.]})).unwrap_err(),"lights required");
 }
 #[test]fn obj_and_glb_exports_write_exactly_the_reported_bytes(){
  let d=temp("export");
  let obj=d.join("t.obj");let r=run("export_vertex_stream_obj",json!({"stream":tri(),"path":obj.to_str().unwrap()})).unwrap();
  let text=std::fs::read_to_string(&obj).unwrap();assert_eq!(r["bytes"].as_u64(),Some(text.len() as u64));
  assert_eq!(text.lines().filter(|l|l.starts_with("v ")).count(),3);assert_eq!(text.lines().filter(|l|l.starts_with("f ")).count(),1);
  assert_eq!(run("export_vertex_stream_obj",json!({"stream":tri(),"path":"t.txt"})).unwrap_err(),"OBJ path required");
  assert!(run("export_vertex_stream_obj",json!({"stream":tri(),"path":d.join("missing/t.obj").to_str().unwrap()})).is_err(),"unwritable path");
  for minimal in [false,true]{let glb=d.join(format!("t{minimal}.glb"));
   let r=run("export_vertex_stream_gltf",json!({"stream":tri(),"path":glb.to_str().unwrap(),"spec_minimal":minimal})).unwrap();
   let bytes=std::fs::read(&glb).unwrap();assert_eq!(&bytes[..4],b"glTF");assert_eq!(r["bytes"].as_u64(),Some(bytes.len() as u64));assert_eq!(r["spec_minimal"],minimal);}
  assert_eq!(run("export_vertex_stream_gltf",json!({"stream":tri(),"path":"t.gltf"})).unwrap_err(),"GLB path required");
  assert!(run("export_vertex_stream_gltf",json!({"stream":tri(),"path":d.join("missing/t.glb").to_str().unwrap()})).is_err(),"unwritable path");
  assert_eq!(run("export_vertex_stream_gltf",json!({"path":"t.glb"})).unwrap_err(),"stream required");
  std::fs::remove_dir_all(&d).unwrap();
 }
 #[test]fn pbr_render_writes_a_png_of_the_covered_triangle(){
  let d=temp("pbr");let png=d.join("t.png");
  let cam=json!({"eye":[0.25,0.25,4.],"pan":180.,"tilt":0.,"roll":0.,"sensor_height":24.,"focal_length":35.,"near":0.1});
  let r=run("render_vertex_stream_pbr",json!({"stream":tri(),"camera":cam,"material":{"albedo":[0.5,0.5,0.5],"metallic":0.,"roughness":0.5},"lights":[{"kind":"directional","travel_direction":[0.,0.,-1.],"radiance":[1.,1.,1.]}],"width":16,"height":16,"path":png.to_str().unwrap()})).unwrap();
  assert!(r["covered_pixels"].as_u64().unwrap()>0);assert_eq!(r["depth_covered"],r["covered_pixels"]);
  assert!(std::fs::metadata(&png).unwrap().len()>0);
  assert_eq!(run("render_vertex_stream_pbr",json!({"path":"t.jpg"})).unwrap_err(),"PNG required");
  assert_eq!(run("render_vertex_stream_pbr",json!({"path":"t.png"})).unwrap_err(),"stream required");
  std::fs::remove_dir_all(&d).unwrap();
 }
}
