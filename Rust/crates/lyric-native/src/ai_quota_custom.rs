// SPDX-License-Identifier: Apache-2.0
//! A restricted host for the BrimDeck fetchUsage(ctx) contract. A script is
//! explicitly entered by the user, sees only its fixed JSON endpoint's reply,
//! and has no native callbacks for network, files, shell, IPC or secrets.
use super::{metric, num, quota, text, time, valid_token, Metric, Value};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Deserialize;
use std::{
    cell::RefCell, collections::VecDeque, ffi::c_void, mem::transmute_copy, ptr::null_mut,
    sync::mpsc, thread, time::Duration,
};
use windows::{
    core::{s, w, PCSTR},
    Win32::{
        Foundation::{FreeLibrary, HMODULE},
        System::LibraryLoader::{GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32},
    },
};

const INVALID: &str = "自定义脚本无效、超时或系统脚本引擎不可用";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    code: String,
    #[serde(default)]
    bearer: String,
}

pub(super) fn accepts_site(site: &str) -> bool {
    if site.is_empty() {
        return true;
    }
    if site.len() > 512
        || !site
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".:/_-~".contains(&b))
    {
        return false;
    }
    let Some(tail) = site.strip_prefix("https://") else {
        return false;
    };
    let (host, path) = tail.split_once('/').unwrap_or((tail, ""));
    host.contains('.')
        && host.len() <= 253
        && host.split('.').all(|p| {
            !p.is_empty()
                && p.len() <= 63
                && !p.starts_with('-')
                && !p.ends_with('-')
                && p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
        && !path.split('/').any(|p| matches!(p, "." | ".."))
        && !path.contains("//")
}
pub(super) fn read(
    site: &str,
    blob: &str,
    get: &dyn Fn(&str, &[(&str, &str)]) -> Result<Value, String>,
) -> Result<(Vec<Metric>, String), String> {
    let mut bytes = URL_SAFE_NO_PAD.decode(blob).map_err(|_| INVALID)?;
    let cfg = serde_json::from_slice::<Config>(&bytes);
    bytes.fill(0);
    let cfg = cfg.map_err(|_| INVALID)?;
    if !accepts_site(site)
        || cfg.code.trim().is_empty()
        || cfg.code.len() > 4096
        || cfg.code.contains('\0')
        || (!cfg.bearer.is_empty()
            && (site.is_empty() || cfg.bearer.len() > 1024 || !valid_token(&cfg.bearer)))
    {
        return Err(INVALID.into());
    }
    let engine = EngineApi::load().ok_or(INVALID)?;
    let body = if site.is_empty() {
        "null".into()
    } else {
        let auth = format!("Bearer {}", cfg.bearer);
        let headers = if cfg.bearer.is_empty() {
            &[][..]
        } else {
            &[("Authorization", auth.as_str())][..]
        };
        let value = get(site, headers)?.to_string();
        if value.len() > 192 * 1024 || (!cfg.bearer.is_empty() && value.contains(&cfg.bearer)) {
            return Err(INVALID.into());
        }
        value
    };
    let output = engine.run(site, &body, &cfg.code).ok_or(INVALID)?;
    let doc = serde_json::from_str::<Value>(&output).map_err(|_| INVALID)?;
    let mut rows = Vec::new();
    for row in doc.as_array().ok_or(INVALID)?.iter().take(16) {
        let name = text(&row["title"]);
        if name.trim().is_empty() {
            continue;
        }
        let used = num(&row["percent"]).filter(|x| (0. ..=100.).contains(x));
        // Interpret only explicitly named, structured period lengths.
        let window = row["windowMinutes"]
            .as_u64()
            .filter(|x| matches!(*x, 300 | 10080));
        let mut m = if let Some(used) = used {
            quota(&name, window, used, time(&row["resetAt"]))
        } else {
            metric(&name, String::new())
        };
        let mut own = text(&row["value"]);
        if own.is_empty() {
            own = text(&row["note"]);
        }
        if !own.is_empty() {
            m.value = own;
        }
        if m.value.is_empty() {
            continue;
        }
        if !cfg.bearer.is_empty() {
            m.label = m.label.replace(&cfg.bearer, "••••");
            m.value = m.value.replace(&cfg.bearer, "••••");
        }
        rows.push(m);
    }
    if rows.is_empty() {
        return Err("未返回可识别的额度".into());
    }
    Ok((rows, String::new()))
}

type Ref = *mut c_void;
type Code = u32;
type Notify = unsafe extern "system" fn(Ref, Ref);
type Api1 = unsafe extern "system" fn(Ref) -> Code;
type Root = unsafe extern "system" fn(Ref, *mut u32) -> Code;
struct Library(HMODULE);
impl Drop for Library {
    fn drop(&mut self) {
        unsafe {
            let _ = FreeLibrary(self.0);
        }
    }
}
struct EngineApi {
    _dll: Library,
    runtime: unsafe extern "system" fn(u32, Ref, *mut Ref) -> Code,
    context: unsafe extern "system" fn(Ref, *mut Ref) -> Code,
    activate: Api1,
    limit: unsafe extern "system" fn(Ref, usize) -> Code,
    interrupt: Api1,
    dispose: Api1,
    continuation: unsafe extern "system" fn(Option<Notify>, Ref) -> Code,
    run: unsafe extern "system" fn(*const u16, usize, *const u16, *mut Ref) -> Code,
    undefined: unsafe extern "system" fn(*mut Ref) -> Code,
    call: unsafe extern "system" fn(Ref, *const Ref, u16, *mut Ref) -> Code,
    string: unsafe extern "system" fn(Ref, *mut *const u16, *mut usize) -> Code,
    add_ref: Root,
    release: Root,
}
impl Library {
    fn get<T: Copy>(&self, name: PCSTR) -> Option<T> {
        let p = unsafe { GetProcAddress(self.0, name) }?;
        if std::mem::size_of::<T>() != std::mem::size_of_val(&p) {
            return None;
        }
        Some(unsafe { transmute_copy(&p) })
    }
}
impl EngineApi {
    fn load() -> Option<Self> {
        let lib = Library(
            unsafe { LoadLibraryExW(w!("chakra.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32) }.ok()?,
        );
        Some(Self {
            runtime: lib.get(s!("JsCreateRuntime"))?,
            context: lib.get(s!("JsCreateContext"))?,
            activate: lib.get(s!("JsSetCurrentContext"))?,
            limit: lib.get(s!("JsSetRuntimeMemoryLimit"))?,
            interrupt: lib.get(s!("JsDisableRuntimeExecution"))?,
            dispose: lib.get(s!("JsDisposeRuntime"))?,
            continuation: lib.get(s!("JsSetPromiseContinuationCallback"))?,
            run: lib.get(s!("JsRunScript"))?,
            undefined: lib.get(s!("JsGetUndefinedValue"))?,
            call: lib.get(s!("JsCallFunction"))?,
            string: lib.get(s!("JsStringToPointer"))?,
            add_ref: lib.get(s!("JsAddRef"))?,
            release: lib.get(s!("JsRelease"))?,
            _dll: lib,
        })
    }
    fn run(&self, site: &str, input: &str, user: &str) -> Option<String> {
        let site = serde_json::to_string(site).ok()?;
        let input = serde_json::to_string(input).ok()?;
        let source = format!(
            r#"
            var __dotisle_out = '';
            (function () {{
                'use strict';
                var encode = JSON.stringify.bind(JSON), decode = JSON.parse.bind(JSON), freeze = Object.freeze.bind(Object);
                var address = {site}, raw = {input};
                var ctx = freeze({{site:address, language:'zh-CN', secrets:freeze({{}}),
                    data:decode(raw), http:freeze({{get:function (url, options) {{
                        if (!address || url !== address || (options && Object.keys(options).length)) throw new Error('Only the approved JSON URL can be read');
                        return {{status:200, headers:freeze({{}}), json:function(){{return decode(raw);}}, text:function(){{return raw;}}}};
                    }}}})
                }});
                var fetcher = (function () {{
                    {user}
                    return typeof fetchUsage === 'function' ? fetchUsage : null;
                }})();
                if (fetcher) Promise.resolve(fetcher(ctx)).then(function (data) {{ __dotisle_out = encode(data); }},function(){{}});
            }})();
            "#
        );
        let mut runtime = null_mut();
        // No native code generation, eval, debugger or browser host objects. The
        // watchdog may interrupt scripts even if they never yield to the host.
        if unsafe { (self.runtime)(1 | 2 | 8 | 16, null_mut(), &mut runtime) } != 0 {
            return None;
        }
        let guard = Runtime {
            api: self,
            ptr: runtime,
        };
        if unsafe { (self.limit)(runtime, 16 * 1024 * 1024) } != 0 {
            return None;
        }
        let mut context = null_mut();
        if unsafe { (self.context)(runtime, &mut context) } != 0
            || unsafe { (self.activate)(context) } != 0
        {
            return None;
        }
        let jobs = Jobs {
            items: RefCell::new(VecDeque::with_capacity(128)),
            add_ref: self.add_ref,
        };
        if unsafe { (self.continuation)(Some(schedule), (&jobs as *const Jobs).cast_mut().cast()) }
            != 0
        {
            return None;
        }
        let interrupt = self.interrupt;
        let raw = runtime as usize;
        let result = thread::scope(|scope| {
            let (done, wait) = mpsc::sync_channel(1);
            scope.spawn(move || {
                if wait.recv_timeout(Duration::from_millis(400)).is_err() {
                    unsafe { interrupt(raw as Ref) };
                }
            });
            let result = self.run_inner(&source, &jobs);
            let _ = done.send(());
            result
        });
        unsafe {
            let _ = (self.continuation)(None, null_mut());
            for job in jobs.items.into_inner() {
                (self.release)(job, null_mut());
            }
        }
        drop(guard);
        result
    }
    fn run_inner(&self, source: &str, jobs: &Jobs) -> Option<String> {
        let evaluate = |src: &str, id| {
            let src = src.encode_utf16().chain([0]).collect::<Vec<_>>();
            let mut v = null_mut();
            (unsafe { (self.run)(src.as_ptr(), id, w!("dotisle-quota.js").as_ptr(), &mut v) } == 0)
                .then_some(v)
        };
        evaluate(source, 1)?;
        let mut this = null_mut();
        if unsafe { (self.undefined)(&mut this) } != 0 {
            return None;
        }
        for _ in 0..128 {
            let Some(job) = jobs.items.borrow_mut().pop_front() else {
                break;
            };
            let error = unsafe { (self.call)(job, [this].as_ptr(), 1, null_mut()) };
            unsafe {
                (self.release)(job, null_mut());
            }
            if error != 0 {
                return None;
            }
        }
        let v = evaluate("__dotisle_out", 2)?;
        let (mut p, mut len) = (std::ptr::null(), 0);
        if unsafe { (self.string)(v, &mut p, &mut len) } != 0
            || p.is_null()
            || len == 0
            || len > 32 * 1024
        {
            return None;
        }
        String::from_utf16(unsafe { std::slice::from_raw_parts(p, len) }).ok()
    }
}
struct Runtime<'a> {
    api: &'a EngineApi,
    ptr: Ref,
}
impl Drop for Runtime<'_> {
    fn drop(&mut self) {
        unsafe {
            (self.api.activate)(null_mut());
            (self.api.dispose)(self.ptr);
        }
    }
}
struct Jobs {
    items: RefCell<VecDeque<Ref>>,
    add_ref: Root,
}
unsafe extern "system" fn schedule(task: Ref, state: Ref) {
    let state = unsafe { &*(state as *const Jobs) };
    let mut queued = state.items.borrow_mut();
    if queued.len() < 128 && unsafe { (state.add_ref)(task, null_mut()) } == 0 {
        queued.push_back(task);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn custom_fetch_usage_can_read_only_its_statically_approved_input() {
        let s=URL_SAFE_NO_PAD.encode(json!({"code":"async function fetchUsage(ctx) { const response = await ctx.http.get(ctx.site); return [{title:'Fixture', percent:response.json().used, windowMinutes:300}]; }", "bearer":"test-only-secret"}).to_string());
        let (m, _) = read("https://quota.example/usage", &s, &|url, headers| {
            assert_eq!(url, "https://quota.example/usage");
            assert_eq!(headers, &[("Authorization", "Bearer test-only-secret")]);
            Ok(json!({"used":90}))
        })
        .unwrap();
        assert_eq!(m[0].remaining, Some(10.));
        assert_eq!(m[0].window, Some(300));
        assert!(m[0].relay_money.is_none());
        let e = EngineApi::load().unwrap();
        assert!(e
            .run(
                "https://quota.example/usage",
                "null",
                "function fetchUsage(ctx){ return ctx.http.get('https://evil.example/'); }"
            )
            .is_none());
        assert!(e
            .run("", "null", "function fetchUsage(){ return eval('1'); }")
            .is_none());
    }
    #[test]
    fn non_terminating_script_has_a_wall_clock_budget() {
        let e = EngineApi::load().unwrap();
        let start = std::time::Instant::now();
        assert!(e
            .run("", "null", "function fetchUsage(){while (true){} }")
            .is_none());
        assert!(start.elapsed() < Duration::from_secs(3));
    }
}
