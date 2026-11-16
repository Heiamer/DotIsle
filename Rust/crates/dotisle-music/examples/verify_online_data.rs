//! Opt-in real requests through the same metadata/artwork HTTP adapters as music.
use dotisle_music::{netease,http::{self,RequestContext}};
use serde_json::json;
use std::{path::PathBuf,time::Duration};
fn main()->Result<(),Box<dyn std::error::Error>>{
    let out=PathBuf::from(std::env::args().nth(1).ok_or("report path required")?);
    let detail=netease::track_detail("347230")?;
    assert_eq!(detail["id"],347230);assert!(!detail["name"].as_str().unwrap_or("").is_empty());
    let cover=detail["album"]["picUrl"].as_str().ok_or("artwork URL missing")?;
    let rest=cover.strip_prefix("https://").or_else(||cover.strip_prefix("http://")).ok_or("invalid artwork URL")?;
    let host=rest.split('/').next().unwrap_or("");assert!(host.ends_with(".music.126.net"));
    let bytes=http::request(&format!("https://{rest}?param=300y300"),"GET",&[],"","",&RequestContext::new(Duration::from_secs(5)),4*1024*1024)?;
    assert!(bytes.starts_with(&[0xff,0xd8])||bytes.starts_with(b"\x89PNG"),"unexpected artwork image format");
    std::fs::write(out,serde_json::to_vec_pretty(&json!({"success":true,"songId":detail["id"],"title":detail["name"],"artworkBytes":bytes.len(),"imageSignatureValid":true,"playerRequired":false}))?)?;
    Ok(())
}
