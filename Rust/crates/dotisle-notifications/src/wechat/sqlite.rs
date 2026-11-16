// Demand-decrypting, read-only SQLite VFS. No plaintext database or key files.
use super::crypto::{Key, PAGE};
use std::{
    cell::UnsafeCell,
    collections::HashMap,
    ffi::{c_char, c_void, CString},
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::SystemTime,
};
type Ptr = *mut c_void;
pub type Stamp = (Option<(SystemTime, u64)>, Option<(SystemTime, u64)>);
fn wal_path(p: &Path) -> PathBuf {
    let mut name = p.as_os_str().to_os_string();
    name.push("-wal");
    name.into()
}
pub fn stamp(p: &Path) -> Stamp {
    let meta = |p: &Path| {
        std::fs::metadata(p)
            .ok()
            .and_then(|m| m.modified().ok().map(|t| (t, m.len())))
    };
    (meta(p), meta(&wal_path(p)))
}
fn be(b: &[u8]) -> u32 {
    u32::from_be_bytes(b[..4].try_into().unwrap())
}
fn checksum(mut sum: (u32, u32), bytes: &[u8], big: bool) -> (u32, u32) {
    for pair in bytes.chunks_exact(8) {
        let word = |b: &[u8]| {
            if big {
                u32::from_be_bytes(b.try_into().unwrap())
            } else {
                u32::from_le_bytes(b.try_into().unwrap())
            }
        };
        sum.0 = sum.0.wrapping_add(word(&pair[..4])).wrapping_add(sum.1);
        sum.1 = sum.1.wrapping_add(word(&pair[4..])).wrapping_add(sum.0);
    }
    sum
}
struct Source {
    base: File,
    wal: Option<File>,
    frames: HashMap<u32, u64>,
    pages: u32,
    key: Key,
    reads: u64,
}
impl Source {
    fn new(path: &Path, key: Key) -> Result<Self, String> {
        let base = File::open(path).map_err(|_| "微信消息库暂不可读")?;
        let len = base.metadata().map_err(|_| "微信消息库暂不可读")?.len();
        if len < PAGE as u64 || len % PAGE as u64 != 0 || len / PAGE as u64 > u32::MAX as u64 {
            return Err("微信消息库页格式不兼容".into());
        }
        let mut s = Self {
            base,
            wal: File::open(wal_path(path)).ok(),
            frames: HashMap::new(),
            pages: (len / PAGE as u64) as u32,
            key,
            reads: 0,
        };
        if let Some(wal) = &mut s.wal {
            let mut head = [0; 32];
            if wal.read_exact(&mut head).is_err() {
                return Ok(s);
            }
            let magic = be(&head);
            let big = magic & 1 != 0;
            if ![0x377f0682, 0x377f0683].contains(&magic)
                || be(&head[8..]) != PAGE as u32
                || checksum((0, 0), &head[..24], big) != (be(&head[24..]), be(&head[28..]))
            {
                return Err("微信日志正在更新，稍后自动重试".into());
            }
            let mut sum = (be(&head[24..]), be(&head[28..]));
            let mut pending = HashMap::new();
            let mut off = 32u64;
            let mut frame = [0; 24];
            let mut page = [0; PAGE];
            let mut count = 0usize;
            while wal.read_exact(&mut frame).is_ok() && wal.read_exact(&mut page).is_ok() {
                if frame[8..16] != head[16..24] {
                    break;
                }
                let pg = be(&frame);
                if pg == 0 {
                    break;
                }
                sum = checksum(checksum(sum, &frame[..8], big), &page, big);
                if sum != (be(&frame[16..]), be(&frame[20..])) {
                    break;
                }
                pending.insert(pg, off + 24);
                let commit = be(&frame[4..]);
                if commit > 0 {
                    s.frames.extend(pending.drain());
                    s.pages = commit;
                    s.frames.retain(|p, _| *p <= commit);
                }
                off += 24 + PAGE as u64;
                count += 1;
                if count > 65_536 {
                    return Err("微信日志较大，等待客户端整理后重试".into());
                }
            }
        }
        Ok(s)
    }
    fn page(&mut self, pg: u32) -> Result<[u8; PAGE], String> {
        let mut data = [0; PAGE];
        let (file, off) = if let Some(off) = self.frames.get(&pg) {
            (self.wal.as_mut().ok_or("微信日志暂不可读")?, *off)
        } else {
            (&mut self.base, (pg as u64 - 1) * PAGE as u64)
        };
        file.seek(SeekFrom::Start(off))
            .and_then(|_| file.read_exact(&mut data))
            .map_err(|_| "微信数据正在更新")?;
        self.key.decrypt(&mut data, pg)?;
        self.reads += 1;
        Ok(data)
    }
}
#[repr(C)]
struct Io {
    version: i32,
    close: unsafe extern "C" fn(*mut IoFile) -> i32,
    read: unsafe extern "C" fn(*mut IoFile, Ptr, i32, i64) -> i32,
    write: unsafe extern "C" fn(*mut IoFile, Ptr, i32, i64) -> i32,
    truncate: unsafe extern "C" fn(*mut IoFile, i64) -> i32,
    sync: unsafe extern "C" fn(*mut IoFile, i32) -> i32,
    size: unsafe extern "C" fn(*mut IoFile, *mut i64) -> i32,
    lock: unsafe extern "C" fn(*mut IoFile, i32) -> i32,
    unlock: unsafe extern "C" fn(*mut IoFile, i32) -> i32,
    reserved: unsafe extern "C" fn(*mut IoFile, *mut i32) -> i32,
    control: unsafe extern "C" fn(*mut IoFile, i32, Ptr) -> i32,
    sector: unsafe extern "C" fn(*mut IoFile) -> i32,
    device: unsafe extern "C" fn(*mut IoFile) -> i32,
}
#[repr(C)]
struct IoFile {
    methods: *const Io,
    source: *mut Source,
}
#[repr(C)]
struct Vfs {
    version: i32,
    sz: i32,
    maxpath: i32,
    next: *mut Vfs,
    name: *const c_char,
    app: Ptr,
    open: unsafe extern "C" fn(*mut Vfs, *const c_char, *mut IoFile, i32, *mut i32) -> i32,
    delete: Option<unsafe extern "C" fn()>,
    access: unsafe extern "C" fn(*mut Vfs, *const c_char, i32, *mut i32) -> i32,
    full: unsafe extern "C" fn(*mut Vfs, *const c_char, i32, *mut c_char) -> i32,
    dlopen: Option<unsafe extern "C" fn()>,
    dlerror: Option<unsafe extern "C" fn()>,
    dlsym: Option<unsafe extern "C" fn()>,
    dlclose: Option<unsafe extern "C" fn()>,
    random: unsafe extern "C" fn(*mut Vfs, i32, Ptr) -> i32,
    sleep: unsafe extern "C" fn(*mut Vfs, i32) -> i32,
    time: unsafe extern "C" fn(*mut Vfs, *mut f64) -> i32,
    error: Option<unsafe extern "C" fn()>,
}
unsafe extern "C" fn close(_: *mut IoFile) -> i32 {
    0
}
unsafe extern "C" fn read(f: *mut IoFile, out: Ptr, n: i32, off: i64) -> i32 {
    if n < 0 || off < 0 {
        return 10;
    }
    let s = &mut *(*f).source;
    let out = std::slice::from_raw_parts_mut(out.cast::<u8>(), n as usize);
    out.fill(0);
    let mut pos = off as u64;
    let mut written = 0;
    while written < out.len() {
        let pg = pos / PAGE as u64 + 1;
        if pg > s.pages as u64 {
            return 522;
        }
        let data = match s.page(pg as u32) {
            Ok(p) => p,
            Err(_) => return 10,
        };
        let from = pos as usize % PAGE;
        let len = (PAGE - from).min(out.len() - written);
        out[written..written + len].copy_from_slice(&data[from..from + len]);
        written += len;
        pos += len as u64;
    }
    0
}
unsafe extern "C" fn write(_: *mut IoFile, _: Ptr, _: i32, _: i64) -> i32 {
    8
}
unsafe extern "C" fn truncate(_: *mut IoFile, _: i64) -> i32 {
    8
}
unsafe extern "C" fn noop(_: *mut IoFile, _: i32) -> i32 {
    0
}
unsafe extern "C" fn size(f: *mut IoFile, n: *mut i64) -> i32 {
    *n = (*(*f).source).pages as i64 * PAGE as i64;
    0
}
unsafe extern "C" fn reserved(_: *mut IoFile, n: *mut i32) -> i32 {
    *n = 0;
    0
}
unsafe extern "C" fn control(_: *mut IoFile, _: i32, _: Ptr) -> i32 {
    12
}
unsafe extern "C" fn sector(_: *mut IoFile) -> i32 {
    4096
}
unsafe extern "C" fn device(_: *mut IoFile) -> i32 {
    0
}
static IO: Io = Io {
    version: 1,
    close,
    read,
    write,
    truncate,
    sync: noop,
    size,
    lock: noop,
    unlock: noop,
    reserved,
    control,
    sector,
    device,
};
unsafe extern "C" fn open(
    v: *mut Vfs,
    _: *const c_char,
    f: *mut IoFile,
    flags: i32,
    out: *mut i32,
) -> i32 {
    if flags & 0x100 == 0 || flags & 6 != 0 {
        return 14;
    }
    (*f).methods = &IO;
    (*f).source = (*v).app.cast();
    if !out.is_null() {
        *out = 1
    }
    0
}
unsafe extern "C" fn access(_: *mut Vfs, _: *const c_char, _: i32, out: *mut i32) -> i32 {
    *out = 0;
    0
}
unsafe extern "C" fn full(_: *mut Vfs, name: *const c_char, n: i32, out: *mut c_char) -> i32 {
    let bytes = std::ffi::CStr::from_ptr(name).to_bytes_with_nul();
    if bytes.len() > n as usize {
        return 14;
    }
    std::ptr::copy_nonoverlapping(bytes.as_ptr().cast(), out, bytes.len());
    0
}
unsafe extern "C" fn random(_: *mut Vfs, n: i32, out: Ptr) -> i32 {
    std::ptr::write_bytes(out, 0, n.max(0) as usize);
    n
}
unsafe extern "C" fn sleep(_: *mut Vfs, _: i32) -> i32 {
    0
}
unsafe extern "C" fn time(_: *mut Vfs, out: *mut f64) -> i32 {
    *out = 2440587.5;
    0
}
pub struct Row {
    pub bytes: Vec<Vec<u8>>,
    pub ints: Vec<i64>,
}
impl Row {
    pub fn text(&self, i: usize) -> String {
        String::from_utf8(self.bytes[i].clone()).unwrap_or_default()
    }
}
pub struct Snapshot {
    db: Ptr,
    lib: libloading::Library,
    vfs: Box<Vfs>,
    _name: CString,
    source: Box<UnsafeCell<Source>>,
    path: PathBuf,
    stamp: Stamp,
}
impl Drop for Snapshot {
    fn drop(&mut self) {
        unsafe {
            if let Ok(f) = self
                .lib
                .get::<unsafe extern "C" fn(Ptr) -> i32>(b"sqlite3_close\0")
            {
                f(self.db);
            }
            if let Ok(f) = self
                .lib
                .get::<unsafe extern "C" fn(*mut Vfs) -> i32>(b"sqlite3_vfs_unregister\0")
            {
                f(&mut *self.vfs);
            }
        }
    }
}
impl Snapshot {
    pub fn new(path: &Path, key: Key) -> Result<Self, String> {
        unsafe {
            let before = stamp(path);
            let source = Box::new(UnsafeCell::new(Source::new(path, key)?));
            let _ = (*source.get()).page(1)?;
            let dll = std::env::current_exe()
                .map_err(|_| "无法定位微信读取组件")?
                .parent()
                .ok_or("无法定位微信读取组件")?
                .join("e_sqlite3.dll");
            #[cfg(test)]
            let dll = std::env::var_os("DOTISLE_TEST_SQLITE3")
                .map(PathBuf::from)
                .unwrap_or(dll);
            let lib =
                libloading::Library::new(dll).map_err(|_| "微信数据库组件缺失，请完整解压程序")?;
            static NEXT: AtomicU64 = AtomicU64::new(1);
            let name = CString::new(format!(
                "dotisle-wechat-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ))
            .unwrap();
            let mut vfs = Box::new(Vfs {
                version: 1,
                sz: std::mem::size_of::<IoFile>() as i32,
                maxpath: 512,
                next: std::ptr::null_mut(),
                name: name.as_ptr(),
                app: source.get().cast(),
                open,
                delete: None,
                access,
                full,
                dlopen: None,
                dlerror: None,
                dlsym: None,
                dlclose: None,
                random,
                sleep,
                time,
                error: None,
            });
            let register = lib
                .get::<unsafe extern "C" fn(*mut Vfs, i32) -> i32>(b"sqlite3_vfs_register\0")
                .map_err(|_| "微信数据库组件不兼容")?;
            let open = lib
                .get::<unsafe extern "C" fn(*const c_char, *mut Ptr, i32, *const c_char) -> i32>(
                    b"sqlite3_open_v2\0",
                )
                .map_err(|_| "微信数据库组件不兼容")?;
            if register(&mut *vfs, 0) != 0 {
                return Err("无法初始化微信读取组件".into());
            }
            let mut db = std::ptr::null_mut();
            let rc = open(
                b"wechat\0".as_ptr().cast(),
                &mut db,
                1 | 0x8000,
                name.as_ptr(),
            );
            let s = Self {
                db,
                lib,
                vfs,
                _name: name,
                source,
                path: path.into(),
                stamp: before,
            };
            if rc != 0 {
                return Err("微信消息库暂不可读".into());
            }
            Ok(s)
        }
    }
    pub fn stable(&self) -> bool {
        stamp(&self.path) == self.stamp
    }
    pub fn reads(&self) -> u64 {
        unsafe { (*self.source.get()).reads }
    }
    pub fn query(&self, sql: &str) -> Result<Vec<Row>, String> {
        unsafe {
            type Prepare =
                unsafe extern "C" fn(Ptr, *const c_char, i32, *mut Ptr, *mut *const c_char) -> i32;
            let prepare = self
                .lib
                .get::<Prepare>(b"sqlite3_prepare_v2\0")
                .map_err(|_| "微信查询组件不兼容")?;
            let step = self
                .lib
                .get::<unsafe extern "C" fn(Ptr) -> i32>(b"sqlite3_step\0")
                .map_err(|_| "微信查询组件不兼容")?;
            let finalize = self
                .lib
                .get::<unsafe extern "C" fn(Ptr) -> i32>(b"sqlite3_finalize\0")
                .map_err(|_| "微信查询组件不兼容")?;
            let count = self
                .lib
                .get::<unsafe extern "C" fn(Ptr) -> i32>(b"sqlite3_column_count\0")
                .map_err(|_| "微信查询组件不兼容")?;
            let blob = self
                .lib
                .get::<unsafe extern "C" fn(Ptr, i32) -> *const u8>(b"sqlite3_column_blob\0")
                .map_err(|_| "微信查询组件不兼容")?;
            let bytes = self
                .lib
                .get::<unsafe extern "C" fn(Ptr, i32) -> i32>(b"sqlite3_column_bytes\0")
                .map_err(|_| "微信查询组件不兼容")?;
            let int = self
                .lib
                .get::<unsafe extern "C" fn(Ptr, i32) -> i64>(b"sqlite3_column_int64\0")
                .map_err(|_| "微信查询组件不兼容")?;
            let sql = CString::new(sql).map_err(|_| "微信查询格式无效")?;
            let mut statement = std::ptr::null_mut();
            let rc = prepare(
                self.db,
                sql.as_ptr(),
                -1,
                &mut statement,
                std::ptr::null_mut(),
            );
            if rc != 0 {
                if !statement.is_null() {
                    finalize(statement);
                }
                return Err("微信数据结构暂不兼容或正在更新".into());
            }
            let result = (|| {
                let mut rows = vec![];
                loop {
                    match step(statement) {
                        100 => {
                            if rows.len() >= 16384 {
                                return Err("微信查询超出单轮限制".into());
                            }
                            let mut row = Row {
                                bytes: vec![],
                                ints: vec![],
                            };
                            for i in 0..count(statement) {
                                let n = bytes(statement, i);
                                if !(0..=1_048_576).contains(&n) {
                                    return Err("微信消息超出单条限制".into());
                                }
                                let ptr = blob(statement, i);
                                row.bytes.push(if n == 0 || ptr.is_null() {
                                    vec![]
                                } else {
                                    std::slice::from_raw_parts(ptr, n as usize).to_vec()
                                });
                                row.ints.push(int(statement, i));
                            }
                            rows.push(row);
                        }
                        101 => return Ok(rows),
                        _ => return Err("微信消息正在更新，稍后自动重试".into()),
                    }
                }
            })();
            finalize(statement);
            result
        }
    }
}
#[cfg(test)]
pub(super) use tests::fixture;
#[cfg(test)]
mod tests {
    use super::super::crypto::tests::seal;
    use super::*;
    fn wal(frames: &[(u32, u32, [u8; PAGE])]) -> Vec<u8> {
        let mut out = vec![];
        for word in [0x377f0682u32, 3007000, PAGE as u32, 0, 123, 456] {
            out.extend_from_slice(&word.to_be_bytes())
        }
        let mut sum = checksum((0, 0), &out, false);
        out.extend_from_slice(&sum.0.to_be_bytes());
        out.extend_from_slice(&sum.1.to_be_bytes());
        for (pg, commit, page) in frames {
            let mut head = vec![];
            head.extend_from_slice(&pg.to_be_bytes());
            head.extend_from_slice(&commit.to_be_bytes());
            head.extend_from_slice(&123u32.to_be_bytes());
            head.extend_from_slice(&456u32.to_be_bytes());
            sum = checksum(checksum(sum, &head[..8], false), page, false);
            head.extend_from_slice(&sum.0.to_be_bytes());
            head.extend_from_slice(&sum.1.to_be_bytes());
            out.extend_from_slice(&head);
            out.extend_from_slice(page)
        }
        out
    }
    #[test]
    fn wal_commit_tail_checksums_and_overflow_pages() {
        if std::env::var_os("DOTISLE_TEST_SQLITE3").is_none() {
            return;
        }
        let root = std::env::temp_dir().join(format!("dotisle-wal-fixture-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let p = root.join("base.db");
        let other = root.join("next.db");
        let _ = std::fs::remove_file(&p);
        let _ = std::fs::remove_file(&other);
        let (enc, salt) = fixture(
            &p,
            "CREATE TABLE sample(body TEXT);INSERT INTO sample VALUES('prior');",
        );
        fixture(
            &other,
            "CREATE TABLE sample(body TEXT);INSERT INTO sample VALUES('fresh');",
        );
        let base = std::fs::read(&p).unwrap();
        let next = std::fs::read(&other).unwrap();
        let old = <&[u8; PAGE]>::try_from(&base[PAGE..2 * PAGE]).unwrap();
        let new = <&[u8; PAGE]>::try_from(&next[PAGE..2 * PAGE]).unwrap();
        let check = |expected: &str| {
            let k = Key::verified(enc, &salt, &base[..PAGE]).unwrap();
            let db = Snapshot::new(&p, k).unwrap();
            assert_eq!(
                db.query("SELECT body FROM sample").unwrap()[0].text(0),
                expected
            );
        };
        std::fs::write(wal_path(&p), wal(&[(2, 2, *new), (2, 0, *old)])).unwrap();
        check("fresh");
        let mut torn = wal(&[(2, 2, *new), (2, 2, *old)]);
        let end = torn.len() - 1;
        torn[end] ^= 1;
        std::fs::write(wal_path(&p), torn).unwrap();
        check("fresh");
        std::fs::write(wal_path(&p), wal(&[(2, 0, *new)])).unwrap();
        check("prior");
        std::fs::remove_file(wal_path(&p)).unwrap();
        std::fs::remove_file(&other).unwrap();
        fixture(
            &other,
            &format!(
                "CREATE TABLE sample(body TEXT);INSERT INTO sample VALUES('{}');",
                "长".repeat(12000)
            ),
        );
        let encrypted = std::fs::read(&other).unwrap();
        let k = Key::verified(enc, &salt, &encrypted[..PAGE]).unwrap();
        let db = Snapshot::new(&other, k).unwrap();
        assert_eq!(
            db.query("SELECT body FROM sample").unwrap()[0]
                .text(0)
                .chars()
                .count(),
            12000
        );
        drop(db);
        std::fs::remove_dir_all(root).unwrap();
    }
    pub fn fixture(path: &Path, sql: &str) -> ([u8; 32], [u8; 16]) {
        unsafe {
            let lib = libloading::Library::new(std::env::var_os("DOTISLE_TEST_SQLITE3").unwrap())
                .unwrap();
            let open = lib
                .get::<unsafe extern "C" fn(*const c_char, *mut Ptr, i32, *const c_char) -> i32>(
                    b"sqlite3_open_v2\0",
                )
                .unwrap();
            let exec = lib
                .get::<unsafe extern "C" fn(Ptr, *const c_char, Ptr, Ptr, Ptr) -> i32>(
                    b"sqlite3_exec\0",
                )
                .unwrap();
            let close = lib
                .get::<unsafe extern "C" fn(Ptr) -> i32>(b"sqlite3_close\0")
                .unwrap();
            let control = lib
                .get::<unsafe extern "C" fn(Ptr, *const c_char, i32, Ptr) -> i32>(
                    b"sqlite3_file_control\0",
                )
                .unwrap();
            let name = CString::new(path.to_str().unwrap()).unwrap();
            let mut db = std::ptr::null_mut();
            assert_eq!(open(name.as_ptr(), &mut db, 6, std::ptr::null()), 0);
            let mut reserve = 80i32;
            assert_eq!(
                control(db, std::ptr::null(), 38, (&mut reserve as *mut i32).cast()),
                0
            );
            let sql = CString::new(format!("PRAGMA page_size=4096;{sql}")).unwrap();
            assert_eq!(
                exec(
                    db,
                    sql.as_ptr(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                0
            );
            close(db);
            let mut data = std::fs::read(path).unwrap();
            let key = [7; 32];
            let salt = [3; 16];
            for (i, page) in data.chunks_exact_mut(PAGE).enumerate() {
                let plain = <&[u8; PAGE]>::try_from(&*page).unwrap();
                page.copy_from_slice(&seal(key, salt, plain, i as u32 + 1));
            }
            std::fs::write(path, data).unwrap();
            (key, salt)
        }
    }
    #[test]
    fn reads_encrypted_sqlite_without_plaintext_files() {
        if std::env::var_os("DOTISLE_TEST_SQLITE3").is_none() {
            return;
        }
        let p = std::env::temp_dir().join(format!("dotisle-wechat-vfs-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&p);
        let (enc, salt) = fixture(
            &p,
            "CREATE TABLE sample(id INTEGER,body TEXT);INSERT INTO sample VALUES(1,'正文测试');",
        );
        let page = std::fs::read(&p).unwrap();
        let key = Key::verified(enc, &salt, &page[..PAGE]).unwrap();
        let db = Snapshot::new(&p, key).unwrap();
        let rows = db.query("SELECT id,body FROM sample").unwrap();
        assert_eq!(rows[0].text(1), "正文测试");
        assert_eq!(rows[0].ints[0], 1);
        assert!(db.stable());
        assert!(db.reads() < 16);
        drop(db);
        std::fs::remove_file(p).unwrap();
    }
}
