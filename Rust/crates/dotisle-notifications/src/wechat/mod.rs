//! Current-user incoming WeChat content. No sending, injection or version whitelist.
mod crypto;
mod discovery;
mod sqlite;
use crate::notifications::{Notice, Policy};
use crypto::{Key, PAGE};
use md5::{Digest, Md5};
use sqlite::{Snapshot, Stamp};
use std::{
    collections::{HashMap, VecDeque},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
pub enum Update {
    Notice(Notice, u64),
    State(serde_json::Value),
}
pub struct Reader {
    pub events: mpsc::Receiver<Update>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Drop for Reader {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
impl Reader {
    pub fn new(policy: Arc<Mutex<Policy>>, generation: Arc<AtomicU64>) -> Self {
        let (tx, events) = mpsc::sync_channel(32);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let thread = std::thread::spawn(move || {
            use windows::Win32::System::WinRT::{
                RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED,
            };
            let com = unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_ok();
            let mut icon = None;
            let mut active: Option<Account> = None;
            let mut retry = Instant::now();
            let mut last_state = serde_json::Value::Null;
            let mut last_process_check = Instant::now() - Duration::from_secs(10);
            let mut processes = vec![];
            let mut failures = 0;
            let mut epoch = generation.load(Ordering::Acquire);
            let mut pending: VecDeque<Notice> = VecDeque::new();
            while !stopping.load(Ordering::Acquire) {
                let current_epoch = generation.load(Ordering::Acquire);
                if epoch != current_epoch {
                    active = None;
                    pending.clear();
                    retry = Instant::now();
                    epoch = current_epoch;
                }
                let p = policy.lock().map(|p| p.clone()).unwrap_or_default();
                let enabled = p.should_show() && p.permits("WeChat", "微信");
                let mut status = "disabled";
                let mut message = "微信正文提醒已关闭";
                let mut ready = false;
                if !enabled {
                    active = None;
                    pending.clear();
                    retry = Instant::now();
                    if p.enabled && p.paused {
                        status = "paused";
                        message = "微信正文提醒已暂停，恢复时只接收新消息";
                    }
                } else {
                    if last_process_check.elapsed() >= Duration::from_secs(3) {
                        let current = discovery::processes();
                        if current != processes {
                            active = None;
                            processes = current;
                            retry = Instant::now();
                            icon = processes
                                .iter()
                                .filter_map(|(pid, _)| discovery::executable(*pid))
                                .find_map(|path| crate::app_icon::shell_icon(&path));
                        }
                        last_process_check = Instant::now();
                    }
                    if processes.is_empty() {
                        status = "waiting";
                        message = "等待微信启动并登录";
                    } else {
                        if active.is_none() && Instant::now() >= retry {
                            let candidates = discovery::candidates(&processes, &stopping);
                            if stopping.load(Ordering::Acquire) {
                                break;
                            }
                            let mut matches = vec![];
                            for path in discovery::accounts() {
                                if let Ok(a) = Account::discover(path, &candidates) {
                                    matches.push(a)
                                }
                            }
                            if matches.len() == 1 {
                                let mut a = matches.remove(0);
                                if a.poll(true).is_ok()
                                    && generation.load(Ordering::Acquire) == epoch
                                {
                                    active = Some(a);
                                    failures = 0;
                                }
                            }
                            retry = Instant::now() + Duration::from_secs(30);
                        }
                        status = "unavailable";
                        message = "微信正文暂不可读，将自动重新检测";
                        if let Some(a) = &mut active {
                            if a.last_discovery.elapsed() >= Duration::from_secs(10) {
                                a.last_discovery = Instant::now();
                                let files = discovery::message_files(&a.path);
                                if files.iter().any(|p| !a.shards.iter().any(|s| &s.path == p)) {
                                    let candidates = discovery::candidates(&processes, &stopping);
                                    for path in files {
                                        if !a.shards.iter().any(|s| s.path == path) {
                                            if let Some(key) = find_key(&path, &candidates) {
                                                a.shards.push(Shard {
                                                    path,
                                                    key,
                                                    stamp: None,
                                                    watermarks: HashMap::new(),
                                                    baseline: false,
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                            match if pending.is_empty() {
                                a.poll(false)
                            } else {
                                Ok(vec![])
                            } {
                                Ok(notices) => {
                                    failures = 0;
                                    status = "ready";
                                    message = "微信正文已连接";
                                    ready = true;
                                    for mut notice in notices {
                                        notice.icon = Some(
                                            icon.clone()
                                                .unwrap_or_else(crate::app_icon::wechat_icon),
                                        );
                                        let current =
                                            policy.lock().map(|p| p.clone()).unwrap_or_default();
                                        if current.should_show()
                                            && current.permits("WeChat", "微信")
                                        {
                                            if generation.load(Ordering::Acquire) == epoch {
                                                pending.push_back(notice);
                                            }
                                        }
                                    }
                                }
                                Err(_) => {
                                    failures += 1;
                                    status = "retrying";
                                    message = "微信数据正在更新，稍后自动重试";
                                }
                            }
                            if failures >= 5 {
                                active = None;
                                retry = Instant::now();
                                failures = 0;
                            }
                        }
                    }
                }
                while let Some(notice) = pending.pop_front() {
                    if generation.load(Ordering::Acquire) != epoch {
                        pending.clear();
                        break;
                    }
                    match tx.try_send(Update::Notice(notice, epoch)) {
                        Ok(()) => {}
                        Err(mpsc::TrySendError::Full(Update::Notice(n, _))) => {
                            pending.push_front(n);
                            break;
                        }
                        Err(_) => break,
                    }
                }
                let state = serde_json::json!({"source":"local-wcdb","status":status,"ready":ready,"readOnly":true,"message":message,"plaintextFiles":false,"versionRestricted":false});
                if state != last_state {
                    if tx.try_send(Update::State(state.clone())).is_ok() {
                        last_state = state;
                    }
                }
                for _ in 0..6 {
                    if stopping.load(Ordering::Acquire) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
            if com {
                unsafe { RoUninitialize() }
            }
        });
        Self {
            events,
            stop,
            thread: Some(thread),
        }
    }
}
fn seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
fn page1(path: &Path) -> Option<[u8; PAGE]> {
    let mut b = [0; PAGE];
    File::open(path).ok()?.read_exact(&mut b).ok()?;
    Some(b)
}
fn find_key(path: &Path, candidates: &[discovery::Candidate]) -> Option<Key> {
    let page = page1(path)?;
    for c in candidates {
        if let Some(k) = Key::verified(c.0, &page[..16], &page) {
            return Some(k);
        }
        if let Some(salt) = c.1 {
            if let Some(k) = Key::verified(c.0, &salt, &page) {
                return Some(k);
            }
        }
    }
    None
}
struct Shard {
    path: PathBuf,
    key: Key,
    stamp: Option<Stamp>,
    watermarks: HashMap<String, i64>,
    baseline: bool,
}
struct Thread {
    username: String,
    title: String,
    hidden: bool,
}
#[derive(Default)]
struct Filters {
    muted: bool,
    folded: bool,
    blocked: bool,
}
// WeChat 4.x contact.flag bits used by the public contact.db layout:
// 0x8 = blacklisted, 0x200 = notifications muted, 0x10000000 = folded chat.
const FLAG_BLOCKED: i64 = 0x8;
const FLAG_MUTED: i64 = 0x200;
const FLAG_FOLDED: i64 = 0x1000_0000;
struct Account {
    path: PathBuf,
    session: PathBuf,
    session_key: Key,
    session_stamp: Option<Stamp>,
    threads: HashMap<String, Thread>,
    contact: Option<(PathBuf, Key)>,
    contact_stamp: Option<Stamp>,
    display_names: HashMap<String, String>,
    filters: HashMap<String, Filters>,
    shards: Vec<Shard>,
    started: i64,
    last_discovery: Instant,
}
impl Account {
    fn discover(path: PathBuf, candidates: &[discovery::Candidate]) -> Result<Self, String> {
        let session = path.join("db_storage/session/session.db");
        let session_key = find_key(&session, candidates).ok_or("微信会话库暂不可读")?;
        let shards = discovery::message_files(&path)
            .into_iter()
            .filter_map(|p| {
                find_key(&p, candidates).map(|key| Shard {
                    path: p,
                    key,
                    stamp: None,
                    watermarks: HashMap::new(),
                    baseline: true,
                })
            })
            .collect::<Vec<_>>();
        if shards.is_empty() {
            return Err("微信消息库暂不可读".into());
        }
        let contact = path.join("db_storage/contact/contact.db");
        let contact = find_key(&contact, candidates).map(|k| (contact, k));
        Ok(Self {
            path,
            session,
            session_key,
            session_stamp: None,
            threads: HashMap::new(),
            contact,
            contact_stamp: None,
            display_names: HashMap::new(),
            filters: HashMap::new(),
            shards,
            started: seconds(),
            last_discovery: Instant::now(),
        })
    }
    fn poll(&mut self, baseline: bool) -> Result<Vec<Notice>, String> {
        if let Some((path, key)) = &self.contact {
            let stamp = sqlite::stamp(path);
            if self.contact_stamp.as_ref() != Some(&stamp) {
                self.display_names.clear();
                if let Ok(filters) = contact_filters(path, key.clone()) {
                    self.filters = filters;
                }
                self.contact_stamp = Some(stamp);
            }
        }
        let ss = sqlite::stamp(&self.session);
        if self.session_stamp.as_ref() != Some(&ss) {
            let db = Snapshot::new(&self.session, self.session_key.clone())?;
            let columns = db
                .query("PRAGMA table_info(\"SessionTable\")")?
                .into_iter()
                .map(|r| r.text(1))
                .collect::<Vec<_>>();
            let hidden = if columns.iter().any(|c| c == "is_hidden") {
                "is_hidden"
            } else {
                "0"
            };
            let rows = db.query(&format!(
                "SELECT username,last_sender_display_name,{hidden} FROM SessionTable LIMIT 16384"
            ))?;
            let mut threads = HashMap::new();
            for row in rows {
                let user = row.text(0);
                let title = row.text(1);
                let hash = format!("{:x}", Md5::digest(user.as_bytes()));
                threads.insert(
                    format!("Msg_{hash}"),
                    Thread {
                        username: user,
                        title,
                        hidden: row.ints.get(2).copied().unwrap_or(0) != 0,
                    },
                );
            }
            if !db.stable() {
                return Err("微信会话正在更新".into());
            }
            self.threads = threads;
            self.session_stamp = Some(ss);
        }
        // New shards use the original start time; their first incoming message is not baselined away.

        let mut notices = vec![];
        let mut updates = vec![];
        for (index, shard) in self.shards.iter().enumerate() {
            let stamp = sqlite::stamp(&shard.path);
            if shard.stamp.as_ref() == Some(&stamp) {
                continue;
            }
            let db = Snapshot::new(&shard.path, shard.key.clone())?;
            let names = db
                .query("SELECT rowid,user_name FROM Name2Id LIMIT 16384")?
                .into_iter()
                .map(|r| (r.ints[0], r.text(1)))
                .collect::<HashMap<_, _>>();
            let account = self.path.file_name().unwrap_or_default().to_string_lossy();
            let own = names
                .values()
                .filter(|n| {
                    !n.is_empty()
                        && (account == n.as_str() || account.starts_with(&format!("{n}_")))
                })
                .max_by_key(|n| n.len())
                .ok_or("暂不能确认微信收发方向")?;
            let tables=db.query("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'Msg_%' LIMIT 16384")?;
            let mut watermarks = shard.watermarks.clone();
            let mut batch = vec![];
            let mut more = false;
            'tables: for table in tables {
                let table = table.text(0);
                if !valid_table(&table) {
                    continue;
                }
                if baseline && shard.baseline {
                    let max = db.query(&format!("SELECT MAX(local_id) FROM \"{table}\""))?;
                    watermarks.insert(table, max[0].ints[0]);
                    continue;
                }
                // Hidden, muted, folded and blacklisted conversations advance their
                // watermark without producing a banner, so unmuting never replays them.
                if self.thread_blocked(&table) {
                    let max = db.query(&format!("SELECT MAX(local_id) FROM \"{table}\""))?;
                    let max = max.first().map(|r| r.ints[0]).unwrap_or(0);
                    let watermark = *watermarks.get(&table).unwrap_or(&0);
                    watermarks.insert(table, max.max(watermark));
                    continue;
                }
                let watermark = *watermarks.get(&table).unwrap_or(&0);
                let columns = db
                    .query(&format!("PRAGMA table_info(\"{table}\")"))?
                    .into_iter()
                    .map(|r| r.text(1))
                    .collect::<Vec<_>>();
                if ![
                    "local_id",
                    "local_type",
                    "message_content",
                    "real_sender_id",
                    "create_time",
                ]
                .iter()
                .all(|c| columns.iter().any(|v| v == c))
                {
                    return Err("微信消息结构暂不兼容".into());
                }
                let compressed = if columns.iter().any(|c| c == "compress_content") {
                    ",compress_content"
                } else {
                    ",NULL"
                };
                let rows=db.query(&format!("SELECT local_id,local_type,message_content,real_sender_id,create_time{compressed} FROM \"{table}\" WHERE local_id>{watermark} ORDER BY local_id LIMIT 128"))?;
                if rows.len() == 128 {
                    more = true
                }
                for row in rows {
                    if batch.len() >= 32 {
                        more = true;
                        break 'tables;
                    }
                    watermarks.insert(table.clone(), row.ints[0]);
                    if row.ints[4] < self.started - 1 {
                        continue;
                    }
                    let Some(sender) = names.get(&row.ints[3]) else {
                        continue;
                    };
                    if sender == own {
                        continue;
                    }
                    let mut body = decode(&row.bytes[2], row.ints[1]);
                    if body.is_none() {
                        body = decode(&row.bytes[5], row.ints[1]);
                    }
                    let Some(mut body) = body else { continue };
                    if let Some(rest) = body.strip_prefix(&format!("{sender}:\n")) {
                        body = rest.into()
                    }
                    if body.trim().is_empty() {
                        continue;
                    }
                    let fallback = self
                        .threads
                        .get(&table)
                        .map(|t| t.title.clone())
                        .filter(|t| !t.trim().is_empty())
                        .unwrap_or_else(|| sender.clone());
                    let title = sender_name(&self.contact, &mut self.display_names, sender)
                        .unwrap_or(fallback);
                    if batch.len() < 32 {
                        batch.push(Notice {
                            app_id: "WeChat".into(),
                            app_name: "微信".into(),
                            title: title.chars().take(128).collect(),
                            body: body.chars().take(1024).collect(),
                            icon: None,
                        });
                    }
                }
            }
            if !db.stable() {
                return Err("微信消息正在更新".into());
            }
            updates.push((index, watermarks, if more { None } else { Some(stamp) }));
            notices.extend(batch);
            if notices.len() >= 32 {
                break;
            }
        }
        for (index, watermarks, stamp) in updates {
            let shard = &mut self.shards[index];
            shard.watermarks = watermarks;
            shard.baseline = false;
            shard.stamp = stamp;
        }
        Ok(notices)
    }
    fn thread_blocked(&self, table: &str) -> bool {
        let Some(thread) = self.threads.get(table) else {
            return false;
        };
        thread.hidden
            || self
                .filters
                .get(&thread.username)
                .is_some_and(|f| f.muted || f.folded || f.blocked)
    }
}
fn contact_filters(path: &Path, key: Key) -> Result<HashMap<String, Filters>, String> {
    let db = Snapshot::new(path, key)?;
    let columns = db
        .query("PRAGMA table_info(\"contact\")")?
        .into_iter()
        .map(|r| r.text(1))
        .collect::<Vec<_>>();
    if !columns.iter().any(|c| c == "username") || !columns.iter().any(|c| c == "flag") {
        return Ok(HashMap::new());
    }
    let mut selected = vec!["username", "flag"];
    if columns.iter().any(|c| c == "chat_room_notify") {
        selected.push("chat_room_notify");
    }
    if columns.iter().any(|c| c == "local_type") {
        selected.push("local_type");
    }
    let rows = db.query(&format!(
        "SELECT {} FROM contact LIMIT 16384",
        selected.join(",")
    ))?;
    let mut out = HashMap::new();
    for row in rows {
        let name = row.text(0);
        if name.is_empty() {
            continue;
        }
        let flag = row.ints.get(1).copied().unwrap_or(0);
        let notify = selected
            .iter()
            .position(|c| *c == "chat_room_notify")
            .and_then(|i| row.ints.get(i).copied());
        let kind = selected
            .iter()
            .position(|c| *c == "local_type")
            .and_then(|i| row.ints.get(i).copied());
        let group = name.ends_with("@chatroom");
        let muted = if group {
            notify == Some(0)
        } else {
            kind.is_none_or(|kind| kind == 1) && flag & FLAG_MUTED != 0
        };
        out.insert(
            name,
            Filters {
                muted,
                folded: flag & FLAG_FOLDED != 0,
                blocked: flag & FLAG_BLOCKED != 0,
            },
        );
    }
    if !db.stable() {
        return Err("微信联系人正在更新".into());
    }
    Ok(out)
}
fn sender_name(
    contact: &Option<(PathBuf, Key)>,
    cache: &mut HashMap<String, String>,
    sender: &str,
) -> Option<String> {
    if let Some(name) = cache.get(sender) {
        return Some(name.clone());
    }
    let (path, key) = contact.as_ref()?;
    let db = Snapshot::new(path, key.clone()).ok()?;
    let rows = db
        .query(&format!(
            "SELECT remark,nick_name FROM contact WHERE username='{}' LIMIT 1",
            sender.replace('\'', "''")
        ))
        .ok()?;
    let row = rows.first()?;
    let name = [row.text(0), row.text(1)]
        .into_iter()
        .find(|s| !s.trim().is_empty())?;
    if !db.stable() {
        return None;
    }
    if cache.len() >= 256 {
        cache.clear();
    }
    let name = name.chars().take(128).collect::<String>();
    cache.insert(sender.into(), name.clone());
    Some(name)
}
fn valid_table(s: &str) -> bool {
    s.strip_prefix("Msg_")
        .is_some_and(|s| s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit()))
}
fn decode(bytes: &[u8], kind: i64) -> Option<String> {
    if kind as u32 != 1 {
        return None;
    }
    let text = if bytes.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
        if !bounded_zstd(bytes) {
            return None;
        }
        let mut decoder = ruzstd::decoding::StreamingDecoder::new(bytes).ok()?;
        let mut out = vec![];
        std::io::Read::by_ref(&mut decoder)
            .take(262145)
            .read_to_end(&mut out)
            .ok()?;
        if out.len() > 262144 {
            return None;
        }
        String::from_utf8(out).ok()?
    } else {
        std::str::from_utf8(bytes).ok()?.into()
    };
    let text = text.trim_matches(['\0', '\u{feff}']).trim();
    if text.is_empty() {
        None
    } else {
        Some(text.into())
    }
}
fn bounded_zstd(bytes: &[u8]) -> bool {
    let Some(&descriptor) = bytes.get(4) else {
        return false;
    };
    let single = descriptor & 0x20 != 0;
    let mut offset = 5;
    let mut window = 0u64;
    if !single {
        let Some(&w) = bytes.get(offset) else {
            return false;
        };
        let base = 1u64 << ((w >> 3) as u32 + 10);
        window = base + (base / 8) * (w & 7) as u64;
        offset += 1;
    }
    offset += [0, 1, 2, 4][(descriptor & 3) as usize];
    let n = match descriptor >> 6 {
        0 => usize::from(single),
        1 => 2,
        2 => 4,
        _ => 8,
    };
    let Some(content) = bytes.get(offset..offset + n) else {
        return false;
    };
    let mut size = 0u64;
    for (i, b) in content.iter().enumerate() {
        size |= (*b as u64) << (8 * i)
    }
    if n == 2 {
        size += 256
    }
    if single {
        window = size
    }
    window <= 1_048_576 && (n == 0 || size <= 262144)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_text_only_and_compound_type() {
        assert_eq!(decode("正文😀".as_bytes(), 1).unwrap(), "正文😀");
        assert_eq!(decode(b"text", (3i64 << 32) | 1).unwrap(), "text");
        assert!(decode(b"", 1).is_none());
        assert!(decode(b"<xml>image</xml>", 3).is_none());
        assert!(decode(&[0xff], 1).is_none());
        assert!(!valid_table("Msg_a\";DROP TABLE Name2Id"));
    }
    #[test]
    fn zstd_text_and_memory_limits() {
        let text = "长文本正文😀".as_bytes();
        let mut frame = vec![0x28, 0xb5, 0x2f, 0xfd, 0x20, text.len() as u8];
        let block = ((text.len() as u32) << 3) | 1;
        frame.extend_from_slice(&block.to_le_bytes()[..3]);
        frame.extend_from_slice(text);
        assert_eq!(decode(&frame, 1).unwrap(), "长文本正文😀");
        assert!(!bounded_zstd(&[0x28, 0xb5, 0x2f, 0xfd, 0, 0xff]));
    }
    #[test]
    #[ignore]
    fn local_readonly_capability() {
        let stop = AtomicBool::new(false);
        let pids = discovery::processes();
        let candidates = discovery::candidates(&pids, &stop);
        let mut matches = 0;
        for path in discovery::accounts() {
            if let Ok(mut a) = Account::discover(path, &candidates) {
                matches += 1;
                let result = a.poll(true);
                eprintln!(
                    "verified_shards={} baseline_ok={} reason={}",
                    a.shards.len(),
                    result.is_ok(),
                    result.err().unwrap_or_default()
                );
                assert!(a.poll(false).is_ok());
            }
        }
        eprintln!(
            "processes={} candidates={} verified_accounts={}",
            pids.len(),
            candidates.len(),
            matches
        );
        assert_eq!(matches, 1);
    }
    #[test]
    fn incoming_direction_baseline_and_new_table() {
        if std::env::var_os("DOTISLE_TEST_SQLITE3").is_none() {
            return;
        }
        let root = std::env::temp_dir().join(format!(
            "dotisle-wcdb-pipeline-{}/wxid_self_abcd",
            std::process::id()
        ));
        let message = root.join("db_storage/message");
        let session = root.join("db_storage/session/session.db");
        std::fs::create_dir_all(&message).unwrap();
        std::fs::create_dir_all(session.parent().unwrap()).unwrap();
        let path = message.join("message_0.db");
        let table = "Msg_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let make = |extra: &str| {
            let _ = std::fs::remove_file(&path);
            sqlite::fixture(&path,&format!("CREATE TABLE Name2Id(user_name TEXT);INSERT INTO Name2Id(rowid,user_name) VALUES(5,'wxid_self'),(8,'wxid_friend');CREATE TABLE {table}(local_id INTEGER PRIMARY KEY,local_type INTEGER,message_content BLOB,real_sender_id INTEGER,create_time INTEGER);INSERT INTO {table} VALUES(1,1,'old',8,{});{extra}",seconds()));
        };
        let _ = std::fs::remove_file(&session);
        sqlite::fixture(
            &session,
            "CREATE TABLE SessionTable(username TEXT,last_sender_display_name TEXT);",
        );
        make("");
        let contact = root.join("db_storage/contact/contact.db");
        std::fs::create_dir_all(contact.parent().unwrap()).unwrap();
        sqlite::fixture(&contact,"CREATE TABLE contact(username TEXT,remark TEXT,nick_name TEXT);INSERT INTO contact VALUES('wxid_friend','好友备注','好友昵称');");
        let candidates = [discovery::Candidate([7; 32], None)];
        let mut a = Account::discover(root.clone(), &candidates).unwrap();
        assert!(a.poll(true).unwrap().is_empty());
        make(&format!("INSERT INTO {table} VALUES(2,1,'outgoing',5,{}),(3,1,'incoming',8,{});CREATE TABLE Msg_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb AS SELECT 1 AS local_id,1 AS local_type,'first new chat' AS message_content,8 AS real_sender_id,{} AS create_time;",seconds(),seconds(),seconds()));
        let notices = a.poll(false).unwrap();
        assert_eq!(notices.len(), 2);
        assert!(notices.iter().all(|n| n.title == "好友备注"));
        assert!(notices.iter().any(|n| n.body == "incoming"));
        assert!(notices.iter().any(|n| n.body == "first new chat"));
        assert!(a.poll(false).unwrap().is_empty());
        drop(a);
        std::fs::remove_dir_all(root.parent().unwrap()).unwrap();
    }
    #[test]
    fn hidden_muted_folded_and_blocked_conversations_do_not_notify() {
        if std::env::var_os("DOTISLE_TEST_SQLITE3").is_none() {
            return;
        }
        let root = std::env::temp_dir().join(format!(
            "dotisle-wcdb-filters-{}/wxid_self_abcd",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(root.parent().unwrap());
        let message = root.join("db_storage/message");
        let session = root.join("db_storage/session/session.db");
        let contact = root.join("db_storage/contact/contact.db");
        std::fs::create_dir_all(&message).unwrap();
        std::fs::create_dir_all(session.parent().unwrap()).unwrap();
        std::fs::create_dir_all(contact.parent().unwrap()).unwrap();
        let path = message.join("message_0.db");
        let table = |name: &str| format!("Msg_{:x}", Md5::digest(name.as_bytes()));
        let specs = [
            ("wxid_friend", "好友", 1i64, 0i64, 0i64),
            ("wxid_muted", "免打扰", 0x201, 0, 0),
            ("wxid_folded", "折叠", 0x1000_0001, 0, 0),
            ("wxid_blocked", "黑名单", 9, 0, 0),
            ("wxid_hidden", "隐藏", 1, 0, 1),
            ("room@chatroom", "群聊", 2, 0, 0),
        ];
        let make = |id: i64, body: &str| {
            let _ = std::fs::remove_file(&path);
            let mut sql = String::from(
                "CREATE TABLE Name2Id(user_name TEXT);INSERT INTO Name2Id(rowid,user_name) VALUES(5,'wxid_self'),(8,'wxid_friend');",
            );
            for (name, _title, _flag, _notify, _hidden) in specs {
                let t = table(name);
                sql.push_str(&format!("CREATE TABLE {t}(local_id INTEGER PRIMARY KEY,local_type INTEGER,message_content BLOB,real_sender_id INTEGER,create_time INTEGER);INSERT INTO {t} VALUES({id},1,'{body}-{name}',8,{});", seconds()));
            }
            sqlite::fixture(&path, &sql);
        };
        make(1, "old");
        let _ = std::fs::remove_file(&session);
        let mut sessions = String::from(
            "CREATE TABLE SessionTable(username TEXT,last_sender_display_name TEXT,is_hidden INTEGER);",
        );
        for (name, title, _flag, _notify, hidden) in specs {
            sessions.push_str(&format!(
                "INSERT INTO SessionTable VALUES('{name}','{title}',{hidden});"
            ));
        }
        sqlite::fixture(&session, &sessions);
        let contacts = |mute_muted: bool| {
            let _ = std::fs::remove_file(&contact);
            let mut sql = String::from(
                "CREATE TABLE contact(username TEXT,flag INTEGER,chat_room_notify INTEGER,local_type INTEGER);",
            );
            for (name, _title, flag, notify, _hidden) in specs {
                let flag = if name == "wxid_muted" && !mute_muted {
                    flag & !FLAG_MUTED
                } else {
                    flag
                };
                let kind = if name.ends_with("@chatroom") { 2 } else { 1 };
                sql.push_str(&format!(
                    "INSERT INTO contact VALUES('{name}',{flag},{notify},{kind});"
                ));
            }
            sqlite::fixture(&contact, &sql);
        };
        contacts(true);
        let candidates = [discovery::Candidate([7; 32], None)];
        let mut a = Account::discover(root.clone(), &candidates).unwrap();
        assert!(a.poll(true).unwrap().is_empty());
        make(2, "new");
        let notices = a.poll(false).unwrap();
        assert_eq!(
            notices.len(),
            1,
            "{:?}",
            notices.iter().map(|n| n.body.clone()).collect::<Vec<_>>()
        );
        assert_eq!(notices[0].title, "好友");
        assert_eq!(notices[0].body, "new-wxid_friend");
        assert!(a.poll(false).unwrap().is_empty());
        // Unmuting is picked up from contact.db without restarting the reader.
        contacts(false);
        make(3, "again");
        let notices = a.poll(false).unwrap();
        assert_eq!(notices.len(), 2);
        assert!(notices
            .iter()
            .any(|n| n.title == "好友" && n.body == "again-wxid_friend"));
        assert!(notices
            .iter()
            .any(|n| n.title == "免打扰" && n.body == "again-wxid_muted"));
        drop(a);
        std::fs::remove_dir_all(root.parent().unwrap()).unwrap();
    }
}
