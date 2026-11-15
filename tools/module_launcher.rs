// Tiny stable launcher: no renderer, feature logic, or Rust ABI crosses this boundary.
use std::{ffi::{c_void,CString},os::windows::ffi::OsStrExt};
#[link(name="kernel32")] extern "system" {
 fn LoadLibraryExW(name:*const u16,file:*mut c_void,flags:u32)->*mut c_void;
 fn GetProcAddress(module:*mut c_void,name:*const u8)->*mut c_void;
}
#[link(name="user32")] extern "system" {fn MessageBoxW(window:*mut c_void,text:*const u16,title:*const u16,flags:u32)->i32;}
fn wide(s:impl AsRef<std::ffi::OsStr>)->Vec<u16>{s.as_ref().encode_wide().chain(Some(0)).collect()}
pub fn launch(name:&str,expected_kind:u32) {
 let result=(||->Result<i32,String>{
  let exe=std::env::current_exe().map_err(|e|e.to_string())?;
  let library=exe.parent().ok_or("程序目录无效")?.join(name);
  let path=wide(library.as_os_str());
  unsafe {
   // Absolute path and restricted dependency search; the DLL stays loaded until exit.
   let module=LoadLibraryExW(path.as_ptr(),std::ptr::null_mut(),0x00000100|0x00001000);
   if module.is_null(){return Err(format!("无法加载 {}：{}",library.display(),std::io::Error::last_os_error()));}
   let abi=GetProcAddress(module,CString::new("dotisle_module_abi").unwrap().as_ptr().cast());
   let kind=GetProcAddress(module,CString::new("dotisle_module_kind").unwrap().as_ptr().cast());
   let run=GetProcAddress(module,CString::new("dotisle_module_run").unwrap().as_ptr().cast());
   if abi.is_null()||run.is_null()||kind.is_null(){return Err("模块缺少启动接口".into());}
   let abi:unsafe extern "C" fn()->u32=std::mem::transmute(abi);
   let kind:unsafe extern "C" fn()->u32=std::mem::transmute(kind);
   if kind()!=expected_kind{return Err("模块角色不匹配".into());}
   if abi()!=1{return Err("模块与启动器接口不兼容".into());}
   if std::env::args().any(|a|a=="--module-check") {return Ok(0);}
   let run:unsafe extern "C" fn()->i32=std::mem::transmute(run);Ok(run())
  }
 })();
 match result {Ok(code)=>if code!=0 {std::process::exit(code)},Err(e)=>{if std::env::args().any(|a|a=="--module-check"){eprintln!("{e}");std::process::exit(1);}let message=wide(format!("{e}\n请恢复备份模块，或使用独立更新器修复。"));unsafe{MessageBoxW(std::ptr::null_mut(),message.as_ptr(),wide("点屿模块启动失败").as_ptr(),0x10);}std::process::exit(1);}}
}
