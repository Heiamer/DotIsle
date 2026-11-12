//! A small V3 schema. Legacy configuration is read only during explicit import.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub backgrounds: Vec<Background>,
    pub schema: u32,
    pub dark: bool,
    pub monet: bool,
    pub seed: u32,
    pub translation: bool,
    pub word_highlight: bool,
    pub online: bool,
    pub include_browser: bool,
    pub offset_ms: i32,
    pub reduced_motion: bool,
    pub island: bool,
    pub always_on_top: bool,
    pub reminder_sound: bool,
    pub local_folders: Vec<String>,
    pub control_center_theme: String,
    pub font_scope: String,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            backgrounds: vec![Background::default(); 6],
            schema: 3,
            dark: false,
            monet: true,
            seed: 0x496b55,
            translation: true,
            word_highlight: true,
            online: true,
            include_browser: false,
            offset_ms: 0,
            reduced_motion: false,
            island: false,
            always_on_top: true,
            reminder_sound: true,
            local_folders: vec![],
            control_center_theme: "Original".into(),
            font_scope: "Both".into(),
        }
    }
}
impl Preferences {
    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        if bytes.len() > 1024 * 1024 {
            return Err("偏好文件过大，原文件已保留".into());
        }
        let mut p: Self = serde_json::from_slice(&bytes)
            .map_err(|e| format!("偏好读取失败，原文件已保留：{e}"))?;
        if p.schema != 3 {
            return Err("不支持此偏好版本，原文件已保留".into());
        }
        p.backgrounds.resize_with(6, Background::default);
        p.backgrounds.truncate(6);
        for bg in &mut p.backgrounds {
            bg.opacity = finite(bg.opacity, 0.3).clamp(0., 1.);
            bg.brightness = finite(bg.brightness, 1.).clamp(0., 2.);
            bg.blur = finite(bg.blur, 0.).clamp(0., 40.);
            if bg.mode != "Contain" {
                bg.mode = "Cover".into();
            }
        }
        if matches!(p.control_center_theme.as_str(), "Nocturne" | "Nothing") {
            p.control_center_theme = "Original".into();
        }
        if p.control_center_theme == "Swiss" {
            p.control_center_theme = "Gallery".into();
        }
        if p.control_center_theme == "Porcelain" {
            p.control_center_theme = "Miuix".into();
        }
        p.offset_ms = p.offset_ms.clamp(-10000, 10000);
        p.seed &= 0xffffff;
        Ok(p)
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        lyric_backend::io::save_settings(
            path,
            &serde_json::to_value(self).map_err(|e| e.to_string())?,
        )
    }
    pub fn backend(&self) -> Value {
        json!({"Translation":self.translation,"WordHighlight":self.word_highlight,"OnlineSongInfo":self.online,"OffsetMilliseconds":self.offset_ms,"LocalFolders":self.local_folders,"IgnoredSources":if self.include_browser{Vec::<String>::new()}else{vec!["Browser".into()]}})
    }
}
fn finite(v: f64, default: f64) -> f64 {
    if v.is_finite() {
        v
    } else {
        default
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Background {
    pub path: String,
    pub enabled: bool,
    pub opacity: f64,
    pub brightness: f64,
    pub blur: f64,
    pub mode: String,
}
impl Default for Background {
    fn default() -> Self {
        Self {
            path: String::new(),
            enabled: false,
            opacity: 0.3,
            brightness: 1.,
            blur: 0.,
            mode: "Cover".into(),
        }
    }
}
pub fn data_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join("DotIsle/V3")
}
pub fn import_legacy(source: &Path, dest: &Path) -> Result<Preferences, String> {
    for name in ["preferences.json", "todos.json", "timers.json"] {
        if dest.join(name).exists() {
            return Err("V3 已有数据。请先备份并使用空的数据目录导入。".into());
        }
    }
    if !source.join("settings.json").exists() {
        return Err("没有找到 2.x 设置文件".into());
    }
    let legacy: Value = serde_json::from_slice(
        &std::fs::read(source.join("settings.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mut p = Preferences::default();
    p.dark = legacy["SettingsTheme"] == "Dark";
    p.translation = legacy["Translation"].as_bool().unwrap_or(true);
    p.monet = legacy["ExpandedMonetEnabled"].as_bool().unwrap_or(true);
    p.online = legacy["OnlineSongInfo"].as_bool().unwrap_or(true);
    p.offset_ms = legacy["OffsetMilliseconds"]
        .as_i64()
        .unwrap_or(0)
        .clamp(-10000, 10000) as i32;
    p.reduced_motion = legacy["Animation"] == false;
    p.local_folders = legacy["LocalFolders"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let mut files = vec![(
        "preferences.json",
        serde_json::to_value(&p).map_err(|e| e.to_string())?,
    )];
    for name in ["todos.json", "timers.json"] {
        let path = source.join(name);
        if path.exists() {
            let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
            let value: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            if name == "todos.json" {
                let _: Vec<crate::todos::Todo> =
                    serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
                let store = crate::todos::Store::new(path);
                if let Some(e) = store.error {
                    return Err(e);
                }
            } else {
                let store = crate::timers::Store::new(path);
                if let Some(e) = store.error {
                    return Err(e);
                }
            }
            files.push((name, value));
        }
    }
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let mut written = vec![];
    for (name, value) in files {
        let path = dest.join(name);
        if let Err(e) = lyric_backend::io::save_settings(&path, &value) {
            for path in written {
                let _ = std::fs::remove_file(path);
            }
            return Err(e);
        }
        written.push(path);
    }
    Ok(p)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn corrupt_preferences_are_not_replaced() {
        let p = std::env::temp_dir().join(format!("dotisle-v3-prefs-{}", std::process::id()));
        std::fs::write(&p, b"bad").unwrap();
        assert!(Preferences::load(&p).is_err());
        assert_eq!(std::fs::read(&p).unwrap(), b"bad");
        std::fs::remove_file(p).unwrap();
    }
    #[test]
    fn import_refuses_to_overwrite_existing_v3_data() {
        let p = std::env::temp_dir().join(format!("dotisle-v3-import-{}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(p.join("todos.json"), b"keep").unwrap();
        assert!(import_legacy(&p, &p).is_err());
        assert_eq!(std::fs::read(p.join("todos.json")).unwrap(), b"keep");
        std::fs::remove_dir_all(p).unwrap();
    }
}

#[cfg(test)]
mod wallpaper_migration_tests {
    use super::*;
    #[test]
    fn older_theme_wallpapers_survive_adding_themes() {
        let path = std::env::temp_dir().join(format!(
            "dotisle-wallpaper-migration-{}.json",
            std::process::id()
        ));
        let value = json!({"schema":3,"controlCenterTheme":"Porcelain","backgrounds":[{"path":"original.png","enabled":true},{"path":"nothing.png","opacity":0.4},{"path":"porcelain.png","blur":12.},{"path":"terminal.png","opacity":0.55},{"path":"pixel.png","brightness":0.8}]});
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let p = Preferences::load(&path).unwrap();
        assert_eq!(p.backgrounds.len(), 6);
        assert_eq!(p.control_center_theme, "Miuix");
        assert_eq!(p.backgrounds[1].path, "nothing.png");
        assert_eq!(p.backgrounds[1].opacity, 0.4);
        assert_eq!(p.backgrounds[2].blur, 12.);
        assert_eq!(p.backgrounds[3].path, "terminal.png");
        assert_eq!(p.backgrounds[4].path, "pixel.png");
        assert!(!p.backgrounds.last().unwrap().enabled);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn swiss_rename_keeps_its_wallpaper() {
        let path = std::env::temp_dir().join(format!(
            "dotisle-gallery-migration-{}.json",
            std::process::id()
        ));
        let mut backgrounds = vec![serde_json::to_value(Background::default()).unwrap(); 6];
        backgrounds[5] = json!({"path":"my-gallery.png","enabled":true,"opacity":0.45});
        std::fs::write(
            &path,
            serde_json::to_vec(
                &json!({"schema":3,"controlCenterTheme":"Swiss","backgrounds":backgrounds}),
            )
            .unwrap(),
        )
        .unwrap();
        let p = Preferences::load(&path).unwrap();
        assert_eq!(p.control_center_theme, "Gallery");
        assert_eq!(p.backgrounds[5].path, "my-gallery.png");
        assert!(p.backgrounds[5].enabled);
        assert_eq!(p.backgrounds[5].opacity, 0.45);
        assert_eq!(p.backgrounds.len(), 6);
        p.save(&path).unwrap();
        let restored = Preferences::load(&path).unwrap();
        assert_eq!(restored.control_center_theme, "Gallery");
        assert_eq!(restored.backgrounds[5].path, "my-gallery.png");
        std::fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
mod removed_theme_tests {
    use super::*;
    #[test]
    fn nothing_preferences_fall_back_without_moving_other_backgrounds() {
        let path = std::env::temp_dir().join(format!(
            "dotisle-nothing-removal-{}.json",
            std::process::id()
        ));
        let backgrounds = (0..6)
            .map(|i| json!({"path":format!("wallpaper-{i}.png")}))
            .collect::<Vec<_>>();
        std::fs::write(&path, serde_json::to_vec(&json!({
            "schema":3,"controlCenterTheme":"Nothing","nothingDynamic":true,"backgrounds":backgrounds
        })).unwrap()).unwrap();
        let preferences = Preferences::load(&path).unwrap();
        assert_eq!(preferences.control_center_theme, "Original");
        for (index, background) in preferences.backgrounds.iter().enumerate() {
            assert_eq!(background.path, format!("wallpaper-{index}.png"));
        }
        assert!(serde_json::to_value(&preferences)
            .unwrap()
            .get("nothingDynamic")
            .is_none());
        let style: crate::desktop_widgets::Style = serde_json::from_str("\"nothing\"").unwrap();
        assert_eq!(style, crate::desktop_widgets::Style::Original);
        assert_eq!(crate::desktop_widgets::Style::ALL.len(), 5);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn removed_theme_falls_back_without_shifting_existing_wallpapers() {
        let path =
            std::env::temp_dir().join(format!("dotisle-removed-theme-{}.json", std::process::id()));
        let backgrounds = (0..7)
            .map(|i| json!({"path":format!("theme-{i}.png")}))
            .collect::<Vec<_>>();
        std::fs::write(
            &path,
            serde_json::to_vec(
                &json!({"schema":3,"controlCenterTheme":"Nocturne","backgrounds":backgrounds}),
            )
            .unwrap(),
        )
        .unwrap();
        let p = Preferences::load(&path).unwrap();
        assert_eq!(p.control_center_theme, "Original");
        assert_eq!(p.backgrounds.len(), 6);
        for i in 0..6 {
            assert_eq!(p.backgrounds[i].path, format!("theme-{i}.png"));
        }
        p.save(&path).unwrap();
        assert_eq!(
            Preferences::load(&path).unwrap().control_center_theme,
            "Original"
        );
        std::fs::remove_file(path).unwrap();
    }
}
