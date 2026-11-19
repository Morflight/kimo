use super::kit::*;fn read<T:serde::de::DeserializeOwned>(a:&Value,k:&str)->Result<T,String>{serde_json::from_value(a.get(k).ok_or_else(||format!("{k} required"))?.clone()).map_err(|e|e.to_string())}
pub fn commands()->Vec<Command>{vec![Command{name:"render_offscreen_mesh_normal",description:"Optional wgpu headless world-normal raster with RGBA32f readback, camera-v2 clipping/projection. Explicit software-only adapter request and allow_cpu policy; CPU normal raster is real fallback, host lavapipe not guaranteed. Reports actual backend/reason. No GPU PBR/texture parity claim.",mutates:false,schema:obj(vec![("stream",json!({"type":"object"})),("camera",json!({"type":"object"})),("width",json!({"type":"integer"})),("height",json!({"type":"integer"})),("software_only",json!({"type":"boolean"})),("allow_cpu",json!({"type":"boolean"})),("path",s("PNG"))],&["stream","camera","width","height","software_only","allow_cpu","path"]),run:|_,a|{let path=str_arg(a,"path")?;if !path.to_lowercase().ends_with(".png"){return Err("PNG required".into());}let(f,status)=crate::threed::offscreen_wgpu::render_normal(&read(a,"stream")?,&read(a,"camera")?,read(a,"width")?,read(a,"height")?,read(a,"software_only")?,read(a,"allow_cpu")?)?;crate::render::write_image(&f,path)?;Ok(json!({"path":path,"status":status,"float_framebuffer":true,"png_clamped":true}))}}]}

#[cfg(test)]
mod tests {
 use super::*;
 fn run(a:Value)->Result<Value,String>{(commands()[0].run)(&mut Session::new(),&a)}
 fn args(path:&str)->Value{let v=|p:[f32;3]|json!({"position":p,"normal":[0.,0.,-1.],"uv":[0.,0.],"tangent":[1.,0.,0.,1.]});
  json!({"stream":{"vertices":[v([-1.,-1.,0.]),v([1.,-1.,0.]),v([0.,1.,0.])],"indices":[[0,1,2]]},"camera":{"eye":[0.,0.,-4.],"pan":0.,"tilt":0.,"roll":0.,"sensor_height":24.,"focal_length":35.,"near":0.1},"width":8,"height":8,"software_only":true,"allow_cpu":true,"path":path})}
 #[test]fn normal_pass_writes_a_png_and_reports_the_backend_actually_used(){
  let dir=std::env::temp_dir().join(format!("kimo-offscreen-mesh-{}",std::process::id()));std::fs::create_dir_all(&dir).unwrap();
  let path=dir.join("n.png");let r=run(args(path.to_str().unwrap())).unwrap();
  assert!(std::fs::metadata(&path).unwrap().len()>0);
  assert!(r["status"]["actual_backend"].as_str().is_some_and(|s|!s.is_empty()));assert_eq!(r["float_framebuffer"],true);
  assert_eq!(run(args("n.jpg")).unwrap_err(),"PNG required");
  let mut a=args(path.to_str().unwrap());a["width"]=json!(0);
  assert_eq!(run(a).unwrap_err(),"viewport1..4096");
  let mut a=args(path.to_str().unwrap());a.as_object_mut().unwrap().remove("allow_cpu");
  assert_eq!(run(a).unwrap_err(),"allow_cpu required");
  std::fs::remove_dir_all(&dir).unwrap();
 }
}
