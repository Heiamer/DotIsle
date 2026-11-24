//! Cleanup is committed only after the new process confirms its version.
use crate::{error::{Error,Result},paths,payload};
use serde_json::{json,Value};
use std::{collections::BTreeMap,path::{Path,PathBuf}};

use payload::RUNTIME_INVENTORY as INVENTORY;
pub struct Retired { relative:String,hash:Option<String> }
fn managed(name:&str)->bool{
    let name=name.replace('\\',"/");
    if name.split('/').any(|part|part.is_empty()||part=="."||part==".."||part.contains(':')){return false}
    let first=name.split('/').next().unwrap_or("").to_ascii_lowercase();
    matches!(first.as_str(),"assets"|"licenses"|"docs"|"examples")||!name.contains('/')&&
        (name.ends_with(".exe")||name.ends_with(".dll")||matches!(name.as_str(),"README.md"|"LICENSE"|"CHANGELOG.md"|"FONT-NOTICE.md"|"THIRD-PARTY-NOTICES.md"|"REWRITE.md"|"GESTURE-TUTORIAL.md"|"NETEASE-SEEKFIX.md"|"ReleaseNotes.md"|"发布说明.md"))
}
fn inventory(root:&Path)->Option<BTreeMap<String,String>>{
    let value=paths::read_json(&root.join(INVENTORY)).ok()?;
    if value["product"]!="DotIsle"||value["schemaVersion"]!=1{return None}
    value["files"].as_object()?.iter().map(|(name,hash)|{
        let hash=hash.as_str()?;
        (hash.len()==64&&hash.bytes().all(|b|b.is_ascii_hexdigit())).then(||(name.to_ascii_lowercase(),hash.to_ascii_lowercase()))
    }).collect()
}
pub fn plan(stage:&Path,target:&Path)->Result<Vec<Retired>>{
    // A partial module transaction has no full inventory and cannot retire files.
    let Some(new)=inventory(stage)else{return Ok(vec![])};
    let old=inventory(target);
    let mut owned=Vec::new();
    if old.is_some(){
        // Preserve original case from the inventory rather than guessing paths.
        let value=paths::read_json(&target.join(INVENTORY))?;
        owned.extend(value["files"].as_object().unwrap().keys().cloned());
    }else if let Ok(marker)=payload::read_marker(target){
        if payload::marker_product(&marker)=="DotIsle"{owned=payload::marker_files(&marker);}
    }
    Ok(owned.into_iter().filter(|name|managed(name)&&!new.contains_key(&name.to_ascii_lowercase()))
        .map(|relative|Retired{hash:old.as_ref().and_then(|old|old.get(&relative.to_ascii_lowercase()).cloned()),relative}).collect())
}
pub(crate) fn ordinary_path(root:&Path,path:&Path)->bool{
    if !paths::starts_with_path(path,root)||paths::has_reparse_point(root){return false}
    let mut cursor=Some(path);
    while let Some(current)=cursor {
        if paths::has_reparse_point(current){return false}
        if paths::same_path(root,current){return true}cursor=current.parent();
    }
    false
}
fn remove_tree(path:&Path,root:&Path)->Result<u64>{
    if !ordinary_path(root,path){return Err(Error("更新清理路径无效".into()))}
    let mut count=0;
    for entry in std::fs::read_dir(path)?{
        let path=entry?.path();let metadata=std::fs::symlink_metadata(&path)?;
        if paths::has_reparse_point(&path){return Err(Error("更新清理遇到链接，已保留目录".into()))}
        if metadata.is_dir(){count+=remove_tree(&path,root)?}else{std::fs::remove_file(path)?;count+=1;}
    }
    std::fs::remove_dir(path)?;Ok(count)
}
pub fn finish(work:&Path,target:&Path,data:&Path,retired:&[Retired],version:&str)->Value{
    let temp=std::env::temp_dir();let name=work.file_name().and_then(|v|v.to_str()).unwrap_or("");
    let confirmed=!version.is_empty()&&name.starts_with("LyricIsland.Update.")
        &&work.parent().is_some_and(|p|paths::same_path(p,&temp))&&ordinary_path(&temp,work)
        &&ordinary_path(target,target)&&!paths::starts_with_path(target,work)&&!paths::starts_with_path(data,work)
        &&std::fs::read_to_string(work.join("success.txt")).ok().as_deref()==Some(version);
    if !confirmed{return json!({"success":false,"version":version,"updateConfirmed":false,"removedOldFiles":[],
        "preservedFiles":retired.iter().map(|f|&f.relative).collect::<Vec<_>>(),"temporaryCleanupPending":true,"userDataPreserved":true})}
    let mut removed=Vec::new();let mut kept=Vec::new();
    for item in retired {
        let result=(||->Result<()>{
            let file=paths::safe_child(target,&item.relative)?;
            if !managed(&item.relative)||!ordinary_path(target,&file)||paths::starts_with_path(&file,data){return Err(Error("保留用户文件".into()))}
            if file.is_file(){
                let Some(hash)=item.hash.as_ref()else{return Err(Error("无法确认原文件内容，已保留".into()))};
                if lyric_archive::sha256_file(&file).ok().as_ref()!=Some(hash){return Err(Error("文件已修改".into()))}
                std::fs::remove_file(file)?;
            }
            Ok(())
        })();
        if result.is_ok(){removed.push(item.relative.clone())}else{kept.push(item.relative.clone())}
    }
    if !removed.is_empty(){
        if let Ok(mut marker)=payload::read_marker(target){
            if payload::marker_product(&marker)=="DotIsle" {
                marker["files"]=json!(payload::marker_files(&marker).into_iter().filter(|name|!removed.contains(name)).collect::<Vec<_>>());
                let _=paths::write_json(&target.join(payload::INSTALLED_MARKER),&marker);
            }
        }
    }
    let scratch=remove_tree(work,work);
    json!({"success":scratch.is_ok(),"version":version,"updateConfirmed":true,"removedOldFiles":removed,
        "preservedFiles":kept,"temporaryFilesRemoved":scratch.as_ref().ok(),"temporaryCleanupPending":scratch.is_err(),"userDataPreserved":true})
}
pub fn schedule_helper_cleanup()->bool{
    use std::os::windows::process::CommandExt;
    let Ok(exe)=std::env::current_exe()else{return false};let Some(parent)=exe.parent()else{return false};
    let name=parent.file_name().and_then(|s|s.to_str()).unwrap_or("");
    if !(name.starts_with("DotIsle.UpdateHelper.")||name.starts_with("LyricIsland.UpdateHelper."))
        ||!parent.parent().is_some_and(|p|paths::same_path(p,std::env::temp_dir()))||!ordinary_path(parent,&exe){return false}
    let quote=|p:&Path|format!("'{}'",p.to_string_lossy().replace('\'',"''"));
    // The helper's own image can only be deleted after its process exits.
    // Native PowerShell cmdlets delete these two exact paths, never a wildcard tree.
    let script=format!("Wait-Process -Id {} -ErrorAction SilentlyContinue; Remove-Item -LiteralPath {} -Force -ErrorAction SilentlyContinue; if (-not (Get-ChildItem -LiteralPath {} -Force -ErrorAction SilentlyContinue)) {{ Remove-Item -LiteralPath {} -Force -ErrorAction SilentlyContinue }}",std::process::id(),quote(&exe),quote(parent),quote(parent));
    std::process::Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-WindowStyle","Hidden","-Command",&script])
        .creation_flags(0x08000000).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().is_ok()
}

#[cfg(test)]mod tests{
    use super::*;
    fn fixture()->(PathBuf,PathBuf,PathBuf,PathBuf){let root=paths::temp_unique("DotIsle.CleanupTest");let work=paths::temp_unique("LyricIsland.Update");let target=root.join("app");let data=target.join("Data");std::fs::create_dir_all(&data).unwrap();std::fs::create_dir_all(work.join("backup")).unwrap();std::fs::write(data.join("settings.json"),b"keep").unwrap();(root,work,target,data)}
    fn write_inventory(root:&Path,names:&[&str]){let files=names.iter().map(|name|(name.to_string(),json!(lyric_archive::sha256_file(&root.join(name)).unwrap()))).collect::<serde_json::Map<_,_>>();paths::write_json(&root.join(INVENTORY),&json!({"schemaVersion":1,"product":"DotIsle","files":files})).unwrap();}
    #[test]fn confirmed_update_cleans_backups_and_only_unmodified_owned_obsolete_files(){
        let(root,work,target,data)=fixture();let stage=root.join("stage");std::fs::create_dir(&stage).unwrap();
        for name in ["LyricIsland.exe","old.dll","modified.dll"]{std::fs::write(target.join(name),b"old").unwrap();}
        std::fs::write(target.join("my-file.txt"),b"keep").unwrap();write_inventory(&target,&["LyricIsland.exe","old.dll","modified.dll"]);
        std::fs::write(stage.join("LyricIsland.exe"),b"new").unwrap();write_inventory(&stage,&["LyricIsland.exe"]);
        let plan=plan(&stage,&target).unwrap();std::fs::write(target.join("modified.dll"),b"user modification").unwrap();
        std::fs::write(work.join("backup/old.dll"),b"old").unwrap();std::fs::write(work.join("success.txt"),"5.0.5").unwrap();
        let report=finish(&work,&target,&data,&plan,"5.0.5");assert_eq!(report["updateConfirmed"],true);assert!(!work.exists());assert!(!target.join("old.dll").exists());
        assert!(target.join("modified.dll").exists()&&target.join("my-file.txt").exists()&&data.join("settings.json").exists());std::fs::remove_dir_all(root).unwrap();
    }
    #[test]fn absent_success_receipt_and_partial_updates_keep_backups(){
        let(root,work,target,data)=fixture();assert!(plan(&root,&target).unwrap().is_empty());
        std::fs::write(target.join("old.dll"),b"old").unwrap();
        let retired=vec![Retired{relative:"old.dll".into(),hash:Some(lyric_archive::sha256_file(&target.join("old.dll")).unwrap())}];
        let report=finish(&work,&target,&data,&retired,"5.0.5");assert_eq!(report["temporaryCleanupPending"],true);assert_eq!(report["updateConfirmed"],false);
        assert!(work.exists()&&target.join("old.dll").exists());
        std::fs::remove_dir_all(work).unwrap();std::fs::remove_dir_all(root).unwrap();
    }
    #[test]fn cleanup_never_traverses_mutable_or_parent_paths(){for name in ["../old.dll","Data/settings.json","Logs/a.log","Cache/file.dll","Assets/../../old.dll"]{assert!(!managed(name),"{name}");}}
}
