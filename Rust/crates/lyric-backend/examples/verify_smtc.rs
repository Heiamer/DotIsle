//! Isolated Windows SMTC publisher: exercises production metadata and artwork reading.
use std::{path::PathBuf,time::{Duration,Instant}};
use windows::{core::{factory,w,HSTRING},Win32::{System::WinRT::{RoInitialize,RO_INIT_MULTITHREADED,ISystemMediaTransportControlsInterop},UI::WindowsAndMessaging::*},Media::{SystemMediaTransportControls,MediaPlaybackType,MediaPlaybackStatus},Storage::{StorageFile,Streams::RandomAccessStreamReference}};
use lyric_backend::media::{Media,MediaSample,SourceFilter};
macro_rules! wait{($op:expr)=>{{let op=$op?;let until=Instant::now()+Duration::from_secs(3);while op.Status()?.0==0{if Instant::now()>until{let _=op.Cancel();return Err("SMTC fixture async operation timed out".into())}std::thread::sleep(Duration::from_millis(10));}op.GetResults()?}};}
fn main()->Result<(),Box<dyn std::error::Error>>{unsafe{
    RoInitialize(RO_INIT_MULTITHREADED)?;
    let out=PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);std::fs::create_dir_all(&out)?;
    let cover_a=PathBuf::from(std::env::args().nth(2).ok_or("cover A required")?);let cover_b=PathBuf::from(std::env::args().nth(3).ok_or("cover B required")?);
    let window=CreateWindowExW(WINDOW_EX_STYLE::default(),w!("STATIC"),w!("DotIsle isolated SMTC regression"),WS_OVERLAPPED,0,0,100,100,None,None,None,None)?;
    let interop:ISystemMediaTransportControlsInterop=factory::<SystemMediaTransportControls,ISystemMediaTransportControlsInterop>()?;let controls:SystemMediaTransportControls=interop.GetForWindow(window)?;
    controls.SetIsEnabled(true)?;controls.SetIsPlayEnabled(true)?;controls.SetIsPauseEnabled(true)?;controls.SetPlaybackStatus(MediaPlaybackStatus::Playing)?;
    let display=controls.DisplayUpdater()?;display.SetType(MediaPlaybackType::Music)?;let music=display.MusicProperties()?;music.SetTitle(&HSTRING::from("First track"))?;music.SetArtist(&HSTRING::from("Fixture singer"))?;
    let set_cover=|path:&PathBuf|->Result<(),Box<dyn std::error::Error>>{let file=wait!(StorageFile::GetFileFromPathAsync(&HSTRING::from(path.to_string_lossy().as_ref())));display.SetThumbnail(&RandomAccessStreamReference::CreateFromFile(&file)?)?;display.Update()?;Ok(())};
    set_cover(&cover_a)?;
    let mut reader=Media::new()?;let filter=SourceFilter{only:vec!["verify_smtc".into()],ignored:vec![]};
    let mut await_sample=|predicate:&dyn Fn(&MediaSample)->bool|->Result<MediaSample,Box<dyn std::error::Error>>{let until=Instant::now()+Duration::from_secs(8);loop{let sample=reader.read(&filter)?;if predicate(&sample){return Ok(sample)}if Instant::now()>until{return Err(format!("SMTC fixture timeout: {:?}",sample.playback.track).into())}std::thread::sleep(Duration::from_millis(60));}};
    let first=await_sample(&|s|s.playback.track.as_ref().is_some_and(|t|t.title=="First track")&&s.playback.cover_image_bytes.is_some())?;let a=first.playback.cover_image_bytes.unwrap();
    music.SetTitle(&HSTRING::from("Second track"))?;display.Update()?;
    let switched=await_sample(&|s|s.playback.track.as_ref().is_some_and(|t|t.title=="Second track"))?;
    let old_cleared=switched.playback.cover_image_bytes.as_ref()!=Some(&a);assert!(old_cleared,"old thumbnail survived the track boundary");
    // Artwork arrives after metadata, then changes again without changing any text.
    std::thread::sleep(Duration::from_millis(400));set_cover(&cover_b)?;
    let second=await_sample(&|s|s.playback.track.as_ref().is_some_and(|t|t.title=="Second track")&&s.playback.cover_image_bytes.as_ref().is_some_and(|b|b!=&a))?;let b=second.playback.cover_image_bytes.unwrap();assert_ne!(a,b);
    set_cover(&cover_a)?;await_sample(&|s|s.playback.cover_image_bytes.as_ref()==Some(&a))?;
    controls.SetPlaybackStatus(MediaPlaybackStatus::Stopped)?;let stopped=await_sample(&|s|s.status=="Stopped")?;assert!(stopped.playback.track.is_some());
    controls.SetIsEnabled(false)?;DestroyWindow(window)?;
    std::fs::write(out.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"success":true,"oldCoverClearedOnTrackChange":old_cleared,"delayedThumbnailArrived":true,"sameTrackThumbnailRefresh":true,"stoppedMetadataPreserved":true}))?)?;
    Ok(())
}}
