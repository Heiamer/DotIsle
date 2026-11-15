//! Read-only presentation state. The native process owns all persistence and actions.
use dotisle_domain::{desktop_widgets, timers, todos};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::mpsc};

#[derive(Clone, Copy, Debug)]
pub enum Page { Home, Todos, Focus, Widgets, Notifications, Settings }
#[derive(Clone, Debug)]
pub enum Action {
    Navigate(Page), EditTodo(Option<String>), ToggleTodo(String),
    WidgetFocus, WidgetConfigure(String), WidgetRemove(String),
    WidgetResize(String, desktop_widgets::Size), WidgetLaunch(String),
    Timer(String, &'static str), QuickFocus, Media(&'static str),
}
#[derive(Default)]
pub struct Preferences { pub dark: bool, pub reduced_motion: bool }
pub struct Artwork { pub image: vello::peniko::ImageData }
pub struct Model {
    pub desktop_widgets: desktop_widgets::Store,
    pub todos: todos::Store,
    pub timers: timers::Store,
    pub native_state: Value,
    pub snapshot: Value,
    pub prefs: Preferences,
    pub artwork: Option<Artwork>,
    pub demo: bool,
    commands: mpsc::Sender<(String, Value)>,
}
impl Model {
    pub fn new() -> Self {
        let (commands, recv) = mpsc::channel::<(String, Value)>();
        std::thread::spawn(move || {
            let pipe = std::env::var("LYRICISLAND_TEST_PIPE").unwrap_or_else(|_| "LyricIsland.Native.Settings.v1".into());
            while let Ok((command, value)) = recv.recv() {
                // Mutations are submitted once. The Go pump delivers the resulting snapshot.
                match lyric_backend::io::pipe_request(&pipe, &json!({"version":1,"command":command,"value":value})) {
                    Ok(reply) if reply["success"] == false => eprintln!("desktop command {command}: {}", reply["message"]),
                    Err(error) => eprintln!("desktop command {command}: {error}"),
                    _ => {},
                }
            }
        });
        Self {desktop_widgets:desktop_widgets::Store::new(PathBuf::new()),todos:todos::Store::new(PathBuf::new()),timers:timers::Store::new(PathBuf::new()),native_state:Value::Null,snapshot:Value::Null,prefs:Preferences::default(),artwork:None,demo:false,commands}
    }
    pub fn apply(&mut self, state: Value) {
        if let Ok(items) = serde_json::from_value(state["desktopWidgets"].clone()) { self.desktop_widgets.items=items; }
        if let Ok(items) = serde_json::from_value(state["todos"].clone()) { self.todos.items=items; }
        if let Ok(items) = serde_json::from_value(state["timers"].clone()) { self.timers.items=items; }
        self.prefs.dark=match state["settings"]["SettingsTheme"].as_str(){Some("Dark")=>true,Some("Light")=>false,_=>state["settingsDark"].as_bool().unwrap_or(false)};
        self.prefs.reduced_motion=state["settings"]["Animation"]==false;
        let cover=state["musicPreview"]["coverPath"].as_str().unwrap_or("");
        if cover != self.native_state["musicPreview"]["coverPath"].as_str().unwrap_or("") {
            self.artwork=None;
            if let Ok(decoded)=image::open(cover){
                use vello::peniko::{Blob,ImageData,ImageFormat,ImageAlphaType};
                let rgba=decoded.thumbnail(512,512).to_rgba8();let(width,height)=rgba.dimensions();
                self.artwork=Some(Artwork{image:ImageData{data:Blob::new(std::sync::Arc::new(rgba.into_raw())),format:ImageFormat::Rgba8,alpha_type:ImageAlphaType::Alpha,width,height}});
            }
        }
        self.snapshot=json!({"track":{"id":state["title"],"title":state["title"],"artist":state["artist"],"durationSeconds":state["duration"]},"positionSeconds":state["position"],"isPlaying":state["isPlaying"],"mediaCommand":state["mediaCommand"]});
        self.native_state=state;
    }
    pub fn title(&self)->&str {self.native_state["title"].as_str().filter(|v|!v.is_empty()).unwrap_or("还没有播放音乐")}
    pub fn artist(&self)->&str {self.native_state["artist"].as_str().unwrap_or("")}
    pub fn current_lyric(&self)->&str {
        let preview=&self.native_state["musicPreview"];
        preview["lyricFrame"]["currentLine"].as_str().or_else(||preview["lyric"].as_str()).filter(|v|!v.trim().is_empty()).unwrap_or_else(||self.title())
    }
    pub fn playing(&self)->bool {self.native_state["isPlaying"]==true}
    pub fn current_timer(&self)->Option<&timers::Timer>{self.timers.items.first()}
    pub fn message(&mut self, message:String){eprintln!("desktop: {message}");}
    pub fn native_command(&self,name:&str,value:Value){let _=self.commands.send((name.into(),value));}
    pub fn dispatch(&self, action:Action){
        match action {
            Action::Navigate(page)=>self.native_command("settings",json!({"page":match page{Page::Todos=>"todos",Page::Focus=>"timers",Page::Widgets=>"widgets",Page::Notifications=>"notifications",Page::Settings=>"settings",Page::Home=>"overview"}})),
            Action::EditTodo(id)=>self.native_command("settings",json!({"action":"todoEdit","id":id.unwrap_or_default()})),
            Action::WidgetConfigure(id)=>self.native_command("settings",json!({"action":"widgetConfigure","id":id})),
            Action::WidgetRemove(id)=>self.native_command("widgetRemove",json!({"id":id})),
            Action::WidgetResize(id,size)=>self.native_command("widgetUpdate",json!({"id":id,"size":size})),
            Action::WidgetLaunch(id)=>self.native_command("widgetLaunch",json!({"id":id})),
            Action::ToggleTodo(id)=>{
                let done=self.todos.items.iter().find(|t|t.id==id).is_some_and(|t|t.done(&todos::Now::local()));
                self.native_command("todoComplete",json!({"id":id,"completed":!done}));
            },
            Action::Timer(id,action)=>if let Some(timer)=self.timers.items.iter().find(|t|t.id==id){self.native_command("timerAction",json!({"id":id,"action":action,"revision":timer.revision}));},
            Action::QuickFocus=>self.native_command("timerCreate",json!({"mode":"pomodoro","duration":1500,"focus":1500,"rest":300,"rounds":4})),
            Action::Media(action)=>self.native_command("media",json!({"action":action})),
            Action::WidgetFocus=>{},
        }
    }
}
