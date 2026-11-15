//! Official, hash-pinned component packs. Reuses the updater transaction and rollback.
use crate::{error::{Error,Result},http,paths,payload,updater::{self,Reporter}};
use serde_json::{json,Value};
use std::{collections::BTreeSet,path::Path};

fn fail(message:&str)->Error {Error(message.into())}
fn digest(v:&Value)->Result<&str> {v.as_str().filter(|s|s.len()==64&&s.bytes().all(|b|b.is_ascii_hexdigit())).ok_or_else(||fail("模块摘要无效"))}
fn allowed(path:&str)->bool {
    !path.contains('\\') && !path.split('/').any(|s|s.is_empty()||s=="."||s==".."||s.contains(':')) &&
    (path.starts_with("Assets/") || !path.contains('/') && (path.ends_with(".exe")||path.ends_with(".dll")))
}
pub fn validate(index:&Value,version:&str)->Result<()> {
    if index["schemaVersion"]!=1 || index["version"]!=version || index["runtimeVersion"].as_str().is_none_or(|s|s.is_empty()) {return Err(fail("模块清单版本不兼容"));}
    let packages=index["packages"].as_array().filter(|a|!a.is_empty()&&a.len()<=128).ok_or_else(||fail("模块列表无效"))?;
    let mut paths=BTreeSet::new();let mut ids=BTreeSet::new();
    for p in packages {
        let id=p["id"].as_str().ok_or_else(||fail("模块标识无效"))?;
        if !ids.insert(id) {return Err(fail("重复的模块标识"));}
        let archive=p["archive"].as_str().ok_or_else(||fail("模块文件名无效"))?;
        if archive.contains(['/', '\\',':']) || !archive.ends_with(".zip") || archive.starts_with('.') {return Err(fail("模块文件名无效"));}
        digest(&p["sha256"])?;
        let files=p["files"].as_object().filter(|f|!f.is_empty()&&f.len()<=10000).ok_or_else(||fail("模块文件列表无效"))?;
        for (path,hash) in files {if !allowed(path)||!paths.insert(path.to_ascii_lowercase()){return Err(fail("模块包含非法或重复路径"));}digest(hash)?;}
    }
    for required in payload::required_runtime() {if !paths.contains(&required.to_ascii_lowercase()) {return Err(fail("模块清单缺少运行时必要文件"));}}
    Ok(())
}
/// None means the publisher supplied no component index: the legacy full update remains available.
pub fn prepare(target:&Path,version:&str,scratch:&Path,reporter:&mut Reporter)->Result<bool> {
    let base=format!("https://github.com/Heiamer/DotIsle/releases/download/v{version}/");
    let name=format!("LyricIsland-{version}-Modules.json");
    let hashes=match http::get_bytes(&format!("{base}LyricIsland-{version}-SHA256.txt"),2*1024*1024) {Ok(b)=>b,Err(_)=>return Ok(false)};
    let hashes=String::from_utf8_lossy(&hashes);
    let expected=hashes.lines().find_map(|line|{let mut parts=line.split_whitespace();let h=parts.next()?;let n=parts.next()?;(n==name).then_some(h)});
    let Some(expected)=expected else {return Ok(false)};
    let bytes=http::get_bytes(&format!("{base}{name}"),4*1024*1024)?;
    if lyric_archive::sha256_bytes(&bytes)!=expected {return Err(fail("模块索引校验失败"));}
    let index:Value=serde_json::from_slice(&bytes)?;validate(&index,version)?;
    let stage=scratch.join("stage");std::fs::create_dir_all(&stage)?;
    let mut received=0;
    for (i,pack) in index["packages"].as_array().unwrap().iter().enumerate() {
        let files=pack["files"].as_object().unwrap();
        let unchanged=files.iter().all(|(name,hash)| paths::safe_child(target,name).ok().filter(|p|p.is_file()).and_then(|p|lyric_archive::sha256_file(&p).ok()).as_deref()==hash.as_str());
        if unchanged {continue;}
        let archive=scratch.join(format!("module-{i}.zip"));
        let url=format!("{base}{}",pack["archive"].as_str().unwrap());
        let (count,_)=http::download(&url,&archive,256*1024*1024,|n,_|{reporter.report("downloading",received+n,None,"正在下载变化的功能或资源包",false).map(|_|())})?;
        received+=count;reporter.report("verifying",received,None,"正在校验模块文件",true)?;
        updater::verify(&archive,digest(&pack["sha256"])?)?;
        let extracted=scratch.join(format!("module-{i}"));payload::extract_zip_file(&archive,&extracted,"Module",&[],512*1024*1024)?;
        let actual=payload::relative_files(&extracted)?;
        if actual.len()!=files.len() {return Err(fail("模块内容与索引不一致"));}
        for name in actual {
            let hash=files.get(&name).ok_or_else(||fail("模块含未声明文件"))?;
            let source=paths::safe_child(&extracted,&name)?;
            if lyric_archive::sha256_file(&source)?!=digest(hash)? {return Err(fail("模块文件校验失败"));}
            let destination=paths::safe_child(&stage,&name)?;if let Some(p)=destination.parent(){std::fs::create_dir_all(p)?;}std::fs::copy(source,destination)?;
        }
    }
    // Both the unchanged local files and staged replacements must match the pinned index.
    for pack in index["packages"].as_array().unwrap() {for (name,hash) in pack["files"].as_object().unwrap(){let staged=paths::safe_child(&stage,name)?;let file=if staged.is_file(){staged}else{paths::safe_child(target,name)?};if lyric_archive::sha256_file(&file)?!=digest(hash)?{return Err(fail("安装前模块校验失败"));}}}
    paths::write_json(&stage.join("ModuleState.json"),&json!({"version":version,"runtimeVersion":index["runtimeVersion"],"schemaVersion":1}))?;
    Ok(true)
}
#[cfg(test)]mod tests {
 use super::*;
 #[test]fn paths_reject_traversal_and_userdata(){for p in ["../x.exe","Assets/../x.exe","Assets\\x","Settings.json","Assets/a:b"]{assert!(!allowed(p),"{p}");}assert!(allowed("Assets/Sounds/bell.wav"));assert!(allowed("LyricIsland.exe"));}
 #[test]fn incompatible_indexes_are_rejected(){assert!(validate(&json!({"schemaVersion":2}),"3.5.2").is_err());assert!(validate(&json!({"schemaVersion":1,"version":"3.5.2","runtimeVersion":"3.5.1","packages":[]}),"3.5.2").is_err());}
}
