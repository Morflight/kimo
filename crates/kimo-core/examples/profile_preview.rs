use kimo_core::{model::Project,render,preview_stream};
use std::{time::Instant,sync::{Mutex,atomic::{AtomicUsize,Ordering}}};
fn main()->Result<(),String>{
 let args:Vec<String>=std::env::args().collect();
 let p:Project=serde_json::from_slice(&std::fs::read(&args[1]).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
 let out=std::path::Path::new(&args[2]);std::fs::create_dir_all(out).map_err(|e|e.to_string())?;
 let _nb=kimo_core::analysis_proxy::NonBlocking::new();
 if std::env::var_os("KIMO_PROFILE_VERIFY").is_some() {
  let mut original=render::Cache::new(); original.decode_budget=Some(1<<30);
  render::prepare_stream_range(&p,&mut original,2.,2.25)?;
  let before=render::export_frame_rgba8(&p,2.,&original)?;
  let mut prefetched=original.snapshot();let mut worker=render::StreamPrefetch::new();
  worker.start(std::sync::Arc::new(p.clone()),&original,7,3.,3.25);
  worker.finish(8,&mut original);
  assert_eq!(before,render::export_frame_rgba8(&p,2.,&original)?,"stale generation must not replace current cache");
  worker.start(std::sync::Arc::new(p.clone()),&prefetched,9,3.,3.25);worker.finish(9,&mut prefetched);
  render::prepare_stream_range(&p,&mut original,3.,3.25)?;
  for t in [3.,3.1,3.25]{assert_eq!(render::export_frame_rgba8(&p,t,&original)?,render::export_frame_rgba8(&p,t,&prefetched)?,"prefetch changed pixels");}
  let mut sparse=render::Cache::new();sparse.decode_budget=Some(1<<30);sparse.sample_step=2;
  render::prepare_stream_range(&p,&mut original,4.,4.5)?;
  render::prepare_stream_range(&p,&mut sparse,4.,4.5)?;
  for i in 0..16{let t=(240+i*2) as f64/p.fps;assert_eq!(render::export_frame_rgba8(&p,t,&original)?,render::export_frame_rgba8(&p,t,&sparse)?,"sparse preview changed a selected image");}
  for t in [4.,4.+2./60.,4.5] {let direct=render::opaque_preview_frame(&p,t,&sparse).ok_or("plain clip did not select direct preview")?;assert_eq!(direct,render::export_frame_rgba8(&p,t,&original)?,"direct preview changed pixels");}
  for property in ["opacity","scale","rotation","x"] {let mut changed=p.clone();std::sync::Arc::make_mut(&mut changed.tracks[0].clips[0]).props.get_mut(property).unwrap().base=0.5;assert!(render::opaque_preview_frame(&changed,4.,&sparse).is_none(),"transformed clip must use compositor");}
  let mut layered=p.clone();let extra=layered.tracks[0].clips[0].clone();layered.tracks[0].clips.push(extra);assert!(render::opaque_preview_frame(&layered,4.,&sparse).is_none());
  let mut cuts=p.clone();let mut later=(*cuts.tracks[0].clips[0]).clone();later.id="later-clip".into();later.start=23.8;later.duration=6.2;cuts.tracks[0].clips.push(std::sync::Arc::new(later));
  assert!(render::opaque_preview_frame(&cuts,4.,&sparse).is_some(),"inactive clip must not block the direct path");
  render::prepare_stream_range(&cuts,&mut sparse,24.,24.3)?;
  for t in [24.,24.+2./60.,24.3]{let direct=render::opaque_preview_frame(&cuts,t,&sparse).ok_or("direct path missing after cut")?;assert_eq!(direct,render::export_frame_rgba8(&cuts,t,&sparse)?,"direct path changed pixels after cut");}
  println!("prefetch pixel equality and stale-generation rejection passed");return Ok(());
 }
 let pipeline=std::env::var_os("KIMO_PROFILE_PIPELINE").is_some();
 for stride in if pipeline {vec![1usize,2]} else {vec![1usize,2,4,16]}{
  let mut cache=render::Cache::new();cache.sample_step=if std::env::var_os("KIMO_PROFILE_SPARSE").is_some(){stride.min(2)}else{1};cache.decode_budget=Some((kimo_core::guard::limits().mem_bytes/3).min(1<<30).max(1<<20));
  let mut prefetch=render::StreamPrefetch::new();
  for round in 0..if pipeline {12}else{3}{
   let frame0=if pipeline {120+round*16*stride}else{((2.0+round as f64*5.0)*p.fps).round() as usize};let t0=frame0 as f64/p.fps;let mut n=16usize;let start=Instant::now();
   prefetch.finish(1,&mut cache);
   loop{match render::prepare_stream_range(&p,&mut cache,t0,(frame0+(n-1)*stride) as f64/p.fps){Ok(())=>break,Err(e) if e.contains("exceeds cache budget")&&n>1=>n/=2,Err(e)=>return Err(e)}}
   if pipeline && std::env::var_os("KIMO_PROFILE_NO_PREFETCH").is_none() && round<11 {let a=(frame0+n*stride) as f64/p.fps;let b=(frame0+(n+15)*stride) as f64/p.fps;prefetch.start(std::sync::Arc::new(p.clone()),&cache,1,a,b);}
   let prep_ms=start.elapsed().as_secs_f64()*1000.;
   let render_start=Instant::now();let next=AtomicUsize::new(0);let results:Vec<_>=(0..n).map(|_|Mutex::new(None)).collect();
   std::thread::scope(|s|{for _ in 0..std::env::var("KIMO_PROFILE_WORKERS").ok().and_then(|s|s.parse::<usize>().ok()).unwrap_or(8).min(n){let (p,cache,next,results)=(&p,&cache,&next,&results);s.spawn(move||{loop{let i=next.fetch_add(1,Ordering::Relaxed);if i>=n{break}let a=Instant::now();let at=(frame0+i*stride) as f64/p.fps;let direct=if std::env::var_os("KIMO_PROFILE_DIRECT").is_some(){render::opaque_preview_frame(p,at,cache)}else{None};let owned=if direct.is_none(){Some(render::export_frame_rgba8(p,at,cache).unwrap())}else{None};let rgba=direct.unwrap_or_else(||owned.as_ref().unwrap());let compose=a.elapsed().as_secs_f64()*1000.;let b=Instant::now();let small=kimo_core::ext::preview_range::box_downscale(rgba,p.width as usize,p.height as usize,540,960);let bmp=preview_stream::bmp(&small,540,960);*results[i].lock().unwrap()=Some((bmp,compose,b.elapsed().as_secs_f64()*1000.));}});}});
   let render_wall_ms=render_start.elapsed().as_secs_f64()*1000.;let mut compose_cpu_ms=0.;let mut shrink_cpu_ms=0.;let write_start=Instant::now();
   for (i,r) in results.into_iter().enumerate(){let(bytes,c,d)=r.into_inner().unwrap().unwrap();compose_cpu_ms+=c;shrink_cpu_ms+=d;let path=out.join(format!("{i}.part"));let dest=out.join(format!("{i}.bmp"));std::fs::write(&path,bytes).map_err(|e|e.to_string())?;if dest.exists(){std::fs::remove_file(&dest).map_err(|e|e.to_string())?;}std::fs::rename(path,dest).map_err(|e|e.to_string())?;}
   println!("{}",serde_json::json!({"stride":stride,"round":round,"frames":n,"prepare_ms":prep_ms,"render_wall_ms":render_wall_ms,"compose_sum_ms":compose_cpu_ms,"downscale_bmp_sum_ms":shrink_cpu_ms,"write_ms":write_start.elapsed().as_secs_f64()*1000.,"total_ms":start.elapsed().as_secs_f64()*1000.,"fps":n as f64/start.elapsed().as_secs_f64(),"proxies":cache.proxy_assets()}));
  }
 }
 Ok(())
}
