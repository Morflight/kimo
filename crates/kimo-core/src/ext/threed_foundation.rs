use super::kit::*;
fn value<T:serde::de::DeserializeOwned>(a:&Value,k:&str)->Result<T,String>{serde_json::from_value(a.get(k).ok_or_else(||format!("{k} required"))?.clone()).map_err(|e|e.to_string())}
fn command(name:&'static str,description:&'static str,props:Vec<(&str,Value)>,req:&[&str],run:fn(&mut Session,&Value)->Result<Value,String>)->Command{Command{name,description,mutates:false,schema:obj(props,req),run}}
pub fn commands()->Vec<Command>{vec![
 command("inspect_threed_backend","Report compiled backend scaffold and exact fallback limits. GPU render is not implemented; CPU raster is orthographic reference only.",vec![],&[],|_,_|Ok(json!(crate::threed::backend::capabilities()))),
 command("inspect_mesh_buffers","Validate raw position/index buffers and return bounds/counts. No project mutation, material or texture import.",vec![("mesh",json!({"type":"object"}))],&["mesh"],|_,a|{let m:crate::threed::mesh::Mesh=value(a,"mesh")?;Ok(json!({"bounds":m.bounds()?,"vertices":m.positions.len(),"triangles":m.indices.len()}))}),
 command("inspect_scene_matrices","Evaluate explicit affine parent hierarchy; reject cycles/missing references. Stateless raw nodes, not persisted project scene.",vec![("nodes",json!({"type":"array"})),("mesh_count",json!({"type":"integer","minimum":0}))],&["nodes","mesh_count"],|_,a|Ok(json!({"world":crate::threed::mesh::world_matrices(&value::<Vec<crate::threed::mesh::Node>>(a,"nodes")?,value(a,"mesh_count")?)?}))),
 command("raycast_mesh_raw","Build bounded original BVH and raycast raw mesh. Return triangle/t/barycentrics; no persistent cache.",vec![("mesh",json!({"type":"object"})),("ray",json!({"type":"object"}))],&["mesh","ray"],|_,a|{let m=value(a,"mesh")?;let b=crate::threed::spatial::Bvh::build(&m)?;Ok(json!({"hit":b.raycast(&m,&value(a,"ray")?)?,"nodes":b.nodes.len()}))}),
 command("sample_sdf_raw","Sample explicit sphere/box/CSG tree, bounded depth. No mesh polygonization or preset.",vec![("field",json!({"type":"object"})),("point",json!({"type":"array"}))],&["field","point"],|_,a|{let f:crate::threed::sdf::Field=value(a,"field")?;Ok(json!({"distance_field":f.sample(value(a,"point")?)?,"mesh_generated":false}))}),
 command("sample_pbr_raw","Raw GGX Smith Schlick plus Lambert/pi BRDF, not radiance and not multiplied by N.L. No legacy material change.",vec![("material",json!({"type":"object"})),("normal",json!({"type":"array"})),("light",json!({"type":"array"})),("view",json!({"type":"array"}))],&["material","normal","light","view"],|_,a|Ok(json!(crate::threed::pbr::sample(&value(a,"material")?,value(a,"normal")?,value(a,"light")?,value(a,"view")?)?))),
 command("sample_diffuse_sh_raw","Evaluate nine Y-up real SH irradiance coefficients, no projection/prefilter and no specular IBL. Returns unclamped signed value.",vec![("coefficients",json!({"type":"array"})),("normal",json!({"type":"array"}))],&["coefficients","normal"],|_,a|Ok(json!({"irradiance":crate::threed::pbr::diffuse_sh(value(a,"coefficients")?,value(a,"normal")?)?,"specular_ibl":false}))),
 command("render_mesh_cpu_reference","Write actual PNG through existing image encoder. Orthographic NDC x/y, depth0..1, constant straight RGBA, no perspective/PBR/textures. Returns finite depth statistics; background invalid depth is not JSON infinity.",vec![("mesh",json!({"type":"object"})),("width",json!({"type":"integer"})),("height",json!({"type":"integer"})),("color",json!({"type":"array"})),("path",s("PNG output"))],&["mesh","width","height","color","path"],|_,a|{let path=str_arg(a,"path")?;if !path.to_lowercase().ends_with(".png"){return Err("PNG path required".into());}let (f,d)=crate::threed::cpu::render(&value(a,"mesh")?,value(a,"width")?,value(a,"height")?,value(a,"color")?)?;crate::render::write_image(&f,path)?;Ok(json!({"path":path,"backend":"cpu_reference","covered_pixels":d.iter().filter(|v|v.is_finite()).count(),"hdr_png":false,"scene_mutated":false}))}),
 command("import_mesh_raw","Feature-gated bounded position-only triangular OBJ text or embedded buffer-only GLB bytes. Unsupported hierarchy/material/normal/UV semantics rejected; no external asset I/O and no scene mutation.",vec![("format",s("obj|glb")),("text",s("OBJ")),("bytes",json!({"type":"array"}))],&["format"],|_,a|{
 #[cfg(feature="mesh-import")]{match str_arg(a,"format")?{"obj"=>Ok(json!({"meshes":crate::threed::import::obj(str_arg(a,"text")?)?})),"glb"=>Ok(json!({"meshes":crate::threed::import::glb(&value::<Vec<u8>>(a,"bytes")?)?})),_=>Err("format obj|glb".into())}}
 #[cfg(not(feature="mesh-import"))]{let _=a;Err("mesh-import feature not compiled".into())}
 }),
]}

#[cfg(test)]
mod tests {
 use super::*;
 fn run(name:&str,a:Value)->Result<Value,String>{let c=commands().into_iter().find(|c|c.name==name).unwrap();(c.run)(&mut Session::new(),&a)}
 fn tri()->Value{json!({"positions":[[0.,0.,0.],[2.,0.,0.],[0.,3.,0.]],"indices":[[0,1,2]]})}
 fn translate(x:f64,y:f64,z:f64)->Value{json!([[1.,0.,0.,x],[0.,1.,0.,y],[0.,0.,1.,z],[0.,0.,0.,1.]])}
 #[test]fn backend_report_is_cpu_reference_only(){
  let r=run("inspect_threed_backend",json!({})).unwrap();
  assert_eq!(r["cpu_reference"],true);assert_eq!(r["gpu_render_implemented"],false);
 }
 #[test]fn mesh_buffers_report_bounds_and_counts(){
  assert_eq!(run("inspect_mesh_buffers",json!({"mesh":tri()})).unwrap(),json!({"bounds":[[0.,0.,0.],[2.,3.,0.]],"vertices":3,"triangles":1}));
  assert_eq!(run("inspect_mesh_buffers",json!({})).unwrap_err(),"mesh required");
  assert!(run("inspect_mesh_buffers",json!({"mesh":{"positions":[[0.,0.,0.]],"indices":[[0,1,2]]}})).is_err(),"index out of range");
  assert!(run("inspect_mesh_buffers",json!({"mesh":{"positions":[],"indices":[],"extra":1}})).is_err(),"unknown field");
 }
 #[test]fn scene_matrices_compose_parent_translations(){
  let nodes=json!([{"parent":null,"local":translate(1.,0.,0.),"mesh":0},{"parent":0,"local":translate(0.,2.,0.),"mesh":null}]);
  let r=run("inspect_scene_matrices",json!({"nodes":nodes,"mesh_count":1})).unwrap();
  assert_eq!(r["world"][0],translate(1.,0.,0.));assert_eq!(r["world"][1],translate(1.,2.,0.));
  let cycle=json!([{"parent":0,"local":translate(0.,0.,0.),"mesh":null}]);
  assert!(run("inspect_scene_matrices",json!({"nodes":cycle,"mesh_count":0})).is_err(),"self-parent cycle");
  assert_eq!(run("inspect_scene_matrices",json!({"nodes":nodes})).unwrap_err(),"mesh_count required");
 }
 #[test]fn raycast_hits_the_triangle_at_the_expected_distance(){
  let mesh=json!({"positions":[[0.,0.,0.],[1.,0.,0.],[0.,1.,0.]],"indices":[[0,1,2]]});
  let r=run("raycast_mesh_raw",json!({"mesh":mesh,"ray":{"origin":[0.25,0.25,1.],"direction":[0.,0.,-1.],"t_min":0.,"t_max":10.}})).unwrap();
  assert_eq!(r["hit"]["triangle"],0);assert!((r["hit"]["t"].as_f64().unwrap()-1.).abs()<1e-12);
  // (0.25,0.25) = 0.5*A + 0.25*B + 0.25*C
  let mut b:Vec<f64>=r["hit"]["barycentric"].as_array().unwrap().iter().map(|v|v.as_f64().unwrap()).collect();b.sort_by(f64::total_cmp);
  for(g,w)in b.iter().zip([0.25,0.25,0.5]){assert!((g-w).abs()<1e-12,"{b:?}");}
  assert!(r["nodes"].as_u64().unwrap()>=1);
  let miss=run("raycast_mesh_raw",json!({"mesh":mesh,"ray":{"origin":[0.25,0.25,1.],"direction":[0.,0.,1.],"t_min":0.,"t_max":10.}})).unwrap();
  assert!(miss["hit"].is_null(),"pointing away misses");
  assert_eq!(run("raycast_mesh_raw",json!({"mesh":mesh})).unwrap_err(),"ray required");
  assert!(run("raycast_mesh_raw",json!({"mesh":{"positions":[],"indices":[]},"ray":{}})).is_err());
 }
 #[test]fn sdf_sphere_distance_is_centre_distance_minus_radius(){
  let f=json!({"kind":"sphere","center":[0.,0.,0.],"radius":1.});
  assert_eq!(run("sample_sdf_raw",json!({"field":f,"point":[3.,0.,0.]})).unwrap(),json!({"distance_field":2.,"mesh_generated":false}));
  assert_eq!(run("sample_sdf_raw",json!({"field":f,"point":[1e10,0.,0.]})).unwrap_err(),"SDF finite point abs<=1e9 required");
  assert_eq!(run("sample_sdf_raw",json!({"point":[0.,0.,0.]})).unwrap_err(),"field required");
 }
 #[test]fn pbr_sample_is_black_below_the_horizon_and_validates_the_material(){
  let m=json!({"albedo":[0.5,0.5,0.5],"metallic":0.,"roughness":0.5});
  assert_eq!(run("sample_pbr_raw",json!({"material":m,"normal":[0.,0.,1.],"light":[0.,0.,-1.],"view":[0.,0.,1.]})).unwrap(),json!({"diffuse":[0.,0.,0.],"specular":[0.,0.,0.]}));
  let lit=run("sample_pbr_raw",json!({"material":m,"normal":[0.,0.,1.],"light":[0.,0.,1.],"view":[0.,0.,1.]})).unwrap();
  assert!(lit["diffuse"][0].as_f64().unwrap()>0.);
  assert_eq!(run("sample_pbr_raw",json!({"material":{"albedo":[0.5,0.5,0.5],"metallic":0.,"roughness":0.},"normal":[0.,0.,1.],"light":[0.,0.,1.],"view":[0.,0.,1.]})).unwrap_err(),"albedo/metallic0..1 roughness0.001..1 required");
 }
 #[test]fn sh_irradiance_of_a_constant_term_is_its_basis_value(){
  let mut c=vec![[0.;3];9];c[0]=[1.,2.,4.];
  // Y00 = 0.2820947918 for every normal
  let r=run("sample_diffuse_sh_raw",json!({"coefficients":c,"normal":[0.3,-0.2,0.9]})).unwrap();
  for(j,k)in[1.,2.,4.].iter().enumerate(){assert!((r["irradiance"][j].as_f64().unwrap()-0.2820947918*k).abs()<1e-12);}
  assert_eq!(r["specular_ibl"],false);
  assert_eq!(run("sample_diffuse_sh_raw",json!({"coefficients":c,"normal":[0.,0.,0.]})).unwrap_err(),"direction nonzero required");
 }
 #[test]fn cpu_reference_render_writes_a_png(){
  let dir=std::env::temp_dir().join(format!("kimo-threed-foundation-{}",std::process::id()));std::fs::create_dir_all(&dir).unwrap();
  let path=dir.join("ref.png");let p=path.to_str().unwrap();
  let mesh=json!({"positions":[[-1.,-1.,0.5],[3.,-1.,0.5],[-1.,3.,0.5]],"indices":[[0,1,2]]});
  let r=run("render_mesh_cpu_reference",json!({"mesh":mesh,"width":4,"height":4,"color":[1.,0.,0.,1.],"path":p})).unwrap();
  let(_,d)=crate::threed::cpu::render(&serde_json::from_value(mesh).unwrap(),4,4,[1.,0.,0.,1.]).unwrap();
  assert_eq!(r["covered_pixels"].as_u64(),Some(d.iter().filter(|v|v.is_finite()).count() as u64));
  assert!(r["covered_pixels"].as_u64().unwrap()>0);assert!(std::fs::metadata(&path).unwrap().len()>0);
  assert_eq!(run("render_mesh_cpu_reference",json!({"path":"ref.jpg"})).unwrap_err(),"PNG path required");
  assert_eq!(run("render_mesh_cpu_reference",json!({"path":p,"width":4,"height":4,"color":[1.,0.,0.,1.]})).unwrap_err(),"mesh required");
  std::fs::remove_dir_all(&dir).unwrap();
 }
 #[cfg(not(feature="mesh-import"))]
 #[test]fn raw_mesh_import_reports_the_missing_feature(){assert_eq!(run("import_mesh_raw",json!({"format":"obj","text":""})).unwrap_err(),"mesh-import feature not compiled");}
}
