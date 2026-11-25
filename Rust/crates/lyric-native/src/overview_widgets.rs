//! Overview tile content. These placements feed the overview and scroll pages only;
//! they never draw on the desktop, so a card can be configured without adding a
//! desktop widget first. The native process is the sole writer.
use dotisle_domain::desktop_widgets::{Kind, Placement, Size, Style};
use dotisle_domain::widget_config::Config;
use serde_json::{json, Value};
use std::path::PathBuf;

pub struct Store {
    path: PathBuf,
    pub items: Vec<Placement>,
    pub error: Option<String>,
}

/// Stable per-kind id shared with the control center. Kind is serialized camelCase,
/// which already satisfies the placement id alphabet.
pub fn tile_id(kind: Kind) -> String {
    let name = serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "widget".into());
    format!("overview-{name}")
}

fn valid(item: &Placement) -> bool {
    !item.id.is_empty()
        && item.id.len() <= 96
        && item.id.starts_with("overview-")
        && item.id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        && item.kind != Kind::Plugin
        && item.config.validate(item.kind).is_ok()
}

impl Store {
    pub const LIMIT: usize = 32;

    pub fn new(path: PathBuf) -> Self {
        let mut store = Self {
            path,
            items: vec![],
            error: None,
        };
        if store.path.is_file() {
            match std::fs::read(&store.path)
                .map_err(|e| e.to_string())
                .and_then(|b| {
                    serde_json::from_slice::<Vec<Placement>>(&b).map_err(|e| e.to_string())
                }) {
                Ok(items)
                    if items.len() <= Self::LIMIT
                        && items.iter().all(valid)
                        && items
                            .iter()
                            .enumerate()
                            .all(|(i, p)| !items[..i].iter().any(|a| a.id == p.id)) =>
                {
                    store.items = items
                }
                _ => store.error = Some("概览组件数据无效，原文件已保留".into()),
            }
        }
        store
    }

    fn persist(&self) -> Result<(), String> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        if self.path.as_os_str().is_empty() {
            return Ok(());
        }
        lyric_backend::io::save_settings(&self.path, &json!(self.items))
    }

    pub fn list(&self) -> Value {
        json!(self.items)
    }

    pub fn find(&self, kind: Kind) -> Option<&Placement> {
        self.items.iter().find(|p| p.kind == kind)
    }

    /// Upsert one kind. Config validation rejects the whole update so a bad city or
    /// date never replaces a working tile.
    pub fn set(&mut self, value: &Value) -> Result<(), String> {
        let kind: Kind =
            serde_json::from_value(value["kind"].clone()).map_err(|_| "组件类型无效")?;
        if kind == Kind::Plugin {
            return Err("概览不支持插件组件".into());
        }
        if self.find(kind).is_none() && self.items.len() >= Self::LIMIT {
            return Err("概览组件数量已达上限".into());
        }
        let config = value
            .get("config")
            .map(|v| serde_json::from_value::<Config>(v.clone()))
            .transpose()
            .map_err(|_| "组件配置无效")?
            .unwrap_or_default();
        config.validate(kind)?;
        let old = self.items.clone();
        if let Some(item) = self.items.iter_mut().find(|p| p.kind == kind) {
            item.config = config;
            item.revision = item.revision.saturating_add(1);
        } else {
            self.items.push(Placement {
                id: tile_id(kind),
                kind,
                size: Size::Medium,
                style: Style::Original,
                config,
                x: 0,
                y: 0,
                width: 0.,
                height: 0.,
                hidden: false,
                revision: 1,
            });
        }
        if let Err(error) = self.persist() {
            self.items = old;
            return Err(error);
        }
        Ok(())
    }

    pub fn remove(&mut self, kind: Kind) -> Result<(), String> {
        let Some(index) = self.items.iter().position(|p| p.kind == kind) else {
            return Ok(());
        };
        let old = self.items.clone();
        self.items.remove(index);
        if let Err(error) = self.persist() {
            self.items = old;
            return Err(error);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "DotIsle.Overview.{name}.{}.{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn overview_tiles_persist_without_desktop_placements() {
        let path = temporary("persist");
        let mut store = Store::new(path.clone());
        store
            .set(&json!({"kind":"weather","config":{"city":"上海"}}))
            .unwrap();
        store
            .set(&json!({"kind":"storage","config":{"drive":"D:"}}))
            .unwrap();
        let restored = Store::new(path.clone());
        assert!(restored.error.is_none());
        assert_eq!(restored.items.len(), 2);
        let weather = restored.find(Kind::Weather).unwrap();
        assert_eq!(weather.id, "overview-weather");
        assert_eq!(weather.config.city, "上海");
        assert_eq!(weather.revision, 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn updates_are_atomic_and_invalid_content_is_rejected() {
        let path = temporary("invalid");
        let mut store = Store::new(path.clone());
        store
            .set(&json!({"kind":"worldClock","config":{"city":"东京"}}))
            .unwrap();
        let before = store.items.clone();
        assert!(store
            .set(&json!({"kind":"worldClock","config":{"city":"火星"}}))
            .is_err());
        assert_eq!(store.items, before);
        assert!(store
            .set(&json!({"kind":"anniversary","config":{"date":"2027-02-29"}}))
            .is_err());
        store
            .set(&json!({"kind":"worldClock","config":{"city":"伦敦"}}))
            .unwrap();
        assert_eq!(store.find(Kind::WorldClock).unwrap().revision, 2);
        store.remove(Kind::WorldClock).unwrap();
        assert!(store.find(Kind::WorldClock).is_none());
        let _ = std::fs::remove_file(path);
    }
}
