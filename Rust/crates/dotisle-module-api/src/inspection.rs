//! Runs only inside the updater's disposable inspection process.
use super::*;
pub fn inspect(path:&std::path::Path)->Result<Value,String>{
 use std::os::windows::ffi::OsStrExt;
 let path=path.canonicalize().map_err(|e|e.to_string())?;let wide=path.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<_>>();
 unsafe {let handle=LoadLibraryExW(wide.as_ptr(),std::ptr::null_mut(),0x100|0x1000);if handle.is_null(){return Err(format!("模块无法加载：{}",std::io::Error::last_os_error()));}
  let symbol=|name:&str|GetProcAddress(handle,CString::new(name).unwrap().as_ptr().cast());let call=symbol("dotisle_service_call");let free=symbol("dotisle_service_free");
  if !call.is_null()&&!free.is_null(){let lib=Library{_handle:handle as usize,call:std::mem::transmute(call),free:std::mem::transmute(free)};let mut info=lib.invoke(&json!({"method":"describe"}))?;let health=lib.invoke(&json!({"method":"health"}))?;if health["success"]!=true{return Err("模块未通过兼容性自检".into());}info["health"]=health;return Ok(info);}
  let abi=symbol("dotisle_module_abi");let kind=symbol("dotisle_module_kind");
  if abi.is_null()||kind.is_null(){return Err("文件不是点屿内部模块".into());}
  let abi:unsafe extern "C" fn()->u32=std::mem::transmute(abi);let kind:unsafe extern "C" fn()->u32=std::mem::transmute(kind);
  let id=match kind(){1=>"core",2=>"settings",_=>return Err("模块角色无效".into())};let mut v=metadata(id,"unknown");v["abi"]=json!(abi());Ok(v)
 }
}
pub fn validate_set(items:&[Value])->Result<(),String>{
 let core=items.iter().find(|i|i["id"]=="core").ok_or("缺少 CORE")?;let api=core["coreApi"].as_u64().ok_or("CORE 接口版本缺失")?;
 for item in items {let id=item["id"].as_str().ok_or("模块标识缺失")?;
  if filename(id).is_none()||item["abi"]!=ABI||item["dataSchema"]!=1{return Err(format!("{id} 的模块接口或数据格式不兼容"));}
  if item["coreMin"].as_u64().is_none_or(|n|api<n)||item["coreMax"].as_u64().is_none_or(|n|api>n){return Err(format!("{id} 不兼容 CORE API {api}"));}
  for dependency in item["dependencies"].as_array().into_iter().flatten(){if !items.iter().any(|i|i["id"]==*dependency){return Err(format!("{id} 缺少依赖 {dependency}"));}}
 }
 for (id,_) in MODULES{if !items.iter().any(|i|i["id"]==*id){return Err(format!("缺少模块 {id}"));}}
 Ok(())
}
#[cfg(test)]mod tests{
 use super::*;
 fn valid()->Vec<Value>{MODULES.iter().map(|(id,_)|metadata(id,"1.0.0")).collect()}
 #[test]fn compatibility_checks_core_range_and_required_dependencies(){let modules=valid();assert!(validate_set(&modules).is_ok());let mut wrong=modules.clone();wrong.iter_mut().find(|m|m["id"]=="focus").unwrap()["coreMin"]=json!(2);assert!(validate_set(&wrong).is_err());let mut missing=modules.clone();missing.retain(|m|m["id"]!="lyrics");assert!(validate_set(&missing).is_err());let mut abi=modules;abi[0]["abi"]=json!(2);assert!(validate_set(&abi).is_err());}
}
