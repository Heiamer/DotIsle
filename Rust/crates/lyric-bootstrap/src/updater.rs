use crate::error::{Error, Result};
use crate::http;
use crate::paths;
use crate::payload;
use crate::vello_overlay::Overlay;
use crate::win;
use lyric_archive::ZipWriter;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const CANCELLED_MESSAGE: &str = "更新已取消，原程序保持不变。";
const PINNED_MISMATCH: &str = "同一版本的更新源摘要不一致，已停止更新。";

#[derive(Clone, Default)]
pub struct OverlayState {
    pub phase: String,
    pub received: u64,
    pub length: Option<u64>,
    pub message: String,
}

pub fn shared_state() -> Arc<Mutex<OverlayState>> { Arc::new(Mutex::new(OverlayState::default())) }

pub struct Reporter {
    authenticated: bool,
    job: String,
    nonce: String,
    pipe: String,
    seq: u64,
    last: Option<Instant>,
    directive: String,
    state: Arc<Mutex<OverlayState>>,
}

impl Reporter {
    pub fn new(authenticated: bool, job: String, nonce: String, pipe: String, state: Arc<Mutex<OverlayState>>) -> Self {
        Reporter { authenticated, job, nonce, pipe, seq: 0, last: None, directive: String::new(), state }
    }

    pub fn state(&self) -> Arc<Mutex<OverlayState>> { self.state.clone() }

    pub fn set_message(&self, phase: &str, message: &str) {
        if let Ok(mut state) = self.state.lock() {
            state.phase = phase.to_owned();
            state.message = message.to_owned();
        }
    }

    pub fn report(&mut self, phase: &str, received: u64, length: Option<u64>, message: &str, force: bool) -> Result<String> {
        if let Ok(mut state) = self.state.lock() {
            state.phase = phase.to_owned();
            state.received = received;
            state.length = length;
            state.message = message.to_owned();
        }
        if !self.authenticated { return Ok(String::new()); }
        if !force && self.last.is_some_and(|last| last.elapsed() < Duration::from_millis(250)) { return Ok(self.directive.clone()); }
        self.last = Some(Instant::now());
        self.seq += 1;
        let request = json!({
            "version": 1,
            "command": "updateProgress",
            "value": {
                "jobId": self.job,
                "nonce": self.nonce,
                "seq": self.seq,
                "phase": phase,
                "receivedBytes": received,
                "totalBytes": length,
                "message": message,
            }
        });
        match win::pipe_request(&self.pipe, &request, Duration::from_secs(2)) {
            Ok(reply) => {
                if let Some(action) = reply.get("action").and_then(Value::as_str) {
                    self.directive = action.to_owned();
                }
                if self.directive == "cancel" && phase != "applying" && phase != "restarting" && phase != "cancelled" {
                    return Err(Error::cancelled(CANCELLED_MESSAGE));
                }
                Ok(self.directive.clone())
            }
            Err(_) if phase == "ready" => Err(Error("主程序已退出；未确认安装，已保留原版本。".into())),
            Err(_) => Ok(self.directive.clone()),
        }
    }
}

pub fn official_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() || authority.contains('@') { return false; }
    let host = authority.split(':').next().unwrap_or("");
    host.eq_ignore_ascii_case("github.com") || host.eq_ignore_ascii_case("gitee.com") || host.eq_ignore_ascii_case("gitcode.com")
}

pub fn verify(zip: &Path, expected: &str) -> Result<()> {
    let digest = Regex::new(r"^[0-9a-f]{64}$").unwrap();
    if !digest.is_match(expected) || lyric_archive::sha256_file(zip)? != expected {
        return Err(Error("下载校验失败，原程序保持不变".into()));
    }
    Ok(())
}

pub fn extract(zip: &Path, stage: &Path, version: &str) -> Result<PathBuf> {
    payload::extract_update(zip, stage, version)
}

pub fn extract_7z(archive: &Path, stage: &Path, version: &str) -> Result<PathBuf> {
    payload::extract_update_7z(archive, stage, version)
}

fn asset_digest(manifest: &str, name: &str) -> Option<String> {
    for line in manifest.trim_start_matches('\u{feff}').lines() {
        let mut parts = line.split_whitespace();
        let digest = parts.next().unwrap_or("");
        let file = parts.next().unwrap_or("");
        if file == name && digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Some(digest.to_lowercase());
        }
    }
    None
}

pub fn manifest_digest(manifest: &str, version: &str) -> Option<String> {
    asset_digest(manifest, &format!("LyricIsland-{version}-Portable.zip"))
}

pub fn mirror_pair(target: &Path, version: &str, host: &str, key: &str) -> Result<(String, String)> {
    let config = target.join("Assets/UpdateSources.json");
    if !config.is_file() { return Err(Error("未配置国内更新源".into())); }
    let settings: Value = paths::read_json(&config)?;
    let repository = settings.get(key).and_then(Value::as_str).unwrap_or("").trim_end_matches('/');
    if repository.is_empty() { return Err(Error("未配置国内更新源".into())); }
    let escaped = host.replace('.', r"\.");
    let pattern = Regex::new(&format!(r"^https://{escaped}/([A-Za-z0-9_.-]+)/([A-Za-z0-9_.-]+)$")).unwrap();
    let captures = pattern.captures(repository).ok_or_else(|| Error("国内更新源地址无效".into()))?;
    let api = format!("https://{host}/api/v5/repos/{}/{}", &captures[1], &captures[2]);
    let release: Value = serde_json::from_slice(&http::get_bytes(&format!("{api}/releases/tags/v{version}"), 2 * 1024 * 1024)?)?;
    let pairs = [
        (format!("DotIsle-{version}-SHA256.txt"), format!("DotIsle-{version}-Portable.7z")),
        (format!("LyricIsland-{version}-SHA256.txt"), format!("LyricIsland-{version}-Portable.zip")),
    ];
    let direct = |name: &str| release["assets"].as_array().and_then(|items| items.iter().find(|item| item["name"].as_str() == Some(name))).and_then(|item| item["browser_download_url"].as_str()).map(str::to_owned);
    for (manifest_name, package_name) in &pairs {
        if let (Some(manifest), Some(package)) = (direct(manifest_name), direct(package_name)) {
            return Ok((manifest, package));
        }
    }
    let id = release["id"].as_u64().filter(|value| *value > 0).ok_or_else(|| Error("国内发行标识无效".into()))?;
    let attachments: Value = serde_json::from_slice(&http::get_bytes(&format!("{api}/releases/{id}/attach_files?per_page=100"), 4 * 1024 * 1024)?)?;
    let mut listed: Vec<(String, String)> = Vec::new();
    for item in attachments.as_array().into_iter().flatten() {
        let Some(file_id) = item["id"].as_u64().filter(|value| *value > 0) else { continue };
        let name = item["name"].as_str().or_else(|| item["file_name"].as_str()).unwrap_or("");
        listed.push((name.to_owned(), format!("{api}/releases/{id}/attach_files/{file_id}/download")));
    }
    for (manifest_name, package_name) in &pairs {
        let manifest = listed.iter().find(|(name, _)| name == manifest_name).map(|(_, url)| url.clone());
        let package = listed.iter().find(|(name, _)| name == package_name).map(|(_, url)| url.clone());
        if let (Some(manifest), Some(package)) = (manifest, package) { return Ok((manifest, package)); }
    }
    Err(Error("国内源缺少更新附件".into()))
}

pub fn domestic_pair(target: &Path, version: &str) -> Result<(String, String)> {
    mirror_pair(target, version, "gitee.com", "giteeRepository")
}

pub fn gitcode_pair(target: &Path, version: &str) -> Result<(String, String)> {
    mirror_pair(target, version, "gitcode.com", "gitcodeRepository")
}

fn primary_pair(version: &str, _preferred: &Option<(String, String)>) -> (String, String) {
    let prefix = "https://github.com/Heiamer/DotIsle/releases/download/";
    let root = format!("{prefix}v{version}/DotIsle-{version}");
    (
        format!("{root}-SHA256.txt"),
        format!("{root}-Portable.7z"),
    )
}

fn prepare_download(target: &Path, version: &str, scratch: &Path, preferred: Option<(String, String)>, reporter: &mut Reporter) -> Result<Option<crate::module_update::Prepared>> {
    let primary = primary_pair(version, &preferred);
    let mut errors = Vec::new();
    let mut tried = HashSet::new();
let mut pinned: Option<(String, String)> = None;
    let mut candidates: Vec<(String, String)> = vec![primary.clone()];
    if let Some(pair) = preferred.clone() { candidates.push(pair); }
    for (host, key, label) in [("gitee.com", "giteeRepository", "Gitee"), ("gitcode.com", "gitcodeRepository", "GitCode")] {
        match mirror_pair(target, version, host, key) {
            Ok(pair) => candidates.push(pair),
            Err(error) => errors.push(format!("{label}：{error}")),
        }
    }
    for (step, pair) in candidates.iter().enumerate() {
        let (manifest_url, zip_url) = (pair.0.clone(), pair.1.clone());
        if !tried.insert(manifest_url.clone()) { continue; }
        reporter.report(if step == 0 { "downloading" } else { "switching" }, 0, None, if step == 0 { "开始下载" } else { "切换到备用更新源" }, true)?;
        let attempt = (|| -> Result<Option<crate::module_update::Prepared>> {
            if !official_url(&manifest_url) || !official_url(&zip_url) { return Err(Error("更新下载地址无效".into())); }
            let asset_name = if zip_url.ends_with("/download") {
                format!("LyricIsland-{version}-Portable.zip")
            } else {
                zip_url.rsplit('/').next().unwrap_or("").to_owned()
            };
            if !(asset_name.ends_with(".7z") || asset_name.ends_with(".zip")) {
                return Err(Error("更新包类型无效".into()));
            }
            let manifest = http::get_bytes(&manifest_url, 64 * 1024)?;
            let manifest = String::from_utf8_lossy(&manifest);
            let expected = asset_digest(&manifest, &asset_name).ok_or_else(|| Error("校验清单缺少更新包".into()))?;
            if let Some((pinned_name, pinned_hash)) = &pinned {
                if pinned_name == &asset_name && pinned_hash != &expected {
                    return Err(Error(PINNED_MISMATCH.into()));
                }
            }
            pinned = Some((asset_name.clone(), expected.clone()));
            let github = format!("https://github.com/Heiamer/DotIsle/releases/download/v{version}/");
            let index = format!("LyricIsland-{version}-Modules.json");
            if step == 0 && manifest_url.starts_with(&github) {
                if let Some(index_hash) = asset_digest(&manifest, &index) {
                    match crate::module_update::prepare(target, version, scratch, &github, &index, &index_hash, reporter) {
                        Ok(prepared) => return Ok(Some(prepared)),
                        Err(error) if error.is_cancelled() => return Err(error),
                        Err(error) => {
                            let _ = std::fs::write(scratch.join("component-error.txt"), error.to_string());
                            reporter.report("switching", 0, None, "组件下载或校验失败，切换到全量更新", true)?;
                        }
                    }
                }
            }
            let payload = scratch.join(if asset_name.ends_with(".7z") { "update.7z" } else { "update.zip" });
            let other = scratch.join(if asset_name.ends_with(".7z") { "update.zip" } else { "update.7z" });
            let _ = paths::remove_file_if_present(&other);
            let _ = paths::remove_file_if_present(&payload);
            let (received, length) = http::download(&zip_url, &payload, 256 * 1024 * 1024, |received, length| {
                reporter.report("downloading", received, length, &format!("正在下载已选版本 {version}"), false).map(|_| ())
            })?;
            reporter.report("verifying", received, length, "正在校验下载内容", true)?;
            verify(&payload, &expected)?;
            Ok(None)
        })();
        match attempt {
            Ok(prepared) => return Ok(prepared),
            Err(error) if error.is_cancelled() => return Err(error),
            Err(error) if error.0 == PINNED_MISMATCH => return Err(error),
            Err(error) => {
                errors.push(format!("更新源：{error}"));
                let _ = std::fs::write(scratch.join("download-errors.txt"), errors.join("\r\n"));
            }
        }
    }
    Err(Error(format!("国内源与 GitHub 均未能完成更新下载：{}", errors.join("；"))))
}

#[cfg(test)]
mod source_priority_tests{
 use super::*;
 #[test]fn github_download_is_primary_even_when_the_candidate_came_from_gitee(){
  let domestic=Some(("https://gitee.com/mirror/checksum".into(),"https://gitee.com/mirror/archive".into()));
  assert_eq!(primary_pair("5.0.3-rc3",&domestic),("https://github.com/Heiamer/DotIsle/releases/download/v5.0.3-rc3/DotIsle-5.0.3-rc3-SHA256.txt".into(),"https://github.com/Heiamer/DotIsle/releases/download/v5.0.3-rc3/DotIsle-5.0.3-rc3-Portable.7z".into()));
  let exact=Some(("https://github.com/Heiamer/DotIsle/releases/download/v5.0.3-rc3/custom-hash".into(),"https://github.com/Heiamer/DotIsle/releases/download/v5.0.3-rc3/custom-zip".into()));
  assert_eq!(primary_pair("5.0.3-rc3",&exact).1,"https://github.com/Heiamer/DotIsle/releases/download/v5.0.3-rc3/DotIsle-5.0.3-rc3-Portable.7z");
 }
 #[test]fn official_hosts_include_gitcode_and_reject_lookalikes(){
  assert!(official_url("https://gitcode.com/Heiamer/Dotisle/releases/download/v5.2.3/DotIsle-5.2.3-Portable.7z"));
  assert!(official_url("https://gitee.com/api/v5/repos/heiamer/abc/releases/1/attach_files/2/download"));
  assert!(!official_url("https://gitcode.com.evil.test/x"));
  assert!(!official_url("http://gitcode.com/x"));
  assert!(!official_url("https://evil.test/gitcode.com/x"));
 }
 #[test]
 #[ignore]
 fn gitcode_mirror_prefers_the_7z_pair() {
  let target = std::env::temp_dir().join("dotisle-gitcode-mirror-test");
  let _ = std::fs::remove_dir_all(&target);
  std::fs::create_dir_all(target.join("Assets")).unwrap();
  std::fs::write(target.join("Assets/UpdateSources.json"), include_str!("../../../../Assets/UpdateSources.json")).unwrap();
  let (manifest, package) = gitcode_pair(&target, "5.2.3").unwrap();
  assert!(manifest.ends_with("DotIsle-5.2.3-SHA256.txt"), "{manifest}");
  assert!(package.ends_with("DotIsle-5.2.3-Portable.7z"), "{package}");
  assert!(official_url(&manifest) && official_url(&package));
  let _ = std::fs::remove_dir_all(&target);
 }
}

/// Stage each replacement beside its destination. A failed rename leaves the installed
/// file untouched, so rollback only visits files that were actually replaced.
fn replace_runtime_file(input: &Path, destination: &Path) -> std::io::Result<()> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let serial = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let pending = destination.with_file_name(format!(".dotisle-ota-{}-{serial}.tmp", std::process::id()));
    let mut output = std::fs::OpenOptions::new().write(true).create_new(true).open(&pending)?;
    let copied = (|| {
        std::io::copy(&mut std::fs::File::open(input)?, &mut output)?;
        output.sync_all()
    })();
    drop(output);
    let result = copied.and_then(|()| {
        // Windows can hold a departing process's images briefly, even after it signals exit.
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match std::fs::rename(&pending, destination) {
                Err(error) if matches!(error.raw_os_error(), Some(5 | 32 | 33)) && Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                result => break result,
            }
        }
    });
    if result.is_err() { let _ = std::fs::remove_file(&pending); }
    result
}

pub fn apply(stage: &Path, target: &Path, backup: &Path, fail_after: usize, after: Option<&mut dyn FnMut() -> Result<()>>) -> Result<()> {
    let mut changed: Vec<String> = Vec::new();
    let mut existed: HashSet<String> = HashSet::new();
    let mut after = after;
    let outcome = (|| -> Result<()> {
        let files = payload::relative_files(stage)?;
        for relative in &files {
            let destination = paths::safe_child(target, relative)?;
            if !crate::update_cleanup::ordinary_path(target, &destination) {
                return Err(Error(format!("更新路径包含链接：{relative}")));
            }
        }
        for relative in files {
            let input = stage.join(relative.replace('/', "\\"));
            let destination = target.join(relative.replace('/', "\\"));
            let old = backup.join(relative.replace('/', "\\"));
            if destination.is_file() {
                // Windows may retain font mappings after the app exits. Identical
                // assets need no write, so leave them untouched during an update.
                if std::fs::metadata(&input)?.len() == std::fs::metadata(&destination)?.len()
                    && lyric_archive::sha256_file(&input)? == lyric_archive::sha256_file(&destination)? {
                    continue;
                }
                if let Some(parent) = old.parent() { std::fs::create_dir_all(parent)?; }
                std::fs::copy(&destination, &old)?;
                existed.insert(relative.clone());
            }
            if let Some(parent) = destination.parent() { std::fs::create_dir_all(parent)?; }
            replace_runtime_file(&input, &destination)
                .map_err(|error| Error(format!("无法替换 {relative}：{error}")))?;
            changed.push(relative.clone());
            if fail_after > 0 && changed.len() >= fail_after { return Err(Error("simulated write failure".into())); }
        }
        if let Some(after) = after.as_mut() { after()?; }
        Ok(())
    })();
    if let Err(error) = outcome {
        let mut rollback_errors = Vec::new();
        for relative in changed.iter().rev() {
            let destination = target.join(relative.replace('/', "\\"));
            let old = backup.join(relative.replace('/', "\\"));
            let outcome: Result<()> = if existed.contains(relative) {
                replace_runtime_file(&old, &destination).map_err(Error::from)
            } else {
                paths::remove_file_if_present(&destination)
            };
            if let Err(rollback) = outcome { rollback_errors.push(format!("{relative}：{rollback}")); }
        }
        if !rollback_errors.is_empty() {
            return Err(Error(format!("更新回滚未完成，备份保留在：{}；{}", backup.display(), rollback_errors.join("；"))));
        }
        return Err(error);
    }
    Ok(())
}

pub fn stop_app(pipe: &str) -> Result<()> {
    let reply = win::pipe_request(pipe, &json!({"version": 1, "command": "prepareUpdateExit"}), Duration::from_secs(5))?;
    if reply.get("success").and_then(Value::as_bool) != Some(true) { return Err(Error("应用未确认保存设置，已取消更新".into())); }
    Ok(())
}

pub fn wait(pid: i32) -> Result<()> {
    if pid <= 0 { return Ok(()); }
    if !win::wait_process(pid as u32, Duration::from_secs(15)) { return Err(Error("应用尚未退出，更新已取消".into())); }
    Ok(())
}

pub fn start(target: &Path, data: &Path) -> Result<()> {
    let executable = target.join("LyricIsland.exe");
    let mut command = std::process::Command::new(&executable);
    if !data.as_os_str().is_empty() { command.arg("--data-dir").arg(data); }
    command.current_dir(target);
    command.spawn().map_err(|error| Error(format!("无法启动新版：{error}")))?;
    Ok(())
}

pub fn merge_marker(stage: &Path, target: &Path, version: &str) -> Result<()> {
    let marker_path = target.join(payload::INSTALLED_MARKER);
    if !marker_path.is_file() { return Ok(()); }
    let mut marker = payload::read_marker(target)?;
    let mut files: HashSet<String> = payload::marker_files(&marker).into_iter().collect();
    for file in payload::relative_files(stage)? { files.insert(file); }
    let mut files: Vec<String> = files.into_iter().collect();
    files.sort();
    marker["version"] = json!(version);
    marker["files"] = json!(files);
    paths::write_json(&stage.join(payload::INSTALLED_MARKER), &marker)
}

pub fn executable_hash(target: &Path) -> Result<String> { Ok(lyric_archive::sha256_file(&target.join("LyricIsland.exe"))?) }

pub fn self_test(report: &Path) -> Result<()> {
    let root = paths::temp_unique("LyricIsland.UpdateTest");
    std::fs::create_dir_all(&root)?;
    let outcome = (|| -> Result<()> {
        let target = root.join("target");
        let stage = root.join("stage");
        std::fs::create_dir_all(&target)?;
        std::fs::create_dir_all(&stage)?;
        std::fs::create_dir_all(target.join("Data"))?;
        std::fs::write(target.join("Data/settings.json"), "keep")?;
        std::fs::write(target.join("a"), "old")?;
        std::fs::write(stage.join("a"), "new")?;
        std::fs::write(stage.join("b"), "added")?;
        let failed = apply(&stage, &target, &root.join("backup"), 1, None).is_err();
        if !failed || std::fs::read_to_string(target.join("a"))? != "old" { return Err(Error("rollback failed".into())); }
        apply(&stage, &target, &root.join("backup2"), 0, None)?;
        if std::fs::read_to_string(target.join("a"))? != "new" || std::fs::read_to_string(target.join("Data/settings.json"))? != "keep" {
            return Err(Error("apply failed".into()));
        }
        let evil = root.join("evil.zip");
        let mut writer = ZipWriter::create(&evil)?;
        writer.add_bytes("LyricIsland-9.9/../escape", b"bad")?;
        writer.finish()?;
        if extract(&evil, &root.join("extract"), "9.9").is_ok() { return Err(Error("unsafe ZIP accepted".into())); }
        // Exercise a major update that ships neither the old DLLs nor the old font set.
        let installed = root.join("installed");
        let user_data = installed.join("Data");
        std::fs::create_dir_all(installed.join("Assets"))?;
        std::fs::create_dir_all(&user_data)?;
        std::fs::write(user_data.join("settings.json"), b"keep")?;
        std::fs::write(installed.join("LyricIsland.exe"), b"old")?;
        for obsolete in ["dotisle_glow.dll", "dotisle_osd.dll"] {
            std::fs::write(installed.join(obsolete), b"shipped")?;
        }
        paths::write_json(&installed.join(payload::RUNTIME_INVENTORY), &json!({"schemaVersion":1,"product":"DotIsle","version":"9.8","files":{
            "dotisle_glow.dll":lyric_archive::sha256_bytes(b"shipped"),"dotisle_osd.dll":lyric_archive::sha256_bytes(b"shipped")
        }}))?;
        std::fs::write(installed.join("dotisle_osd.dll"), b"user replacement")?;

        for (kind, extra, corrupt, missing, wrong_version, alias) in [
            ("valid", false, false, false, false, false), ("extra", true, false, false, false, false),
            ("corrupt", false, true, false, false, false), ("missing", false, false, true, false, false),
            ("wrong-version", false, false, false, true, false), ("alias", false, false, false, false, true),
        ] {
            let zip = root.join(format!("{kind}.zip"));
            let mut writer = ZipWriter::create(&zip)?;
            let inventory = json!({"schemaVersion":1,"product":"DotIsle",
                "version": if wrong_version { "9.8" } else { "9.9" },
                "files":{"LyricIsland.exe":lyric_archive::sha256_bytes(b"new"),"Assets/New.txt":lyric_archive::sha256_bytes(b"added")}});
            writer.add_bytes(&format!("LyricIsland-9.9/{}", payload::RUNTIME_INVENTORY), &serde_json::to_vec(&inventory)?)?;
            writer.add_bytes("LyricIsland-9.9/LyricIsland.exe", if corrupt { b"bad" } else { b"new" })?;
            if !missing { writer.add_bytes("LyricIsland-9.9/Assets/New.txt", b"added")?; }
            if extra { writer.add_bytes("LyricIsland-9.9/extra.dll", b"extra")?; }
            if alias { writer.add_bytes("LyricIsland-9.9/assets/new.TXT", b"duplicate")?; }
            writer.finish()?;
            let candidate = extract(&zip, &root.join(format!("stage-{kind}")), "9.9");
            if kind != "valid" {
                if candidate.is_ok() { return Err(Error(format!("invalid update accepted: {kind}"))); }
                continue;
            }
            let stage = candidate?;
            let retired = crate::update_cleanup::plan(&stage, &installed)?;
            let work = paths::temp_unique("LyricIsland.Update");
            let mut bad_start = || Err(Error("simulated launch failure".into()));
            if apply(&stage, &installed, &root.join("failed-start"), 0, Some(&mut bad_start)).is_ok()
                || std::fs::read(installed.join("LyricIsland.exe"))? != b"old"
                || !installed.join("dotisle_glow.dll").is_file() {
                return Err(Error("launch rollback failed".into()));
            }
            apply(&stage, &installed, &work.join("backup"), 0, None)?;
            std::fs::write(work.join("success.txt"), "9.9")?;
            let cleanup = crate::update_cleanup::finish(&work, &installed, &user_data, &retired, "9.9");
            if cleanup["success"] != true || installed.join("dotisle_glow.dll").exists()
                || std::fs::read(installed.join("dotisle_osd.dll"))? != b"user replacement"
                || std::fs::read(user_data.join("settings.json"))? != b"keep" {
                return Err(Error("obsolete bundled DLL or user data cleanup failed".into()));
            }
        }
        paths::write_json(report, &json!({"success": true, "rollback": true, "preservesData": true,
            "rejectsTraversal": true, "packageInventory": true, "removedComponent": true,
            "preservesModifiedComponent":true,"rejectsCorruptMissingExtraAliasAndWrongVersion": true}))
    })();
    let _ = std::fs::remove_dir_all(&root);
    outcome
}

fn confirm_started(pipe: &str, version: &str, old_pid: u32) -> bool {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if let Ok(state) = win::pipe_request(pipe, &json!({"version": 1, "command": "state"}), Duration::from_secs(1)) {
            if (state["version"].as_str() == Some(version) || state["bundleVersion"].as_str() == Some(version)) && state["processId"].as_u64() != Some(old_pid as u64) { return true; }
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    false
}

pub fn run(args: Vec<String>) -> i32 {
    if let Some(code)=crate::local_modules::cli(&args){return code;}
    if let Some(code) = crate::vello_overlay::cli(&args) { return code; }
    if args.len() == 2 && args[0] == "--self-test" {
        return match self_test(Path::new(&args[1])) { Ok(()) => 0, Err(error) => { eprintln!("{error}"); 1 } };
    }
    let mut overlaid: Option<Overlay> = None;
    let mut target: Option<PathBuf> = None;
    let mut data: Option<PathBuf> = None;
    let mut stopped = false;
    let mut scratch: Option<PathBuf> = None;
    let mut reporter: Option<Reporter> = None;
    let result = (|| -> Result<()> {
        if !matches!(args.len(), 6 | 8 | 10) { return Err(Error("更新参数无效".into())); }
        let version = args[0].clone();
        if !Regex::new(r"^[0-9]+\.[0-9]+(?:\.[0-9]+)?(?:(?:-?mini)|-(?:(?:alpha|beta|preview)\.[0-9]+|rc\.?[0-9]+))?$").unwrap().is_match(&version) { return Err(Error("版本号无效".into())); }
        let target_path = paths::full_path(&args[1])?;
        let data_path = paths::full_path(&args[2])?;
        let pid: i32 = args[3].parse().map_err(|_| Error("更新参数无效".into()))?;
        let settings: i32 = args[4].parse().map_err(|_| Error("更新参数无效".into()))?;
        let pipe = args[5].clone();
        target = Some(target_path.clone());
        data = Some(data_path.clone());
        let (authenticated, job, nonce) = if args.len() == 10 { (true, args[8].clone(), args[9].clone()) } else { (false, String::new(), String::new()) };
        if !target_path.join("LyricIsland.exe").is_file() { return Err(Error("找不到原应用".into())); }
        let mutex_name = format!("Local.LyricIsland.OTA.{}", executable_hash(&target_path)?);
        let _gate = win::NamedMutex::acquire(&mutex_name)?.ok_or_else(|| Error("已有更新正在进行".into()))?;
        let work = paths::temp_unique("LyricIsland.Update");
        std::fs::create_dir_all(&work)?;
        scratch = Some(work.clone());
        let preferred = if args.len() >= 8 && !args[6].is_empty() && !args[7].is_empty() { Some((args[6].clone(), args[7].clone())) } else { None };
        let state = shared_state();
        reporter = Some(Reporter::new(authenticated, job.clone(), nonce, pipe.clone(), state.clone()));
        let reporter = reporter.as_mut().unwrap();
        let components = prepare_download(&target_path, &version, &work, preferred, reporter)?;
        let stage = if let Some(prepared) = &components {
            prepared.stage.clone()
        } else {
            let stage = work.join("stage");
            if work.join("update.7z").is_file() {
                extract_7z(&work.join("update.7z"), &stage, &version)?;
            } else {
                extract(&work.join("update.zip"), &stage, &version)?;
            }
            // A portable archive is also the OTA payload for installed copies. Do
            // not switch their storage mode during the transaction.
            if !target_path.join("Portable.json").is_file() {
                paths::remove_file_if_present(&stage.join("Portable.json"))?;
                let path = stage.join(payload::RUNTIME_INVENTORY);
                let mut inventory = paths::read_json(&path)?;
                inventory["files"].as_object_mut().unwrap().remove("Portable.json");
                paths::write_json(&path, &inventory)?;
            }
            stage
        };
        let retired=crate::update_cleanup::plan(&stage,&target_path)?;
        merge_marker(&stage, &target_path, &version)?;

        if authenticated {
            reporter.report("ready", 0, None, "校验完成，等待你点击重启并安装", true)?;
            loop {
                if reporter.report("ready", 0, None, "校验完成，等待你点击重启并安装", true)? == "apply" { break; }
                std::thread::sleep(Duration::from_millis(500));
            }
        }
        reporter.report("applying", 0, None, "正在保存数据并安装", true)?;
        // A window failure must never abort the install; the update continues headless.
        overlaid = Overlay::show(state.clone(),&target_path,&data_path,&version,if authenticated{Some(job.as_str())}else{None}).ok();
        stop_app(&pipe)?;
        stopped = true;
        wait(pid)?;
        wait(settings)?;
        if let Some(prepared) = &components { prepared.check_unchanged(&target_path)?; }
        let confirmation_pipe = pipe.clone();
        let confirmation_version = version.clone();
        let mut after = || -> Result<()> {
            reporter.set_message("restarting", "正在启动并确认新版…");
            start(&target_path, &data_path)?;
            if !confirm_started(&confirmation_pipe, &confirmation_version, pid as u32) {
                let _ = stop_app(&confirmation_pipe);
                std::thread::sleep(Duration::from_millis(600));
                return Err(Error("新版未确认启动，准备回滚".into()));
            }
            Ok(())
        };
        apply(&stage, &target_path, &work.join("backup"), 0, Some(&mut after))?;
        stopped = false;
        reporter.set_message("completed", "新版已启动，设置已保留。");
        std::thread::sleep(Duration::from_millis(1200));
        if let Some(mut overlay) = overlaid.take() { overlay.close(); }
        let _=std::fs::write(work.join("success.txt"), &version);
        let mut cleanup=crate::update_cleanup::finish(&work,&target_path,&data_path,&retired,&version);
        cleanup["helperCleanupScheduled"]=json!(crate::update_cleanup::schedule_helper_cleanup());
        let _=paths::write_json(&data_path.join("update-cleanup.json"),&cleanup);
        Ok(())
    })();
    match result {
        Ok(()) => 0,
        Err(error) if error.is_cancelled() => {
            if let Some(mut overlay) = overlaid.take() { overlay.close(); }
            if let Some(reporter) = reporter.as_mut() {
                let _ = reporter.report("cancelled", 0, None, CANCELLED_MESSAGE, true);
            }
            2
        }
        Err(error) => {
            if let Some(mut overlay) = overlaid.take() { overlay.close(); }
            // Keep the same sequence so the hub accepts the terminal report.
            if let Some(reporter) = reporter.as_mut() {
                let _ = reporter.report("failed", 0, None, &error.to_string(), true);
            } else {
                let _ = report_failure(&args, &error);
            }
            if stopped { if let (Some(target), Some(data)) = (target.as_ref(), data.as_ref()) { let _ = start(target, data); } }
            let details = scratch.map(|path| {
                let _ = std::fs::write(path.join("failure.txt"), error.to_string());
                format!("\n诊断和备份：{}", path.display())
            }).unwrap_or_default();
            win::message_box(&format!("在线更新未完成：{error}{details}"), "点屿在线更新", true);
            1
        }
    }
}

fn report_failure(args: &[String], error: &Error) -> Result<()> {
    if args.len() != 10 { return Ok(()); }
    let state = shared_state();
    let mut reporter = Reporter::new(true, args[8].clone(), args[9].clone(), args[5].clone(), state.clone());
    let _ = state.lock().map(|mut value| value.phase = "failed".into());
    reporter.report("failed", 0, None, &error.to_string(), true)?;
    Ok(())
}

#[cfg(test)]
mod update_file_tests {
    use super::*;
    use std::os::windows::fs::OpenOptionsExt;

    #[test]
    fn locked_destination_does_not_get_rewritten_during_rollback() {
        let root = paths::temp_unique("DotIsle.LockedDestinationTest");
        let stage = root.join("stage"); let target = root.join("target"); let backup = root.join("backup");
        std::fs::create_dir_all(&stage).unwrap(); std::fs::create_dir_all(&target).unwrap();
        for (file, original) in [("a.bin", b"old a"), ("b.bin", b"old b")] {
            std::fs::write(target.join(file), original).unwrap();
            std::fs::write(stage.join(file), b"new bytes").unwrap();
        }
        let locked = std::fs::OpenOptions::new().read(true).share_mode(1).open(target.join("b.bin")).unwrap();
        let error = apply(&stage, &target, &backup, 0, None).unwrap_err();
        assert!(error.to_string().contains("b.bin"), "{error}");
        assert_eq!(std::fs::read(target.join("a.bin")).unwrap(), b"old a");
        assert_eq!(std::fs::read(target.join("b.bin")).unwrap(), b"old b");
        assert_eq!(std::fs::read_dir(&target).unwrap().count(), 2);
        assert!(!error.to_string().contains("更新回滚未完成"), "{error}");
        drop(locked); std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn transient_sharing_lock_is_retried() {
        let root = paths::temp_unique("DotIsle.TransientLockTest");
        std::fs::create_dir_all(&root).unwrap();
        let input = root.join("input.bin"); let destination = root.join("runtime.bin");
        std::fs::write(&input, b"new").unwrap(); std::fs::write(&destination, b"old").unwrap();
        let lock = std::fs::OpenOptions::new().read(true).share_mode(1).open(&destination).unwrap();
        let releasing = std::thread::spawn(move || { std::thread::sleep(Duration::from_millis(350)); drop(lock); });
        replace_runtime_file(&input, &destination).unwrap();
        releasing.join().unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), b"new");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unchanged_locked_assets_are_preserved_while_changed_files_are_updated() {
        let root = paths::temp_unique("DotIsle.LockedAssetTest");
        let stage = root.join("stage");let target = root.join("target");let backup = root.join("backup");
        std::fs::create_dir_all(&stage).unwrap();std::fs::create_dir_all(&target).unwrap();
        for directory in [&stage, &target] { std::fs::write(directory.join("font.ttf"), b"same font bytes").unwrap(); }
        std::fs::write(stage.join("app.bin"), b"new").unwrap();std::fs::write(target.join("app.bin"), b"old").unwrap();
        let locked = std::fs::OpenOptions::new().read(true).share_mode(1).open(target.join("font.ttf")).unwrap();
        assert!(std::fs::copy(stage.join("font.ttf"), target.join("font.ttf")).is_err());
        apply(&stage, &target, &backup, 0, None).unwrap();
        assert_eq!(std::fs::read(target.join("app.bin")).unwrap(), b"new");
        assert_eq!(std::fs::read(backup.join("app.bin")).unwrap(), b"old");
        assert!(!backup.join("font.ttf").exists());
        drop(locked);std::fs::remove_dir_all(root).unwrap();
    }
}
