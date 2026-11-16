//! Real updater transport, small metadata only. No installation or package data.
use lyric_bootstrap::{http,module_update};
use serde_json::json;
fn main()->Result<(),Box<dyn std::error::Error>>{
    let args=std::env::args().collect::<Vec<_>>();
    let version=args.get(1).ok_or("version required")?;
    let out=args.get(2).ok_or("report required")?;
    let root=format!("https://github.com/Heiamer/DotIsle/releases/download/v{version}/LyricIsland-{version}");
    let hashes=http::get_bytes(&format!("{root}-SHA256.txt"),2*1024*1024)?;
    let text=std::str::from_utf8(&hashes)?;
    assert!(text.lines().any(|s|s.split_whitespace().nth(1)==Some(format!("LyricIsland-{version}-Portable.zip").as_str())),"portable hash missing");
    let expected=text.lines().find_map(|s|{let mut p=s.split_whitespace();let hash=p.next()?;(p.next()?==format!("LyricIsland-{version}-Modules.json")).then_some(hash)});
    let component=if let Some(expected)=expected{
        let bytes=http::get_bytes(&format!("{root}-Modules.json"),4*1024*1024)?;
        assert_eq!(lyric_archive::sha256_bytes(&bytes),expected);
        let index:serde_json::Value=serde_json::from_slice(&bytes)?;
        module_update::validate(&index,version)?;
        json!({"available":true,"hashVerified":true,"packages":index["packages"].as_array().map(Vec::len)})
    }else{json!({"available":false,"fallback":"full update"})};
    std::fs::write(out,serde_json::to_vec_pretty(&json!({"success":true,"version":version,"checksumBytes":hashes.len(),"componentIndex":component,"downloadedApplicationPackages":0,"installed":false}))?)?;
    Ok(())
}
