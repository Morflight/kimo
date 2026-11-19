use super::kit::*;
pub fn commands()->Vec<Command>{vec![Command{name:"effect_uv_sample",mutates:false,description:"Raw UV math: push/radial samples, polar, spherical, cylindrical and axis-selected planar mappings. Explicit seams and singular flags. No rendered transition, easing, blur stack or hidden preset.",schema:obj(vec![("op",json!({"type":"string","enum":["push","radial","polar","spherical","cylindrical","planar"]})),("uv",json!({"type":"array","minItems":2,"maxItems":2})),("point",json!({"type":"array","minItems":3,"maxItems":3})),("axis_u",json!({"type":"integer","minimum":0,"maximum":2})),("axis_v",json!({"type":"integer","minimum":0,"maximum":2})),("progress",n("push progress 0..1")),("direction",json!({"type":"array","minItems":2,"maxItems":2})),("center",json!({"type":"array","minItems":2,"maxItems":2})),("fraction",n("radial sample 0..1")),("strength",n("raw radial strength"))],&["op"]),run:|_,a|{
 let pair=|k:&str|->Result<[f64;2],String>{serde_json::from_value(a.get(k).ok_or_else(||format!("{k} required"))?.clone()).map_err(|e|e.to_string())};let point=||->Result<[f64;3],String>{serde_json::from_value(a.get("point").ok_or("point required")?.clone()).map_err(|e|e.to_string())};
 match str_arg(a,"op")?{"push"=>Ok(json!(crate::uv_math::push(pair("uv")?,num_arg(a,"progress")?,pair("direction")?)?)),"radial"=>Ok(json!({"uv":crate::uv_math::radial(pair("uv")?,pair("center")?,num_arg(a,"fraction")?,num_arg(a,"strength")?)?})),"polar"=>Ok(json!(crate::uv_math::polar(pair("uv")?)?)),
"spherical"=>Ok(json!(crate::uv_math::spherical(point()?)?)),
"cylindrical"=>Ok(json!(crate::uv_math::cylindrical(point()?)?)),
"planar"=>{let axis=|k:&str|->Result<u8,String>{a.get(k).and_then(Value::as_u64).filter(|v|*v<=2).map(|v|v as u8).ok_or_else(||format!("{k} integer 0..2 required"))};Ok(json!(crate::uv_math::planar(point()?,axis("axis_u")?,axis("axis_v")?)?))},
s=>Err(format!("op must be push|radial|polar|spherical|cylindrical|planar, got '{s}'"))}
}}]}

#[cfg(test)]
mod tests {
 use super::*;
 fn uv(a:Value)->Result<Value,String>{(commands()[0].run)(&mut Session::new(),&a)}
 #[test]fn push_moves_the_outgoing_sample_by_progress_times_direction(){
  // (0.25,0.5) + 0.5*(1,0) = (0.75,0.5), still inside the outgoing picture
  assert_eq!(uv(json!({"op":"push","uv":[0.25,0.5],"progress":0.5,"direction":[1.,0.]})).unwrap(),json!({"from_uv":[0.75,0.5],"to_uv":[0.75,0.5],"from_weight":1.,"to_weight":0.}));
  // pushed past the edge: the incoming picture owns it at the wrapped coordinate (1.25 -> 0.25)
  assert_eq!(uv(json!({"op":"push","uv":[0.75,0.5],"progress":0.5,"direction":[1.,0.]})).unwrap(),json!({"from_uv":[1.25,0.5],"to_uv":[0.25,0.5],"from_weight":0.,"to_weight":1.}));
  assert_eq!(uv(json!({"op":"push","uv":[0.25,0.5],"progress":2.,"direction":[1.,0.]})).unwrap_err(),"finite UV/direction and progress 0..1 required");
  assert_eq!(uv(json!({"op":"push","uv":[0.25,0.5],"progress":0.5})).unwrap_err(),"direction required");
  assert!(uv(json!({"op":"push","uv":"middle","progress":0.5,"direction":[1.,0.]})).is_err());
 }
 #[test]fn radial_steps_toward_the_centre_by_fraction_times_strength(){
  assert_eq!(uv(json!({"op":"radial","uv":[0.,0.],"center":[1.,1.],"fraction":0.5,"strength":1.})).unwrap(),json!({"uv":[0.5,0.5]}));
  assert_eq!(uv(json!({"op":"radial","uv":[0.,0.],"center":[1.,1.],"fraction":1.5,"strength":1.})).unwrap_err(),"finite UV/center/strength; fraction 0..1 required");
 }
 #[test]fn mappings_match_the_uv_math_primitives(){
  assert_eq!(uv(json!({"op":"polar","uv":[0.,1.]})).unwrap(),json!(crate::uv_math::polar([0.,1.]).unwrap()));
  assert_eq!(uv(json!({"op":"spherical","point":[0.,0.,1.]})).unwrap(),json!(crate::uv_math::spherical([0.,0.,1.]).unwrap()));
  assert_eq!(uv(json!({"op":"cylindrical","point":[1.,2.,0.]})).unwrap(),json!(crate::uv_math::cylindrical([1.,2.,0.]).unwrap()));
  // planar picks the requested axes verbatim: (x,z) of (1,2,3) is (1,3)
  assert_eq!(uv(json!({"op":"planar","point":[1.,2.,3.],"axis_u":0,"axis_v":2})).unwrap(),json!({"uv":[1.,3.],"singular":false}));
  assert_eq!(uv(json!({"op":"planar","point":[1.,2.,3.],"axis_u":0,"axis_v":0})).unwrap_err(),"planar requires finite point and two distinct axes 0..2");
  assert_eq!(uv(json!({"op":"planar","point":[1.,2.,3.],"axis_u":3,"axis_v":0})).unwrap_err(),"axis_u integer 0..2 required");
  assert_eq!(uv(json!({"op":"planar","point":[1.,2.,3.],"axis_u":0})).unwrap_err(),"axis_v integer 0..2 required");
  assert_eq!(uv(json!({"op":"spherical"})).unwrap_err(),"point required");
  assert!(uv(json!({"op":"cylindrical","point":[1.,2.]})).is_err());
  assert_eq!(uv(json!({"op":"polar","uv":[f64::MAX,f64::MAX]})).unwrap_err(),"polar radius overflow");
  assert_eq!(uv(json!({"op":"swirl"})).unwrap_err(),"op must be push|radial|polar|spherical|cylindrical|planar, got 'swirl'");
 }
}
