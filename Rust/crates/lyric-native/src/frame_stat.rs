//! Present-time histogram gated by `DOTISLE_FPS_LOG`; only the audit run pays for it.
use std::sync::{Mutex,OnceLock};
static PATH:OnceLock<Option<std::path::PathBuf>>=OnceLock::new();
static TICK:OnceLock<Mutex<(std::time::Instant,usize)>>=OnceLock::new();
pub fn tick(){
    let Some(path)=PATH.get_or_init(||std::env::var_os("DOTISLE_FPS_LOG").map(std::path::PathBuf::from)).as_deref() else{return};
    let Ok(mut window)=TICK.get_or_init(||Mutex::new((std::time::Instant::now(),0))).lock() else{return};
    window.1+=1;
    let ms=window.0.elapsed().as_secs_f64()*1000.;
    if ms>=1000.{
        use std::io::Write;
        if let Ok(mut file)=std::fs::OpenOptions::new().create(true).append(true).open(path){let _=writeln!(file,"{{\"frames\":{},\"elapsed_ms\":{:.1}}}",window.1,ms);}
        *window=(std::time::Instant::now(),0);
    }
}
