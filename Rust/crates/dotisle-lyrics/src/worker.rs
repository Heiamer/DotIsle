//! Multiplexed IPC with the windowless Go lyric source worker.
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::HashMap, io::{BufRead, BufReader, Read, Write}, os::windows::process::CommandExt, path::{Path,PathBuf}, process::{Command, Stdio}, sync::{Arc, LazyLock, Mutex, atomic::{AtomicBool, AtomicU64, Ordering}, mpsc}, time::{Duration, Instant}};

#[derive(Clone)]
pub struct RequestContext { pub cancelled:Arc<AtomicBool>, pub deadline:Instant }
impl RequestContext {
 pub fn new(timeout:Duration)->Self{Self{cancelled:Arc::new(AtomicBool::new(false)),deadline:Instant::now()+timeout}}
 pub fn check(&self)->Result<(),String>{if self.cancelled.load(Ordering::Acquire){Err("cancelled".into())}else if Instant::now()>=self.deadline{Err("timeout".into())}else{Ok(())}}
}

const MAX_RESPONSE:u64=12*1024*1024;
static NEXT:AtomicU64=AtomicU64::new(1);
static WORKER:LazyLock<Mutex<Option<Arc<Worker>>>>=LazyLock::new(Default::default);
type Reply=Result<Value,String>;
type Pending=Mutex<HashMap<u64,mpsc::SyncSender<Reply>>>;
struct Worker{outgoing:mpsc::SyncSender<Value>,pending:Arc<Pending>,dead:Arc<AtomicBool>}

#[derive(Deserialize)]
#[serde(rename_all="PascalCase")]
struct Envelope{#[serde(rename="ID")]id:u64, #[serde(default)]error:String, #[serde(default)]hits:Value, #[serde(default)]payload:Value}

impl Worker {
 fn start(path:&Path)->Result<Self,String>{
  let mut child=Command::new(path).arg("--lyrics-worker")
    .creation_flags(windows::Win32::System::Threading::CREATE_NO_WINDOW.0)
    .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e|e.to_string())?;
  let (mut input,output)=(child.stdin.take().unwrap(),child.stdout.take().unwrap());
  let (outgoing,rx)=mpsc::sync_channel::<Value>(64);
  let pending=Arc::new(Pending::default());let dead=Arc::new(AtomicBool::new(false));
  let p=pending.clone();let d=dead.clone();
  std::thread::spawn(move||{
   let mut reader=BufReader::new(output);let mut line=Vec::new();
   loop{
    line.clear();if reader.by_ref().take(MAX_RESPONSE).read_until(b'\n',&mut line).unwrap_or(0)==0 || line.last()!=Some(&b'\n'){break}
    let Ok(reply)=serde_json::from_slice::<Envelope>(&line)else{break};
    if let Some(tx)=p.lock().unwrap().remove(&reply.id){let _=tx.send(if reply.error.is_empty(){Ok(json!({"hits":reply.hits,"payload":reply.payload}))}else{Err(reply.error)});}
   }
   d.store(true,Ordering::Release);p.lock().unwrap().clear();
   // A broken protocol must not leave a live process or a blocked writer behind.
   let _=child.kill();let _=child.wait();
  });
  std::thread::spawn(move||for message in rx{
   if serde_json::to_writer(&mut input,&message).is_err()||input.write_all(b"\n").is_err(){break}
  });
  Ok(Self{outgoing,pending,dead})
 }
 fn call(&self,mut message:Value,ctx:&RequestContext)->Reply{
  ctx.check()?;
  let id=NEXT.fetch_add(1,Ordering::Relaxed);
  message["ID"]=json!(id);message["Version"]=json!(1);
  message["TimeoutMS"]=json!(ctx.deadline.saturating_duration_since(Instant::now()).as_millis().min(12000));
  let(tx,rx)=mpsc::sync_channel(1);
  self.pending.lock().unwrap().insert(id,tx);
  if self.outgoing.try_send(message).is_err(){self.pending.lock().unwrap().remove(&id);return Err("lyric worker unavailable or busy".into())}
  loop{
   if let Err(e)=ctx.check(){self.pending.lock().unwrap().remove(&id);let _=self.outgoing.try_send(json!({"ID":id,"Version":1,"Op":"cancel"}));return Err(e)}
   match rx.recv_timeout(Duration::from_millis(30)){
    Ok(reply)=>return reply,
    Err(mpsc::RecvTimeoutError::Disconnected)=>return Err("lyric worker exited".into()),
    Err(mpsc::RecvTimeoutError::Timeout)=>continue,
   }
  }
 }
}

fn worker_path()->Result<PathBuf,String>{
 let host=std::env::current_exe().map_err(|e|e.to_string())?;
 let dir=host.parent().ok_or("missing app directory")?;
 // Cargo puts the standalone diagnostics under release/examples.
 let dir=if dir.file_name().is_some_and(|n| n=="examples"){dir.parent().unwrap_or(dir)}else{dir};
 Ok(dir.join("LyricIsland.Center.exe"))
}
pub fn call(request:Value,ctx:&RequestContext)->Reply{
 ctx.check()?;
 let worker={let mut slot=WORKER.lock().map_err(|_|"lyric worker lock")?;
  if slot.as_ref().is_none_or(|w|w.dead.load(Ordering::Acquire)){*slot=Some(Arc::new(Worker::start(&worker_path()?)?));}
  slot.as_ref().unwrap().clone()
 };
 worker.call(request,ctx)
}
