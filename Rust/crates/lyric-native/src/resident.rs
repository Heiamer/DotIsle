//! Compact idle rotation and its expanded dashboard. Telemetry comes from the existing widget hub.
use crate::layout::Rect;
use dotisle_domain::desktop_widgets::{Metrics, WirelessDevice};
use std::time::{Duration, Instant};

pub const ID: &str = "resident";
pub const KIND: u8 = 5;
pub const TIME: &str = "IslandResidentShowTime";
pub const SYSTEM: &str = "IslandResidentShowSystem";
pub const DEVICES: &str = "IslandResidentShowDevices";
const DWELL: Duration = Duration::from_secs(6);
const MAX_ROWS: usize = 4;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Time,
    System,
    Device(usize),
}

impl Page {
    pub fn name(self) -> &'static str {
        match self {
            Self::Time => "time",
            Self::System => "system",
            Self::Device(_) => "devices",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Choices {
    pub time: bool,
    pub system: bool,
    pub devices: bool,
}

impl Choices {
    fn pages(self, device_count: usize) -> impl Iterator<Item = Page> + Clone {
        let device_count = if self.devices { device_count } else { 0 };
        let clock = self.time || (!self.system && device_count == 0);
        clock.then_some(Page::Time).into_iter()
            .chain(self.system.then_some(Page::System))
            .chain((0..device_count).map(Page::Device))
    }
}

#[derive(Default, PartialEq, Eq)]
pub struct DeviceRow {
    pub name: String,
    pub battery: String,
}

#[derive(Default, PartialEq, Eq)]
pub struct View {
    pub page: Page,
    pub label: String,
    pub battery: String,
    pub clock: String,
    pub cpu: String,
    pub memory: String,
    pub devices: Vec<DeviceRow>,
    pub more_devices: usize,
}

impl View {
    pub fn height(&self) -> f64 {
        152. + if self.devices.is_empty() { 0. } else {
            29. + 23. * self.devices.len() as f64 + if self.more_devices > 0 { 21. } else { 0. }
        }
    }
}

#[derive(Default)]
pub struct Rotation {
    choices: Option<Choices>,
    next: Option<Instant>,
    refresh_at: Option<Instant>,
    expanded: bool,
    pub view: View,
}

impl Rotation {
    pub fn suspend(&mut self) {
        self.next = None;
        self.refresh_at = None;
    }

    pub fn update(&mut self, now: Instant, choices: Choices, metrics: &Metrics, paused: bool,
                  expanded: bool, clock: impl FnOnce() -> (u16, u16)) -> bool {
        let expansion_changed = std::mem::replace(&mut self.expanded, expanded) != expanded;
        let choices_changed = self.choices.replace(choices) != Some(choices);
        let devices = || metrics.wireless_devices.iter().filter(|d| d.connected);
        let count = if choices.devices || expanded { devices().count() } else { 0 };
        let previous = self.view.page;
        let mut pages = choices.pages(count);
        let first = pages.next().expect("clock fallback guarantees a page");
        let current_available = previous == first || pages.clone().any(|p| p == previous);
        let page = if !current_available || choices_changed {
            self.next = None;
            first
        } else if !paused && self.next.is_some_and(|at| now >= at) {
            self.next = None;
            choices.pages(count).skip_while(|p| *p != previous).nth(1).unwrap_or(first)
        } else {
            previous
        };
        if paused {
            self.next = None;
        } else if first != page || pages.next().is_some() {
            self.next.get_or_insert(now + DWELL);
        }
        if page == previous && !choices_changed && !expansion_changed && self.refresh_at.is_some_and(|at| now < at) {
            return false;
        }
        self.refresh_at = Some(now + Duration::from_secs(1));
        let mut view = View { page, ..Default::default() };
        if expanded || page == Page::Time {
            let (hour, minute) = clock();
            view.clock = format!("{hour:02}:{minute:02}");
        }
        if expanded || page == Page::System {
            view.cpu = percent(metrics.cpu_percent);
            view.memory = percent(metrics.memory_percent);
        }
        view.label = match page {
            Page::Time => view.clock.clone(),
            Page::System => format!("CPU {}  ·  内存 {}", view.cpu, view.memory),
            Page::Device(index) => if let Some(device) = devices().nth(index) {
                view.battery = device_battery(device);
                device_name(device)
            } else { String::new() },
        };
        if expanded {
            view.devices = devices().take(MAX_ROWS).map(|d| DeviceRow {
                name: device_name(d), battery: device_battery(d),
            }).collect();
            view.more_devices = count.saturating_sub(MAX_ROWS);
        }
        if view == self.view { return false; }
        self.view = view;
        true
    }
}

fn percent(value: Option<f64>) -> String {
    value.filter(|v| v.is_finite()).map_or_else(|| "—".into(), |v| format!("{:.0}%", v.clamp(0., 100.)))
}

fn device_name(device: &WirelessDevice) -> String {
    let name = device.name.trim();
    if name.is_empty() { "已连接设备".into() } else { name.chars().take(120).collect() }
}

fn device_battery(device: &WirelessDevice) -> String {
    device.battery_percent.filter(|n| *n <= 100).map_or_else(|| "未知".into(), |n| format!("{n}%"))
}

pub const FOCUS: i32 = 54;
pub const TODO: i32 = 55;
pub const CENTER: i32 = 56;
pub const COLLAPSE: i32 = 57;
pub fn buttons(w: f64) -> [(i32, Rect, &'static str); 3] {
    let width = (w - 56.) / 3.;
    [
        (FOCUS, Rect::new(20., 98., width, 36.), "开始专注"),
        (TODO, Rect::new(28. + width, 98., width, 36.), "快速记待办"),
        (CENTER, Rect::new(36. + 2. * width, 98., width, 36.), "控制中心"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loops_through_selected_pages_and_connected_devices() {
        let start = Instant::now();
        let mut rotation = Rotation::default();
        let choices = Choices { time: true, system: true, devices: true };
        let mut metrics = Metrics { cpu_percent: Some(31.9), memory_percent: Some(44.), ..Default::default() };
        metrics.wireless_devices = vec![
            WirelessDevice { name: "鼠标".into(), connected: true, battery_percent: Some(0), ..Default::default() },
            WirelessDevice { name: "未连接的设备".into(), battery_percent: Some(99), ..Default::default() },
            WirelessDevice { name: "耳机".into(), connected: true, ..Default::default() },
        ];
        for (index, (page, label, battery)) in [
            (Page::Time, "09:01", ""), (Page::System, "CPU 32%  ·  内存 44%", ""),
            (Page::Device(0), "鼠标", "0%"), (Page::Device(1), "耳机", "未知"), (Page::Time, "09:01", ""),
        ].into_iter().enumerate() {
            assert!(rotation.update(start + DWELL * index as u32, choices, &metrics, false, false, || (9, 1)));
            assert_eq!((rotation.view.page, rotation.view.label.as_str(), rotation.view.battery.as_str()), (page, label, battery));
        }
    }

    #[test]
    fn disconnect_skips_device_pages_and_removes_dashboard_rows() {
        let start = Instant::now();
        let mut rotation = Rotation::default();
        let choices = Choices { time: false, system: false, devices: true };
        let mut metrics = Metrics::default();
        assert!(rotation.update(start, choices, &metrics, false, false, || (9, 1)));
        assert_eq!(rotation.view.page, Page::Time);
        metrics.wireless_devices.push(WirelessDevice { name: "超长设备名".repeat(100), connected: true, ..Default::default() });
        rotation.update(start + DWELL, choices, &metrics, true, true, || (9, 2));
        assert_eq!(rotation.view.page, Page::Device(0));
        assert_eq!(rotation.view.devices[0].name.chars().count(), 120);
        metrics.wireless_devices[0].connected = false;
        rotation.update(start + DWELL * 2, choices, &metrics, true, true, || (9, 3));
        assert_eq!(rotation.view.page, Page::Time);
        assert!(rotation.view.devices.is_empty());
        assert_eq!(rotation.view.height(), 152.);
    }

    #[test]
    fn holds_the_same_page_while_obscured_and_refreshes_only_visible_changes() {
        let start = Instant::now();
        let mut rotation = Rotation::default();
        let choices = Choices { time: true, system: true, devices: false };
        let metrics = Metrics::default();
        assert!(rotation.update(start, choices, &metrics, false, false, || (9, 1)));
        assert!(!rotation.update(start + DWELL, choices, &metrics, true, false, || (9, 1)));
        assert!(!rotation.update(start + DWELL * 2, choices, &metrics, false, false, || (9, 1)));
        assert!(rotation.update(start + DWELL * 3, choices, &metrics, false, false, || (9, 1)));
        assert_eq!(rotation.view.page, Page::System);
        assert_eq!(rotation.view.label, "CPU —  ·  内存 —");
    }
}
