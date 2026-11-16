// Read-only WCDB capability discovery; never injects code or writes a process.
// Config.Cipher layout reference: wechatauto-replica (Apache-2.0), see notices.
use std::{
    collections::HashSet,
    ffi::c_void,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
type Handle = *mut c_void;
#[repr(C)]
struct ProcessEntry {
    size: u32,
    usage: u32,
    pid: u32,
    heap: usize,
    module: u32,
    threads: u32,
    parent: u32,
    priority: i32,
    flags: u32,
    name: [u16; 260],
}
#[repr(C)]
#[derive(Default)]
struct Region {
    base: usize,
    allocation: usize,
    protect: u32,
    partition: u16,
    size: usize,
    state: u32,
    protection: u32,
    kind: u32,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> Handle;
    fn Process32FirstW(h: Handle, p: *mut ProcessEntry) -> i32;
    fn Process32NextW(h: Handle, p: *mut ProcessEntry) -> i32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
    fn CloseHandle(h: Handle) -> i32;
    fn GetProcessTimes(h: Handle, c: *mut u64, e: *mut u64, k: *mut u64, u: *mut u64) -> i32;
    fn ReadProcessMemory(h: Handle, addr: usize, out: *mut u8, n: usize, read: *mut usize) -> i32;
    fn VirtualQueryEx(h: Handle, addr: usize, out: *mut Region, n: usize) -> usize;
    fn MultiByteToWideChar(
        cp: u32,
        flags: u32,
        src: *const u8,
        n: i32,
        out: *mut u16,
        max: i32,
    ) -> i32;
    fn QueryFullProcessImageNameW(h: Handle, flags: u32, path: *mut u16, n: *mut u32) -> i32;
}
pub fn executable(pid: u32) -> Option<String> {
    unsafe {
        let h = OpenProcess(0x1000, 0, pid);
        if h.is_null() {
            return None;
        }
        let h = Owned(h);
        let mut path = [0; 32768];
        let mut n = path.len() as u32;
        if QueryFullProcessImageNameW(h.0, 0, path.as_mut_ptr(), &mut n) == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&path[..n as usize]))
    }
}
struct Owned(Handle);
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
pub fn processes() -> Vec<(u32, u64)> {
    unsafe {
        let h = CreateToolhelp32Snapshot(2, 0);
        if h as isize == -1 {
            return vec![];
        }
        let h = Owned(h);
        let mut e: ProcessEntry = std::mem::zeroed();
        e.size = std::mem::size_of::<ProcessEntry>() as u32;
        let mut ok = Process32FirstW(h.0, &mut e);
        let mut out = vec![];
        while ok != 0 {
            let name = String::from_utf16_lossy(
                &e.name[..e.name.iter().position(|c| *c == 0).unwrap_or(260)],
            )
            .to_ascii_lowercase();
            if ["weixin.exe", "wechat.exe"].contains(&name.as_str()) {
                let p = OpenProcess(0x1000, 0, e.pid);
                if !p.is_null() {
                    let p = Owned(p);
                    let (mut c, mut x, mut k, mut u) = (0, 0, 0, 0);
                    if GetProcessTimes(p.0, &mut c, &mut x, &mut k, &mut u) != 0 {
                        out.push((e.pid, c));
                    }
                }
            }
            ok = Process32NextW(h.0, &mut e);
        }
        out.sort_unstable();
        out
    }
}
fn read(h: Handle, addr: usize, n: usize) -> Vec<u8> {
    let mut bytes = vec![0; n];
    let mut got = 0;
    unsafe {
        ReadProcessMemory(h, addr, bytes.as_mut_ptr(), n, &mut got);
    }
    bytes.truncate(got);
    bytes
}
fn scan(h: Handle, needles: &[Vec<u8>], stop: &AtomicBool, mut hit: impl FnMut(usize, usize)) {
    let finders = needles
        .iter()
        .map(|n| memchr::memmem::Finder::new(n))
        .collect::<Vec<_>>();
    let overlap = needles.iter().map(Vec::len).max().unwrap_or(1) - 1;
    let mut addr = 0usize;
    loop {
        if stop.load(Ordering::Acquire) {
            break;
        }
        let mut r = Region::default();
        if unsafe { VirtualQueryEx(h, addr, &mut r, std::mem::size_of::<Region>()) } == 0 {
            break;
        }
        if r.state == 0x1000 && r.protection & 0x100 == 0 && r.protection & 0xe6 != 0 && r.size > 0
        {
            let end = r.base.saturating_add(r.size);
            let mut pos = r.base;
            while pos < end {
                if stop.load(Ordering::Acquire) {
                    return;
                }
                let n = (end - pos).min(4 * 1024 * 1024);
                let bytes = read(h, pos, n);
                for (i, f) in finders.iter().enumerate() {
                    for offset in f.find_iter(&bytes) {
                        hit(i, pos + offset)
                    }
                }
                if n <= overlap {
                    break;
                }
                pos += n - overlap;
            }
        }
        let next = r.base.saturating_add(r.size);
        if next <= addr {
            break;
        }
        addr = next;
    }
}
fn u64at(bytes: &[u8], at: usize) -> usize {
    bytes
        .get(at..at + 8)
        .and_then(|b| b.try_into().ok())
        .map(u64::from_le_bytes)
        .unwrap_or(0) as usize
}
fn hex(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() % 2 != 0 {
        return None;
    }
    bytes
        .chunks_exact(2)
        .map(|b| {
            let a = (b[0] as char).to_digit(16)?;
            let c = (b[1] as char).to_digit(16)?;
            Some((a * 16 + c) as u8)
        })
        .collect()
}
pub struct Candidate(pub [u8; 32], pub Option<[u8; 16]>);
impl Drop for Candidate {
    fn drop(&mut self) {
        for b in &mut self.0 {
            unsafe { std::ptr::write_volatile(b, 0) }
        }
        if let Some(s) = &mut self.1 {
            for b in s {
                unsafe { std::ptr::write_volatile(b, 0) }
            }
        }
    }
}
pub fn candidates(pids: &[(u32, u64)], stop: &AtomicBool) -> Vec<Candidate> {
    const NAME: &[u8] = b"com.Tencent.WCDB.Config.Cipher";
    const MASK: [u8; 32] = [
        0xd2, 0xc7, 0x44, 0x24, 0x58, 0x02, 0, 0, 0, 0x48, 0x89, 0x44, 0x24, 0x50, 0x48, 0x8b,
        0x45, 0, 0x48, 0x84, 0x4c, 0x24, 0x48, 0x48, 0x89, 0x44, 0x25, 0x40, 0x48, 0x58, 0x4c,
        0x24,
    ];
    let mut out = vec![];
    let mut seen = HashSet::new();
    for &(pid, _) in pids {
        if stop.load(Ordering::Acquire) {
            break;
        }
        let h = unsafe { OpenProcess(0x410, 0, pid) };
        if h.is_null() {
            continue;
        }
        let h = Owned(h);
        let mut names = vec![];
        scan(h.0, &[NAME.to_vec()], stop, |_, addr| {
            if names.len() < 4096 {
                names.push(addr)
            }
        });
        let pairs = names
            .iter()
            .map(|a| {
                let mut p = a.to_le_bytes().to_vec();
                p.extend_from_slice(&(NAME.len() as u64).to_le_bytes());
                p
            })
            .collect::<Vec<_>>();
        if pairs.is_empty() {
            continue;
        }
        scan(h.0, &pairs, stop, |_, addr| {
            if addr < 0x10 {
                return;
            }
            let node = read(h.0, addr - 0x10, 0x50);
            let config = u64at(&node, 0x28);
            if !(0x10000..0x800000000000).contains(&config) {
                return;
            }
            let obj = read(h.0, config + 0x88, 0x28);
            let ptr = u64at(&obj, 8);
            let len = u64at(&obj, 16);
            if !(1..=1024).contains(&len) || !(0x10000..0x800000000000).contains(&ptr) {
                return;
            }
            let mut blob = read(h.0, ptr, len);
            for (i, b) in blob.iter_mut().enumerate() {
                *b ^= MASK[i % 32]
            }
            let mut pos = 0;
            while pos + 3 < blob.len() {
                if ![b'x', b'X'].contains(&blob[pos]) || blob[pos + 1] != b'\'' {
                    pos += 1;
                    continue;
                }
                let start = pos + 2;
                let mut end = start;
                while end < blob.len() && blob[end].is_ascii_hexdigit() {
                    end += 1
                }
                if end >= blob.len() || blob[end] != b'\'' || !(64..=192).contains(&(end - start)) {
                    pos = end.max(pos + 1);
                    continue;
                }
                let run = &blob[start..end];
                let mut offsets = vec![0];
                if run.len() > 96 {
                    offsets.extend((0..=run.len() - 64).step_by(32));
                    offsets.push(run.len() - 64)
                }
                for offset in offsets {
                    if let Some(key) =
                        hex(&run[offset..offset + 64]).and_then(|v| <[u8; 32]>::try_from(v).ok())
                    {
                        let salt = run
                            .get(offset + 64..offset + 96)
                            .and_then(hex)
                            .and_then(|v| v.try_into().ok());
                        if seen.insert((key, salt)) && out.len() < 4096 {
                            out.push(Candidate(key, salt));
                        }
                    }
                }
                pos = end + 1;
            }
            blob.fill(0);
        });
    }
    out
}
fn decode(bytes: &[u8]) -> String {
    if bytes.starts_with(&[255, 254]) {
        return String::from_utf16_lossy(
            &bytes[2..]
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect::<Vec<_>>(),
        );
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.into();
    }
    let mut out = vec![0; bytes.len()];
    let n = unsafe {
        MultiByteToWideChar(
            936,
            0,
            bytes.as_ptr(),
            bytes.len() as i32,
            out.as_mut_ptr(),
            out.len() as i32,
        )
    };
    String::from_utf16_lossy(&out[..n.max(0) as usize])
}
fn config_paths(text: &str) -> Vec<PathBuf> {
    let mut out = vec![];
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(m) = v.as_object() {
            for k in [
                "dataDir",
                "data_dir",
                "fileSavePath",
                "savePath",
                "path",
                "defaultFileSavePath",
            ] {
                if let Some(s) = m.get(k).and_then(|v| v.as_str()) {
                    out.push(s.into())
                }
            }
        }
    }
    let text = text.trim_matches(['\u{feff}', '\0', '\r', '\n', ' ']);
    if text.as_bytes().get(1) == Some(&b':') {
        out.push(text.into())
    }
    out
}
#[repr(C)]
struct Guid {
    a: u32,
    b: u16,
    c: u16,
    d: [u8; 8],
}
#[link(name = "shell32")]
unsafe extern "system" {
    fn SHGetKnownFolderPath(id: *const Guid, flags: u32, token: Handle, path: *mut *mut u16)
        -> i32;
}
#[link(name = "ole32")]
unsafe extern "system" {
    fn CoTaskMemFree(ptr: Handle);
}
#[link(name = "advapi32")]
unsafe extern "system" {
    fn RegOpenKeyExW(
        key: Handle,
        name: *const u16,
        opts: u32,
        access: u32,
        out: *mut Handle,
    ) -> i32;
    fn RegEnumValueW(
        key: Handle,
        index: u32,
        name: *mut u16,
        n: *mut u32,
        res: Handle,
        kind: *mut u32,
        data: *mut u8,
        len: *mut u32,
    ) -> i32;
    fn RegCloseKey(key: Handle) -> i32;
}
pub fn accounts() -> Vec<PathBuf> {
    let mut roots = vec![];
    for env in ["APPDATA", "LOCALAPPDATA"] {
        if let Some(base) = std::env::var_os(env) {
            for part in [
                "Tencent/xwechat",
                "Tencent/xwechat/config",
                "Tencent/WeChat",
            ] {
                if let Ok(entries) = std::fs::read_dir(PathBuf::from(&base).join(part)) {
                    for e in entries.flatten().take(128) {
                        if e.metadata().is_ok_and(|m| m.is_file() && m.len() <= 8192) {
                            if let Ok(b) = std::fs::read(e.path()) {
                                roots.extend(config_paths(&decode(&b)))
                            }
                        }
                    }
                }
            }
        }
    }
    unsafe {
        for sub in [
            "Software\\Tencent\\xwechat",
            "Software\\Tencent\\xwechat\\config",
            "Software\\Tencent\\WeChat",
        ] {
            let name = sub.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
            let mut key = std::ptr::null_mut();
            if RegOpenKeyExW(
                0x80000001u32 as i32 as isize as Handle,
                name.as_ptr(),
                0,
                0x20019,
                &mut key,
            ) == 0
            {
                for i in 0..128 {
                    let mut name = [0; 256];
                    let mut n = 256;
                    let mut data = [0u8; 8192];
                    let mut len = 8192;
                    let mut kind = 0;
                    if RegEnumValueW(
                        key,
                        i,
                        name.as_mut_ptr(),
                        &mut n,
                        std::ptr::null_mut(),
                        &mut kind,
                        data.as_mut_ptr(),
                        &mut len,
                    ) != 0
                    {
                        break;
                    }
                    if [1, 2].contains(&kind) {
                        let value = String::from_utf16_lossy(
                            &data[..len as usize]
                                .chunks_exact(2)
                                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                                .take_while(|c| *c != 0)
                                .collect::<Vec<_>>(),
                        );
                        roots.push(value.into());
                    }
                }
                RegCloseKey(key);
            }
        }
        let guid = Guid {
            a: 0xfdd39ad0,
            b: 0x238f,
            c: 0x46af,
            d: [0xad, 0xb4, 0x6c, 0x85, 0x48, 0x03, 0x69, 0xc7],
        };
        let mut p = std::ptr::null_mut();
        if SHGetKnownFolderPath(&guid, 0, std::ptr::null_mut(), &mut p) >= 0 && !p.is_null() {
            let mut n = 0;
            while n < 32768 && *p.add(n) != 0 {
                n += 1
            }
            roots.push(String::from_utf16_lossy(std::slice::from_raw_parts(p, n)).into());
            CoTaskMemFree(p.cast());
        }
    }
    if let Some(p) = std::env::var_os("USERPROFILE") {
        roots.push(PathBuf::from(&p).join("Documents"));
        roots.push(p.into());
    }
    let mut out = vec![];
    for root in roots {
        for folder in ["", "xwechat_files", "WeChat Files", "xwechat_files_data"] {
            let root = root.join(folder);
            if root.join("db_storage").is_dir() {
                out.push(root.clone())
            }
            if let Ok(entries) = std::fs::read_dir(root) {
                for e in entries.flatten().take(256) {
                    if e.path().join("db_storage").is_dir() {
                        out.push(e.path())
                    }
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}
pub fn message_files(account: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    if let Ok(entries) = std::fs::read_dir(account.join("db_storage/message")) {
        for e in entries.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.strip_prefix("message_")
                .and_then(|s| s.strip_suffix(".db"))
                .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            {
                out.push(e.path())
            }
        }
    }
    out.sort();
    out
}
