//! 任务栏模式专属屏幕正上方独立弹窗层（通知、系统状态 OSD、胶囊与验证码提醒照常在屏幕上方居中弹出）。
use std::{
    path::Path,
    time::{Duration, Instant},
};
use windows::{
    core::{w, Result},
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{HiDpi::*, Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON}, WindowsAndMessaging::*},
    },
};
use crate::{
    capsule_alert::Alert,
    layout::Rect,
    notifications::Notice,
    renderer::{Material, MaterialTheme, Renderer},
};

const HOST_W: f64 = 488.;
const HOST_H: f64 = 146.;

pub enum Event {
    ClickNotice(Notice, Option<String>),
    CopyCode(String),
}

#[derive(Clone)]
enum Content {
    Notice {
        notice: Notice,
        todo: Option<String>,
        rect: Rect,
        radius: f64,
    },
    Alert {
        alert: Alert,
        rect: Rect,
        radius: f64,
    },
    Osd {
        notice: Notice,
        rect: Rect,
        radius: f64,
    },
}

impl Content {
    fn rect(&self) -> Rect {
        match self {
            Self::Notice { rect, .. } | Self::Alert { rect, .. } | Self::Osd { rect, .. } => *rect,
        }
    }
    fn radius(&self) -> f64 {
        match self {
            Self::Notice { radius, .. } | Self::Alert { radius, .. } | Self::Osd { radius, .. } => *radius,
        }
    }
    fn interactive(&self) -> bool {
        !matches!(self, Self::Osd { .. })
    }
}

pub struct Overlay {
    theme: MaterialTheme,
    hwnd: HWND,
    renderer: Option<Renderer>,
    content: Option<Content>,
    opacity: f64,
    from: f64,
    target: f64,
    began: Instant,
    until: Instant,
    animate: bool,
    dirty: bool,
    idle: Instant,
    was_left: bool,
    dpi: f32,
}

impl Overlay {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            theme: MaterialTheme::new(Material::Acrylic, false),
            hwnd: HWND::default(),
            renderer: None,
            content: None,
            opacity: 0.,
            from: 0.,
            target: 0.,
            began: now,
            until: now,
            animate: true,
            dirty: false,
            idle: now,
            was_left: false,
            dpi: 96.,
        }
    }

    pub fn visible(&self) -> bool {
        self.content.is_some() && (self.opacity > 0.001 || self.target > 0.)
    }

    pub fn notice_visible(&self) -> bool {
        self.visible() && matches!(self.content, Some(Content::Notice { .. }))
    }

    pub fn osd_visible(&self) -> bool {
        self.visible() && matches!(self.content, Some(Content::Osd { .. }))
    }

    pub fn alert_visible(&self) -> bool {
        self.visible() && matches!(self.content, Some(Content::Alert { .. }))
    }

    unsafe fn ensure_window(&mut self, monitor: HMONITOR, font: &Path, interactive: bool, rect: Rect, radius: f64) -> Result<()> {
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = GetMonitorInfoW(monitor, &mut info);
        let work = info.rcWork;
        let dpi_x = 96u32;
        let mut d_x = dpi_x;
        let mut d_y = dpi_x;
        let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut d_x, &mut d_y);
        let dpi = d_x.max(96) as f32;
        self.dpi = dpi;
        let scale = dpi as f64 / 96.;
        let width = (HOST_W * scale).round() as i32;
        let height = (HOST_H * scale).round() as i32;
        let x = work.left + (work.right - work.left - width) / 2;
        let y = work.top - (24. * scale).round() as i32;

        if self.hwnd == HWND::default() {
            let instance = GetModuleHandleW(None)?;
            let class = w!("DotIsle.TopPopup.v512");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(proc),
                hInstance: instance.into(),
                lpszClassName: class,
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                ..Default::default()
            };
            RegisterClassW(&wc);
            self.hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TRANSPARENT,
                class,
                w!("点屿 · 顶部弹窗"),
                WS_POPUP,
                x,
                y,
                width,
                height,
                None,
                None,
                Some(instance.into()),
                None,
            )?;
        }

        let style = GetWindowLongW(self.hwnd, GWL_EXSTYLE) as u32;
        let next_style = if interactive {
            style & !WS_EX_TRANSPARENT.0
        } else {
            style | WS_EX_TRANSPARENT.0
        };
        if style != next_style {
            SetWindowLongW(self.hwnd, GWL_EXSTYLE, next_style as i32);
        }

        if interactive {
            let l = (rect.x * scale).floor() as i32;
            let t = (rect.y * scale).floor() as i32;
            let r = ((rect.x + rect.w) * scale).ceil() as i32;
            let b = ((rect.y + rect.h) * scale).ceil() as i32;
            let rr = (radius * 2. * scale).round() as i32;
            let rgn = CreateRoundRectRgn(l, t, r, b, rr, rr);
            let _ = SetWindowRgn(self.hwnd, Some(rgn), false);
        } else {
            let _ = SetWindowRgn(self.hwnd, None, false);
        }

        SetWindowPos(self.hwnd, Some(HWND_TOPMOST), x, y, width, height, SWP_NOACTIVATE | SWP_SHOWWINDOW)?;

        if !self.renderer.as_ref().is_some_and(|r| r.width == width as u32 && r.height == height as u32 && (r.dpi - dpi).abs() < 0.01) {
            self.renderer = None;
            self.renderer = Some(Renderer::new(self.hwnd, width as u32, height as u32, dpi, font)?);
            if let Some(r) = &mut self.renderer {
                r.set_material_theme(&self.theme)?;
            }
        }
        Ok(())
    }

    pub unsafe fn show_notice(
        &mut self,
        notice: Notice,
        todo: Option<String>,
        monitor: HMONITOR,
        font: &Path,
        animate: bool,
        theme: MaterialTheme,
    ) -> Result<()> {
        let base_rect = Rect::new(24., 24., 440., 80.);
        self.ensure_window(monitor, font, true, base_rect, 24.)?;
        self.set_theme(theme)?;
        if let Some(r) = &mut self.renderer {
            let _ = r.set_notification_icon(notice.icon.as_deref());
        }
        let h = self
            .renderer
            .as_mut()
            .and_then(|r| r.notification_height(&notice, 440.).ok())
            .unwrap_or(80.)
            .clamp(64., 98.);
        let rect = Rect::new((HOST_W - 440.) / 2., 24., 440., h);
        self.ensure_window(monitor, font, true, rect, 24.)?;
        self.content = Some(Content::Notice {
            notice,
            todo,
            rect,
            radius: 24.,
        });
        self.start_motion(animate, 7000);
        let _ = self.tick()?;
        Ok(())
    }

    pub unsafe fn show_alert(
        &mut self,
        alert: Alert,
        monitor: HMONITOR,
        font: &Path,
        animate: bool,
        theme: MaterialTheme,
    ) -> Result<()> {
        let w = alert.width.min(HOST_W - 48.).max(140.);
        let h = alert.height.clamp(40., 112.);
        let radius = (h / 2.).min(28.);
        let rect = Rect::new((HOST_W - w) / 2., 24., w, h);
        let hold_ms = (alert.hold.max(2.) * 1000.) as u64;
        self.ensure_window(monitor, font, true, rect, radius)?;
        self.set_theme(theme)?;
        self.content = Some(Content::Alert { alert, rect, radius });
        self.start_motion(animate, hold_ms);
        let _ = self.tick()?;
        Ok(())
    }

    pub unsafe fn show_osd(
        &mut self,
        notice: Notice,
        monitor: HMONITOR,
        font: &Path,
        animate: bool,
        theme: MaterialTheme,
    ) -> Result<()> {
        let rect = Rect::new((HOST_W - 220.) / 2., 24., 220., 44.);
        self.ensure_window(monitor, font, false, rect, 22.)?;
        self.set_theme(theme)?;
        self.content = Some(Content::Osd {
            notice,
            rect,
            radius: 22.,
        });
        self.start_motion(animate, 1600);
        let _ = self.tick()?;
        Ok(())
    }

    fn start_motion(&mut self, animate: bool, hold_ms: u64) {
        let now = Instant::now();
        self.animate = animate;
        self.from = self.opacity;
        self.target = 1.;
        self.began = now;
        self.until = now + Duration::from_millis(hold_ms);
        self.dirty = true;
        self.was_left = unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) < 0 };
    }

    pub unsafe fn set_theme(&mut self, theme: MaterialTheme) -> Result<()> {
        if self.theme != theme {
            if let Some(r) = &mut self.renderer {
                r.set_material_theme(&theme)?;
            }
            self.theme = theme;
            self.dirty = true;
        }
        Ok(())
    }

    pub unsafe fn clear(&mut self) {
        self.content = None;
        self.opacity = 0.;
        self.target = 0.;
        self.dirty = false;
        self.idle = Instant::now();
        if self.hwnd != HWND::default() {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub unsafe fn tick(&mut self) -> Result<Option<Event>> {
        if self.content.is_none() {
            if self.renderer.is_some() && self.idle.elapsed() >= Duration::from_secs(30) {
                self.renderer = None;
                self.theme.cover = None;
            }
            return Ok(None);
        }

        let mut emitted = None;
        let interactive = self.content.as_ref().is_some_and(Content::interactive);
        let now = Instant::now();

        if interactive && self.target > 0. {
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            let mut local = pt;
            let _ = ScreenToClient(self.hwnd, &mut local);
            let scale = (self.dpi as f64 / 96.).max(1.);
            let lx = local.x as f64 / scale;
            let ly = local.y as f64 / scale;
            let rect = self.content.as_ref().map_or(Rect::default(), Content::rect);
            let hovered = rect.contains(lx, ly);
            if hovered {
                self.until = now + Duration::from_secs(1);
            }
            let left = GetAsyncKeyState(VK_LBUTTON.0 as i32) < 0;
            let clicked = left && !self.was_left && hovered;
            self.was_left = left;

            if clicked {
                match self.content.as_mut() {
                    Some(Content::Notice { notice, todo, rect, .. }) => {
                        let close = Rect::new(rect.x + rect.w - 32., rect.y + 6., 26., 26.);
                        if !close.contains(lx, ly) {
                            emitted = Some(Event::ClickNotice(notice.clone(), todo.clone()));
                        }
                        self.from = self.opacity;
                        self.target = 0.;
                        self.began = now;
                    }
                    Some(Content::Alert { alert, .. }) => {
                        if let Some(code) = &alert.code {
                            emitted = Some(Event::CopyCode(code.clone()));
                            alert.detail = "已复制验证码".into();
                            self.until = now + Duration::from_millis(1600);
                            self.dirty = true;
                        } else {
                            self.from = self.opacity;
                            self.target = 0.;
                            self.began = now;
                        }
                    }
                    _ => {}
                }
            }
        }

        if now >= self.until && self.target > 0. {
            self.from = self.opacity;
            self.target = 0.;
            self.began = now;
        }
        let dur = if self.target > 0. { 0.18 } else { 0.14 };
        let t = if self.animate {
            (now.duration_since(self.began).as_secs_f64() / dur).min(1.)
        } else {
            1.
        };
        let value = self.from + (self.target - self.from) * (1. - (1. - t).powi(3));

        if (value - self.opacity).abs() > 0.0001 || self.dirty {
            self.opacity = value;
            self.dirty = false;
            if let (Some(renderer), Some(content)) = (&mut self.renderer, &self.content) {
                let mut r = content.rect();
                if self.animate {
                    r.y -= 8. * (1. - value);
                }
                let surface = crate::surface::Surface::expanded(self.theme.material, self.theme.cover_background);
                match content {
                    Content::Notice { notice, radius, .. } => {
                        renderer.render_top_popup(Some(notice), None, None, r, *radius, value, surface, true)?;
                    }
                    Content::Alert { alert, radius, .. } => {
                        renderer.render_top_popup(None, Some(alert), None, r, *radius, value, surface, true)?;
                    }
                    Content::Osd { notice, radius, .. } => {
                        renderer.render_top_popup(None, None, Some(notice), r, *radius, value, surface, true)?;
                    }
                }
            }
        }
        if t >= 1. {
            self.opacity = self.target;
            if self.target == 0. {
                self.clear();
            }
        }
        Ok(emitted)
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        self.renderer = None;
        if self.hwnd != HWND::default() {
            unsafe {
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }
}

unsafe extern "system" fn proc(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match message {
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_ERASEBKGND => LRESULT(1),
        _ => DefWindowProcW(hwnd, message, w, l),
    }
}
