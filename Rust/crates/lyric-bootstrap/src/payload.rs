use crate::error::{Error, Result};
use crate::paths::{self, safe_child};
use crate::win;
use lyric_archive::{sha256_bytes, sha256_file, ZipReader};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PAYLOAD_RESOURCE: &str = "DotIsle.InstallPayload.zip";
pub const PAYLOAD_DIGEST_RESOURCE: &str = "DotIsle.InstallPayload.sha256";
pub const LAUNCHER_PAYLOAD_RESOURCE: &str = "LyricIsland.Payload.zip";
pub const INSTALLED_MARKER: &str = "Installed.json";
pub const RUNTIME_INVENTORY: &str = "Assets/RuntimeFiles.json";

pub fn required_runtime() -> &'static [&'static str] {
    &["dotisle_focus.dll","dotisle_todos.dll","dotisle_music.dll","dotisle_lyrics.dll","dotisle_notifications.dll","dotisle_history.dll","dotisle_automation.dll","dotisle_devices.dll","dotisle_osd.dll","dotisle_sound.dll","dotisle_widgets.dll","dotisle_themes.dll","dotisle_glow.dll","dotisle_core.dll", "dotisle_desktop.dll", "LyricIsland.exe", "LyricIsland.Center.exe", "LyricIsland.Updater.exe", "Assets/Fonts/GoogleSansFlex-Regular.ttf", "Assets/Fonts/GoogleSansFlex-Medium.ttf", "Assets/Fonts/GoogleSansFlex-SemiBold.ttf", "Assets/Fonts/GoogleSansFlex-Bold.ttf", "Assets/Fonts/NotoSansSC.ttf", "Assets/Fonts/NotoSansJP.ttf"]
}

pub fn read_marker(target: &Path) -> Result<Value> { paths::read_json(&target.join(INSTALLED_MARKER)) }

pub fn marker_product(marker: &Value) -> &str { marker["product"].as_str().unwrap_or("") }

pub fn marker_version(marker: &Value) -> &str { marker["version"].as_str().unwrap_or("") }

pub fn marker_data(marker: &Value) -> Option<PathBuf> { marker["dataDirectory"].as_str().filter(|text| !text.is_empty()).map(PathBuf::from) }

pub fn marker_files(marker: &Value) -> Vec<String> {
    marker["files"].as_array().map(|items| items.iter().filter_map(|item| item.as_str().map(str::to_owned)).collect()).unwrap_or_default()
}

pub fn write_marker(stage: &Path, data: &Path, files: &[String]) -> Result<()> {
    paths::write_json(&stage.join(INSTALLED_MARKER), &json!({
        "product": "DotIsle",
        "version": VERSION,
        "dataDirectory": data.to_string_lossy(),
        "files": files,
    }))
}

pub fn relative_files(root: &Path) -> Result<Vec<String>> {
    let mut output = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in std::fs::read_dir(&directory)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() { stack.push(path); }
            else if entry.file_type()?.is_file() {
                let relative = path.strip_prefix(root).map_err(|_| Error("安装文件路径无效".into()))?;
                output.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    output.sort();
    Ok(output)
}

pub fn extract_payload(payload: &[u8], directory: &Path) -> Result<PathBuf> {
    let scratch = paths::temp_unique("DotIsle.Payload");
    std::fs::create_dir_all(&scratch)?;
    let zip_path = scratch.join("payload.zip");
    std::fs::write(&zip_path, payload)?;
    let result = extract_zip_file(&zip_path, directory, &format!("LyricIsland-{VERSION}"), required_runtime(), 512 * 1024 * 1024);
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

pub fn extract_zip_file(zip_path: &Path, stage: &Path, folder: &str, required: &[&str], limit: u64) -> Result<PathBuf> {
    let mut reader = ZipReader::open(zip_path)?;
    extract_reader(&mut reader, stage, folder, required, limit)?;
    Ok(stage.to_path_buf())
}

pub(crate) fn update_path(root: &Path, name: &str) -> Result<PathBuf> {
    let first = name.split('/').next().unwrap_or("").to_ascii_lowercase();
    if matches!(first.as_str(), "data" | "logs" | "cache" | "installed.json" | "modulestate.json" | "otainstall.json" | "settings.json")
        || name.contains(['\\', ':'])
        || name.split('/').any(|part| part.is_empty() || part.ends_with(['.', ' ']))
    {
        return Err(Error(format!("更新清单路径无效：{name}")));
    }
    safe_child(root, name)
}

pub fn extract_update(zip: &Path, stage: &Path, version: &str) -> Result<PathBuf> {
    const LIMIT: u64 = 512 * 1024 * 1024;
    let mut reader = ZipReader::open(zip)?;
    let prefix = format!("LyricIsland-{version}/");
    let mut files = std::collections::BTreeMap::new();
    let mut seen = HashSet::new();
    let mut total = 0u64;
    for (index, entry) in reader.entries().iter().enumerate() {
        let name = entry.name.strip_prefix(&prefix).ok_or_else(|| Error("更新包目录无效".into()))?;
        if name.is_empty() && entry.is_dir { continue; }
        let name = if entry.is_dir { name.trim_end_matches('/') } else { name };
        update_path(stage, name)?;
        if !seen.insert(name.to_lowercase()) { return Err(Error("更新包包含重复路径".into())); }
        if entry.is_dir { continue; }
        total = total.saturating_add(entry.size);
        if total > LIMIT { return Err(Error("更新包超过大小限制".into())); }
        files.insert(name.to_owned(), (index, entry.size));
    }
    let &(inventory_index, size) = files.get(RUNTIME_INVENTORY).ok_or_else(|| Error("更新包缺少文件校验清单".into()))?;
    if size > 4 * 1024 * 1024 { return Err(Error("更新清单超过大小限制".into())); }
    let inventory_bytes = reader.read(inventory_index)?;
    let inventory: Value = serde_json::from_slice(&inventory_bytes)?;
    if inventory["schemaVersion"] != 1 || inventory["product"] != "DotIsle" || inventory["version"] != version {
        return Err(Error("更新清单与目标版本不符".into()));
    }
    let declared = inventory["files"].as_object().ok_or_else(|| Error("更新清单缺少文件列表".into()))?;
    // Only the stable launch entry point is required by the transaction itself.
    // Everything else (modules, fonts and other assets) belongs to the new package.
    if !declared.contains_key("LyricIsland.exe") || declared.len() + 1 != files.len() || declared.contains_key(RUNTIME_INVENTORY) {
        return Err(Error("更新包文件与清单不符".into()));
    }
    for (name, hash) in declared {
        update_path(stage, name)?;
        if !files.contains_key(name) || !hash.as_str().is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())) {
            return Err(Error(format!("更新包缺少清单文件或摘要无效：{name}")));
        }
    }
    // The stage is private to this transaction and must not contain unverified files.
    if stage.exists() { return Err(Error("更新暂存目录已存在".into())); }
    std::fs::create_dir_all(stage)?;
    for (name, hash) in declared {
        let bytes = reader.read(files[name].0)?;
        if !sha256_bytes(&bytes).eq_ignore_ascii_case(hash.as_str().unwrap()) {
            return Err(Error(format!("更新文件摘要不符：{name}")));
        }
        let path = update_path(stage, name)?;
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(path, bytes)?;
    }
    let inventory_path = stage.join(RUNTIME_INVENTORY);
    std::fs::create_dir_all(inventory_path.parent().unwrap())?;
    std::fs::write(inventory_path, inventory_bytes)?;
    Ok(stage.to_path_buf())
}

pub fn extract_update_7z(archive: &Path, stage: &Path, version: &str) -> Result<PathBuf> {
    const LIMIT: u64 = 512 * 1024 * 1024;
    if stage.exists() {
        return Err(Error("更新暂存目录已存在".into()));
    }
    let temp = stage.with_extension("7z-extract");
    if temp.exists() {
        std::fs::remove_dir_all(&temp)?;
    }
    std::fs::create_dir_all(&temp)?;
    let mut seen = HashSet::new();
    let mut total = 0u64;
    let mut failure: Option<String> = None;
    let unpacked = sevenz_rust::decompress_file_with_extract_fn(archive, &temp, |entry, reader, _| {
        let mut name = entry.name().replace('\\', "/");
        name = name.trim_end_matches('/').to_owned();
        if name.is_empty() {
            return Ok(true);
        }
        if !seen.insert(name.to_ascii_lowercase()) {
            failure = Some("更新包包含重复路径".into());
            return Ok(false);
        }
        let path = match update_path(&temp, &name) {
            Ok(path) => path,
            Err(error) => {
                failure = Some(error.0);
                return Ok(false);
            }
        };
        if entry.is_directory() {
            if let Err(error) = std::fs::create_dir_all(&path) {
                failure = Some(error.to_string());
                return Ok(false);
            }
            return Ok(true);
        }
        total = total.saturating_add(entry.size());
        if total > LIMIT {
            failure = Some("更新包超过大小限制".into());
            return Ok(false);
        }
        if let Some(parent) = path.parent() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                failure = Some(error.to_string());
                return Ok(false);
            }
        }
        let mut file = match std::fs::File::create(&path) {
            Ok(file) => file,
            Err(error) => {
                failure = Some(error.to_string());
                return Ok(false);
            }
        };
        if let Err(error) = std::io::copy(reader, &mut file) {
            failure = Some(error.to_string());
            return Ok(false);
        }
        Ok(true)
    });
    if let Some(message) = failure {
        let _ = std::fs::remove_dir_all(&temp);
        return Err(Error(message));
    }
    if let Err(error) = unpacked {
        let _ = std::fs::remove_dir_all(&temp);
        return Err(Error(format!("7z 解压失败：{error}")));
    }
    let mut payload = None;
    for entry in std::fs::read_dir(&temp)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() && entry.path().join(RUNTIME_INVENTORY).is_file() {
            payload = Some(entry.path());
        } else if kind.is_file() {
            let _ = std::fs::remove_dir_all(&temp);
            return Err(Error("更新包目录无效".into()));
        }
    }
    let Some(payload) = payload else {
        let _ = std::fs::remove_dir_all(&temp);
        return Err(Error("更新包缺少运行时目录".into()));
    };
    let inventory_path = payload.join(RUNTIME_INVENTORY);
    let inventory_bytes = std::fs::read(&inventory_path)?;
    if inventory_bytes.len() > 4 * 1024 * 1024 {
        let _ = std::fs::remove_dir_all(&temp);
        return Err(Error("更新清单超过大小限制".into()));
    }
    let inventory: Value = serde_json::from_slice(&inventory_bytes)?;
    if inventory["schemaVersion"] != 1
        || inventory["product"] != "DotIsle"
        || inventory["version"] != version
    {
        let _ = std::fs::remove_dir_all(&temp);
        return Err(Error("更新清单与目标版本不符".into()));
    }
    let declared = inventory["files"]
        .as_object()
        .ok_or_else(|| Error("更新清单缺少文件列表".into()))?;
    let mut actual = BTreeSet::new();
    collect_files(&payload, &payload, &mut actual)?;
    actual.remove(RUNTIME_INVENTORY);
    if declared.len() != actual.len()
        || !declared.contains_key("LyricIsland.exe")
        || declared.contains_key(RUNTIME_INVENTORY)
    {
        let _ = std::fs::remove_dir_all(&temp);
        return Err(Error("更新包文件与清单不符".into()));
    }
    for (name, hash) in declared {
        update_path(&payload, name)?;
        let hash = hash
            .as_str()
            .filter(|value| value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(|| Error(format!("更新清单摘要无效：{name}")))?;
        if !actual.contains(name) {
            let _ = std::fs::remove_dir_all(&temp);
            return Err(Error(format!("更新包缺少清单文件：{name}")));
        }
        if !sha256_file(&payload.join(name))?.eq_ignore_ascii_case(hash) {
            let _ = std::fs::remove_dir_all(&temp);
            return Err(Error(format!("更新文件摘要不符：{name}")));
        }
    }
    if stage.exists() {
        let _ = std::fs::remove_dir_all(&temp);
        return Err(Error("更新暂存目录已存在".into()));
    }
    std::fs::rename(&payload, stage)?;
    let _ = std::fs::remove_dir_all(&temp);
    Ok(stage.to_path_buf())
}

fn collect_files(root: &Path, directory: &Path, files: &mut BTreeSet<String>) -> Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err(Error("更新包包含链接".into()));
        }
        if kind.is_dir() {
            collect_files(root, &entry.path(), files)?;
        } else if kind.is_file() {
            let relative = entry
                .path()
                .strip_prefix(root)
                .map_err(|_| Error("更新包目录无效".into()))?
                .components()
                .map(|part| part.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            files.insert(relative);
        }
    }
    Ok(())
}

pub fn extract_reader(reader: &mut ZipReader, stage: &Path, folder: &str, required: &[&str], limit: u64) -> Result<()> {
    let prefix = format!("{folder}/");
    std::fs::create_dir_all(stage)?;
    let mut total = 0u64;
    let mut seen = HashSet::new();
    for index in 0..reader.entries().len() {
        let entry = reader.entries()[index].clone();
        if !entry.name.starts_with(&prefix) { return Err(Error("更新包目录无效".into())); }
        let relative = &entry.name[prefix.len()..];
        if relative.is_empty() { continue; }
        let path = safe_child(stage, relative)?;
        let key = path.to_string_lossy().to_lowercase();
        if !seen.insert(key) { return Err(Error("更新包包含重复路径".into())); }
        if entry.is_dir { std::fs::create_dir_all(&path)?; continue; }
        total = total.saturating_add(entry.size);
        if total > limit { return Err(Error("更新包超过大小限制".into())); }
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
        let data = reader.read(index)?;
        std::fs::write(&path, data)?;
    }
    for name in required {
        if !stage.join(name).is_file() { return Err(Error(format!("更新包缺少必要文件：{name}"))); }
    }
    Ok(())
}

pub fn launcher_payload() -> Result<Vec<u8>> {
    win::resource(LAUNCHER_PAYLOAD_RESOURCE).map_err(|_| Error("启动组件缺失。".into()))
}

pub fn install_payload() -> Result<Vec<u8>> {
    win::resource(PAYLOAD_RESOURCE).map_err(|_| Error("安装组件缺失".into()))
}

pub fn verified_install_payload() -> Result<Vec<u8>> {
    let bytes = install_payload()?;
    let digest = win::resource(PAYLOAD_DIGEST_RESOURCE).map_err(|_| Error("安装组件校验失败".into()))?;
    let expected = String::from_utf8_lossy(&digest).trim().to_ascii_lowercase();
    if expected.is_empty() || sha256_bytes(&bytes) != expected { return Err(Error("安装组件校验失败".into())); }
    Ok(bytes)
}

pub fn write_verification(directory: &Path, executable: &Path, full: bool) -> Result<()> {
    let value = if full {
        json!({
            "success": true,
            "version": VERSION,
            "executable": executable.to_string_lossy(),
            "gui": "Direct2D",
            "settingsUi": "MyGo / Direct3D11",
            "backend": "Rust",
        })
    } else {
        json!({"success": true, "version": VERSION, "installer": true})
    };
    paths::write_json(&directory.join("verification.json"), &value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lyric_archive::ZipWriter;

    fn scratch(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!("dotisle-payload-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[test]
    fn extraction_rejects_traversal_and_folder_escapes() {
        let root = scratch("traversal");
        let zip = root.join("evil.zip");
        let mut writer = ZipWriter::create(&zip).unwrap();
        writer.add_bytes("LyricIsland-9.9/../escape", b"bad").unwrap();
        writer.finish().unwrap();
        let outcome = extract_zip_file(&zip, &root.join("stage"), "LyricIsland-9.9", &[], 1024);
        assert!(outcome.is_err());
        assert!(!root.join("escape").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn extraction_requires_declared_runtime_files() {
        let root = scratch("required");
        let zip = root.join("bundle.zip");
        let mut writer = ZipWriter::create(&zip).unwrap();
        writer.add_bytes(&format!("LyricIsland-{VERSION}/LyricIsland.exe"), b"x").unwrap();
        writer.finish().unwrap();
        let outcome = extract_zip_file(&zip, &root.join("stage"), &format!("LyricIsland-{VERSION}"), required_runtime(), 4096);
        assert!(outcome.unwrap_err().to_string().contains("缺少必要文件"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "set DOTISLE_7Z_TEST to a real portable .7z fixture"]
    fn portable_7z_extracts_and_validates_inventory() {
        let Some(archive) = std::env::var_os("DOTISLE_7Z_TEST") else { return; };
        let version = std::env::var("DOTISLE_7Z_VERSION").unwrap_or_else(|_| VERSION.into());
        let root = scratch("sevenz");
        let stage = root.join("stage");
        let result = extract_update_7z(Path::new(&archive), &stage, &version);
        assert!(result.is_ok(), "{result:?}");
        assert!(stage.join("LyricIsland.exe").is_file());
        assert!(stage.join(RUNTIME_INVENTORY).is_file());
        std::fs::remove_dir_all(root).unwrap();
    }
}
