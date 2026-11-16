//! Check update data only; never download or install an application package.
use lyric_backend::updates;
use serde_json::json;
use std::path::PathBuf;
fn main()->Result<(),Box<dyn std::error::Error>>{
    let out=PathBuf::from(std::env::args().nth(1).ok_or("report path required")?);
    if let Some(current)=std::env::args().nth(2){updates::set_runtime_version(&current);}
    let stable=updates::check_channel("stable")?;
    std::fs::write(&out,serde_json::to_vec_pretty(&json!({"success":false,"stable":stable,"pending":"beta"}))?)?;
    let beta=updates::check_channel("beta")?;
    // The updater has its own transport for checksums and module indexes; use
    // verify_update_manifest there instead of substituting this JSON adapter.
    std::fs::write(out,serde_json::to_vec_pretty(&json!({"success":true,"stable":stable,"beta":beta,"downloadedPackages":0,"installed":false}))?)?;
    Ok(())
}
