//! Read-only presentation state. The native process owns all persistence and actions.
use dotisle_domain::{desktop_widgets, timers, todos};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{mpsc, Mutex},
};

#[derive(Clone, Copy, Debug)]
pub enum Page { Home, Todos, Focus, Widgets, Notifications, Settings }
#[derive(Clone, Debug)]
pub enum Action {
    Navigate(Page), EditTodo(Option<String>), ToggleTodo(String),
    TodoInlineEdit(Option<String>), TodoField(crate::views::TodoField), TodoSave, TodoCancel,
    WidgetFocus, WidgetConfigure(String), WidgetRemove(String), WidgetHide(String),
    WidgetRestore(String), WidgetResize(String, desktop_widgets::Size), WidgetLaunch(String),
    /// 切换组件视觉主题皮肤（5 套主题）。
    WidgetStyle(String, desktop_widgets::Style),
    /// 切换组件光学材质（纯色 / 曜光 / 磨砂 / 透明 / 拍立得）。
    WidgetMaterial(String, String),
    /// 原地进入便签编辑态。
    NoteInlineEdit(String),
    /// 保存原地便签文本。
    NoteSave,
    /// 取消原地便签编辑。
    NoteCancel,
    /// 原地切换便签中的 `- [ ]` 复选框。
    NoteToggleCheck(String, usize),
    /// 智能叠放切到下一页。
    StackNext(String),
    /// 智能叠放切到上一页。
    StackPrev(String),
    /// 从智能叠放中拆出该组件。
    StackUnpack(String),
    /// 用系统默认程序打开文件中转站里的条目。
    DropzoneOpen(String, usize),
    /// 清空文件中转站。
    DropzoneClear(String),
    /// 相框切换下一张本地照片。
    PhotoNext(String),
    /// 日签切换下一句灵感语录。
    QuoteNext(String),
    /// 应用文件夹内一键打开第 N 个应用。
    AppFolderLaunch(String, usize),
    /// 一键切换应用文件夹为 4 宫格或 9 宫格。
    AppFolderToggleGrid(String),
    /// 进入或退出全屏桌面网格排布编辑模式。
    EditModeToggle,
    /// 编辑模式下自动将所有卡片齐整排布到最近网格槽位。
    EditModeAutoAlign,
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
    demo_queue: Mutex<Vec<(String, Value)>>,
}
impl Model {
    pub fn new() -> Self {
        let pipe = std::env::var("LYRICISLAND_TEST_PIPE")
            .unwrap_or_else(|_| "LyricIsland.Native.Settings.v1".into());
        Self::new_with_pipe(pipe)
    }

    /// 使用指定 Named Pipe 名称构建桌面数据模型与异步指令下发线程。
    pub fn new_with_pipe(pipe: String) -> Self {
        let (commands, recv) = mpsc::channel::<(String, Value)>();
        std::thread::spawn(move || {
            while let Ok((command, value)) = recv.recv() {
                // Mutations are submitted once. The Go/native pump delivers the resulting snapshot.
                match lyric_backend::io::pipe_request(&pipe, &json!({"version":1,"command":command,"value":value})) {
                    Ok(reply) if reply["success"] == false => eprintln!("desktop command {command}: {}", reply["message"]),
                    Err(error) => eprintln!("desktop command {command}: {error}"),
                    _ => {},
                }
            }
        });
        Self {
            desktop_widgets: desktop_widgets::Store::new(PathBuf::new()),
            todos: todos::Store::new(PathBuf::new()),
            timers: timers::Store::new(PathBuf::new()),
            native_state: Value::Null,
            snapshot: Value::Null,
            prefs: Preferences::default(),
            artwork: None,
            demo: false,
            commands,
            demo_queue: Mutex::new(Vec::new()),
        }
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
    pub fn native_command(&self, name: &str, value: Value) {
        if self.demo {
            if let Ok(mut q) = self.demo_queue.lock() {
                q.push((name.into(), value));
            }
            crate::wake();
            return;
        }
        let _ = self.commands.send((name.into(), value));
    }

    /// 在 `--demo` 独立预览模式下就地执行队列中的组件更新与移除命令，返回是否有状态变化。
    pub fn flush_demo_commands(&mut self) -> bool {
        if !self.demo {
            return false;
        }
        let drained: Vec<(String, Value)> = match self.demo_queue.lock() {
            Ok(mut q) if !q.is_empty() => q.drain(..).collect(),
            _ => return false,
        };
        let mut modified = false;
        for (cmd, val) in drained {
            match cmd.as_str() {
                "widgetUpdate" => {
                    let Some(id) = val.get("id").and_then(|v| v.as_str()) else {
                        continue;
                    };
                    let Some(p) = self.desktop_widgets.items.iter_mut().find(|p| p.id == id) else {
                        continue;
                    };
                    if let Some(x) = val.get("x").and_then(|v| v.as_i64()) {
                        p.x = x as i32;
                    }
                    if let Some(y) = val.get("y").and_then(|v| v.as_i64()) {
                        p.y = y as i32;
                    }
                    if let Some(size) = val.get("size").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                        p.size = size;
                        if val.get("width").is_none() {
                            p.width = 0.0;
                        }
                        if val.get("height").is_none() {
                            p.height = 0.0;
                        }
                    }
                    if let Some(w) = val.get("width").and_then(|v| v.as_f64()) {
                        p.width = w;
                    }
                    if let Some(h) = val.get("height").and_then(|v| v.as_f64()) {
                        p.height = h;
                    }
                    if let Some(style) = val.get("style").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                        p.style = style;
                    }
                    if let Some(mat) = val.get("material").and_then(|v| v.as_str()) {
                        p.material = mat.to_owned();
                    }
                    if let Some(op) = val.get("opacity").and_then(|v| v.as_f64()) {
                        p.opacity = op as f32;
                    }
                    if let Some(hid) = val.get("hidden").and_then(|v| v.as_bool()) {
                        p.hidden = hid;
                    }
                    if let Some(sid) = val.get("stackId").and_then(|v| v.as_str()) {
                        p.stack_id = sid.to_owned();
                    }
                    if let Some(ord) = val.get("stackOrder").and_then(|v| v.as_u64()) {
                        p.stack_order = ord as u32;
                    }
                    if let Some(cfg) = val.get("config").and_then(|v| serde_json::from_value(v.clone()).ok()) {
                        p.config = cfg;
                    }
                    modified = true;
                }
                "widgetRemove" => {
                    if let Some(id) = val.get("id").and_then(|v| v.as_str()) {
                        let before = self.desktop_widgets.items.len();
                        self.desktop_widgets.items.retain(|p| p.id != id);
                        if self.desktop_widgets.items.len() != before {
                            modified = true;
                        }
                    }
                }
                "widgetAdd" => {
                    if let Ok(mut p) = serde_json::from_value::<desktop_widgets::Placement>(val.clone()) {
                        if p.id.is_empty() {
                            let n = self.desktop_widgets.items.len() + 1;
                            p.id = format!("demo-add-{n}");
                        }
                        self.desktop_widgets.items.push(p);
                        modified = true;
                    }
                }
                "widgetReplaceAll" => {
                    if let Ok(items) = serde_json::from_value::<Vec<desktop_widgets::Placement>>(val["items"].clone()) {
                        self.desktop_widgets.items = items;
                        modified = true;
                    }
                }
                "todoCreate" => {
                    let title = val["title"].as_str().unwrap_or("").trim();
                    if !title.is_empty() {
                        let now = todos::Now::local();
                        let id = format!("demo-todo-{}", self.todos.items.len() + 1);
                        self.todos.items.push(todos::Todo {
                            id,
                            title: title.into(),
                            content: String::new(),
                            date: val["date"].as_str().filter(|s| !s.is_empty()).unwrap_or(&now.date).into(),
                            time: val["time"].as_str().filter(|s| !s.is_empty()).unwrap_or(&now.time).into(),
                            daily: false,
                            color: "#0A84FF".into(),
                            completed: false,
                            completed_date: None,
                            last_reminded_date: None,
                            snooze_until: None,
                        });
                        modified = true;
                    }
                }
                "timerCreate" => {
                    let sec = val["duration"].as_u64().unwrap_or(1500);
                    let id = format!("demo-timer-{}", self.timers.items.len() + 1);
                    self.timers.items.insert(0, timers::Timer {
                        id,
                        name: "深度专注".into(),
                        mode: "pomodoro".into(),
                        duration: sec,
                        focus: sec,
                        rest: 300,
                        rounds: 4,
                        round: 1,
                        phase: "focus".into(),
                        elapsed_ms: 0,
                        running: true,
                        finished: false,
                        todo_id: None,
                        workflow: None,
                        pause_reason: None,
                        revision: 1,
                        laps: Vec::new(),
                        count_up: false,
                    });
                    modified = true;
                }
                _ => {}
            }
        }
        modified
    }
    pub fn dispatch(&self, action:Action){
        match action {
            Action::Navigate(page)=>self.native_command("settings",json!({"page":match page{Page::Todos=>"todos",Page::Focus=>"timers",Page::Widgets=>"widgets",Page::Notifications=>"notifications",Page::Settings=>"settings",Page::Home=>"overview"}})),
            Action::EditTodo(id)=>self.native_command("settings",json!({"action":"todoEdit","id":id.unwrap_or_default()})),
            Action::WidgetConfigure(id)=>self.native_command("settings",json!({"action":"widgetConfigure","id":id})),
            Action::WidgetRemove(id)=>self.native_command("widgetRemove",json!({"id":id})),
            Action::WidgetHide(id)=>self.native_command("widgetUpdate",json!({"id":id,"hidden":true})),
            Action::WidgetRestore(id)=>self.native_command("widgetUpdate",json!({"id":id,"hidden":false})),
            Action::WidgetResize(id,size)=>self.native_command("widgetUpdate",json!({"id":id,"size":size,"width":0.,"height":0.})),
            Action::WidgetLaunch(id)=>self.native_command("widgetLaunch",json!({"id":id})),
            Action::WidgetStyle(id,style)=>self.native_command("widgetUpdate",json!({"id":id,"style":style})),
            Action::WidgetMaterial(id,material)=>self.native_command("widgetUpdate",json!({"id":id,"material":material})),
            Action::TodoInlineEdit(_)|Action::TodoField(_)|Action::TodoSave|Action::TodoCancel
            |Action::NoteInlineEdit(_)|Action::NoteSave|Action::NoteCancel|Action::NoteToggleCheck(_,_)
            |Action::StackNext(_)|Action::StackPrev(_)|Action::StackUnpack(_)
            |Action::DropzoneOpen(_,_)|Action::DropzoneClear(_)|Action::PhotoNext(_)|Action::QuoteNext(_)
            |Action::AppFolderLaunch(_,_)|Action::AppFolderToggleGrid(_)
            |Action::EditModeToggle|Action::EditModeAutoAlign=>{},
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
