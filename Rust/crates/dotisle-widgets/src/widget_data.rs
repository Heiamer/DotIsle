use dotisle_module_storage as lyric_backend;
// Background data for widgets. Network requests run on worker threads.
use crate::{
    desktop_widgets::{Date, Kind, Placement},
    widget_config,
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, Instant},
};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::SYSTEMTIME,
        Storage::FileSystem::GetDiskFreeSpaceExW,
        System::{
            SystemInformation::{GetLocalTime, GetSystemTime},
            Time::{
                EnumDynamicTimeZoneInformation, SystemTimeToTzSpecificLocalTimeEx,
                DYNAMIC_TIME_ZONE_INFORMATION,
            },
        },
        UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
    },
};

type WeatherResult = (String, String, Result<Value, String>);
pub struct Hub {
    pub value: Value,
    path: PathBuf,
    focus: BTreeMap<String, u64>,
    dirty: bool,
    last: Instant,
    checkpoint: Instant,
    weather: HashMap<String, (String, Instant, bool)>,
    tx: mpsc::Sender<WeatherResult>,
    rx: mpsc::Receiver<WeatherResult>,
    zones: HashMap<String, DYNAMIC_TIME_ZONE_INFORMATION>,
}
fn local_date() -> Date {
    let t = unsafe { GetLocalTime() };
    Date {
        year: t.wYear as i32,
        month: t.wMonth as u32,
        day: t.wDay as u32,
        hour: t.wHour as u32,
        minute: t.wMinute as u32,
        second: t.wSecond as u32,
    }
}
fn date_key(d: Date) -> String {
    format!("{:04}-{:02}-{:02}", d.year, d.month, d.day)
}
impl Hub {
    pub fn new(path: PathBuf) -> Self {
        let focus = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let (tx, rx) = mpsc::channel();
        Self {
            value: json!({}),
            path,
            focus,
            dirty: false,
            last: Instant::now() - Duration::from_secs(2),
            checkpoint: Instant::now(),
            weather: HashMap::new(),
            tx,
            rx,
            zones: HashMap::new(),
        }
    }
    pub fn advance_focus(&mut self, timers: &[crate::timers::Timer], elapsed: u64) {
        let amount = timers
            .iter()
            .filter(|t| {
                t.running
                    && !t.finished
                    && (t.mode == "workflow" || (t.mode == "pomodoro" && t.phase == "focus"))
            })
            .map(|t| elapsed.min(t.remaining_ms()))
            .fold(0u64, u64::saturating_add);
        if amount > 0 {
            let entry = self.focus.entry(date_key(local_date())).or_default();
            *entry = entry.saturating_add(amount);
            self.dirty = true;
        }
    }
    pub fn flush(&mut self) {
        if !self.dirty || self.path.as_os_str().is_empty() {
            return;
        }
        if lyric_backend::io::save_settings(&self.path, &json!(self.focus)).is_ok() {
            self.dirty = false;
        }
    }
    pub fn poll(&mut self, placements: &[Placement]) {
        while let Ok((id, city, result)) = self.rx.try_recv() {
            if !placements
                .iter()
                .any(|p| p.id == id && p.kind == Kind::Weather && p.config.city == city)
            {
                continue;
            }
            if let Some((_, at, busy)) = self.weather.get_mut(&id) {
                *busy = false;
                *at = Instant::now();
            }
            match result {
                Ok(data) => self.value[&id] = data,
                Err(message) => {
                    self.value[&id]["error"] = json!(message);
                    self.value[&id]["stale"] = json!(true);
                }
            }
        }
        if self.last.elapsed() < Duration::from_secs(1) {
            return;
        }
        self.last = Instant::now();
        let today = local_date();
        let ordinal = widget_config::ordinal(today);
        self.focus
            .retain(|date, _| widget_config::date_ordinal(date).is_some_and(|d| ordinal - d < 90));
        let week = (0..7)
            .map(|offset| {
                self.focus
                    .iter()
                    .filter(|(date, _)| {
                        widget_config::date_ordinal(date) == Some(ordinal - 6 + offset)
                    })
                    .map(|(_, ms)| *ms)
                    .sum::<u64>()
            })
            .collect::<Vec<_>>();
        self.value["focusStats"] =
            json!({"todayMs":self.focus.get(&date_key(today)).copied().unwrap_or(0),"week":week});
        for p in placements {
            match p.kind {
                Kind::Weather if p.config.city.trim().is_empty() => {
                    self.value[&p.id] = Value::Null;
                    self.weather.remove(&p.id);
                }
                Kind::Weather => {
                    let retry = if self.value[&p.id]["error"].is_null() {
                        600
                    } else {
                        60
                    };
                    let start = self.weather.get(&p.id).is_none_or(|(city, at, busy)| {
                        city != &p.config.city
                            || (!busy && at.elapsed() >= Duration::from_secs(retry))
                    });
                    if start {
                        if self
                            .weather
                            .get(&p.id)
                            .is_none_or(|(city, _, _)| city != &p.config.city)
                        {
                            self.value[&p.id] = json!({"loading":true,"city":p.config.city});
                        }
                        self.weather
                            .insert(p.id.clone(), (p.config.city.clone(), Instant::now(), true));
                        let (tx, id, city) = (self.tx.clone(), p.id.clone(), p.config.city.clone());
                        std::thread::spawn(move || {
                            let result = fetch_weather(&city);
                            let _ = tx.send((id, city, result));
                        });
                    }
                }
                Kind::Storage => {
                    let root = widget_config::drive_root(if p.config.drive.is_empty() {
                        "C:"
                    } else {
                        &p.config.drive
                    })
                    .unwrap_or_else(|| "C:\\".into());
                    let root_wide = root.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
                    let (mut free, mut total) = (0, 0);
                    self.value[&p.id] = if unsafe {
                        GetDiskFreeSpaceExW(
                            PCWSTR(root_wide.as_ptr()),
                            Some(&mut free),
                            Some(&mut total),
                            None,
                        )
                    }
                    .is_ok()
                    {
                        json!({"drive":root,"free":free,"total":total})
                    } else {
                        json!({"error":"暂时无法读取磁盘"})
                    };
                }
                Kind::WorldClock => {
                    let city = if p.config.city.is_empty() {
                        "UTC"
                    } else {
                        &p.config.city
                    };
                    self.value[&p.id] = self.world_clock(city);
                }
                _ => {}
            }
        }
        self.weather
            .retain(|id, _| placements.iter().any(|p| &p.id == id));
        if self.checkpoint.elapsed() >= Duration::from_secs(15) {
            self.flush();
            self.checkpoint = Instant::now();
        }
    }
    fn world_clock(&mut self, city: &str) -> Value {
        let Some(name) = widget_config::timezone(city) else {
            return json!({"error":"请选择城市"});
        };
        if !self.zones.contains_key(name) {
            for index in 0..512 {
                let mut zone = DYNAMIC_TIME_ZONE_INFORMATION::default();
                if unsafe { EnumDynamicTimeZoneInformation(index, &mut zone) } != 0 {
                    break;
                }
                let length = zone
                    .TimeZoneKeyName
                    .iter()
                    .position(|c| *c == 0)
                    .unwrap_or(zone.TimeZoneKeyName.len());
                if String::from_utf16_lossy(&zone.TimeZoneKeyName[..length]) == name {
                    self.zones.insert(name.into(), zone);
                    break;
                }
            }
        }
        let Some(zone) = self.zones.get(name) else {
            return json!({"error":"时区不可用"});
        };
        let utc = unsafe { GetSystemTime() };
        let mut time = SYSTEMTIME::default();
        if unsafe { SystemTimeToTzSpecificLocalTimeEx(Some(zone), &utc, &mut time) }.is_err() {
            return json!({"error":"时区读取失败"});
        }
        json!({"city":city,"clock":format!("{:02}:{:02}",time.wHour,time.wMinute),"date":format!("{}月{}日",time.wMonth,time.wDay),"hour":time.wHour})
    }
}
fn weather_json(url: &str) -> Result<Value, String> {
    crate::weather_http::get_json(url).map_err(|_| "天气服务暂不可用，请稍后重试".into())
}
fn fetch_weather(city: &str) -> Result<Value, String> {
    let location = weather_json(&format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=zh&format=json",
        lyric_backend::http::encode(city)
    ))?;
    let result = location["results"]
        .as_array()
        .and_then(|v| v.first())
        .ok_or("未找到城市，请补充省份或国家")?;
    let latitude = result["latitude"].as_f64().ok_or("城市坐标缺失")?;
    let longitude = result["longitude"].as_f64().ok_or("城市坐标缺失")?;
    let response=weather_json(&format!("https://api.open-meteo.com/v1/forecast?latitude={latitude}&longitude={longitude}&current=temperature_2m,weather_code,is_day&daily=temperature_2m_max,temperature_2m_min&forecast_days=1&timezone=auto"))?;
    let current = &response["current"];
    let temperature = current["temperature_2m"]
        .as_f64()
        .filter(|v| v.is_finite())
        .ok_or("天气数据暂不可用")?;
    Ok(
        json!({"city":result["name"],"temperature":temperature,"code":current["weather_code"],"day":current["is_day"],"time":current["time"],"high":response["daily"]["temperature_2m_max"][0],"low":response["daily"]["temperature_2m_min"][0],"source":"Open-Meteo","stale":false}),
    )
}
pub fn launch(placements: &[Placement], id: &str) -> Value {
    let Some(p) = placements
        .iter()
        .find(|p| p.id == id && p.kind == Kind::Launcher)
    else {
        return json!({"success":false,"message":"快捷启动组件不存在"});
    };
    if !widget_config::launch_target(&p.config.target) {
        return json!({"success":false,"message":"请先配置有效的启动目标"});
    }
    if !p.config.target.starts_with("https://") && !Path::new(&p.config.target).exists() {
        return json!({"success":false,"message":"启动目标不存在"});
    }
    let target = p
        .config
        .target
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR::null(),
            PCWSTR(target.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    json!({"success":result.0 as usize>32,"message":if result.0 as usize>32{"已打开"}else{"无法打开目标"}})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn statistics_count_only_running_focus_and_bound_the_last_tick() {
        let mut store = crate::timers::Store::new(PathBuf::new());
        store
            .create(&json!({"mode":"pomodoro","focus":60}))
            .unwrap();
        store.create(&json!({"mode":"stopwatch"})).unwrap();
        let mut hub = Hub::new(PathBuf::new());
        store.items[0].elapsed_ms = 59000;
        hub.advance_focus(&store.items, 10000);
        assert_eq!(hub.focus.values().sum::<u64>(), 1000);
        store.items[0].running = false;
        hub.advance_focus(&store.items, 10000);
        assert_eq!(hub.focus.values().sum::<u64>(), 1000);
    }

    #[test]
    fn world_clock_and_disk_widgets_receive_actual_system_data() {
        let mut store = crate::desktop_widgets::Store::new(PathBuf::new());
        let disk = store
            .add(&json!({"kind":"storage","config":{"drive":"C:"}}))
            .unwrap();
        let clock = store
            .add(&json!({"kind":"worldClock","config":{"city":"纽约"}}))
            .unwrap();
        let mut hub = Hub::new(PathBuf::new());
        hub.poll(&store.items);
        assert!(hub.value[&disk]["total"].as_u64().is_some_and(|n| n > 0));
        assert!(hub.value[&clock]["clock"]
            .as_str()
            .is_some_and(|s| s.len() == 5 && s.contains(':')));
    }

    #[test]
    #[ignore = "requires the public Open-Meteo service"]
    fn live_weather_returns_real_city_temperature_and_timestamp() {
        let data = fetch_weather("Beijing").unwrap();
        assert!(data["temperature"].as_f64().is_some_and(f64::is_finite));
        assert!(data["time"].as_str().is_some_and(|s| !s.is_empty()));
        assert_eq!(data["source"], "Open-Meteo");
    }
}
