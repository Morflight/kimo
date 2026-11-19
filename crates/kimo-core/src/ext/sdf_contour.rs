use super::kit::*;fn read<T:serde::de::DeserializeOwned>(a:&Value,k:&str)->Result<T,String>{serde_json::from_value(a.get(k).ok_or_else(||format!("{k} required"))?.clone()).map_err(|e|e.to_string())}
pub fn commands()->Vec<Command>{vec![
 Command{name:"inspect_sdf_graph",description:"Validate raw sphere/box/plane and hard/smooth union/intersect/subtract indexed CSG graph. Explicit earlier-node references, no presets, no exact-distance/topology claim.",mutates:false,schema:obj(vec![("graph",json!({"type":"object"}))],&["graph"]),run:|_,a|{let g:crate::threed::sdf_graph::Graph=read(a,"graph")?;g.validate()?;Ok(json!({"nodes":g.nodes,"output":g.output,"validated":true,"universally_exact_distance":false}))}},
 Command{name:"sample_sdf_graph",description:"Sample explicit bounded CSG field at caller point; original primitive/CSG math, no mesh or opaque pipeline.",mutates:false,schema:obj(vec![("graph",json!({"type":"object"})),("point",json!({"type":"array"}))],&["graph","point"]),run:|_,a|{let g:crate::threed::sdf_graph::Graph=read(a,"graph")?;Ok(json!({"field":g.sample(read(a,"point")?)?}))}},
 Command{name:"contour_sdf_graph",description:"Feature-gated wrap of Apache isosurface DualContouring/MinimiseQEF. Explicit bounds/resolution/normal epsilon; returns editable48-byte stream plus sampled field residual and welded-topology census (boundary/nonmanifold edges, Euler, CCW winding, signed volume, slivers). topology_verified only when closed 2-manifold, consistently outward-wound, positive volume. Analytic closed-form Hermite normals at cell corners with FD fallback (count reported). Empty isosurface returns empty:true. Finite-grid approximation, not exact mesh; near-tangent smooth blends with fillet radius under ~2 cells can crumple in the crease (geometry limit, octree unit planned). No UV parameterization.",mutates:false,schema:obj(vec![("graph",json!({"type":"object"})),("config",json!({"type":"object"}))],&["graph","config"]),run:|_,a|{
 #[cfg(feature="surface-contour")]{Ok(json!(crate::threed::contour_wrapped::polygonize(&read(a,"graph")?,&read(a,"config")?)?))}
 #[cfg(not(feature="surface-contour"))]{let _=a;Err("surface-contour feature not compiled".into())}
 }}]}

#[cfg(test)]
mod tests {
 use super::*;
 fn run(name:&str,a:Value)->Result<Value,String>{let c=commands().into_iter().find(|c|c.name==name).unwrap();(c.run)(&mut Session::new(),&a)}
 fn sphere()->Value{json!({"nodes":[{"kind":"sphere","center":[0.,0.,0.],"radius":1.}],"output":0})}
 #[test]fn inspect_and_sample_a_unit_sphere_graph(){
  let r=run("inspect_sdf_graph",json!({"graph":sphere()})).unwrap();
  assert_eq!(r["validated"],true);assert_eq!(r["output"],0);assert_eq!(r["nodes"],sphere()["nodes"]);
  assert!(run("inspect_sdf_graph",json!({"graph":{"nodes":[{"kind":"sphere","center":[0.,0.,0.],"radius":1.}],"output":3}})).is_err(),"output past the last node");
  assert_eq!(run("inspect_sdf_graph",json!({})).unwrap_err(),"graph required");
  // |(2,0,0)| - 1 = 1; inside at the centre: -1
  assert_eq!(run("sample_sdf_graph",json!({"graph":sphere(),"point":[2.,0.,0.]})).unwrap(),json!({"field":1.}));
  assert_eq!(run("sample_sdf_graph",json!({"graph":sphere(),"point":[0.,0.,0.]})).unwrap(),json!({"field":-1.}));
  assert_eq!(run("sample_sdf_graph",json!({"graph":sphere()})).unwrap_err(),"point required");
 }
 #[cfg(feature="surface-contour")]
 #[test]fn contour_of_a_sphere_is_a_non_empty_mesh_and_of_empty_space_is_empty(){
  let cfg=json!({"bounds":[[-2.,-2.,-2.],[2.,2.,2.]],"resolution":8,"normal_epsilon":1e-4});
  let r=run("contour_sdf_graph",json!({"graph":sphere(),"config":cfg})).unwrap();
  assert_eq!(r["empty"],false);
  let far=json!({"nodes":[{"kind":"sphere","center":[10.,0.,0.],"radius":1.}],"output":0});
  assert_eq!(run("contour_sdf_graph",json!({"graph":far,"config":cfg})).unwrap()["empty"],true);
  assert_eq!(run("contour_sdf_graph",json!({"graph":sphere()})).unwrap_err(),"config required");
 }
}
