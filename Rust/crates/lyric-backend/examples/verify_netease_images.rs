//! Static compatibility evidence; deliberately does not claim runtime validation.
fn main(){
 let mut failed=false;
 for path in std::env::args_os().skip(1){let path=std::path::PathBuf::from(path);let result=lyric_backend::netease::inspect_native_image(&path);
  match result{Ok(image)=>println!("{}",serde_json::json!({"path":path,"success":true,"image":image})),Err(error)=>{failed=true;println!("{}",serde_json::json!({"path":path,"success":false,"error":error}));}}
 }
 if failed{std::process::exit(1)}
}
