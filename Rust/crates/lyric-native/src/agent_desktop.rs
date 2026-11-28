// SPDX-License-Identifier: MIT
//! Question-only Rust port of the Win.13 QuotaView Desktop follower protocol.
//! Copyright (c) 2026 Duoasa (upstream); see licenses/QuotaView-Windows-MIT.txt.
//! Never opens a new agent turn or responds to permission/approval requests.
use crate::agent_island::{Prompt, Question, Submission, OPTIONS};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::windows::io::AsRawHandle,
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant, SystemTime},
};
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE},
    Security::{EqualSid, GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER},
    Storage::Packaging::Appx::GetPackageFamilyName,
    System::{
        Pipes::{GetNamedPipeServerProcessId, PeekNamedPipe},
        Threading::{
            GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
        },
    },
};

const WAIT: Duration = Duration::from_secs(5);
const MAX_FRAME: usize = 8 * 1024 * 1024;
const MAX_FOLLOW: usize = 8;
const FORM: &str = "item/tool/requestUserInput";
const SUBMIT: &str = "thread-follower-submit-user-input";
static NEXT_TOKEN: AtomicU64 = AtomicU64::new(1);

enum Event {
    Prompt(Option<Prompt>),
    Feedback(&'static str),
}
#[derive(Default)]
pub struct Bridge {
    tx: Option<mpsc::SyncSender<Submission>>,
    rx: Option<mpsc::Receiver<Event>>,
    prompt: Option<Prompt>,
    attempt: Option<Instant>,
}
impl Bridge {
    pub fn poll(&mut self, active: bool) -> (Option<Prompt>, Option<&'static str>) {
        if !active {
            *self = Self::default();
            return (None, None);
        }
        let mut feedback = None;
        if let Some(rx) = &self.rx {
            loop {
                match rx.try_recv() {
                    Ok(Event::Prompt(v)) => self.prompt = v,
                    Ok(Event::Feedback(s)) => feedback = Some(s),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.rx = None;
                        self.tx = None;
                        self.prompt = None;
                        break;
                    }
                }
            }
        }
        if self.rx.is_none() && self.attempt.is_none_or(|t| t.elapsed() >= WAIT) {
            self.attempt = Some(Instant::now());
            let (tx, requests) = mpsc::sync_channel(1);
            let (events, rx) = mpsc::channel();
            if std::thread::Builder::new()
                .name("codex-question".into())
                .spawn(move || {
                    let _ = run(requests, events);
                })
                .is_ok()
            {
                self.tx = Some(tx);
                self.rx = Some(rx);
            }
        }
        (self.prompt.clone(), feedback)
    }
    pub fn send(&self, s: Submission) -> bool {
        self.tx.as_ref().is_some_and(|tx| tx.try_send(s).is_ok())
    }
}

fn valid_uuid(s: &str) -> bool {
    s.len() == 36
        && s.bytes().enumerate().all(|(i, b)| {
            if matches!(i, 8 | 13 | 18 | 23) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}
fn recent_threads() -> Vec<String> {
    // UUID suffixes are enough for discovery, not authority to send a reply.
    let Some(home) = crate::ai_quota::codex_home() else {
        return vec![];
    };
    let now = SystemTime::now();
    let Some(date) = chrono::DateTime::from_timestamp(
        now.duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64,
        0,
    ) else {
        return vec![];
    };
    let mut found = vec![];
    for day in -2..=1 {
        let dir = home.join("sessions").join(
            (date + chrono::TimeDelta::days(day))
                .format("%Y/%m/%d")
                .to_string(),
        );
        for entry in fs::read_dir(dir).into_iter().flatten().flatten().take(4096) {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if !name.starts_with("rollout-") {
                continue;
            }
            let Some(s) = name.strip_suffix(".jsonl") else {
                continue;
            };
            let Some(id) = s
                .get(s.len().saturating_sub(36)..)
                .filter(|id| valid_uuid(id))
            else {
                continue;
            };
            let Ok(time) = entry.metadata().and_then(|m| m.modified()) else {
                continue;
            };
            if now
                .duration_since(time)
                .is_ok_and(|age| age < Duration::from_secs(12 * 3600))
            {
                found.push((time, id.to_ascii_lowercase()));
            }
        }
    }
    found.sort_unstable_by(|a, b| b.0.cmp(&a.0));
    let mut unique = vec![];
    for (_, id) in found {
        if !unique.contains(&id) {
            unique.push(id);
        }
        if unique.len() == MAX_FOLLOW {
            break;
        }
    }
    unique
}

fn unsupported() -> std::io::Error {
    std::io::ErrorKind::InvalidData.into()
}
struct OwnedHandle(HANDLE);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
unsafe fn user(p: HANDLE) -> Option<Vec<usize>> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(p, TOKEN_QUERY, &mut token).ok()?;
        let token = OwnedHandle(token);
        let mut size = 0;
        let _ = GetTokenInformation(token.0, TokenUser, None, 0, &mut size);
        if (size as usize) < std::mem::size_of::<TOKEN_USER>() || size > 4096 {
            return None;
        }
        let mut data = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
        GetTokenInformation(
            token.0,
            TokenUser,
            Some(data.as_mut_ptr().cast()),
            size,
            &mut size,
        )
        .ok()?;
        Some(data)
    }
}
pub fn verified_package(p: HANDLE) -> bool {
    let mut family = [0u16; 128];
    let mut len = family.len() as u32;
    let success = unsafe {
        GetPackageFamilyName(p, &mut len, Some(windows::core::PWSTR(family.as_mut_ptr())))
    };
    success.0 == 0
        && len > 1
        && len as usize <= family.len()
        && String::from_utf16_lossy(&family[..len as usize - 1]) == "OpenAI.Codex_2p2nqsd0c76g0"
}
fn verified_pipe(file: &File) -> bool {
    unsafe {
        use windows::Win32::System::Threading::{QueryFullProcessImageNameW, PROCESS_NAME_FORMAT};
        let mut pid = 0;
        if GetNamedPipeServerProcessId(HANDLE(file.as_raw_handle()), &mut pid).is_err() {
            return false;
        }
        let Ok(p) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let p = OwnedHandle(p);
        if !verified_package(p.0) {
            return false;
        }
        let mut image = [0u16; 1024];
        let mut len = image.len() as u32;
        if QueryFullProcessImageNameW(
            p.0,
            PROCESS_NAME_FORMAT(0),
            windows::core::PWSTR(image.as_mut_ptr()),
            &mut len,
        )
        .is_err()
            || len as usize > image.len()
        {
            return false;
        }
        let path = String::from_utf16_lossy(&image[..len as usize]);
        if !Path::new(&path)
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|s| {
                s.eq_ignore_ascii_case("Codex.exe") || s.eq_ignore_ascii_case("ChatGPT.exe")
            })
        {
            return false;
        }
        let (Some(a), Some(b)) = (user(p.0), user(GetCurrentProcess())) else {
            return false;
        };
        EqualSid(
            (*a.as_ptr().cast::<TOKEN_USER>()).User.Sid,
            (*b.as_ptr().cast::<TOKEN_USER>()).User.Sid,
        )
        .is_ok()
    }
}

struct Wire {
    file: File,
    buf: Vec<u8>,
    at: Option<Instant>,
}
impl Wire {
    fn open() -> std::io::Result<Self> {
        use std::os::windows::fs::OpenOptionsExt;
        // SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION, no impersonation.
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .security_qos_flags(0x0011_0000)
            .open(r"\\.\pipe\codex-ipc")?;
        if !verified_pipe(&file) {
            return Err(unsupported());
        }
        Ok(Self {
            file,
            buf: vec![],
            at: None,
        })
    }
    fn emit(&mut self, value: Value) -> std::io::Result<()> {
        let bytes = serde_json::to_vec(&value)?;
        if bytes.len() > MAX_FRAME {
            return Err(unsupported());
        }
        let mut msg = Vec::with_capacity(bytes.len() + 4);
        msg.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        msg.extend_from_slice(&bytes);
        self.file.write_all(&msg)
    }
    fn read(&mut self) -> std::io::Result<Option<Value>> {
        let mut available = 0;
        unsafe {
            PeekNamedPipe(
                HANDLE(self.file.as_raw_handle()),
                None,
                0,
                None,
                Some(&mut available),
                None,
            )
            .map_err(std::io::Error::other)?;
        }
        if self.at.is_some_and(|t| t.elapsed() >= WAIT) {
            return Err(unsupported());
        }
        if available > 0 {
            let old = self.buf.len();
            let amount = (available as usize).min(64 * 1024);
            if old + amount > MAX_FRAME + 65540 {
                return Err(unsupported());
            }
            if old == 0 {
                self.at = Some(Instant::now());
            }
            self.buf.resize(old + amount, 0);
            let n = self.file.read(&mut self.buf[old..])?;
            if n == 0 {
                return Err(std::io::ErrorKind::BrokenPipe.into());
            }
            self.buf.truncate(old + n);
        }
        if self.buf.len() < 4 {
            return Ok(None);
        }
        let size = u32::from_le_bytes(self.buf[..4].try_into().unwrap()) as usize;
        if size == 0 || size > MAX_FRAME {
            return Err(unsupported());
        }
        if self.buf.len() < size + 4 {
            return Ok(None);
        }
        let value = serde_json::from_slice(&self.buf[4..size + 4])?;
        self.buf.drain(..size + 4);
        self.at = if self.buf.is_empty() {
            None
        } else {
            Some(Instant::now())
        };
        Ok(Some(value))
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Candidate {
    thread: String,
    turn: String,
    owner: String,
    request: Value,
    questions: Vec<Question>,
}
fn parse_form(raw: &Value, thread: &str, turn: &str, owner: &str) -> Option<Candidate> {
    if raw["method"] != FORM
        || raw["completed"] == true
        || raw["params"]["threadId"] != thread
        || raw["params"]["turnId"] != turn
    {
        return None;
    }
    let req = &raw["id"];
    if !req
        .as_str()
        .is_some_and(|s| !s.is_empty() && s.len() <= 256)
        && !req
            .as_i64()
            .is_some_and(|x| x.unsigned_abs() <= (1_u64 << 53) - 1)
    {
        return None;
    }
    let fields = raw["params"]["questions"].as_array()?;
    if fields.is_empty() || fields.len() > 8 {
        return None;
    }
    let mut questions: Vec<Question> = vec![];
    for field in fields {
        let id = field["id"].as_str()?;
        let text = field["question"].as_str()?;
        if id.is_empty()
            || id.len() > 256
            || questions.iter().any(|q| q.id == id)
            || text.trim().is_empty()
            || text.chars().count() > 140
            || field["isSecret"] == true
        {
            return None;
        }
        let mut labels = vec![];
        if !field["options"].is_null() {
            let options = field["options"].as_array()?;
            if options.len() > OPTIONS {
                return None;
            }
            for option in options {
                let label = option.as_str().or_else(|| option["label"].as_str())?;
                if label.trim().is_empty()
                    || label.chars().count() > 52
                    || labels.iter().any(|x: &String| x == label)
                {
                    return None;
                }
                labels.push(label.into());
            }
        }
        let custom = field["isOther"] != false;
        if labels.is_empty() && !custom {
            return None;
        }
        questions.push(Question {
            id: id.into(),
            text: text.into(),
            labels,
            custom,
            multi: field["multiSelect"] == true,
        });
    }
    Some(Candidate {
        thread: thread.into(),
        turn: turn.into(),
        owner: owner.into(),
        request: raw.clone(),
        questions,
    })
}
fn project(state: &Value, thread: &str, owner: &str) -> Option<Candidate> {
    if state["id"] != thread {
        return None;
    }
    let turns = state["turns"].as_array();
    let turn = if state["turnHistory"]["kind"] == "canonical" {
        let history = &state["turnHistory"]["history"];
        let last = history["islands"].as_array()?.last()?;
        if last["newerBoundary"]["status"] != "exhausted" {
            return None;
        }
        let entity =
            &history["entitiesByKey"][last["entries"].as_array()?.last()?["value"].as_str()?];
        let canon_id = entity["turnId"].as_str()?;
        match turns
            .and_then(|list| list.last())
            .filter(|t| !t["turnId"].is_null())
        {
            Some(live)
                if live["turnId"] == canon_id
                    || live["turnStartedAtMs"]
                        .as_i64()
                        .zip(entity["turnStartedAtMs"].as_i64())
                        .is_some_and(|(a, b)| a > b) =>
            {
                live
            }
            Some(_) => return None,
            None => entity,
        }
    } else {
        turns?.last()?
    };
    if turn["status"] != "inProgress" {
        return None;
    }
    let turn = turn["turnId"].as_str()?;
    let requests = state["requests"].as_array()?;
    if requests.len() > 128 {
        return None;
    }
    let mut pending = requests
        .iter()
        .filter(|r| r["method"] == FORM && r["completed"] != true && r["params"]["turnId"] == turn);
    let raw = pending.next()?;
    if pending.any(|p| p["id"] == raw["id"]) {
        return None;
    }
    parse_form(raw, thread, turn, owner)
}
fn response(c: &Candidate, s: &Submission) -> Option<Value> {
    let mut answers = serde_json::Map::new();
    if s.answers.len() != c.questions.len() {
        return None;
    }
    for q in &c.questions {
        let a = s.answers.get(&q.id)?;
        if a.is_empty()
            || a.len() > OPTIONS
            || (!q.multi && a.len() != 1)
            || a.iter().any(|s| {
                s.trim().is_empty() || s.len() > 1000 || (!q.custom && !q.labels.contains(s))
            })
        {
            return None;
        }
        answers.insert(q.id.clone(), json!({"answers":a}));
    }
    Some(
        json!({"conversationId":c.thread,"requestId":c.request["id"],"response":{"answers":answers}}),
    )
}

fn apply_patches(state: &mut Value, list: &Value) -> Option<()> {
    let list = list.as_array()?;
    if list.len() > 2048 {
        return None;
    }
    for patch in list {
        let op = patch["op"].as_str()?;
        if !matches!(op, "add" | "replace" | "remove") {
            return None;
        }
        let path = patch["path"].as_array()?;
        if path.len() > 64 {
            return None;
        }
        let Some((key, parents)) = path.split_last() else {
            if op == "remove" || !patch["value"].is_object() {
                return None;
            }
            *state = patch["value"].clone();
            continue;
        };
        let mut value = &mut *state;
        for key in parents {
            value = if let Some(s) = key.as_str() {
                value.as_object_mut()?.get_mut(s)?
            } else {
                value.as_array_mut()?.get_mut(key.as_u64()? as usize)?
            };
        }
        if let Some(obj) = value.as_object_mut() {
            let key = key.as_str()?;
            if op == "remove" {
                obj.remove(key)?;
            } else {
                if op == "replace" && !obj.contains_key(key) {
                    return None;
                }
                obj.insert(key.into(), patch["value"].clone());
            }
        } else if let Some(a) = value.as_array_mut() {
            if key == "length" && op == "replace" {
                let n = patch["value"].as_u64()? as usize;
                if n > a.len() {
                    return None;
                }
                a.truncate(n);
                continue;
            }
            let i = key.as_u64()? as usize;
            if op == "add" {
                if i > a.len() {
                    return None;
                }
                a.insert(i, patch["value"].clone());
            } else if i >= a.len() {
                return None;
            } else if op == "remove" {
                a.remove(i);
            } else {
                a[i] = patch["value"].clone();
            }
        } else {
            return None;
        }
    }
    Some(())
}

#[derive(Default)]
struct Follow {
    owner: String,
    discovery: String,
    discovering: Option<Instant>,
    state: Value,
    revision: u64,
    bytes: usize,
    supports_input: bool,
    request_at: Option<Instant>,
}
struct Refresh {
    candidate: Candidate,
    submission: Submission,
    at: Instant,
}
trait Transport {
    fn emit(&mut self, value: Value) -> std::io::Result<()>;
    fn read(&mut self) -> std::io::Result<Option<Value>>;
}
impl Transport for Wire {
    fn emit(&mut self, value: Value) -> std::io::Result<()> {
        Wire::emit(self, value)
    }
    fn read(&mut self) -> std::io::Result<Option<Value>> {
        Wire::read(self)
    }
}
fn ask(
    w: &mut impl Transport,
    seq: &mut u64,
    client: &str,
    method: &str,
    version: u32,
    owner: Option<&str>,
    params: Value,
) -> std::io::Result<String> {
    *seq += 1;
    let id = format!("dotisle-{}-{}", std::process::id(), seq);
    let mut m = json!({"type":"request","requestId":id,"sourceClientId":client,"version":version,"method":method,"params":params});
    if let Some(owner) = owner {
        m["targetClientId"] = json!(owner);
    }
    w.emit(m)?;
    Ok(id)
}
fn follow(
    w: &mut impl Transport,
    client: &str,
    id: &str,
    owner: &str,
    following: bool,
) -> std::io::Result<()> {
    w.emit(json!({"type":"broadcast","method":"thread-stream-following-changed","version":1,"sourceClientId":client,"targetClientIds":[owner],"params":{"conversationId":id,"hostId":"local","following":following}}))
}
fn run(rx: mpsc::Receiver<Submission>, tx: mpsc::Sender<Event>) -> std::io::Result<()> {
    run_with(Wire::open()?, rx, tx, recent_threads)
}
fn run_with(
    mut w: impl Transport,
    rx: mpsc::Receiver<Submission>,
    tx: mpsc::Sender<Event>,
    threads: impl Fn() -> Vec<String>,
) -> std::io::Result<()> {
    let mut seq = 0;
    let init = ask(
        &mut w,
        &mut seq,
        "",
        "initialize",
        0,
        None,
        json!({"clientType":"dotisle"}),
    )?;
    let began = Instant::now();
    let mut client = String::new();
    let mut last: Option<Instant> = None;
    let mut order = vec![];
    let mut entries: HashMap<String, Follow> = HashMap::new();
    let mut active: Option<(u64, Candidate)> = None;
    let mut refresh: Option<Refresh> = None;
    let mut ack: Option<(String, String, Instant)> = None;
    let mut sent: Vec<Candidate> = vec![];
    loop {
        match rx.try_recv() {
            Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            Err(mpsc::TryRecvError::Empty) => {}
            Ok(s) => {
                if let Some((_, c)) = active
                    .as_ref()
                    .filter(|(token, c)| {
                        *token == s.token && !sent.contains(c) && response(c, &s).is_some()
                    })
                    .filter(|_| refresh.is_none() && ack.is_none())
                {
                    follow(&mut w, &client, &c.thread, &c.owner, true)?;
                    refresh = Some(Refresh {
                        candidate: c.clone(),
                        submission: s,
                        at: Instant::now(),
                    });
                } else {
                    let _ = tx.send(Event::Feedback("问题已变化，请回 Codex 确认"));
                }
            }
        }
        if client.is_empty() && began.elapsed() >= WAIT {
            return Err(unsupported());
        }
        if refresh.as_ref().is_some_and(|r| r.at.elapsed() >= WAIT) {
            return Err(unsupported());
        }
        if ack.as_ref().is_some_and(|(_, _, t)| t.elapsed() >= WAIT) {
            let _ = tx.send(Event::Feedback("发送结果未知，请在 Codex 中确认"));
            ack = None;
        }
        if !client.is_empty() && last.is_none_or(|t| t.elapsed() >= Duration::from_secs(3)) {
            last = Some(Instant::now());
            order = threads();
            if let Some((_, c)) = &active {
                if !order.contains(&c.thread) {
                    order.push(c.thread.clone());
                }
            }
            let stale: Vec<_> = entries
                .keys()
                .filter(|id| !order.contains(id))
                .cloned()
                .collect();
            for id in stale {
                if let Some(e) = entries.remove(&id).filter(|e| !e.owner.is_empty()) {
                    follow(&mut w, &client, &id, &e.owner, false)?;
                }
            }
            for id in &order {
                let e = entries.entry(id.clone()).or_default();
                if e.owner.is_empty() && e.discovering.is_none_or(|d| d.elapsed() >= WAIT) {
                    e.discovery = ask(
                        &mut w,
                        &mut seq,
                        &client,
                        "thread-owner-discovery",
                        1,
                        None,
                        json!({"conversationId":id,"hostId":"local"}),
                    )?;
                    e.discovering = Some(Instant::now());
                } else if !e.owner.is_empty()
                    && e.state.is_null()
                    && e.request_at.is_none_or(|d| d.elapsed() >= WAIT)
                {
                    follow(&mut w, &client, id, &e.owner, true)?;
                    e.request_at = Some(Instant::now());
                }
            }
        }
        for _ in 0..16 {
            let Some(m) = w.read()? else { break };
            match m["type"].as_str(){
                Some("response")=>{
                    let req=m["requestId"].as_str().unwrap_or(""); let method=m["method"].as_str().unwrap_or("");
                    let ok=m["resultType"]=="success";
                    if req==init&&method=="initialize"&&ok&&client.is_empty(){
                        client=m["result"]["clientId"].as_str().filter(|s|s.len()<=256).unwrap_or("").to_owned();
                    }else if method==SUBMIT&&ack.as_ref().is_some_and(|(id,owner,_)|req==id&&m["handledByClientId"]==*owner){
                        ack=None; let _=tx.send(Event::Feedback(if ok&&m["result"]["ok"]==true{"已发送到 Codex"}else{"发送结果未知，请在 Codex 中确认"}));
                    }else if method=="thread-owner-discovery"&&ok {
                        if let Some((id,e))=entries.iter_mut().find(|(_,e)|!e.discovery.is_empty()&&e.discovery==req){
                            e.discovery.clear();
                            e.owner=m["handledByClientId"].as_str().filter(|s|s.len()<=256).unwrap_or("").to_owned();
                            e.supports_input=m["result"]["supportsUntrustedAppInput"]==true;
                            if e.supports_input&&!e.owner.is_empty(){follow(&mut w,&client,id,&e.owner,true)?;e.request_at=Some(Instant::now());}
                        }
                    }
                }
                Some("client-discovery-request")=>w.emit(json!({"type":"client-discovery-response","requestId":m["requestId"],"response":{"canHandle":false}}))?,
                Some("request")=>w.emit(json!({"type":"response","requestId":m["requestId"],"resultType":"error","error":"no-handler-for-request"}))?,
                Some("broadcast") if !client.is_empty()=>{
                    if m.get("targetClientIds").is_some()&&!m["targetClientIds"].as_array().is_some_and(|a|a.iter().any(|v|v==&client)){continue;}
                    if m["method"]=="ipc-connection-reset" || (m["method"]=="client-status-changed"&&m["params"]["status"]=="disconnected"&&entries.values().any(|e|!e.owner.is_empty()&&m["params"]["clientId"]==e.owner)){return Err(unsupported());}
                    if m["params"]["hostId"]!="local"{continue;}
                    let Some(id)=m["params"]["conversationId"].as_str().map(str::to_owned) else{continue;};
                    let id=id.as_str();
                    let Some(entry)=entries.get_mut(id) else{continue;};
                    if m["method"]=="thread-stream-following-status-requested"&&m["sourceClientId"]!=entry.owner {return Err(unsupported());}
                    if m["method"]!="thread-stream-state-changed"||entry.owner.is_empty()||m["sourceClientId"]!=entry.owner||!entry.supports_input{continue;}
                    if m["version"]!=11{return Err(unsupported());}
                    let bytes=serde_json::to_vec(&m)?.len();
                    let mut change=m;
                    let change=&mut change["params"]["change"];
                    let rev=change["revision"].as_u64().ok_or_else(unsupported)?;
                    if rev<entry.revision {continue;}
                    let snapshot=change["type"]=="snapshot";
                    if snapshot {
                        entry.state=change["conversationState"].take();entry.bytes=bytes;entry.revision=rev;
                    }else if change["type"]=="patches" {
                        entry.bytes=entry.bytes.saturating_add(bytes);
                        if entry.state.is_null()||rev<=entry.revision||change["baseRevision"].as_u64()!=Some(entry.revision)||entry.bytes>MAX_FRAME||apply_patches(&mut entry.state,&change["patches"]).is_none(){
                            entry.state=Value::Null;entry.request_at=Some(Instant::now());follow(&mut w,&client,id,&entry.owner,true)?;
                        }else{entry.revision=rev;}
                    }else{continue;}
                    if snapshot&&refresh.as_ref().is_some_and(|r|r.candidate.thread==id){
                        let r=refresh.take().unwrap();
                        if project(&entry.state,id,&entry.owner).as_ref()==Some(&r.candidate)&&!sent.contains(&r.candidate){
                            // Mark before writing: even if the pipe breaks after accepting the bytes, never auto-retry a reply.
                            let payload=response(&r.candidate,&r.submission).ok_or_else(unsupported)?;
                            sent.push(r.candidate);
                            if sent.len()>128{return Err(unsupported());}
                            let request=ask(&mut w,&mut seq,&client,SUBMIT,1,Some(&entry.owner),payload)?;
                            ack=Some((request,entry.owner.clone(),Instant::now()));
                        }else{let _=tx.send(Event::Feedback("问题已变化，未发送"));}
                    }
                }
                _=>{}
            }
        }
        let candidate = order
            .iter()
            .filter_map(|id| {
                entries.get(id).and_then(|e| {
                    e.supports_input
                        .then(|| project(&e.state, id, &e.owner))
                        .flatten()
                })
            })
            .find(|c| !sent.contains(c));
        if candidate.as_ref() != active.as_ref().map(|(_, c)| c) {
            active = candidate.map(|c| (NEXT_TOKEN.fetch_add(1, Ordering::Relaxed), c));
            let prompt = active.as_ref().map(|(token, c)| Prompt {
                token: *token,
                questions: c.questions.clone(),
            });
            if tx.send(Event::Prompt(prompt)).is_err() {
                return Ok(());
            }
        }
        std::thread::sleep(Duration::from_millis(60));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn raw() -> Value {
        json!({"method":FORM,"id":10,"params":{"threadId":"thread","turnId":"turn","questions":[{"id":"a","question":"请选择","isOther":false,"options":[{"label":"是"}]}]}})
    }
    #[test]
    fn only_offered_options_and_current_questions_can_be_sent() {
        let c = parse_form(&raw(), "thread", "turn", "owner").unwrap();
        let mut s = Submission {
            token: 1,
            answers: [("a".into(), vec!["是".into()])].into(),
        };
        assert_eq!(response(&c, &s).unwrap()["requestId"], 10);
        s.answers.get_mut("a").unwrap()[0] = "forged".into();
        assert!(response(&c, &s).is_none());
        let mut blocked = raw();
        blocked["method"] = json!("item/permissions/requestApproval");
        assert!(parse_form(&blocked, "thread", "turn", "owner").is_none());
        let mut blocked = raw();
        blocked["params"]["questions"][0]["isSecret"] = json!(true);
        assert!(parse_form(&blocked, "thread", "turn", "owner").is_none());
        assert!(parse_form(&raw(), "thread", "other-turn", "owner").is_none());
    }
    struct Fixture {
        queue: std::collections::VecDeque<Value>,
        sent: mpsc::Sender<Value>,
        replace_on_refresh: bool,
        snapshots: u64,
    }
    impl Transport for Fixture {
        fn emit(&mut self, m: Value) -> std::io::Result<()> {
            let method = m["method"].as_str().unwrap_or("");
            match method {
                "initialize" | "thread-owner-discovery" | SUBMIT => {
                    let result = match method {
                        "initialize" => json!({"clientId":"self"}),
                        "thread-owner-discovery" => json!({"supportsUntrustedAppInput":true}),
                        _ => {
                            self.sent.send(m.clone()).unwrap();
                            json!({"ok":true})
                        }
                    };
                    self.queue.push_back(json!({"type":"response","requestId":m["requestId"],"method":method,"resultType":"success","handledByClientId":"owner","result":result}));
                }
                "thread-stream-following-changed" => {
                    self.snapshots += 1;
                    let mut request = raw();
                    if self.replace_on_refresh && self.snapshots > 1 {
                        request["params"]["questions"][0]["question"] = json!("更新的问题");
                    }
                    self.queue.push_back(json!({"type":"broadcast","version":11,"method":"thread-stream-state-changed","targetClientIds":["self"],"sourceClientId":"owner","params":{"conversationId":"thread","hostId":"local","change":{"type":"snapshot","revision":self.snapshots,"conversationState":{"id":"thread","turns":[{"turnId":"turn","status":"inProgress"}],"requests":[request]}}}}));
                }
                _ => {}
            }
            Ok(())
        }
        fn read(&mut self) -> std::io::Result<Option<Value>> {
            Ok(self.queue.pop_front())
        }
    }
    #[test]
    fn refreshes_the_exact_request_before_sending() {
        for replace_on_refresh in [false, true] {
            let (s, rx) = mpsc::channel();
            let (tx, r) = mpsc::channel();
            let (sent, traffic) = mpsc::channel();
            let f = Fixture {
                queue: Default::default(),
                sent,
                replace_on_refresh,
                snapshots: 0,
            };
            let worker = std::thread::spawn(move || run_with(f, rx, tx, || vec!["thread".into()]));
            let Event::Prompt(Some(p)) = r.recv_timeout(Duration::from_secs(3)).unwrap() else {
                panic!("no prompt")
            };
            s.send(Submission {
                token: p.token,
                answers: [("a".into(), vec!["是".into()])].into(),
            })
            .unwrap();
            let Event::Feedback(status) = r.recv_timeout(Duration::from_secs(3)).unwrap() else {
                panic!("no result")
            };
            if replace_on_refresh {
                assert_eq!(status, "问题已变化，未发送");
                assert!(traffic.try_recv().is_err());
            } else {
                assert_eq!(status, "已发送到 Codex");
                let packet = traffic.recv_timeout(Duration::from_millis(200)).unwrap();
                assert_eq!(packet["targetClientId"], "owner");
                assert_eq!(packet["params"]["conversationId"], "thread");
                assert_eq!(
                    packet["params"]["response"]["answers"]["a"]["answers"][0],
                    "是"
                );
            }
            drop(s);
            worker.join().unwrap().unwrap();
        }
    }
    #[test]
    fn patches_cannot_skip_missing_state() {
        let mut data = json!({"requests":[raw()]});
        assert!(apply_patches(
            &mut data,
            &json!([{"op":"replace","path":["missing",0],"value":null}])
        )
        .is_none());
        assert!(
            apply_patches(&mut data, &json!([{"op":"remove","path":["requests",0]}])).is_some()
        );
        assert_eq!(data["requests"], json!([]));
    }
}
