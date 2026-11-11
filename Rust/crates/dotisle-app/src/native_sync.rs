//! Apply native snapshots and command replies without doing transport work on the UI thread.
use crate::{
    model::{Action, Artwork, DialogKind, Model, Page},
    remote::Packet,
};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

impl Model {
    pub(crate) fn native_tick(&mut self) -> bool {
        let packets = self
            .remote
            .as_ref()
            .map(|r| r.recv.try_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        let mut changed = false;
        for packet in packets {
            changed |= self.apply_native_packet(packet);
        }
        if self
            .toast
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() > Duration::from_secs(5))
        {
            self.toast = None;
            changed = true;
        }
        changed
    }

    fn apply_native_packet(&mut self, packet: Result<Packet, String>) -> bool {
        let packet = match packet {
            Ok(packet) => packet,
            Err(error) => {
                crate::tutorial::save_failed(self, &error);
                self.native_error = Some(error);
                return true;
            }
        };
        // Polling can return the same state many times. Command acknowledgements
        // still have to run, even when their resulting snapshot did not change.
        if packet.reply.is_none()
            && self.native_error.is_none()
            && packet.state == self.native_state
        {
            return false;
        }
        self.native_error = None;
        self.apply_native_state(packet.state, packet.reply.is_some());
        if let Some((name, reply)) = packet.reply {
            self.apply_native_reply(name, reply);
        }
        true
    }

    fn apply_native_state(&mut self, state: Value, force: bool) {
        let preview = &state["musicPreview"];
        let cover = preview["coverPath"].as_str().unwrap_or("");
        let previous_cover = self.snapshot["coverPath"].as_str().unwrap_or("");
        if cover != previous_cover {
            self.artwork = None;
            if !cover.is_empty() {
                if let Ok(bytes) = std::fs::read(cover) {
                    if let Ok(decoded) = image::load_from_memory(&bytes) {
                        self.artwork = Some(Artwork::new(decoded));
                    }
                }
            }
        }
        let track = if state["title"].is_string() {
            json!({
                "id": state["title"],
                "title": state["title"],
                "artist": state["artist"],
                "durationSeconds": state["duration"],
                "sourceApp": state["player"],
            })
        } else {
            Value::Null
        };
        self.snapshot = json!({
            "track": track,
            "isPlaying": state["isPlaying"],
            "positionSeconds": state["position"],
            "canSeek": state["canSeek"],
            "seekStatus": state["seekStatus"],
            "mediaCommand": state["mediaCommand"],
            "coverPath": cover,
            "frame": preview.get("lyricFrame").filter(|frame| frame.is_object()).cloned().unwrap_or_else(|| json!({
                "currentLine": preview["lyric"],
                "isPureMusic": preview["isPureMusic"],
                "currentTranslation": preview["secondary"],
                "wordScanProgress": preview["highlight"],
            })),
        });
        if force
            || state["settings"] != self.native_state["settings"]
            || state["settingsDark"] != self.native_state["settingsDark"]
            || state["islandEnabled"] != self.native_state["islandEnabled"]
        {
            self.sync_native_preferences(&state);
        }
        self.sync_native_collections(&state, force);
        if let Some(path) = state["dataDirectory"].as_str() {
            self.data = PathBuf::from(path);
        }
        if let Some(page) = state["pendingPage"].as_str() {
            self.page = match page {
                "timers" => Page::Focus,
                "todos" => Page::Todos,
                "overview" | "music" => Page::Home,
                "widgets" => Page::Widgets,
                "notifications" => Page::Notifications,
                "island" => Page::Settings,
                _ => Page::Settings,
            };
            self.settings_tab = match page {
                "island" => 128,
                "lyrics" => 1,
                "about" | "updates" | "update" => 3,
                _ => 255,
            };
            self.native_command("pageSeen", Value::Null);
            self.show_center = true;
            self.scroll = 0.;
        }
        if state["mediaCommand"]["state"] == "failed"
            && self.native_state["mediaCommand"] != state["mediaCommand"]
        {
            self.message(
                state["mediaCommand"]["error"]
                    .as_str()
                    .unwrap_or("播放器未确认操作，请重试")
                    .to_string(),
            );
        }
        self.native_state = state;
        crate::workflow::sync(self);
        crate::tutorial::sync(self);
        if self.native_state["pendingAction"] == "quickReminder" {
            if !self
                .dialog
                .as_ref()
                .is_some_and(|d| matches!(d.kind, DialogKind::QuickReminder))
            {
                self.dispatch(Action::QuickReminder);
            }
            self.native_command("pendingActionSeen", Value::Null);
            self.show_center = true;
        }
    }

    fn sync_native_preferences(&mut self, state: &Value) {
        let settings = &state["settings"];
        self.prefs.dark = match settings["SettingsTheme"].as_str().unwrap_or("System") {
            "Dark" => true,
            "Light" => false,
            _ => state["settingsDark"].as_bool().unwrap_or(false),
        };
        self.prefs.monet = settings["ControlCenterMonetEnabled"]
            .as_bool()
            .unwrap_or(true);
        self.prefs.translation = settings["Translation"].as_bool().unwrap_or(true);
        self.prefs.word_highlight = settings["WordHighlight"].as_bool().unwrap_or(true);
        self.prefs.online = settings["OnlineSongInfo"].as_bool().unwrap_or(true);
        self.prefs.reduced_motion = settings["Animation"] == false;
        self.prefs.always_on_top = settings["AlwaysOnTop"] == true;
        self.prefs.island = state["islandEnabled"].as_bool().unwrap_or(true);
        self.prefs.offset_ms = settings["OffsetMilliseconds"].as_i64().unwrap_or(0) as i32;
        self.prefs.local_folders = settings["LocalFolders"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        self.prefs.include_browser = !settings["IgnoredSources"]
            .as_array()
            .is_some_and(|a| a.iter().any(|v| v == "Browser"));
        self.prefs.reminder_sound = settings["TodoSoundEnabled"] == true;
    }

    fn sync_native_collections(&mut self, state: &Value, force: bool) {
        if force || state["desktopWidgets"] != self.native_state["desktopWidgets"] {
            if let Ok(items) = serde_json::from_value(state["desktopWidgets"].clone()) {
                self.desktop_widgets.items = items;
            }
        }
        self.desktop_widgets.error = state["desktopWidgetError"].as_str().map(str::to_owned);
        if force || state["todos"] != self.native_state["todos"] {
            if let Ok(items) = serde_json::from_value(state["todos"].clone()) {
                self.todos.items = items;
            }
        }
        if force || state["timers"] != self.native_state["timers"] {
            if let Ok(items) = serde_json::from_value(state["timers"].clone()) {
                self.timers.items = items;
            }
        }
    }

    fn apply_native_reply(&mut self, name: String, reply: Value) {
        if name == "update" {
            if let Some(e) = &mut self.layout_editor {
                e.reply(
                    reply["success"] == true,
                    &self.native_state["settings"]["MusicCustomLayout"],
                    reply["message"].as_str().unwrap_or("布局保存失败"),
                );
            }
        }
        if name == "quickReminderPreview" && reply["success"] == true {
            self.quick_preview = reply.clone();
        }
        if name == "quickReminderCommit" && reply["success"] == true {
            self.dialog = None;
            self.quick_preview = Value::Null;
            self.message("提醒已创建");
        }
        if reply["success"] == false {
            if name == "update" {
                crate::tutorial::save_failed(self, reply["message"].as_str().unwrap_or("操作失败"));
            }
            let e = reply["message"].as_str().unwrap_or("操作失败").to_string();
            if matches!(
                name.as_str(),
                "todoSave" | "appRuleSave" | "quickReminderPreview" | "quickReminderCommit"
            ) || name == "update" && self.dialog.is_some()
            {
                if let Some(d) = &mut self.dialog {
                    d.error = e.clone();
                }
            }
            self.message(e);
        } else {
            if name == "widgetAdd" {
                self.message("已添加到桌面，拖动卡片可调整位置");
            }
            if name == "timerCreate" {
                self.selected = reply["id"].as_str().map(str::to_owned);
            }
            if name == "workflowTemplateSave" {
                if let Some(id) = reply["id"].as_str() {
                    self.workflow.template_id = Some(id.to_owned());
                }
            }
            if matches!(name.as_str(), "todoSave" | "appRuleSave")
                || name == "update" && self.dialog.is_some()
            {
                self.dialog = None;
            } // Ordinary successful controls already communicate through their state.
            if matches!(
                name.as_str(),
                "todoDelete"
                    | "todoSave"
                    | "workflowTemplateSave"
                    | "updateDownload"
                    | "updateApply"
            ) {
                if let Some(message) = reply["message"].as_str().filter(|s| !s.is_empty()) {
                    self.message(message);
                }
            }
        }
    }
}

pub fn verify(data: &Path) -> Result<(), String> {
    let mut model = Model::new(data.to_path_buf(), true);
    model.toast = None;
    let state = json!({
        "success": true,
        "settings": {},
        "islandEnabled": true,
        "islandVisible": false,
        "musicPreview": {},
        "desktopWidgets": [],
        "todos": [],
        "timers": [],
    });
    if !model.apply_native_packet(Ok(Packet {
        state: state.clone(),
        reply: None,
    })) || !model.prefs.island
    {
        return Err("idle opacity was mistaken for the user's display choice".into());
    }
    if model.apply_native_packet(Ok(Packet {
        state: state.clone(),
        reply: None,
    })) {
        return Err("duplicate native snapshot invalidated the UI".into());
    }
    if !model.apply_native_packet(Ok(Packet {
        state: state.clone(),
        reply: Some((
            "update".into(),
            json!({"success":false,"message":"expected validation error"}),
        )),
    })) || model.toast.is_none()
    {
        return Err("duplicate-state command acknowledgement was discarded".into());
    }
    let mut hidden = state;
    hidden["islandEnabled"] = json!(false);
    model.apply_native_packet(Ok(Packet {
        state: hidden,
        reply: None,
    }));
    if model.prefs.island {
        return Err("manual hiding did not update the display choice".into());
    }
    Ok(())
}
