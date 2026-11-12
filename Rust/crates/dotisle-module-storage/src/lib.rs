use std::path::Path;
pub fn atomic_write(path:&Path,data:&[u8])->Result<(),String>{
    let parent=path.parent().ok_or("missing parent")?;std::fs::create_dir_all(parent).map_err(|e|e.to_string())?;
    let temporary=path.with_extension(format!("{}.tmp",std::process::id()));
    let result=(||{use std::io::Write;let mut f=std::fs::File::create(&temporary)?;f.write_all(data)?;f.sync_all()?;drop(f);
        use windows::Win32::Storage::FileSystem::{MoveFileExW,MOVEFILE_REPLACE_EXISTING,MOVEFILE_WRITE_THROUGH};
        use windows::core::PCWSTR;let s=temporary.as_os_str().to_string_lossy().encode_utf16().chain(Some(0)).collect::<Vec<_>>();let d=path.as_os_str().to_string_lossy().encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        unsafe{MoveFileExW(PCWSTR(s.as_ptr()),PCWSTR(d.as_ptr()),MOVEFILE_REPLACE_EXISTING|MOVEFILE_WRITE_THROUGH).map_err(|e|std::io::Error::other(e.to_string()))}
    })();if result.is_err(){let _=std::fs::remove_file(temporary);}result.map_err(|e:std::io::Error|e.to_string())
}
pub mod io {
pub fn save_settings(path:&std::path::Path,value:&serde_json::Value)->Result<(),String>{super::atomic_write(path,&serde_json::to_vec_pretty(value).map_err(|e|e.to_string())?)}
}

pub mod http{pub fn encode(s:&str)->String{s.bytes().map(|b|if b.is_ascii_alphanumeric()||b"-_.~".contains(&b){(b as char).to_string()}else{format!("%{b:02X}")}).collect()}}
