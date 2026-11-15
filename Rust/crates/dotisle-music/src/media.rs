//! Native SMTC sessions. Every WinRT operation has an explicit bounded wait.
use std::{sync::{Arc,atomic::{AtomicU64,Ordering}},time::{Duration,Instant,SystemTime,UNIX_EPOCH},sync::mpsc};
use windows::{core::Result,Foundation::TypedEventHandler,Media::Control::*,Storage::Streams::{DataReader,InputStreamOptions}};
use lyric_core::models::{Track,Playback};

macro_rules! wait {
    ($operation:expr)=>{{
        let operation=$operation?;let deadline=Instant::now()+Duration::from_secs(2);
        while operation.Status()?.0==0{
            if Instant::now()>deadline{let _=operation.Cancel();return Err(windows::core::Error::from_hresult(windows::core::HRESULT(0x800705B4u32 as i32)))}
            std::thread::sleep(Duration::from_millis(5));
        }
        operation.GetResults()?
    }};
}
pub fn unix_ms()->i64{SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as i64}
/// Read-only diagnostics: report the capabilities advertised by every session.
/// Never issues a seek, play/pause or other media command.
pub fn control_diagnostics()->Result<serde_json::Value>{
    unsafe{let _=windows::Win32::System::WinRT::RoInitialize(windows::Win32::System::WinRT::RO_INIT_MULTITHREADED);}
    let manager=wait!(GlobalSystemMediaTransportControlsSessionManager::RequestAsync());
    let current=manager.GetCurrentSession().ok();let sessions=manager.GetSessions()?;let mut rows=vec![];
    for i in 0..sessions.Size()?{let s=sessions.GetAt(i)?;let info=s.GetPlaybackInfo()?;let timeline=s.GetTimelineProperties()?;
        rows.push(serde_json::json!({"sourceId":s.SourceAppUserModelId()?.to_string(),"isCurrent":current.as_ref()==Some(&s),"status":info.PlaybackStatus()?.0,"canSeek":info.Controls()?.IsPlaybackPositionEnabled()?,"positionSeconds":timeline.Position()?.Duration as f64/10000000.,"startSeconds":timeline.StartTime()?.Duration as f64/10000000.,"endSeconds":timeline.EndTime()?.Duration as f64/10000000.}));
    }Ok(serde_json::json!({"readOnly":true,"sessions":rows}))
}
pub fn normalize_source(source:&str)->String{
    let s=source.to_lowercase();
    for (needles,name) in [(&["qqmusic","qq音乐"][..],"QQMusic"),(&["cloudmusic","netease","163music","music.163","wyy"][..],"Netease"),(&["kugou","kugoomusic","酷狗"][..],"Kugou"),(&["kwmusic","kuwo","酷我"][..],"Kuwo"),(&["spotify"][..],"Spotify"),(&["sodamusic","汽水音乐"][..],"SodaMusic"),(&["migu","咪咕"][..],"Migu"),(&["applemusic","apple music"][..],"AppleMusic"),(&["youtubemusic","youtube music"][..],"YouTubeMusic"),(&["potplayer"][..],"PotPlayer"),(&["saltplayer","salt player"][..],"SaltPlayer"),(&["lx-music","lxmusic"][..],"LXMusic"),(&["musicfree"][..],"MusicFree"),(&["yesplaymusic"][..],"YesPlayMusic"),(&["cider"][..],"Cider"),(&["foobar"][..],"Foobar2000"),(&["musicbee"][..],"MusicBee"),(&["aimp"][..],"AIMP"),(&["chrome","msedge","firefox","browser","opera","brave","vivaldi"][..],"Browser")]{
        if needles.iter().any(|n|s.contains(n)){return name.into()}
    }
    source.into()
}
pub fn normalize_metadata(source:&str,title:&str,artist:&str)->(String,String){
    let title=title.trim();let artist=artist.trim();
    if artist.is_empty()&&["Kugou","QQMusic","Kuwo","SodaMusic"].contains(&source){
        for separator in [" - "," – "," — "]{if let Some((singer,song))=title.split_once(separator){if !singer.trim().is_empty()&&!song.trim().is_empty(){return(song.trim().into(),singer.trim().into())}}}
    }
    (title.into(),artist.into())
}
/// Which media sessions may drive the island.
///
/// Expressed as exclusions, not as an allow-list: a session responds unless its source is named in
/// `ignored`. That is what lets a player nobody has heard of work without being added first — the
/// old allow-list silently ignored 汽水音乐, PotPlayer and every local player that was not one of the
/// six defaults. `only` is the diagnostic counterpart: the verification harnesses pin the island to
/// a single session with it, and it stays empty in normal operation.
#[derive(Debug,Clone,Default)]
pub struct SourceFilter{pub ignored:Vec<String>,pub only:Vec<String>}
impl SourceFilter{
    pub fn allows(&self,source:&str)->bool{
        if blocked(source){return false}
        if !self.only.is_empty()&&!self.only.iter().any(|rule|matches(rule,source)){return false}
        !self.ignored.iter().any(|rule|matches(rule,source))
    }
}
pub fn matches(rule:&str,source:&str)->bool{
    if rule.eq_ignore_ascii_case(source)||rule.eq_ignore_ascii_case(&normalize_source(source)){return true}
    let program=|s:&str|s.trim().trim_matches('"').rsplit(['\\','/']).next().unwrap_or(s).trim_end_matches(".exe").to_lowercase();
    let r=program(rule);r==program(source)||!r.contains('.')&&source.to_lowercase().split(['.','!','/','\\',':']).any(|p|p==r)
}
pub fn blocked(source:&str)->bool{let s=source.to_lowercase();["explorer","shellexperiencehost","searchhost","startmenuexperiencehost"].iter().any(|p|s.contains(p))}
pub fn netease_window_metadata(filter:&SourceFilter)->Option<(String,String)>{
    if !filter.allows("cloudmusic.exe"){return None}
    type Cache=(Instant,Option<(String,String)>);
    static CACHE:std::sync::OnceLock<std::sync::Mutex<Cache>>=std::sync::OnceLock::new();
    let mut cache=CACHE.get_or_init(||std::sync::Mutex::new((Instant::now()-Duration::from_secs(2),None))).lock().ok()?;
    if cache.0.elapsed()<Duration::from_secs(1){return cache.1.clone()}
    use windows::Win32::{UI::WindowsAndMessaging::{EnumWindows,IsWindowVisible,GetWindowTextW,GetWindowThreadProcessId},Foundation::{HWND,LPARAM,CloseHandle},System::Threading::{OpenProcess,PROCESS_QUERY_LIMITED_INFORMATION,QueryFullProcessImageNameW,PROCESS_NAME_WIN32}};
    unsafe extern "system" fn visit(window:HWND,param:LPARAM)->windows::core::BOOL{
        if !IsWindowVisible(window).as_bool(){return windows::core::BOOL(1)}let result=&mut *(param.0 as *mut Option<(String,String)>);
        let mut title=[0u16;1024];let count=GetWindowTextW(window,&mut title);if count<=0{return windows::core::BOOL(1)}
        let title=String::from_utf16_lossy(&title[..count as usize]);let Some((song,artist))=title.split_once(['-','|','—'])else{return windows::core::BOOL(1)};
        if song.trim().is_empty()||artist.trim().is_empty(){return windows::core::BOOL(1)}
        let mut pid=0;GetWindowThreadProcessId(window,Some(&mut pid));if let Ok(process)=OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION,false,pid){
            let mut name=[0u16;32768];let mut length=name.len() as u32;let ok=QueryFullProcessImageNameW(process,PROCESS_NAME_WIN32,windows::core::PWSTR(name.as_mut_ptr()),&mut length).is_ok();let _=CloseHandle(process);
            if ok{let path=String::from_utf16_lossy(&name[..length as usize]);let name=path.rsplit('\\').next().unwrap_or("").to_ascii_lowercase();if name=="cloudmusic.exe"{*result=Some((song.trim().into(),artist.trim().into()));return windows::core::BOOL(0)}}
        }windows::core::BOOL(1)
    }
    let mut result=None;unsafe{let _=EnumWindows(Some(visit),LPARAM(&mut result as *mut _ as isize));}*cache=(Instant::now(),result.clone());result
}
/// No session may be playing or current, but a paused player is still the one
/// the island must control; only compare against a priority that exists.
fn better(priority:Option<(bool,bool)>,candidate:(bool,bool))->bool{priority.is_none_or(|p|candidate<p)}
#[derive(Clone)]pub struct MediaSample { pub playback:Playback,pub updated_ms:i64,pub raw:f64,pub status:String,pub rate:f64,pub cover_identity:String }
pub struct Media {
    manager:GlobalSystemMediaTransportControlsSessionManager,
    pub active:Option<GlobalSystemMediaTransportControlsSession>,
    pub events:Arc<AtomicU64>,manager_tokens:(i64,i64),session_tokens:Option<(GlobalSystemMediaTransportControlsSession,i64,i64,i64)>,
    cached_info:Option<GlobalSystemMediaTransportControlsSessionPlaybackInfo>,cached_timeline:Option<GlobalSystemMediaTransportControlsSessionTimelineProperties>,cached_properties:Option<GlobalSystemMediaTransportControlsSessionMediaProperties>,properties_at:Instant,properties_events:u64,blank_since:Option<Instant>,
    artwork:crate::artwork::ArtworkCache,cover_rx:Option<mpsc::Receiver<(String,Option<Vec<u8>>)>>,cover_retry:Instant,property_events:Arc<AtomicU64>,cover_events:u64,
}
impl Media{
    pub fn new()->Result<Self>{
        unsafe{let _=windows::Win32::System::WinRT::RoInitialize(windows::Win32::System::WinRT::RO_INIT_MULTITHREADED);}
        let manager=wait!(GlobalSystemMediaTransportControlsSessionManager::RequestAsync());let events=Arc::new(AtomicU64::new(0));
        let count=events.clone();let a=manager.SessionsChanged(&TypedEventHandler::new(move|_,_|{count.fetch_add(1,Ordering::Relaxed);Ok(())}))?;
        let count=events.clone();let b=manager.CurrentSessionChanged(&TypedEventHandler::new(move|_,_|{count.fetch_add(1,Ordering::Relaxed);Ok(())}))?;
        Ok(Self{manager,active:None,events,manager_tokens:(a,b),session_tokens:None,cached_info:None,cached_timeline:None,cached_properties:None,properties_at:Instant::now(),properties_events:u64::MAX,blank_since:None,artwork:Default::default(),cover_rx:None,cover_retry:Instant::now(),property_events:Arc::new(AtomicU64::new(0)),cover_events:u64::MAX})
    }
    pub fn read(&mut self,filter:&SourceFilter)->Result<MediaSample>{
        let sessions=self.manager.GetSessions()?;let current=self.manager.GetCurrentSession().ok();let mut best=None;let mut priority:Option<(bool,bool)>=None;
        for i in 0..sessions.Size()?{
            let Ok(s)=sessions.GetAt(i)else{continue};let Ok(id)=s.SourceAppUserModelId()else{continue};let source=id.to_string();if !filter.allows(&source){continue}
            let playing=s.GetPlaybackInfo().ok().and_then(|p|p.PlaybackStatus().ok())==Some(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing);
            let candidate=(!playing,current.as_ref()!=Some(&s));if better(priority,candidate)||priority==Some(candidate)&&self.active.as_ref()==Some(&s){priority=Some(candidate);best=Some(s)}
        }
        let Some(session)=best else{self.set_active(None);return Ok(MediaSample{playback:Playback::idle(),updated_ms:0,raw:0.,status:"Closed".into(),rate:1.,cover_identity:String::new()})};
        self.set_active(Some(session.clone()));
        let source_id=session.SourceAppUserModelId()?.to_string();let source=normalize_source(&source_id);
        let events=self.property_events.load(Ordering::Acquire);
        if self.cached_properties.is_none() || events!=self.properties_events || self.properties_at.elapsed()>=Duration::from_millis(800){
            // A transient provider error must not tear down a valid session.
            match Self::properties(&session){Ok(p)=>{let blank=p.Title().map(|s|s.is_empty()).unwrap_or(true);let had_title=self.cached_properties.as_ref().and_then(|p|p.Title().ok()).is_some_and(|s|!s.is_empty());let now=Instant::now();if blank&&had_title{let since=*self.blank_since.get_or_insert(now);if now.duration_since(since)>=Duration::from_millis(1500){self.cached_properties=Some(p);}}else{self.blank_since=None;self.cached_properties=Some(p);}self.properties_events=events;},Err(e)if self.cached_properties.is_none()=>return Err(e),Err(_)=>{}}
            self.properties_at=Instant::now();
        }
        let props=self.cached_properties.as_ref().unwrap();
        let mut title=props.Title().map(|s|s.to_string()).unwrap_or_default();let mut artist=props.Artist().map(|s|s.to_string()).unwrap_or_default();
        if artist.trim().is_empty(){artist=props.AlbumArtist().map(|s|s.to_string()).unwrap_or_default()}
        if source=="Netease"&&(title.trim().is_empty()||title.eq_ignore_ascii_case("Unknown Title")){if let Some((name,singer))=netease_window_metadata(filter){title=name;artist=singer;}}
        let mut song_id=None;if let Ok(genres)=props.Genres(){for i in 0..genres.Size().unwrap_or(0){if let Ok(genre)=genres.GetAt(i){if let Some(id)=genre_song_id(&source,&genre.to_string()){song_id=Some(id);break}}}}

        let (title,artist)=normalize_metadata(&source,&title,&artist);let album=props.AlbumTitle().map(|s|s.to_string()).unwrap_or_default();let info=session.GetPlaybackInfo().ok().or_else(||self.cached_info.clone());self.cached_info=info.clone();let status=info.as_ref().and_then(|i|i.PlaybackStatus().ok()).unwrap_or(GlobalSystemMediaTransportControlsSessionPlaybackStatus::Paused);
        let playing=status==GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing;
        let status_name=match status{GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing=>"Playing",GlobalSystemMediaTransportControlsSessionPlaybackStatus::Paused=>"Paused",GlobalSystemMediaTransportControlsSessionPlaybackStatus::Stopped=>"Stopped",GlobalSystemMediaTransportControlsSessionPlaybackStatus::Closed=>"Closed",_=>"Changing"};
        let timeline=session.GetTimelineProperties().ok().or_else(||self.cached_timeline.clone());self.cached_timeline=timeline.clone();
        let raw=timeline.as_ref().and_then(|t|t.Position().ok()).map(|t|t.Duration as f64/10000000.).unwrap_or(0.);
        let duration=timeline.as_ref().and_then(|t|t.EndTime().ok().filter(|d|d.Duration>0).or_else(||t.MaxSeekTime().ok())).map(|d|d.Duration as f64/10000000.).unwrap_or(0.).clamp(0.,86400.);
        let updated_ms=timeline.as_ref().and_then(|t|t.LastUpdatedTime().ok()).map(|d|d.UniversalTime/10000-11644473600000).unwrap_or(0);
        let cover_key=format!("{source_id}|{title}|{artist}|{album}|{}",song_id.as_deref().unwrap_or(""));let now=Instant::now();
        if self.artwork.select(cover_key.clone(),now){self.cover_rx=None;self.cover_retry=now;}
        if events!=self.cover_events{self.cover_retry=now;self.cover_events=events;}
        if let Some((key,bytes))=self.cover_rx.as_ref().and_then(|r|r.try_recv().ok()){
            self.cover_rx=None;self.artwork.accept(&key,bytes,now);
        }
        // Thumbnail streams are independent of song metadata/timeline polling.
        if self.cover_rx.is_none()&&now>=self.cover_retry{
            self.cover_retry=now+Duration::from_millis(if self.artwork.bytes.is_none(){350}else{1500});
            let props=props.clone();let key=cover_key.clone();let(tx,rx)=mpsc::channel();self.cover_rx=Some(rx);
            std::thread::spawn(move||{unsafe{let _=windows::Win32::System::WinRT::RoInitialize(windows::Win32::System::WinRT::RO_INIT_MULTITHREADED);}let bytes=Self::cover(&props).ok().flatten();let _=tx.send((key,bytes));});
        }
        let is_media=!title.trim().is_empty()&&status_name!="Closed";
        let track=if is_media{Some(Track{id:format!("{source}|{title}|{artist}"),title,artist,album,source_app:source,duration:Duration::from_secs_f64(duration.min(86400.)),song_id})}else{None};
        Ok(MediaSample{playback:Playback{track,is_playing:playing,position:Duration::from_secs_f64(raw.clamp(0.,86400.)),raw_position:Some(Duration::from_secs_f64(raw.clamp(0.,86400.))),extrapolated_position:None,cover_image_bytes:self.artwork.bytes.clone(),is_cover_loading:self.artwork.bytes.is_none(),source_app_user_model_id:source_id,can_seek:info.as_ref().and_then(|i|i.Controls().ok()).and_then(|c|c.IsPlaybackPositionEnabled().ok()).unwrap_or(false)},updated_ms,raw,status:status_name.into(),rate:info.as_ref().and_then(|i|i.PlaybackRate().ok()).and_then(|r|r.Value().ok()).unwrap_or(1.),cover_identity:cover_key})
    }
    fn properties(session:&GlobalSystemMediaTransportControlsSession)->Result<GlobalSystemMediaTransportControlsSessionMediaProperties>{Ok(wait!(session.TryGetMediaPropertiesAsync()))}
    fn cover(props:&GlobalSystemMediaTransportControlsSessionMediaProperties)->Result<Option<Vec<u8>>>{
        let Ok(thumbnail)=props.Thumbnail()else{return Ok(None)};
        let stream=wait!(thumbnail.OpenReadAsync());let size=stream.Size()?;
        if size==0||size>4*1024*1024{return Ok(None)}
        let input=stream.GetInputStreamAt(0)?;let reader=DataReader::CreateDataReader(&input)?;
        reader.SetInputStreamOptions(InputStreamOptions::None)?;let count=wait!(reader.LoadAsync(size as u32));
        let mut bytes=vec![0u8;count as usize];reader.ReadBytes(&mut bytes)?;let _=reader.Close();Ok(Some(bytes))
    }
    fn set_active(&mut self,active:Option<GlobalSystemMediaTransportControlsSession>){
        if self.active==active{return}
        if let Some((s,p,m,t))=self.session_tokens.take(){let _=s.RemovePlaybackInfoChanged(p);let _=s.RemoveMediaPropertiesChanged(m);let _=s.RemoveTimelinePropertiesChanged(t);}
        self.cached_info=None;self.cached_timeline=None;self.cached_properties=None;self.blank_since=None;self.artwork.select(String::new(),Instant::now());self.cover_rx=None;self.cover_retry=Instant::now();
        self.active=active.clone();
        if let Some(s)=active{
            let c=self.events.clone();let p=s.PlaybackInfoChanged(&TypedEventHandler::new(move|_,_|{c.fetch_add(1,Ordering::Relaxed);Ok(())})).unwrap_or(0);
            let c=self.events.clone();let properties=self.property_events.clone();let m=s.MediaPropertiesChanged(&TypedEventHandler::new(move|_,_|{c.fetch_add(1,Ordering::Relaxed);properties.fetch_add(1,Ordering::Release);Ok(())})).unwrap_or(0);
            let c=self.events.clone();let t=s.TimelinePropertiesChanged(&TypedEventHandler::new(move|_,_|{c.fetch_add(1,Ordering::Relaxed);Ok(())})).unwrap_or(0);
            self.session_tokens=Some((s,p,m,t));
        }
    }
    pub fn command(session:&GlobalSystemMediaTransportControlsSession,action:&str,seconds:f64)->Result<bool>{
        Ok(match action{"toggle"=>wait!(session.TryTogglePlayPauseAsync()),"play"=>wait!(session.TryPlayAsync()),"pause"=>wait!(session.TryPauseAsync()),"next"=>wait!(session.TrySkipNextAsync()),"previous"=>wait!(session.TrySkipPreviousAsync()),"seek"=>{if !seconds.is_finite()||seconds<0.||!session.GetPlaybackInfo()?.Controls()?.IsPlaybackPositionEnabled()?{return Ok(false)}wait!(session.TryChangePlaybackPositionAsync((seconds*10000000.) as i64))},_=>false})
    }
}
impl Drop for Media{fn drop(&mut self){self.set_active(None);let _=self.manager.RemoveSessionsChanged(self.manager_tokens.0);let _=self.manager.RemoveCurrentSessionChanged(self.manager_tokens.1);}}
#[cfg(test)]mod tests{use super::*;#[test]fn rules_preserve_whitelist_boundaries(){assert!(matches("QQMusic","QQMusic.exe"));assert!(matches("Browser","chrome.exe"));assert!(!matches("Spotify","chrome.exe"));assert!(blocked("Microsoft.Windows.ShellExperienceHost"));}
/// The filter answers "does this respond?" for every player the app has never heard of, which is the
/// whole point of turning the old allow-list around. Only what is named here stays silent.
#[test]fn only_named_players_are_excluded(){
    let filter=SourceFilter{ignored:vec!["Browser".into()],only:vec![]};
    assert!(filter.allows("SodaMusic.exe"));
    assert!(filter.allows("PotPlayerMini64.exe"));
    assert!(filter.allows("QQMusic.exe"));
    assert!(!filter.allows("chrome.exe"));
    assert!(!filter.allows("Microsoft.Windows.ShellExperienceHost"));
}
/// A blocked built-in still reaches its aliases: 网易云 is one session id in SMTC and another in the
/// window-title fallback, and both have to go quiet together.
#[test]fn exclusion_follows_aliases_and_the_diagnostic_pin(){
    let netease=SourceFilter{ignored:vec!["Netease".into()],only:vec![]};
    assert!(!netease.allows("cloudmusic.exe"));
    assert!(!netease.allows("Netease"));
    assert!(netease.allows("QQMusic.exe"));
    let pinned=SourceFilter{ignored:vec![],only:vec!["LyricIsland.Verify13".into()]};
    assert!(pinned.allows("LyricIsland.Verify13"));
    assert!(!pinned.allows("QQMusic.exe"));
}
/// A paused player without SMTC focus is the normal resume case; it must not
/// lose to an empty priority and leave the island without a controllable session.
#[test]fn a_paused_non_current_session_can_still_be_controlled(){
    assert!(better(None,(true,true)));
    assert!(!better(Some((true,true)),(true,true)));
    assert!(better(Some((true,true)),(true,false)));
    assert!(better(Some((true,true)),(false,true)));
}}



fn genre_song_id(source:&str,genre:&str)->Option<String>{let prefixes:&[&str]=match source{"Netease"=>&["NCM-"],"QQMusic"=>&["QQ-"],"Kugou"=>&["KG-","KUGOU-","KGM-"],_=>&[]};for prefix in prefixes{if genre.get(..prefix.len()).is_some_and(|p|p.eq_ignore_ascii_case(prefix)){let id=&genre[prefix.len()..];if source=="Kugou"{if id.len()==32&&id.bytes().all(|b|b.is_ascii_hexdigit()){return Some(id.into())}}else if !id.is_empty()&&id.bytes().all(|b|b.is_ascii_alphanumeric()){return Some(id.into())}}}None}
#[cfg(test)]mod domestic_tests{use super::*;
 #[test]fn player_aliases_and_combined_titles_are_normalized(){for alias in ["KuGouMusic.exe","Kugou","酷狗音乐"]{assert_eq!(normalize_source(alias),"Kugou");}assert_eq!(normalize_metadata("Kugou","歌手 - 歌曲",""),("歌曲".into(),"歌手".into()));assert_eq!(normalize_metadata("Kugou","歌曲 - Live","歌手"),("歌曲 - Live".into(),"歌手".into()));assert_eq!(normalize_metadata("Spotify","A - B",""),("A - B".into(),"".into()));}
 #[test]fn hash_namespaces_do_not_confuse_numeric_song_ids(){assert!(genre_song_id("Kugou","KG-12345").is_none());assert!(genre_song_id("QQMusic","NCM-12345").is_none());assert_eq!(genre_song_id("Kugou","kg-0123456789abcdef0123456789ABCDEF").as_deref(),Some("0123456789abcdef0123456789ABCDEF"));}
}
