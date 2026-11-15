//! Shared host storage, transport and first-party module adapters.
pub mod updates;pub mod http;pub mod storage;pub mod io;pub mod shell;pub mod ipc;pub mod runtime;
pub mod netease_control{pub fn attention_diagnostics()->serde_json::Value{dotisle_module_api::call("music","attention",serde_json::json!({})).unwrap_or_else(|e|serde_json::json!({"error":e}))}}
pub mod media{pub fn control_diagnostics()->Result<serde_json::Value,String>{dotisle_module_api::call("music","diagnostics",serde_json::json!({}))}}
