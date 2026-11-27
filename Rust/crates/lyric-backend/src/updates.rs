//! GitHub Releases is the update source; installation is performed in place.
use serde_json::{json,Value};
use std::time::Duration;
static RUNTIME_VERSION:std::sync::OnceLock<String>=std::sync::OnceLock::new();
pub fn set_runtime_version(version:&str){let _=RUNTIME_VERSION.set(version.into());}
fn runtime_version()->&'static str{RUNTIME_VERSION.get().map(String::as_str).unwrap_or(env!("CARGO_PKG_VERSION"))}
pub fn installed_version()->String {
 let state=std::env::current_exe().ok().and_then(|p|p.parent().map(|p|p.join("ModuleState.json"))).and_then(|p|std::fs::read(p).ok()).and_then(|b|serde_json::from_slice::<Value>(&b).ok()).unwrap_or(Value::Null);
 if state["schemaVersion"]==1 && state["runtimeVersion"]==runtime_version() {if let Some(v)=state["version"].as_str().filter(|v|release_version(v).is_some()){return v.into();}}
 runtime_version().into()
}
pub const RELEASES:&str="https://github.com/Heiamer/DotIsle/releases/latest";
const RELEASE_MANIFEST:&str="https://raw.githubusercontent.com/Heiamer/DotIsle/main/Assets/UpdateManifest.json";
fn version(input:&str)->Option<[u32;4]>{
    let input=input.strip_prefix('v').unwrap_or(input);let mini=input.ends_with("mini");let input=if mini{input.strip_suffix("mini").unwrap().trim_end_matches('-')}else{input};
    let parts=input.split('.').collect::<Vec<_>>();
    if !(2..=3).contains(&parts.len()){return None}
    let mut out=[0;4];out[3]=if mini{1}else{0};for (i,p) in parts.iter().enumerate(){if p.is_empty()||!p.bytes().all(|c|c.is_ascii_digit()){return None}out[i]=p.parse().ok()?;}Some(out)
}

// Prerelease sequences are numeric and ordered before their final release.
// Restricting the spelling also makes every accepted tag safe in asset paths.
fn release_version(input:&str)->Option<([u32;4],u8,u32)> {
 let input=input.strip_prefix('v').unwrap_or(input);
 if let Some(base)=version(input){return Some((base,4,0))}
 let(base,suffix)=input.split_once('-')?;
 let base=version(base)?;if base[3]!=0{return None}
 for(name,rank)in [("alpha.",0),("beta.",1),("preview.",2),("rc.",3),("rc",3)]{
  if let Some(sequence)=suffix.strip_prefix(name){
   if sequence.is_empty()||!sequence.bytes().all(|b|b.is_ascii_digit()){return None}
   return Some((base,rank,sequence.parse().ok()?))
  }
 }
 None
}
pub fn valid_channel(channel:&str)->bool{matches!(channel,"stable"|"beta")}
pub fn describe(release:&Value,current:&str)->Result<Value,String>{describe_channel(release,current,"stable")}
pub fn describe_channel(release:&Value,current:&str,channel:&str)->Result<Value,String>{
 if !valid_channel(channel){return Err("更新渠道无效".into())}
 if release["draft"]==true{return Err("草稿不能用于更新".into())}
 let tag=release["tag_name"].as_str().ok_or("更新响应缺少版本号")?;
 let latest=release_version(tag).ok_or("更新版本号格式无效")?;
 let preview=release["prerelease"]==true;
 if channel=="stable"&&(preview||latest.1!=4){return Err("未找到正式更新".into())}
 if !preview&&latest.1!=4{return Err("测试版本缺少预发行标记".into())}
 let installed=release_version(current).ok_or("当前版本号无效")?;
 let newer=latest>installed;
 let notes = if newer { release["body"].as_str().map(notes_text).unwrap_or_default() } else { String::new() };
 Ok(json!({"success":true,"message":if newer{format!("发现新版本 {tag}，下载校验后由你确认安装。")}else{format!("当前渠道暂无更新（{current}）")},"newer":newer,"version":tag,"channel":channel,"prerelease":preview,"url":RELEASES,"releaseNotes":notes}))
}
fn notes_text(text: &str) -> String { text.trim_start_matches('\u{feff}').trim().chars().take(6_000).collect() }

fn fill_release_notes(result: &mut Value, fetch: impl FnOnce() -> Result<Vec<u8>, String>) {
    if result["newer"] != true || result["releaseNotes"].as_str().is_some_and(|s| !s.trim().is_empty()) { return; }
    if let Ok(bytes) = fetch() {
        if let Ok(text) = std::str::from_utf8(&bytes) {
            let notes = notes_text(text);
            if !notes.is_empty() { result["releaseNotes"] = json!(notes); }
        }
    }
}

fn github_notes(result: &mut Value, tag: &str, version: &str) {
    fill_release_notes(result, || crate::http::request_system(
        &format!("https://github.com/Heiamer/DotIsle/releases/download/{tag}/DotIsle-{version}-Notes.md"),
        &crate::http::RequestContext::new(Duration::from_secs(5)), 32 * 1024,
    ));
}
fn choose_release(releases:&Value,channel:&str,current:&str)->Result<Value,String>{
 let items=releases.as_array().ok_or("更新列表格式无效")?;
 items.iter().filter(|r|describe_channel(r,current,channel).is_ok())
  .max_by_key(|r|release_version(r["tag_name"].as_str().unwrap_or("")))
  .cloned().ok_or_else(||"所选渠道还没有有效的已发布版本".into())
}
fn sources()->Value{let path=std::env::current_exe().ok().and_then(|p|p.parent().map(|p|p.join("Assets/UpdateSources.json")));path.and_then(|p|std::fs::read(p).ok()).and_then(|b|serde_json::from_slice(&b).ok()).unwrap_or_else(||serde_json::from_str(include_str!("../../../../Assets/UpdateSources.json")).unwrap_or_else(|_|json!({})))}
fn repository(input:&str,host:&str)->Result<String,String>{let prefix=format!("https://{host}/");let slug=input.trim().strip_prefix(&prefix).ok_or("更新源必须使用官方 HTTPS 仓库地址")?.trim_end_matches('/').trim_end_matches(".git");let parts=slug.split('/').collect::<Vec<_>>();if parts.len()!=2||parts.iter().any(|p|p.is_empty()||!p.bytes().all(|b|b.is_ascii_alphanumeric()||b"-_.".contains(&b))||*p=="."||*p==".."){return Err("更新仓库地址无效".into())}Ok(slug.to_owned())}
fn mirror_asset_url(host:&str,slug:&str,release_id:u64,assets:&Value,name:&str)->Result<String,String>{let item=assets.as_array().and_then(|a|a.iter().find(|a|a["name"].as_str().or_else(||a["file_name"].as_str())==Some(name))).ok_or_else(||format!("国内发行版缺少 {name}"))?;let id=item["id"].as_u64().filter(|n|*n>0).ok_or("国内附件标识无效")?;Ok(format!("https://{host}/api/v5/repos/{slug}/releases/{release_id}/attach_files/{id}/download"))}
fn gitee_asset_url(slug:&str,release_id:u64,assets:&Value,name:&str)->Result<String,String>{mirror_asset_url("gitee.com",slug,release_id,assets,name)}

fn check_mirror(host:&str,repo:&str,label:&str,channel:&str)->Result<Value,String>{
 let slug=repository(repo,host)?;let api=format!("https://{host}/api/v5/repos/{slug}");
 let ctx=crate::http::RequestContext::new(Duration::from_secs(10));
 let release=if channel=="beta"{
  choose_release(&crate::http::get_system(&format!("{api}/releases?per_page=100"),&ctx)?,channel,&installed_version())?
 }else{
  match crate::http::get_system(&format!("{api}/releases/latest"),&ctx){
   Ok(value)=>value,
   Err(_)=>choose_release(&crate::http::get_system(&format!("{api}/releases?per_page=100"),&ctx)?,"stable",&installed_version())?,
  }
 };
 let mut result=describe_channel(&release,&installed_version(),channel)?;
 let tag=release["tag_name"].as_str().ok_or("国内版本号缺失")?;
 result["source"]=json!(label);result["url"]=json!(format!("https://{host}/{slug}/releases/tag/{tag}"));
 if result["newer"]==true{
  let v=tag.trim_start_matches('v');
  let pairs=[(format!("DotIsle-{v}-SHA256.txt"),format!("DotIsle-{v}-Portable.7z")),(format!("LyricIsland-{v}-SHA256.txt"),format!("LyricIsland-{v}-Portable.zip"))];
  let direct=|name:&str|release["assets"].as_array().and_then(|items|items.iter().find(|item|item["name"].as_str()==Some(name))).and_then(|item|item["browser_download_url"].as_str()).map(str::to_owned);
  let mut found=false;
  for (manifest_name,package_name) in &pairs{
   if let (Some(manifest),Some(package))=(direct(manifest_name),direct(package_name)){
    result["manifestUrl"]=json!(manifest);result["zipUrl"]=json!(package);result["packageName"]=json!(package_name);found=true;break;
   }
  }
  if !found{
   let id=release["id"].as_u64().filter(|n|*n>0).ok_or("国内发行版标识无效")?;
   let attachments=crate::http::get_system(&format!("{api}/releases/{id}/attach_files?per_page=100"),&ctx)?;
   let mut listed:Vec<(String,String)>=Vec::new();
   for item in attachments.as_array().into_iter().flatten(){
    let Some(file_id)=item["id"].as_u64().filter(|n|*n>0) else{continue};
    let name=item["name"].as_str().or_else(||item["file_name"].as_str()).unwrap_or("");
    listed.push((name.to_owned(),format!("{api}/releases/{id}/attach_files/{file_id}/download")));
   }
   for (manifest_name,package_name) in &pairs{
    let manifest=listed.iter().find(|(name,_)|name==manifest_name).map(|(_,url)|url.clone());
    let package=listed.iter().find(|(name,_)|name==package_name).map(|(_,url)|url.clone());
    if let (Some(manifest),Some(package))=(manifest,package){
     result["manifestUrl"]=json!(manifest);result["zipUrl"]=json!(package);result["packageName"]=json!(package_name);found=true;break;
    }
   }
  }
  if !found{return Err("国内发行版缺少更新附件".into())}
 }
 Ok(result)
}
fn check_gitee(repo:&str,channel:&str)->Result<Value,String>{check_mirror("gitee.com",repo,"Gitee 国内源",channel)}
fn manifest_release(value:Value)->Result<Value,String>{
    if value["published"]!=true{return Err("备用清单对应版本尚未发布".into())}
    describe(&value,&installed_version())?;Ok(value)
}
fn check_stable_sources(
    current: &str,
    api: impl FnOnce() -> Result<Value, String>,
    page: impl FnOnce() -> Result<Value, String>,
    manifest: impl FnOnce() -> Result<Value, String>,
) -> Result<Value, String> {
    let api = api().and_then(|release| describe(&release, current));
    let api_error = match api {
        Ok(result) => return Ok(result),
        Err(error) => error,
    };
    // A valid but stale manifest can otherwise hide a release when the API is rate limited.
    let page_error = match page().and_then(|release| describe(&release, current)) {
        Ok(result) => return Ok(result),
        Err(error) => error,
    };
    manifest().and_then(manifest_release).and_then(|release| describe(&release, current))
        .map_err(|manifest_error| format!("API：{api_error}；页面：{page_error}；备用清单：{manifest_error}"))
}
fn check_github(channel:&str)->Result<Value,String>{
    if channel=="beta" {return check_github_beta()}
    let mut result = check_stable_sources(
        &installed_version(),
        || crate::http::get_system("https://api.github.com/repos/Heiamer/DotIsle/releases/latest", &crate::http::RequestContext::new(Duration::from_secs(10))),
        || crate::http::request_system(RELEASES, &crate::http::RequestContext::new(Duration::from_secs(10)), 4 * 1024 * 1024)
            .and_then(|bytes| page_release(&String::from_utf8_lossy(&bytes))),
        || crate::http::get_system(RELEASE_MANIFEST, &crate::http::RequestContext::new(Duration::from_secs(6))),
    )?;
    let tag = result["version"].as_str().ok_or("版本号缺失")?.to_owned();
    let v = tag.trim_start_matches('v');
    let base = format!("https://github.com/Heiamer/DotIsle/releases/download/{tag}/LyricIsland-{v}");
    let url = format!("https://github.com/Heiamer/DotIsle/releases/tag/{tag}");
    result["source"] = json!("GitHub");
    result["url"] = json!(url);
    result["manifestUrl"] = json!(format!("{base}-SHA256.txt"));
    result["zipUrl"] = json!(format!("{base}-Portable.zip"));
    github_notes(&mut result, &tag, v);
    Ok(result)
}

fn check_github_beta()->Result<Value,String>{
 let ctx=crate::http::RequestContext::new(Duration::from_secs(10));
 let primary=crate::http::get_system("https://api.github.com/repos/Heiamer/DotIsle/releases?per_page=100",&ctx)
  .and_then(|releases|choose_release(&releases,"beta",&installed_version()));
 let (release,warning)=match primary{
  Ok(release)=>(release,None),
  Err(error)=>{
   // Unlike /latest, the public release list also includes prereleases. Keep
   // the beta channel on GitHub when its API is slow or rate limited.
   let ctx=crate::http::RequestContext::new(Duration::from_secs(10));
   let release=crate::http::request_system("https://github.com/Heiamer/DotIsle/releases",&ctx,4*1024*1024)
    .and_then(|bytes|page_channel_release(&String::from_utf8_lossy(&bytes),"beta",&installed_version()))
    .map_err(|page_error|format!("测试版 API：{error}；发行列表：{page_error}"))?;
   (release,Some(error))
  }
 };
 let mut result=describe_channel(&release,&installed_version(),"beta")?;
 result["metadataRoute"]=json!(if warning.is_some(){"releasePage"}else{"api"});
 if let Some(warning)=warning{result["sourceWarnings"]=json!([format!("GitHub API：{warning}；已使用 GitHub 发行列表")]);}
 let tag=release["tag_name"].as_str().ok_or("版本号缺失")?;
 let v=tag.trim_start_matches('v');
 let base=format!("https://github.com/Heiamer/DotIsle/releases/download/{tag}/LyricIsland-{v}");
 result["source"]=json!("GitHub");result["url"]=json!(format!("https://github.com/Heiamer/DotIsle/releases/tag/{tag}"));
 result["manifestUrl"]=json!(format!("{base}-SHA256.txt"));result["zipUrl"]=json!(format!("{base}-Portable.zip"));
 github_notes(&mut result,tag,v);Ok(result)
}
pub fn check()->Result<Value,String>{check_channel("stable")}
/// The selected source is authoritative for this check; failures fall back in order.
pub fn valid_source(source:&str)->bool{matches!(source,"gitcode"|"gitee"|"github")}
pub fn check_channel(channel:&str)->Result<Value,String>{check_preferred(channel,"gitcode")}
pub fn source_order(preferred:&str)->Result<Vec<&'static str>,String>{
 if !valid_source(preferred){return Err("更新源无效".into())}
 let mut order=vec!["gitcode","gitee","github"];
 order.sort_by_key(|source|*source!=preferred);Ok(order)
}
pub fn check_preferred(channel:&str,preferred:&str)->Result<Value,String>{
 if !valid_channel(channel){return Err("更新渠道无效".into())}
 let config=sources();let order=source_order(preferred)?;
 check_ordered(channel,&installed_version(),preferred,order.into_iter().map(|source|{
  let config=&config;
  (source,Box::new(move||match source{
   "github"=>check_github(channel),
   "gitee"=>check_mirror("gitee.com",config["giteeRepository"].as_str().unwrap_or(""),"Gitee",channel),
   _=>check_mirror("gitcode.com",config["gitcodeRepository"].as_str().unwrap_or(""),"GitCode",channel),
  }) as Box<dyn FnOnce()->Result<Value,String>+'_>)
 }).collect())
}
fn check_ordered(channel:&str,current:&str,preferred:&str,candidates:Vec<(&str,Box<dyn FnOnce()->Result<Value,String>+'_>)>)->Result<Value,String>{
 let mut errors=Vec::new();
 for (source,run) in candidates{
  match run(){
   Ok(mut result)=>{
    if release_version(result["version"].as_str().unwrap_or(""))<release_version(current){
     errors.push(format!("{source} 的版本早于当前版本，无法确认更新"));continue;
    }
    let label=result["source"].as_str().unwrap_or(source).to_owned();
    let message=result["message"].as_str().unwrap_or("").to_owned();
    result["message"]=json!(format!("{message} · {label}"));
    result["channel"]=json!(channel);result["preferredSource"]=json!(preferred);
    if !errors.is_empty(){let mut warnings=result["sourceWarnings"].as_array().cloned().unwrap_or_default();warnings.extend(errors.iter().map(|e|json!(e)));result["sourceWarnings"]=json!(warnings);}
    return Ok(result)
   },
   Err(error)=>errors.push(format!("{source}：{error}")),
  }
 }
 Err(format!("检查更新失败。{}。请检查网络或系统代理。",errors.join("；")))
}
#[cfg(test)]
fn mirror_repositories(config:&Value)->Vec<(&'static str,&'static str,String)>{
 [("gitee.com","Gitee 国内源","giteeRepository"),("gitcode.com","GitCode 国内源","gitcodeRepository")]
  .into_iter()
  .filter_map(|(host,label,key)|config[key].as_str().filter(|repo|!repo.is_empty()).map(|repo|(host,label,repo.to_owned())))
  .collect()
}
#[cfg(test)]
fn check_sources(channel:&str,current:&str,github:impl FnOnce()->Result<Value,String>,mirrors:Vec<(&'static str,Box<dyn FnOnce()->Result<Value,String>+'_>)>)->Result<Value,String>{
 let mut result=match github(){
  Ok(result)=>result,
  Err(primary)=>{
   let mut errors=vec![format!("GitHub：{primary}")];
   let mut fallback=None;
   for (label,run) in mirrors {
    match run(){
     Ok(value)=>{
      if release_version(value["version"].as_str().unwrap_or(""))<release_version(current){
       errors.push(format!("{label}仅有 {}，早于当前版本 {current}，无法确认此渠道是否有更新。",value["version"].as_str().unwrap_or("未知版本")));
      }else{fallback=Some((label,value));break;}
     }
     Err(error)=>errors.push(format!("{label}：{error}")),
    }
   }
   let Some((label,mut fallback))=fallback else{
    return Err(format!("检查更新失败。{}。请检查网络或系统代理。",errors.join("；")))
   };
   let mut warnings=fallback["sourceWarnings"].as_array().cloned().unwrap_or_default();
   warnings.push(json!(format!("GitHub：{primary}；已使用{label}")));
   fallback["sourceWarnings"]=json!(warnings);
   fallback
  }
 };
 let source=result["source"].as_str().unwrap_or("未知源").to_owned();
 let message=result["message"].as_str().unwrap_or("").to_owned();result["message"]=json!(format!("{message} · {source}"));
 result["channel"]=json!(channel);Ok(result)
}
fn page_channel_release(html:&str,channel:&str,current:&str)->Result<Value,String>{
 let links=regex::Regex::new(r"(?is)<a\b[^>]*>").unwrap();
 let attributes=regex::Regex::new(r#"(?i)(?:^|\s)(href|class)\s*=\s*["']([^"']*)["']"#).unwrap();
 let mut releases=vec![];
 for link in links.find_iter(html){
  let(mut href,mut class)=(None,None);
  for attribute in attributes.captures_iter(link.as_str()){
   if attribute[1].eq_ignore_ascii_case("href"){href=attribute.get(2).map(|m|m.as_str());}else{class=attribute.get(2).map(|m|m.as_str());}
  }
  if !class.is_some_and(|c|c.split_whitespace().any(|v|v=="Link--primary")){continue}
  let Some(tag)=href.and_then(|url|url.strip_prefix("https://github.com").unwrap_or(url).strip_prefix("/Heiamer/DotIsle/releases/tag/"))else{continue};
  let Some(version)=release_version(tag)else{continue};
  // GitHub can mark a numeric tag as a preview; read the title's badge too.
  let tail=&html[link.end()..];
  let end=tail.find("</div>").unwrap_or(tail.len());
  let next=links.find(tail).map_or(tail.len(),|m|m.start());
  let header=&tail[..end.min(next)];
  let preview=regex::Regex::new(r#"(?is)<span\b[^>]*class\s*=\s*["'][^"']*\bLabel\b[^"']*["'][^>]*>\s*Pre-release\s*</span>"#).unwrap().is_match(header);
  releases.push(json!({"tag_name":tag,"prerelease":version.1!=4||preview,"draft":false}));
 }
 choose_release(&json!(releases),channel,current)
}
fn page_release(html:&str)->Result<Value,String>{
    let meta=regex::Regex::new(r"(?is)<meta\b[^>]*>").unwrap();
    let attributes=regex::Regex::new(r#"(?i)(?:^|\s)(property|content)\s*=\s*["']([^"']*)["']"#).unwrap();
    for element in meta.find_iter(html){
        let(mut property,mut content)=(None,None);
        for attribute in attributes.captures_iter(element.as_str()){
            if attribute[1].eq_ignore_ascii_case("property"){property=attribute.get(2).map(|m|m.as_str());}
            else{content=attribute.get(2).map(|m|m.as_str());}
        }
        if property!=Some("og:url"){continue}
        if let Some(tag)=content.and_then(|url|url.strip_prefix("https://github.com").unwrap_or(url).strip_prefix("/Heiamer/DotIsle/releases/tag/")){
            if version(tag).is_some(){return Ok(json!({"tag_name":tag}))}
        }
    }
    Err("GitHub 页面未提供正式版本号，请稍后重试。".into())
}
/// The helper runs from a unique temporary directory so it can replace its installed copy.
pub fn install()->Result<Value,String>{
    let release=check()?;if release["newer"]!=true{return Ok(release)}
    let version=release["version"].as_str().ok_or("missing version")?.trim_start_matches('v');
    let base=std::env::current_exe().map_err(|e|e.to_string())?.parent().ok_or("missing directory")?.to_path_buf();
    let data=std::env::var_os("LYRICISLAND_NATIVE_DATA").map(std::path::PathBuf::from).unwrap_or_else(||base.join("Data"));
    let pipe=std::env::var("LYRICISLAND_TEST_PIPE").unwrap_or_else(|_|"LyricIsland.Native.Settings.v1".into());
    let state=crate::io::pipe_request(&pipe,&json!({"version":1,"command":"state"}))?;
    let pid=state["processId"].as_u64().ok_or("无法确认正在运行的应用")?;
    let temp=std::env::temp_dir().join(format!("LyricIsland.UpdateHelper.{}.{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()));
    std::fs::create_dir(&temp).map_err(|e|e.to_string())?;let helper=temp.join("LyricIsland.Updater.exe");
    std::fs::copy(base.join("LyricIsland.Updater.exe"),&helper).map_err(|e|e.to_string())?;
    std::process::Command::new(helper).arg(version).arg(base).arg(data).arg(pid.to_string()).arg(std::process::id().to_string()).arg(pipe).arg(release["manifestUrl"].as_str().unwrap_or("")).arg(release["zipUrl"].as_str().unwrap_or("")).spawn().map_err(|e|e.to_string())?;
    Ok(json!({"success":true,"message":"正在下载并校验更新，完成后会自动重启；下载失败时保留当前版本。"}))
}
#[cfg(test)]mod tests{use super::*;
    #[test]
    fn release_body_is_legacy_compatible_and_empty_metadata_uses_matching_attachment() {
        let body = include_str!("../../../../release-notes/DotIsle-5.2.0-Notes.md");
        let release = json!({"tag_name":"v5.2.0","body":body});
        let mut result = describe(&release, "5.1.2").unwrap();
        fill_release_notes(&mut result, || panic!("release body already available"));
        assert_eq!(result["releaseNotes"], body.trim());
        let mut from_page = describe(&json!({"tag_name":"v5.2.0"}), "5.1.2").unwrap();
        fill_release_notes(&mut from_page, || Ok(body.as_bytes().to_vec()));
        assert_eq!(from_page["releaseNotes"], body.trim());
        assert!(describe(&json!({"tag_name":"v5.2.0","draft":true,"body":body}), "5.1.2").is_err());
    }
    #[test]fn repositories_require_expected_hosts_and_two_safe_segments(){assert_eq!(repository("https://gitee.com/team/dotisle.git","gitee.com").unwrap(),"team/dotisle");for url in ["http://gitee.com/a/b","https://evil.test/a/b","https://gitee.com/a/b/extra","https://gitee.com/a/..","https://gitee.com/a/b?q=x"]{assert!(repository(url,"gitee.com").is_err());}}
    #[test]fn domestic_urls_are_constructed_from_validated_attachment_ids(){let assets=json!([{"name":"LyricIsland-1.3.1-Portable.zip","id":42,"browser_download_url":"https://evil.test"}]);assert_eq!(gitee_asset_url("team/dotisle",7,&assets,"LyricIsland-1.3.1-Portable.zip").unwrap(),"https://gitee.com/api/v5/repos/team/dotisle/releases/7/attach_files/42/download");assert_eq!(mirror_asset_url("gitcode.com","Heiamer/Dotisle",9,&assets,"LyricIsland-1.3.1-Portable.zip").unwrap(),"https://gitcode.com/api/v5/repos/Heiamer/Dotisle/releases/9/attach_files/42/download");assert!(gitee_asset_url("team/dotisle",7,&assets,"missing").is_err());}
    #[test]fn compares_numeric_versions_and_never_downgrades(){
        assert_eq!(describe(&json!({"tag_name":"v1.10"}),"1.2.0").unwrap()["newer"],true);
        assert_eq!(describe(&json!({"tag_name":"v1.1"}),"1.2.0").unwrap()["newer"],false);
        assert_eq!(describe(&json!({"tag_name":"v1.2"}),"1.2.0").unwrap()["newer"],false);
    }
    #[test]fn installed_preview_can_upgrade_to_its_stable_release_without_downgrading(){
        for (tag,newer) in [("v3.2.3",false),("v3.2.4",true),("v3.2.5",true)]{assert_eq!(describe(&json!({"tag_name":tag}),"3.2.4-preview.1").unwrap()["newer"],newer);}
        for current in ["3.2.4-preview.","3.2.4-preview.1/evil","3.2.4-beta"]{assert!(describe(&json!({"tag_name":"v3.2.4"}),current).is_err());}
        assert!(describe(&json!({"tag_name":"v3.2.4-preview.1"}),"3.2.3").is_err());
    }
    #[test]fn installed_rc_can_upgrade_to_stable_without_downgrading(){
        for (tag,newer) in [("v3.2.7",false),("v3.5.0",true),("v3.5.1",true)]{assert_eq!(describe(&json!({"tag_name":tag}),"3.5.0-rc1").unwrap()["newer"],newer);}
        for current in ["3.5.0-rc","3.5.0-rc1/evil","3.5.0-rcx"]{assert!(describe(&json!({"tag_name":"v3.5.0"}),current).is_err());}
        assert!(describe(&json!({"tag_name":"v3.5.0-rc1","prerelease":true}),"3.2.7").is_err());
        assert!(describe(&json!({"tag_name":"v3.5.0-rc1"}),"3.2.7").is_err());
    }
    #[test]fn rejects_untrusted_tags_and_preview_releases(){for tag in ["latest","v1.3-beta","v1.3/evil"]{assert!(describe(&json!({"tag_name":tag}),"1.2.0").is_err());}assert!(describe(&json!({"tag_name":"v2.0","prerelease":true}),"1.2.0").is_err());}
    #[test]fn fallback_reads_only_canonical_repository_metadata(){assert_eq!(page_release(r#"<meta property="og:url" content="/Heiamer/DotIsle/releases/tag/v1.2" />"#).unwrap()["tag_name"],"v1.2");assert!(page_release(r#"<meta property="og:url" content="/other/project/releases/tag/v9.0" />"#).is_err());}
    #[test]fn html_fallback_accepts_attribute_order_quotes_and_spacing(){
        for html in [r#"<META content = 'https://github.com/Heiamer/DotIsle/releases/tag/v2.1.6' data-test='release' property = 'og:url'>"#,
                     "<meta\ncontent=\"/Heiamer/DotIsle/releases/tag/v2.1.6\"\nproperty=\"og:url\">"]{
            assert_eq!(page_release(html).unwrap()["tag_name"],"v2.1.6");
        }
        for html in [r#"<meta data-property='og:url' content='/Heiamer/DotIsle/releases/tag/v9.0'>"#,
                     r#"<meta property='og:url' content='https://evil.test/Heiamer/DotIsle/releases/tag/v9.0'>"#,
                     r#"<meta property='og:url' content='/Heiamer/DotIsle/releases/tag/v2.1.6/evil'>"#]{assert!(page_release(html).is_err());}
    }
    #[test]
    fn rate_limited_stable_api_checks_release_page_before_stale_manifest() {
        let result = check_stable_sources(
            "5.0.6",
            || Err("HTTP 403".into()),
            || page_release(r#"<meta property="og:url" content="https://github.com/Heiamer/DotIsle/releases/tag/v5.0.7">"#),
            || panic!("the stale v5.0.6 manifest must not override public release metadata"),
        ).unwrap();
        assert_eq!(result["version"], "v5.0.7");
        assert_eq!(result["newer"], true);
    }

    #[test]
    fn invalid_stable_api_response_can_fall_back_to_published_manifest() {
        let result = check_stable_sources(
            "5.0.6",
            || Ok(json!({"tag_name":"v5.1.0","prerelease":true})),
            || Err("page unreachable".into()),
            || Ok(json!({"tag_name":"v5.0.7","published":true})),
        ).unwrap();
        assert_eq!(result["version"], "v5.0.7");
        assert_eq!(result["prerelease"], false);
    }
    #[test]fn raw_manifest_requires_a_published_stable_release(){
        let valid=json!({"published":true,"tag_name":"v2.1.6","prerelease":false,"draft":false});
        assert_eq!(manifest_release(valid.clone()).unwrap()["tag_name"],"v2.1.6");
        for key in ["published","prerelease","draft"]{let mut invalid=valid.clone();invalid[key]=json!(key!="published");assert!(manifest_release(invalid).is_err());}
        assert!(manifest_release(json!({"published":true,"tag_name":"latest"})).is_err());
    }
    #[test]fn numeric_stable_release_is_newer_than_installed_and_mini_versions(){
        for current in ["2.1.5","2.1.5mini"]{assert_eq!(describe(&json!({"tag_name":"v2.1.6","prerelease":false}),current).unwrap()["newer"],true);}
        assert_eq!(describe(&json!({"tag_name":"v2.1.6"}),"2.1.6").unwrap()["newer"],false);
    }
}

/// Launch the exact checked candidate without a second latest-version race.
pub fn launch_candidate(release:&Value,base:&std::path::Path,data:&std::path::Path,pipe:&str,job:&str,nonce:&str)->Result<(),String>{
 let version=release["version"].as_str().ok_or("缺少更新版本")?.trim_start_matches('v');
 let pid=std::process::id();
 let temp=std::env::temp_dir().join(format!("DotIsle.UpdateHelper.{}",job.replace(['{','}','-'],"")));std::fs::create_dir_all(&temp).map_err(|e|e.to_string())?;
 let helper=temp.join("LyricIsland.Updater.exe");std::fs::copy(base.join("LyricIsland.Updater.exe"),&helper).map_err(|e|e.to_string())?;
 std::process::Command::new(helper).arg(version).arg(base).arg(data).arg(pid.to_string()).arg("0").arg(pipe).arg(release["manifestUrl"].as_str().unwrap_or("")).arg(release["zipUrl"].as_str().unwrap_or("")).arg(job).arg(nonce).arg(release["packageName"].as_str().unwrap_or("")).spawn().map_err(|e|e.to_string())?;Ok(())
}

#[cfg(test)]mod mini_version_tests{use super::*;
#[test]fn mini_version_orders_between_base_and_next_patch(){assert!(version("2.1.5mini")>version("2.1.5"));assert_eq!(version("v2.1.5mini"),version("2.1.5-mini"));assert!(version("2.1.6")>version("2.1.5mini"));assert!(version("2.1.5mini/evil").is_none());assert!(version("2.1.5-beta").is_none());}
}

#[cfg(test)]mod channel_tests{use super::*;
#[test]fn stable_filters_beta_and_promotes_same_version_final(){
 let beta=json!({"tag_name":"v5.0.1-beta.2","prerelease":true});
 assert!(describe_channel(&beta,"5.0.0","stable").is_err());
 assert_eq!(describe_channel(&beta,"5.0.0","beta").unwrap()["newer"],true);
 assert_eq!(describe_channel(&json!({"tag_name":"v5.0.1"}),"5.0.1-beta.2","stable").unwrap()["newer"],true);
 assert_eq!(describe_channel(&json!({"tag_name":"v5.0.0"}),"5.1.0-beta.1","stable").unwrap()["newer"],false);
}
#[test]fn github_primary_never_queries_or_gets_overridden_by_the_mirror(){
 let primary=describe_channel(&json!({"tag_name":"v5.0.3-beta.2","prerelease":true}),"5.0.2","beta").unwrap();
 let mut primary=primary;primary["source"]=json!("GitHub");primary["zipUrl"]=json!("github-exact-pair");
 let result=check_sources("beta","5.0.2",||Ok(primary),vec![("国内后备源",Box::new(||->Result<Value,String>{panic!("mirror must remain a fallback even if it advertises v99")}))]).unwrap();
 assert_eq!(result["version"],"v5.0.3-beta.2");assert_eq!(result["source"],"GitHub");assert_eq!(result["zipUrl"],"github-exact-pair");
}
#[test]fn outdated_domestic_fallback_cannot_claim_no_updates(){
 let mirror=|tag:&str|{let mut r=describe_channel(&json!({"tag_name":tag}),"5.0.2","beta").unwrap();r["source"]=json!("Gitee 国内源");r};
 assert!(check_sources("beta","5.0.2",||Err("offline".into()),vec![("国内后备源",Box::new(||->Result<Value,String>{Ok(mirror("v3.2.0"))}))]).unwrap_err().contains("无法确认"));
 let result=check_sources("beta","5.0.2",||Err("offline".into()),vec![("国内后备源",Box::new(||->Result<Value,String>{Ok(mirror("v5.0.3"))}))]).unwrap();
 assert_eq!(result["version"],"v5.0.3");assert!(result["sourceWarnings"].is_array());
 assert!(check_sources("beta","5.0.2",||Err("offline".into()),vec![("国内后备源",Box::new(||->Result<Value,String>{Err("offline".into())}))]).is_err());
}
#[test]fn public_release_list_fallback_reads_beta_titles_and_rejects_foreign_links(){
 let html=r#"<a class="Link--primary Link" href="/Heiamer/DotIsle/releases/tag/v5.0.3-rc2">rc</a><a href='/Heiamer/DotIsle/releases/tag/v5.0.3-rc3' class='Link--primary'>rc3</a><a href='/Heiamer/DotIsle/releases/tag/v5.0.2' class='Link--primary'>stable</a><a href='/Heiamer/DotIsle/releases/tag/v99.0.0'>body link</a><a class='Link--primary' href='https://evil.test/Heiamer/DotIsle/releases/tag/v99.0.0'>foreign</a>"#;
 assert_eq!(page_channel_release(html,"beta","5.0.2").unwrap()["tag_name"],"v5.0.3-rc3");
 assert_eq!(page_channel_release(html,"stable","5.0.2").unwrap()["tag_name"],"v5.0.2");
 assert!(page_channel_release("<a class='Link--primary' href='/Heiamer/DotIsle/releases/tag/v5.0.3-rc3/evil'>bad</a>","beta","5.0.2").is_err());
}
#[test]fn release_selection_is_numeric_and_draft_safe(){
 let list=json!([{"tag_name":"v5.0.1-beta.9","prerelease":true},{"tag_name":"v99.0.0","draft":true},{"tag_name":"v5.0.1-beta.10","prerelease":true},{"tag_name":"v5.0.0"}]);
 assert_eq!(choose_release(&list,"beta","5.0.0").unwrap()["tag_name"],"v5.0.1-beta.10");
 assert_eq!(choose_release(&list,"stable","5.0.0").unwrap()["tag_name"],"v5.0.0");
 for tag in ["v5.0.1-beta.1/evil","v5.0.1-beta.1?x","v5.0.1-beta.x"]{assert!(release_version(tag).is_none());}
 assert!(describe_channel(&json!({"tag_name":"v5.0.1-beta.1"}),"5.0.0","beta").is_err());
 assert!(!valid_channel("nightly"));
}
}

#[test]
fn release_page_respects_preview_badge_on_numeric_tags() {
 let html=r#"<div><a class='Link--primary' href='/Heiamer/DotIsle/releases/tag/v5.1.0'>preview</a><span class='Label Label--warning'>Pre-release</span></div><div><a class='Link--primary' href='/Heiamer/DotIsle/releases/tag/v5.0.2'>stable</a></div>"#;
 assert_eq!(page_channel_release(html,"stable","5.0.0").unwrap()["tag_name"],"v5.0.2");
 assert_eq!(page_channel_release(html,"beta","5.0.0").unwrap()["tag_name"],"v5.1.0");
}

#[cfg(test)]mod source_selection_tests{use super::*;
 #[test]fn preferred_order_and_failover_are_real(){
  assert_eq!(source_order("gitcode").unwrap(),vec!["gitcode","gitee","github"]);
  assert_eq!(source_order("github").unwrap(),vec!["github","gitcode","gitee"]);assert!(source_order("bad").is_err());
  let v=check_ordered("stable","5.2.7","gitee",vec![("gitee",Box::new(||Ok(json!({"version":"v5.2.8","source":"Gitee"})))),("github",Box::new(||panic!("do not query fallback after success")))]).unwrap();assert_eq!(v["preferredSource"],"gitee");assert_eq!(v["source"],"Gitee");
  let v=check_ordered("stable","5.2.7","gitcode",vec![("gitcode",Box::new(||Err("offline".into()))),("gitee",Box::new(||Ok(json!({"version":"v5.2.5"})))),("github",Box::new(||Ok(json!({"version":"v5.2.8","source":"GitHub"}))))]).unwrap();assert_eq!(v["source"],"GitHub");assert_eq!(v["sourceWarnings"].as_array().unwrap().len(),2);
 }
}
