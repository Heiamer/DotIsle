//! Isolated SMTC publisher to the production backend, local parser and display frame.
use std::{path::PathBuf,time::{Duration,Instant}};
use serde_json::{json,Value};
use dotisle_music::runtime::Backend;
use windows::{core::{factory,w,HSTRING},Foundation::TimeSpan,Media::{SystemMediaTransportControls,SystemMediaTransportControlsTimelineProperties,MediaPlaybackType,MediaPlaybackStatus},Win32::{System::WinRT::{RoInitialize,RO_INIT_MULTITHREADED,ISystemMediaTransportControlsInterop},UI::WindowsAndMessaging::*}};
fn await_frame(b:&Backend,check:impl Fn(&Value)->bool)->Result<Value,Box<dyn std::error::Error>>{
 let until=Instant::now()+Duration::from_secs(9);loop{let s=b.snapshot(u64::MAX);if check(&s){return Ok(s)}if Instant::now()>until{return Err(format!("isolated lyric pipeline timeout: {s}").into())}std::thread::sleep(Duration::from_millis(60));}
}
fn main()->Result<(),Box<dyn std::error::Error>>{unsafe{
 RoInitialize(RO_INIT_MULTITHREADED)?;
 let out=PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);std::fs::create_dir_all(out.join("local"))?;
 std::fs::write(out.join("local/织梦者 - 晨雾.lrc"),"[00:01.00]<00:01.00>清晨<00:05.00>第一束光<00:09.00>\n[00:10.00]向海面出发")?;
 std::fs::write(out.join("local/织梦者 - 晚风.lrc"),"[00:01.00]另一首歌的晚风")?;
 let hwnd=CreateWindowExW(WINDOW_EX_STYLE::default(),w!("STATIC"),w!("DotIsle isolated lyric pipeline"),WS_OVERLAPPED,0,0,50,50,None,None,None,None)?;
 let interop:ISystemMediaTransportControlsInterop=factory::<SystemMediaTransportControls,ISystemMediaTransportControlsInterop>()?;
 let controls:SystemMediaTransportControls=interop.GetForWindow(hwnd)?;controls.SetIsEnabled(true)?;controls.SetIsPlayEnabled(true)?;controls.SetIsPauseEnabled(true)?;
 let display=controls.DisplayUpdater()?;display.SetType(MediaPlaybackType::Music)?;let music=display.MusicProperties()?;music.SetTitle(&HSTRING::from("晨雾"))?;music.SetArtist(&HSTRING::from("织梦者"))?;display.Update()?;
 let timeline=|seconds:i64|->windows::core::Result<()>{let p=SystemMediaTransportControlsTimelineProperties::new()?;p.SetStartTime(TimeSpan{Duration:0})?;p.SetEndTime(TimeSpan{Duration:30*10000000})?;p.SetMinSeekTime(TimeSpan{Duration:0})?;p.SetMaxSeekTime(TimeSpan{Duration:30*10000000})?;p.SetPosition(TimeSpan{Duration:seconds*10000000})?;controls.UpdateTimelineProperties(&p)};
 timeline(3)?;controls.SetPlaybackStatus(MediaPlaybackStatus::Playing)?;
 let b=Backend::new(json!({"AllowedSources":["verify_lyric_pipeline"],"IgnoredSources":[],"LocalFolders":[out.join("local")],"OnlineSongInfo":false}),out.join("Data"),out.join("LocalAppData"),out.join("Logs"));
 let first=await_frame(&b,|s|s["frame"]["currentLine"]=="清晨第一束光"&&s["frame"]["wordScanProgress"].as_f64().is_some())?;
 timeline(12)?;
 let seek=await_frame(&b,|s|s["frame"]["currentLine"]=="向海面出发"&&s["frame"]["previousLine"]=="清晨第一束光")?;
 controls.SetPlaybackStatus(MediaPlaybackStatus::Paused)?;
 let paused=await_frame(&b,|s|s["isPlaying"]==false)?;std::thread::sleep(Duration::from_millis(650));let after=b.snapshot(u64::MAX);
 assert!((paused["positionSeconds"].as_f64().unwrap()-after["positionSeconds"].as_f64().unwrap()).abs()<0.1,"paused frame drifted");
 music.SetTitle(&HSTRING::from("晚风"))?;display.Update()?;timeline(2)?;
 let changed=await_frame(&b,|s|s["track"]["title"]=="晚风"&&s["frame"]["currentLine"]=="另一首歌的晚风")?;
 b.close();controls.SetIsEnabled(false)?;DestroyWindow(hwnd)?;
 std::fs::write(out.join("report.json"),serde_json::to_vec_pretty(&json!({"success":true,"source":"isolated hidden SMTC fixture","localLyric":true,"realWordProgress":first["frame"]["wordScanProgress"],"seekFrame":seek["frame"],"pauseStable":true,"changedFrame":changed["frame"],"userPlayerCommands":0}))?)?;
 println!("PASS: isolated SMTC -> backend -> local lyric -> word progress, seek, pause and track change");Ok(())
}}
