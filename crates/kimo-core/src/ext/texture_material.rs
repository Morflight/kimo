use super::kit::*;
fn read<T:serde::de::DeserializeOwned>(a:&Value,k:&str)->Result<T,String>{serde_json::from_value(a.get(k).ok_or_else(||format!("{k} required"))?.clone()).map_err(|e|e.to_string())}
pub fn commands()->Vec<Command>{vec![Command{name:"sample_texture_material",description:"Raw level0 nearest/linear decoded RGBA8 metallic-roughness GGX evaluation. Albedo sRGB, MR G/B linear, AO R multiplies explicit diffuse ambient only. Normal OpenGL tangent-space. No mipmaps/alpha/IBL/shadows.",mutates:false,schema:obj(vec![("surface",json!({"type":"object"})),("images",json!({"type":"array"})),("uv",json!({"type":"array"})),("normal",json!({"type":"array"})),("tangent",json!({"type":"array"})),("point",json!({"type":"array"})),("eye",json!({"type":"array"})),("lights",json!({"type":"array"})),("ambient_irradiance",json!({"type":"array"}))],&["surface","images","uv","normal","tangent","point","eye","lights","ambient_irradiance"]),run:|_,a|{let surface:crate::threed::texture_material::Surface=read(a,"surface")?;let images=read::<Vec<crate::threed::texture_material::Image>>(a,"images")?;surface.validate(&images)?;Ok(json!({"linear_radiance":surface.shade(&images,read(a,"uv")?,read(a,"normal")?,read(a,"tangent")?,read(a,"point")?,read(a,"eye")?,&read::<Vec<crate::threed::lighting_stage::Light>>(a,"lights")?,read(a,"ambient_irradiance")?)?}))}},Command{name:"import_gltf_surface",description:"Import a glTF 2 surface (feature-gated) with its materials, decoded textures and baked scene transforms. Feature-gated glTF2 material mapping, explicit decoded image-index RGBA8 inputs, selected scene TRS bake, Mikk generation before node transforms for normal-mapped primitives. Reject extensions, alpha/emissive, mipmap/mixed filter and UV1 rather than silently discard.",mutates:false,schema:obj(vec![("document_bytes",json!({"type":"array"})),("buffers",json!({"type":"array"})),("images",json!({"type":"array"})),("scene",json!({"type":"integer"}))],&["document_bytes","buffers","images","scene"]),run:|_,a|{
#[cfg(all(feature="mesh-import",feature="material-textures"))]{let asset=crate::threed::gltf_surface::load(&read::<Vec<u8>>(a,"document_bytes")?,&read::<Vec<Vec<u8>>>(a,"buffers")?,read(a,"images")?,read(a,"scene")?)?;Ok(json!({"streams":asset.streams,"surfaces":asset.surfaces,"images":asset.images,"metadata":asset.metadata}))}
#[cfg(not(all(feature="mesh-import",feature="material-textures")))]{let _=a;Err("requires mesh-import,material-textures".into())}
}},Command{name:"render_textured_bvh",description:"Feature-gated CPU primary-ray textured GGX through flat SAH; explicit asset/camera/lights/ambient and target PNG. No GPU parity, mipmaps, alpha composition, shadows or specular IBL. Float linear HDR; PNG clamped linear, no display transform.",mutates:false,schema:obj(vec![("asset",json!({"type":"object"})),("camera",json!({"type":"object"})),("width",json!({"type":"integer"})),("height",json!({"type":"integer"})),("lights",json!({"type":"array"})),("ambient_irradiance",json!({"type":"array"})),("path",s("PNG"))],&["asset","camera","width","height","lights","ambient_irradiance","path"]),run:|_,a|{
#[cfg(all(feature="mesh-import",feature="material-textures",feature="flat-sah"))]{let path=str_arg(a,"path")?;if !path.to_lowercase().ends_with(".png"){return Err("PNG required".into());}let out=crate::threed::textured_bvh::render(&read(a,"asset")?,&read(a,"camera")?,read(a,"width")?,read(a,"height")?,&read::<Vec<crate::threed::lighting_stage::Light>>(a,"lights")?,read(a,"ambient_irradiance")?)?;crate::render::write_image(&out.frame,path)?;Ok(json!({"path":path,"covered_pixels":out.covered,"bvh_queries":out.bvh_queries,"backend":"cpu_primary_ray_flat_sah","encoding":"clamped linear, no display transform"}))}
#[cfg(not(all(feature="mesh-import",feature="material-textures",feature="flat-sah")))]{let _=a;Err("requires mesh-import,material-textures,flat-sah".into())}
}}]}

#[cfg(test)]
mod tests {
 use super::*;
 fn run(name:&str,a:Value)->Result<Value,String>{let c=commands().into_iter().find(|c|c.name==name).unwrap();(c.run)(&mut Session::new(),&a)}
 fn surface(albedo:f64)->Value{json!({"albedo":[albedo,albedo,albedo],"metallic":0.,"roughness":0.5,"base":null,"metal_rough":null,"occlusion":null,"normal":null,"ao_strength":1.,"normal_scale":1.,"double_sided":false})}
 fn args(s:Value,ambient:Value)->Value{json!({"surface":s,"images":[],"uv":[0.5,0.5],"normal":[0.,0.,1.],"tangent":[1.,0.,0.,1.],"point":[0.,0.,0.],"eye":[0.,0.,4.],"lights":[],"ambient_irradiance":ambient})}
 #[test]fn untextured_surface_under_ambient_only_is_irradiance_times_albedo_over_pi(){
  let pi=std::f64::consts::PI;
  let r=run("sample_texture_material",args(surface(0.5),json!([pi,2.*pi,0.]))).unwrap();
  for(j,w)in[0.5,1.,0.].iter().enumerate(){assert!((r["linear_radiance"][j].as_f64().unwrap()-w).abs()<1e-12,"{r}");}
  assert_eq!(run("sample_texture_material",args(surface(0.5),json!([-1.,0.,0.]))).unwrap_err(),"bounded ambient irradiance required");
  let mut missing=surface(0.5);missing["base"]=json!({"image":0,"wrap_s":"repeat","wrap_t":"repeat","linear":false});
  assert!(run("sample_texture_material",args(missing,json!([0.,0.,0.]))).is_err(),"slot without an image");
  let mut a=args(surface(0.5),json!([0.,0.,0.]));a.as_object_mut().unwrap().remove("uv");
  assert_eq!(run("sample_texture_material",a).unwrap_err(),"uv required");
  assert_eq!(run("sample_texture_material",json!({"surface":surface(0.5)})).unwrap_err(),"images required");
 }
 #[cfg(not(all(feature="mesh-import",feature="material-textures")))]
 #[test]fn gltf_surface_import_reports_missing_features(){assert_eq!(run("import_gltf_surface",json!({})).unwrap_err(),"requires mesh-import,material-textures");}
 #[cfg(not(all(feature="mesh-import",feature="material-textures",feature="flat-sah")))]
 #[test]fn textured_bvh_render_reports_missing_features(){assert_eq!(run("render_textured_bvh",json!({})).unwrap_err(),"requires mesh-import,material-textures,flat-sah");}
}
