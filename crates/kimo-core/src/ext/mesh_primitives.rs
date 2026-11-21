use super::kit::*;
fn read<T:serde::de::DeserializeOwned>(a:&Value,k:&str)->Result<T,String>{serde_json::from_value(a.get(k).ok_or_else(||format!("{k} required"))?.clone()).map_err(|e|e.to_string())}
pub fn commands()->Vec<Command>{vec![
 Command{name:"generate_mesh_primitive",description:"Raw cube/sphere/ring-torus generator. Explicit dimensions and tessellation; returns interleaved position/normal/UV/tangent vertices plus triangle indices. No scene placement/material/preset.",mutates:false,schema:obj(vec![("primitive",json!({"type":"object"}))],&["primitive"]),run:|_,a|{let g=crate::threed::primitives::generate(&read(a,"primitive")?)?;Ok(json!({"inspection":g.verify()?,"geometry":g}))}},
 Command{name:"mesh_lathe",description:"Raw lathe/revolution surface: revolve an open (radius,y) profile about the Y axis with segment count and optional end caps. Returns interleaved position/normal/UV/tangent vertices plus triangle indices. Radius must stay 1e-6..1e6 (no poles; use small radius + cap). No scene placement/material/preset.",mutates:false,schema:obj(vec![("profile",json!({"type":"array","description":"[[radius,y],...] 2..512 points"})),("segments",json!({"type":"integer","description":"3..256"})),("caps",json!({"type":"boolean"}))],&["profile","segments","caps"]),run:|_,a|{let g=crate::threed::primitives::generate(&crate::threed::primitives::Primitive::Lathe{profile:read(a,"profile")?,segments:read(a,"segments")?,caps:read(a,"caps")?})?;Ok(json!({"inspection":g.verify()?,"geometry":g}))}},
 Command{name:"mesh_extrude_path",description:"Extrude a closed 2D profile along a cubic bezier path into a raw mesh, with optional end caps. Raw path extrusion: sweep a closed 2D profile polygon along a cubic bezier path (4 control points) with parallel-transport frames, arc-length stations and optional end caps. Returns interleaved position/normal/UV/tangent vertices plus triangle indices. No scene placement/material/preset.",mutates:false,schema:obj(vec![("profile",json!({"type":"array","description":"closed polygon [[u,v],...] 3..512 points"})),("path",json!({"type":"array","description":"4 cubic bezier control points [[x,y,z],...]"})),("stations",json!({"type":"integer","description":"2..256"})),("caps",json!({"type":"boolean"}))],&["profile","path","stations","caps"]),run:|_,a|{let g=crate::threed::primitives::generate(&crate::threed::primitives::Primitive::ExtrudePath{profile:read(a,"profile")?,path:read(a,"path")?,stations:read(a,"stations")?,caps:read(a,"caps")?})?;Ok(json!({"inspection":g.verify()?,"geometry":g}))}},
 Command{name:"mesh_subdivide",description:"Catmull-Clark subdivision of a raw triangle mesh (1..6 levels) with optional edge creases. Uniform Catmull-Clark subdivision of a raw triangle mesh via opensubdiv-rs (1..6 levels), with optional edge creases (weight 0..10, 10 = fully sharp). Input positions are welded by exact bit match (seam-duplicated vertices tear). Output is refined raw position/index buffers plus topology inspection: Euler invariance, 2-manifold edge check, boundary preservation, zero-area count. Non-manifold or unstable input is rejected as an error, never a panic. No scene placement/material/preset.",mutates:false,schema:obj(vec![("mesh",json!({"type":"object","description":"{positions:[[x,y,z],...],indices:[[a,b,c],...]}; alternative: pass geometry"})),("geometry",json!({"type":"object","description":"geometry object returned by generate_mesh_primitive/mesh_lathe/mesh_extrude_path"})),("levels",json!({"type":"integer","description":"1..6"})),("creases",json!({"type":"array","description":"optional [[[x,y,z],[x,y,z]],...] base-edge endpoint position pairs"})),("crease_weight",json!({"type":"number","description":"0..10, default 10 = fully sharp"}))],&["levels"]),run:|_,a|{let m:crate::threed::mesh::Mesh=if let Some(g)=a.get("geometry"){let g:crate::threed::primitives::Geometry=serde_json::from_value(g.clone()).map_err(|e|e.to_string())?;g.mesh()}else{read(a,"mesh")?};let creases:Vec<[[f64;3];2]>=if a.get("creases").is_some(){read(a,"creases")?}else{Vec::new()};let w:f32=a.get("crease_weight").and_then(|v|v.as_f64()).unwrap_or(10.0) as f32;let r=crate::threed::subdivide::catmull_clark(&m,read(a,"levels")?,&creases,w)?;Ok(json!({"inspection":{"base_vertices":r.base_vertices,"final_vertices":r.final_vertices,"final_triangles":r.final_triangles,"euler_characteristic":r.euler_characteristic,"boundary_edges":r.boundary_edges,"interior_edges_two_manifold":r.interior_edges_two_manifold,"zero_area_triangles":r.zero_area_triangles,"backend":"opensubdiv-rs 0.1.3 catmark"},"mesh":r.mesh}))}},
 Command{name:"mesh_boolean",description:"Watertight boolean of two closed 2-manifold triangle meshes via vendored Manifold 3.5.3 (Apache-2.0, f32 kernel with exact predicates). Inputs are welded by exact position. Empty results (disjoint intersection, identical difference) return empty:true with null buffers, not an error. Output is re-validated: closed 2-manifold, outward CCW winding, no zero-area slivers (scale-aware). Returns refined raw position/index buffers plus topology inspection and volume. No scene placement/material/preset.",mutates:false,schema:obj(vec![("mesh_a",json!({"type":"object","description":"{positions:[[x,y,z],...],indices:[[a,b,c],...]}; alternative: geometry_a"})),("mesh_b",json!({"type":"object"})),("geometry_a",json!({"type":"object","description":"geometry object from generate_mesh_primitive/mesh_lathe/mesh_extrude_path"})),("geometry_b",json!({"type":"object"})),("op",s("union|difference|intersection"))],&["op"]),run:|_,a|{let read_mesh=|mk:&str,gk:&str|->Result<crate::threed::mesh::Mesh,String>{if let Some(g)=a.get(gk){let g:crate::threed::primitives::Geometry=serde_json::from_value(g.clone()).map_err(|e|e.to_string())?;Ok(g.mesh())}else{read(a,mk)}};let op=match str_arg(a,"op")?{"union"=>crate::threed::boolean::BoolOp::Union,"difference"=>crate::threed::boolean::BoolOp::Difference,"intersection"=>crate::threed::boolean::BoolOp::Intersection,o=>return Err(format!("op union|difference|intersection, got {o}"))};let r=crate::threed::boolean::boolean(&read_mesh("mesh_a","geometry_a")?,&read_mesh("mesh_b","geometry_b")?,op)?;Ok(json!({"inspection":{"empty":r.empty,"volume":r.volume,"vertices":r.vertices,"triangles":r.triangles,"euler_characteristic":r.euler_characteristic,"boundary_edges":r.boundary_edges,"interior_edges_two_manifold":r.interior_edges_two_manifold,"min_triangle_area":r.min_triangle_area,"backend":"manifold 3.5.3 (vendored, Apache-2.0)"},"mesh":r.mesh}))}},
 Command{name:"inspect_mesh_primitive",description:"Inspect/verify generated or edited raw position/index/unit-normal buffers: bounds/counts, zero-area and winding-vs-normal checks. Does not claim topology/watertightness proof.",mutates:false,schema:obj(vec![("geometry",json!({"type":"object"}))],&["geometry"]),run:|_,a|{let g:crate::threed::primitives::Geometry=read(a,"geometry")?;g.verify()}},
 Command{name:"render_mesh_camera_pass",description:"Raw geometry plus explicit camera-v2 pose: perspective depth, world-normal or hidden-line wireframe pass to PNG. Calls existing camera projection and near-clip math; perspective-correct depth/normals. No PBR/textures/transparency, no default camera preset. Returns coverage and depth limits for inspection.",mutates:false,schema:obj(vec![("geometry",json!({"type":"object"})),("camera",json!({"type":"object"})),("width",json!({"type":"integer"})),("height",json!({"type":"integer"})),("shading",s("normal|depth|wireframe")),("depth_range",json!({"type":"array"})),("path",s("PNG output"))],&["geometry","camera","width","height","shading","depth_range","path"]),run:|_,a|{let path=str_arg(a,"path")?;if !path.to_lowercase().ends_with(".png"){return Err("PNG path required".into());}let out=crate::threed::perspective::render(&read(a,"geometry")?,&read(a,"camera")?,read(a,"width")?,read(a,"height")?,read(a,"shading")?,read(a,"depth_range")?)?;crate::render::write_image(&out.frame,path)?;let finite=out.depth.iter().copied().filter(|v|v.is_finite()).collect::<Vec<_>>();Ok(json!({"path":path,"covered_pixels":out.covered,"clipped_triangles":out.clipped_triangles,"depth_min":finite.iter().copied().reduce(f64::min),"depth_max":finite.iter().copied().reduce(f64::max),"backend":"cpu_camera_v2_reference","normal_space":"world","perspective_correct":true}))}},
]}

#[cfg(test)]
mod tests {
 use super::*;
 fn run(name:&str,a:Value)->Result<Value,String>{let c=commands().into_iter().find(|c|c.name==name).unwrap();(c.run)(&mut Session::new(),&a)}
 fn cube()->Value{json!({"kind":"cube","size":[2.,2.,2.],"subdivisions":1})}
 /// Welded cube of side 2 centred at (dx,0,0): vertex i = (x,y,z) bits of i, outward CCW triangles.
 fn welded_cube(dx:f64)->Value{let p:Vec<[f64;3]>=(0..8).map(|i|[(if i&1==0{-1.}else{1.})+dx,if i&2==0{-1.}else{1.},if i&4==0{-1.}else{1.}]).collect();
  json!({"positions":p,"indices":[[0,4,6],[0,6,2],[1,3,7],[1,7,5],[0,1,5],[0,5,4],[2,6,7],[2,7,3],[0,2,3],[0,3,1],[4,5,7],[4,7,6]]})}
 fn tetra()->Value{json!({"positions":[[1.,1.,1.],[1.,-1.,-1.],[-1.,1.,-1.],[-1.,-1.,1.]],"indices":[[0,2,1],[0,1,3],[0,3,2],[1,2,3]]})}
 #[test]fn generate_returns_a_verified_cube_with_analytic_counts(){
  let r=run("generate_mesh_primitive",json!({"primitive":cube()})).unwrap();
  // 6 faces x (1+1)^2 vertices, 6 faces x 2 triangles
  assert_eq!((r["inspection"]["vertex_count"].as_u64(),r["inspection"]["triangle_count"].as_u64()),(Some(24),Some(12)));
  assert_eq!(r["inspection"]["dimensions"],json!([[-1.,-1.,-1.],[1.,1.,1.]]));
  assert_eq!(r["inspection"]["ccw_outward"],true);assert_eq!(r["inspection"]["zero_area_triangles"],0);
  assert_eq!(r["geometry"]["vertices"].as_array().unwrap().len(),24);
  assert_eq!(run("inspect_mesh_primitive",json!({"geometry":r["geometry"]})).unwrap(),r["inspection"],"inspect re-verifies the same buffers");
  assert_eq!(run("generate_mesh_primitive",json!({})).unwrap_err(),"primitive required");
  assert_eq!(run("generate_mesh_primitive",json!({"primitive":{"kind":"cube","size":[2.,2.,2.],"subdivisions":0}})).unwrap_err(),"cube sizes1e-6..1e6 subdivisions1..128");
  assert!(run("generate_mesh_primitive",json!({"primitive":{"kind":"cone"}})).unwrap_err().contains("unknown variant"));
 }
 #[test]fn lathe_and_extrude_counts_follow_their_formulas(){
  // lathe: (segments+1)*points vertices, 2*segments*(points-1) triangles, caps add 2 centres and 2*segments fans
  for(caps,v,t)in[(false,10,8),(true,12,16)]{let r=run("mesh_lathe",json!({"profile":[[1.,0.],[1.,1.]],"segments":4,"caps":caps})).unwrap();
   assert_eq!((r["inspection"]["vertex_count"].as_u64(),r["inspection"]["triangle_count"].as_u64()),(Some(v),Some(t)),"lathe caps={caps}");}
  assert_eq!(run("mesh_lathe",json!({"profile":[[1.,0.],[1.,1.]],"segments":2,"caps":false})).unwrap_err(),"lathe segments 3..256");
  assert_eq!(run("mesh_lathe",json!({"profile":[[1.,0.],[1.,1.]],"segments":4})).unwrap_err(),"caps required");
  // extrude: stations*points vertices, 2*(stations-1)*points triangles; caps add 2 centres and 2*points fans
  let sq=json!([[-0.5,-0.5],[0.5,-0.5],[0.5,0.5],[-0.5,0.5]]);let path=json!([[0.,0.,0.],[0.,0.,1.],[0.,0.,2.],[0.,0.,3.]]);
  for(caps,v,t)in[(false,8,8),(true,10,16)]{let r=run("mesh_extrude_path",json!({"profile":sq,"path":path,"stations":2,"caps":caps})).unwrap();
   assert_eq!((r["inspection"]["vertex_count"].as_u64(),r["inspection"]["triangle_count"].as_u64()),(Some(v),Some(t)),"extrude caps={caps}");}
  assert_eq!(run("mesh_extrude_path",json!({"profile":sq,"path":path,"stations":1,"caps":false})).unwrap_err(),"extrude stations 2..256");
  assert_eq!(run("mesh_extrude_path",json!({"profile":sq,"stations":2,"caps":false})).unwrap_err(),"path required");
 }
 #[test]fn subdivide_reports_catmull_clark_topology_from_a_mesh_or_a_geometry(){
  // tetrahedron V=4 E=6 F=4: one level gives V+E+F=14 vertices and 3 quads per triangle = 24 triangles
  let r=run("mesh_subdivide",json!({"mesh":tetra(),"levels":1})).unwrap();let i=&r["inspection"];
  assert_eq!((i["base_vertices"].as_u64(),i["final_vertices"].as_u64(),i["final_triangles"].as_u64()),(Some(4),Some(14),Some(24)));
  assert_eq!((i["euler_characteristic"].as_i64(),i["boundary_edges"].as_u64()),(Some(2),Some(0)));
  assert_eq!(i["interior_edges_two_manifold"],true);
  assert_eq!(r["mesh"]["positions"].as_array().unwrap().len(),14);
  let creased=run("mesh_subdivide",json!({"mesh":tetra(),"levels":1,"creases":[[[1.,1.,1.],[1.,-1.,-1.]]],"crease_weight":5.})).unwrap();
  assert_eq!(creased["inspection"]["final_vertices"],14);assert_ne!(creased["mesh"],r["mesh"],"a crease moves the refined points");
  // a generated cube welds its 24 face vertices to 8: V+E+F = 8+18+12 = 38
  let g=run("generate_mesh_primitive",json!({"primitive":cube()})).unwrap();
  let r=run("mesh_subdivide",json!({"geometry":g["geometry"],"levels":1})).unwrap();
  assert_eq!((r["inspection"]["base_vertices"].as_u64(),r["inspection"]["final_vertices"].as_u64(),r["inspection"]["final_triangles"].as_u64()),(Some(8),Some(38),Some(72)));
  assert_eq!(run("mesh_subdivide",json!({"mesh":tetra()})).unwrap_err(),"levels required");
  assert_eq!(run("mesh_subdivide",json!({"levels":1})).unwrap_err(),"mesh required");
  assert!(run("mesh_subdivide",json!({"geometry":{"vertices":[]},"levels":1})).is_err(),"malformed geometry");
  assert!(run("mesh_subdivide",json!({"mesh":tetra(),"levels":1,"creases":"edge"})).is_err(),"malformed creases");
 }
 #[test]fn boolean_of_two_overlapping_cubes_has_inclusion_exclusion_volumes(){
  // two 2x2x2 cubes offset by 1 along x overlap in a 1x2x2 slab: union 12, intersection 4, difference 4
  for(op,v)in[("union",12.),("intersection",4.),("difference",4.)]{
   let r=run("mesh_boolean",json!({"mesh_a":welded_cube(0.),"mesh_b":welded_cube(1.),"op":op})).unwrap();
   assert!((r["inspection"]["volume"].as_f64().unwrap()-v).abs()<1e-4,"{op}: {}",r["inspection"]["volume"]);
   assert_eq!(r["inspection"]["empty"],false);assert_eq!(r["inspection"]["boundary_edges"],0);}
  let g=run("generate_mesh_primitive",json!({"primitive":cube()})).unwrap();
  let r=run("mesh_boolean",json!({"geometry_a":g["geometry"],"mesh_b":welded_cube(0.),"op":"difference"})).unwrap();
  assert_eq!(r["inspection"]["empty"],true,"identical solids leave nothing");
  assert_eq!(run("mesh_boolean",json!({"mesh_a":welded_cube(0.),"mesh_b":welded_cube(1.),"op":"xor"})).unwrap_err(),"op union|difference|intersection, got xor");
  assert_eq!(run("mesh_boolean",json!({"mesh_b":welded_cube(1.),"op":"union"})).unwrap_err(),"mesh_a required");
  assert!(run("mesh_boolean",json!({"geometry_a":{},"mesh_b":welded_cube(1.),"op":"union"})).is_err());
 }
 #[test]fn camera_pass_writes_a_png_and_reports_depth_limits(){
  let g=run("generate_mesh_primitive",json!({"primitive":cube()})).unwrap()["geometry"].clone();
  let cam=json!({"eye":[0.,0.,-4.],"pan":0.,"tilt":0.,"roll":0.,"sensor_height":24.,"focal_length":35.,"near":0.1});
  let dir=std::env::temp_dir().join(format!("kimo-mesh-pass-{}",std::process::id()));std::fs::create_dir_all(&dir).unwrap();
  let path=dir.join("pass.png");let p=path.to_str().unwrap();
  let r=run("render_mesh_camera_pass",json!({"geometry":g,"camera":cam,"width":16,"height":16,"shading":"depth","depth_range":[1.,10.],"path":p})).unwrap();
  assert!(std::fs::metadata(&path).unwrap().len()>0);
  // the cube's front face z=-1 is 3 units from the eye; nothing is nearer
  assert!((r["depth_min"].as_f64().unwrap()-3.).abs()<1e-9,"{r}");
  assert!(r["depth_max"].as_f64().unwrap()>=3.);
  assert!(r["covered_pixels"].as_u64().unwrap()>0);
  assert_eq!(run("render_mesh_camera_pass",json!({"path":"pass.jpg"})).unwrap_err(),"PNG path required");
  assert_eq!(run("render_mesh_camera_pass",json!({"geometry":g,"path":p})).unwrap_err(),"camera required");
  std::fs::remove_dir_all(&dir).unwrap();
 }
}
