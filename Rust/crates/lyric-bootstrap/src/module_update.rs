//! Hash-pinned component downloads for a complete target release.
use crate::{error::{Error, Result}, http, paths, payload, update_cleanup, updater::{self, Reporter}};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::{BTreeMap, BTreeSet}, path::{Path, PathBuf}};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Index { schema_version: u32, version: String, runtime_version: String, packages: Vec<Pack> }

#[derive(Deserialize)]
struct Pack { archive: String, sha256: String, bytes: u64, files: BTreeMap<String, String> }

const MAX_DOWNLOAD: u64 = 256 * 1024 * 1024;
const MAX_EXTRACT: u64 = 512 * 1024 * 1024;
const PORTABLE: &str = "Portable.json";

fn fail(message: &str) -> Error { Error(message.into()) }
fn valid_hash(hash: &str) -> bool { hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) }

fn validate_index(index: &Index, version: &str) -> Result<()> {
    if index.schema_version != 2 || index.version != version || index.runtime_version != version || !(1..=128).contains(&index.packages.len()) {
        return Err(fail("组件清单版本不兼容"));
    }
    let mut paths = BTreeSet::new();
    let mut archives = BTreeSet::new();
    for pack in &index.packages {
        if !pack.archive.ends_with(".zip") || !pack.archive.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
            || !pack.archive.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            || !archives.insert(pack.archive.to_ascii_lowercase()) || !valid_hash(&pack.sha256)
            || !(1..=MAX_DOWNLOAD).contains(&pack.bytes) || pack.files.is_empty() {
            return Err(fail("组件包信息无效"));
        }
        for (name, hash) in &pack.files {
            let key = name.to_ascii_lowercase();
            payload::update_path(Path::new("."), name)?;
            if key == payload::RUNTIME_INVENTORY.to_ascii_lowercase() || !valid_hash(hash) || !paths.insert(key) || paths.len() > 10000 {
                return Err(fail("组件包含无效或重复的文件"));
            }
        }
    }
    if !paths.contains("lyricisland.exe") || !paths.contains("portable.json") {
        return Err(fail("组件清单缺少运行时必要文件"));
    }
    Ok(())
}

pub fn validate(index: &Value, version: &str) -> Result<()> {
    validate_index(&serde_json::from_value(index.clone())?, version)
}

fn matches_file(root: &Path, name: &str, expected: &str) -> Result<bool> {
    let file = payload::update_path(root, name)?;
    if !update_cleanup::ordinary_path(root, &file) { return Err(fail("组件路径包含链接")); }
    Ok(file.is_file() && lyric_archive::sha256_file(&file).is_ok_and(|hash| hash == expected))
}

pub(crate) struct Prepared { pub stage: PathBuf, unchanged: BTreeMap<String, String> }

impl Prepared {
    pub fn check_unchanged(&self, target: &Path) -> Result<()> {
        for (name, hash) in &self.unchanged {
            if !matches_file(target, name, hash)? { return Err(Error(format!("准备更新期间文件已更改，请重试：{name}"))); }
        }
        Ok(())
    }
}

pub(crate) fn prepare(target: &Path, version: &str, scratch: &Path, base: &str, name: &str, expected: &str, reporter: &mut Reporter) -> Result<Prepared> {
    let bytes = http::get_bytes(&format!("{base}{name}"), 4 * 1024 * 1024)?;
    if lyric_archive::sha256_bytes(&bytes) != expected { return Err(fail("组件清单校验失败")); }
    let index = serde_json::from_slice(&bytes)?;
    stage_packages(target, version, scratch, &index, reporter, |pack, archive, progress| {
        http::download(&format!("{base}{}", pack.archive), archive, pack.bytes, |n, _| progress(n)).map(|(n, _)| n)
    })
}

fn stage_packages(target: &Path, version: &str, scratch: &Path, index: &Index, reporter: &mut Reporter,
                  mut fetch: impl FnMut(&Pack, &Path, &mut dyn FnMut(u64) -> Result<()>) -> Result<u64>) -> Result<Prepared> {
    validate_index(index, version)?;
    let stage = scratch.join("component-stage");
    std::fs::create_dir(&stage)?;
    let installed = !target.join(PORTABLE).is_file();
    let mut unchanged = BTreeMap::new();
    let mut inventory = BTreeMap::new();
    let mut selected = Vec::new();
    let mut total = 0u64;
    reporter.report("verifying", 0, None, "正在比对本地文件", true)?;
    for pack in &index.packages {
        let mut changed = Vec::new();
        for (name, hash) in &pack.files {
            if installed && name == PORTABLE { continue; }
            inventory.insert(name.clone(), hash.clone());
            if matches_file(target, name, hash)? { unchanged.insert(name.clone(), hash.clone()); }
            else { changed.push(name); }
        }
        if !changed.is_empty() { selected.push((pack, changed)); total += pack.bytes; }
        reporter.report("verifying", 0, None, "正在比对本地文件", false)?;
    }
    let mut received = 0;
    let mut extracted_size = 0u64;
    for (i, (pack, changed)) in selected.into_iter().enumerate() {
        let archive = scratch.join(format!("component-{i}.zip"));
        let count = fetch(pack, &archive, &mut |n| {
            reporter.report("downloading", received+n, Some(total), "正在下载有变化的组件", false).map(|_| ())
        })?;
        if count != pack.bytes { return Err(fail("组件下载长度不匹配")); }
        received += count;
        reporter.report("verifying", received, Some(total), "正在校验组件包", true)?;
        updater::verify(&archive, &pack.sha256)?;
        let mut zip = lyric_archive::ZipReader::open(&archive)?;
        let mut entries = BTreeMap::new();
        for (idx, entry) in zip.entries().iter().enumerate() {
            let name = entry.name.strip_prefix("Module/").filter(|n| pack.files.contains_key(*n))
                .ok_or_else(|| fail("组件含未声明文件"))?;
            if entry.is_dir || entries.insert(name.to_owned(), idx).is_some() { return Err(fail("组件文件重复或无效")); }
            extracted_size = extracted_size.saturating_add(entry.size);
            if extracted_size > MAX_EXTRACT { return Err(fail("组件超过解包大小限制")); }
        }
        if entries.len() != pack.files.len() { return Err(fail("组件内容与清单不一致")); }
        for name in changed {
            let bytes = zip.read(entries[name])?;
            if lyric_archive::sha256_bytes(&bytes) != pack.files[name] { return Err(fail("组件文件校验失败")); }
            let destination = payload::update_path(&stage, name)?;
            std::fs::create_dir_all(destination.parent().unwrap())?;
            std::fs::write(destination, bytes)?;
        }
        drop(zip);
        paths::remove_file_if_present(&archive)?;
    }
    paths::write_json(&stage.join(payload::RUNTIME_INVENTORY), &json!({"schemaVersion":1,"product":"DotIsle","version":version,"files":inventory}))?;
    let prepared = Prepared { stage, unchanged };
    prepared.check_unchanged(target)?;
    Ok(prepared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    const VERSION: &str = "5.9.0";
    const FIRST: &str = "Assets/Fonts/a.ttf";
    const SECOND: &str = "Assets/Fonts/b.ttf";

    struct Fixture { root: PathBuf, target: PathBuf, index: Index }
    impl Fixture {
        fn new() -> Self {
            let root = paths::temp_unique("DotIsle.ComponentTest");
            let target = root.join("target");
            fs::create_dir_all(target.join("Assets/Fonts")).unwrap();
            fs::write(target.join("LyricIsland.exe"), b"old runtime").unwrap();
            fs::write(target.join(FIRST), b"font A").unwrap();
            fs::write(target.join(SECOND), b"font B").unwrap();
            let index = Index { schema_version: 2, version: VERSION.into(), runtime_version: VERSION.into(), packages: [
                ("app", "LyricIsland.exe", b"new runtime".as_slice()),
                ("font-a", FIRST, b"font A"), ("font-b", SECOND, b"font B"),
                ("mode", PORTABLE, b"new portable marker"),
            ].into_iter().map(|(id, name, contents)| {
                let archive = format!("{id}.zip");
                let path = root.join(&archive);
                let mut zip = lyric_archive::ZipWriter::create(&path).unwrap();
                zip.add_bytes(&format!("Module/{name}"), contents).unwrap();
                zip.finish().unwrap();
                Pack { archive, sha256: lyric_archive::sha256_file(&path).unwrap(), bytes: fs::metadata(path).unwrap().len(),
                    files: BTreeMap::from([(name.into(), lyric_archive::sha256_bytes(contents))]) }
            }).collect() };
            Self { root, target, index }
        }

        fn stage(&self) -> (Prepared, Vec<(String, u64)>) {
            let mut calls = vec![];
            let (mut reporter, scratch) = self.prepare_fixture();
            let prepared = stage_packages(&self.target, VERSION, &scratch, &self.index, &mut reporter, |pack, path, progress| {
                let bytes = fs::copy(self.root.join(&pack.archive), path)?;
                calls.push((pack.archive.clone(), bytes));
                progress(bytes)?;
                Ok(bytes)
            }).unwrap();
            let progress = reporter.state();
            assert_eq!(progress.lock().unwrap().received, calls.iter().map(|call| call.1).sum::<u64>());
            (prepared, calls)
        }

        fn prepare_fixture(&self) -> (Reporter, PathBuf) {
            let scratch = self.root.join("scratch");
            fs::create_dir(&scratch).unwrap();
            (Reporter::new(false, String::new(), String::new(), String::new(), updater::shared_state()), scratch)
        }
    }
    impl Drop for Fixture { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.root); } }

    #[test]
    fn cross_version_without_intermediate_metadata_skips_unchanged_fonts_and_keeps_installed_mode() {
        let f = Fixture::new();
        let (prepared, calls) = f.stage();
        assert_eq!(calls, vec![("app.zip".into(), f.index.packages[0].bytes)]);
        let inventory = paths::read_json(&prepared.stage.join(payload::RUNTIME_INVENTORY)).unwrap();
        assert_eq!(inventory["version"], VERSION);
        assert_eq!(inventory["files"][FIRST], f.index.packages[1].files[FIRST]);
        assert!(inventory["files"].get(PORTABLE).is_none());
        assert!(!prepared.stage.join(FIRST).exists());
        let mut reject = || Err(fail("simulated failure to start"));
        assert!(updater::apply(&prepared.stage, &f.target, &f.root.join("rollback"), 0, Some(&mut reject)).is_err());
        assert_eq!(fs::read(f.target.join("LyricIsland.exe")).unwrap(), b"old runtime");
        updater::apply(&prepared.stage, &f.target, &f.root.join("backup"), 0, None).unwrap();
        assert_eq!(fs::read(f.target.join("LyricIsland.exe")).unwrap(), b"new runtime");
        fs::write(f.target.join(FIRST), b"edited after staging").unwrap();
        assert!(prepared.check_unchanged(&f.target).is_err());
    }

    #[test]
    fn damaged_font_downloads_only_its_pack_and_portable_mode_stays_portable() {
        let f = Fixture::new();
        fs::write(f.target.join(SECOND), b"damaged").unwrap();
        fs::write(f.target.join(PORTABLE), b"old marker").unwrap();
        let (prepared, calls) = f.stage();
        assert_eq!(calls.iter().map(|c| c.0.as_str()).collect::<Vec<_>>(), ["app.zip", "font-b.zip", "mode.zip"]);
        assert_eq!(fs::read(prepared.stage.join(SECOND)).unwrap(), b"font B");
        assert!(prepared.stage.join(PORTABLE).is_file());
    }

    #[test]
    fn cancellation_and_corrupt_components_never_mutate_the_running_installation() {
        for cancel in [true, false] {
            let f = Fixture::new();
            let (mut reporter, scratch) = f.prepare_fixture();
            let result = stage_packages(&f.target, VERSION, &scratch, &f.index, &mut reporter, |pack, path, _| {
                if cancel { return Err(Error::cancelled(updater::CANCELLED_MESSAGE)); }
                fs::write(path, vec![0; pack.bytes as usize])?;
                Ok(pack.bytes)
            }).err().unwrap();
            assert_eq!(result.is_cancelled(), cancel);
            assert_eq!(fs::read(f.target.join("LyricIsland.exe")).unwrap(), b"old runtime");
            assert!(!scratch.join("stage").exists(), "full fallback stage must remain free");
        }
    }

    #[test]
    fn rejects_wrong_schema_version_and_mutable_or_ambiguous_paths() {
        let mut f = Fixture::new();
        assert!(validate_index(&f.index, "6.0.0").is_err());
        f.index.schema_version = 1;
        assert!(validate_index(&f.index, VERSION).is_err());
        f.index.schema_version = 2;
        for name in ["../x.exe", "Data/settings.json", "Assets/../x.exe", "Assets/Fonts/A.ttf", payload::RUNTIME_INVENTORY] {
            f.index.packages[0].files.insert(name.into(), lyric_archive::sha256_bytes(b"file"));
            assert!(validate_index(&f.index, VERSION).is_err(), "{name}");
            f.index.packages[0].files.remove(name);
        }
    }
}
