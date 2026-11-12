//! Screen-wide layout acceptance, including expanded settings and populated pages.
use crate::{
    draw::Text,
    gpu::Gpu,
    model::{Action, Model, Page},
    views::{self, ViewState},
};
use serde_json::{json, Value};
use std::path::Path;
#[derive(Default)]
struct Audit {
    checked: usize,
    collisions: Vec<Value>,
    captures: Vec<String>,
}
impl Audit {
    fn check(
        &mut self,
        m: &Model,
        text: &mut Text,
        gpu: &mut Gpu,
        out: &Path,
        w: f64,
        h: f64,
        label: &str,
        capture: bool,
    ) -> Result<(), String> {
        let view = views::render(m, text, w, h, &ViewState::default());
        self.checked += 1;
        // Canvas handles and overview cards intentionally contain their own controls.
        let parent = |a: &Action| {
            matches!(
                a,
                Action::OverviewDrag
                    | Action::OverviewCard(_)
                    | Action::LayoutDrag(_)
                    | Action::LayoutElement(..)
                    | Action::LayoutResize(..)
            )
        };
        for (i, a) in view.hits.iter().enumerate() {
            if parent(&a.action) {
                continue;
            }
            for b in &view.hits[i + 1..] {
                if parent(&b.action) {
                    continue;
                }
                let r = a.rect.intersect(b.rect);
                if r.width() > 2. && r.height() > 2. {
                    self.collisions.push(json!({"theme":m.prefs.control_center_theme,"dark":m.prefs.dark,"width":w,"case":label,"first":a.label,"second":b.label,"firstAction":format!("{:?}",a.action),"secondAction":format!("{:?}",b.action),"overlap":[r.width(),r.height()]}));
                }
            }
        }
        if capture {
            let name = format!("grid-{label}-{w:.0}-{}.png", m.prefs.dark);
            gpu.capture(&view.scene, w as u32, h as u32, &out.join(&name))?;
            self.captures.push(name);
        }
        Ok(())
    }
    fn scroll(
        &mut self,
        m: &mut Model,
        text: &mut Text,
        gpu: &mut Gpu,
        out: &Path,
        w: f64,
        h: f64,
        label: &str,
        capture: bool,
    ) -> Result<(), String> {
        m.toast = None;
        m.scroll = 0.;
        let extent = views::render(m, text, w, h, &ViewState::default()).max_scroll;
        for (name, scroll) in [("top", 0.), ("middle", extent / 2.), ("end", extent)] {
            m.scroll = scroll;
            self.check(
                m,
                text,
                gpu,
                out,
                w,
                h,
                &format!("{label}-{name}"),
                capture && (name != "middle" || extent > 600.),
            )?;
        }
        Ok(())
    }
}
pub fn run(out: &Path, text: &mut Text, gpu: &mut Gpu) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let mut m = Model::new(out.join("data"), true);
    m.tutorial_checked = true;
    m.tutorial = None;
    m.prefs.island = true;
    m.todos.items.clear();
    m.timers.items.clear();
    m.desktop_widgets.items.clear();
    m.dispatch(Action::EditTodo(None));
    if let Some(d) = &mut m.dialog {
        d.fields[0].value = "整理今天的灵感，留一点时间给重要的小事".into();
        d.fields[1].value = "2099-11-11".into();
        d.fields[2].value = "18:30".into();
    }
    m.dispatch(Action::SaveModal);
    m.dispatch(Action::QuickFocus);
    m.workflow.stages[0].name = "整理当下的思绪，记录那些值得认真对待的灵感".into();
    m.native_state["notificationState"] = json!({"access":"granted"});
    m.native_state["notificationHistory"] = json!([
 {"id":"layout-1","kind":"notice","title":"把重要的消息，放在刚刚好的位置","body":"一条用于验证长文本、时间和操作按钮间距的通知。","appName":"点屿","date":"2026-11-11","clock":"18:30"},
 {"id":"layout-2","kind":"notice","title":"A longer notification title for the narrow window","body":"A readable message with an action at the trailing edge.","appName":"Dotisle","date":"2026-11-11","clock":"18:31"}]);
    m.native_state["appRules"] = json!([{"id":"grid-rule","name":"给连续工作很久的自己留一点休息的时间","message":"把注意力放回当下，休息一会儿再继续。","executablePath":"C:/Program Files/Example/Example.exe","enabled":true,"revision":1}]);
    let mut audit = Audit::default();
    for theme in crate::themes::SLOTS {
        m.prefs.control_center_theme = theme.into();
        for dark in [false, true] {
            m.prefs.dark = dark;
            for (w, h) in [(900., 680.), (1200., 800.)] {
                let capture = theme == "Original"
                    && std::env::var("DOTISLE_LAYOUT_AUDIT_CAPTURE").as_deref() != Ok("0");
                for page in Page::ALL {
                    m.page = page;
                    let cases: Vec<(u8, u8)> = match page {
                        Page::Settings => {
                            let mut cases = [
                                255, 240, 241, 242, 0, 1, 2, 3, 4, 5, 6, 7, 8, 128, 129, 130, 131,
                                132, 133, 134, 135,
                            ]
                            .into_iter()
                            .map(|t| (t, 0))
                            .collect::<Vec<_>>();
                            cases.extend([
                                (6, 1),
                                (6, 2),
                                (6, 3),
                                (131, 1),
                                (132, 1),
                                (135, 1),
                                (135, 2),
                            ]);
                            cases
                        }
                        Page::Focus => (0..4).map(|v| (255, v)).collect(),
                        Page::Home => (0..5).map(|v| (255, v)).collect(),
                        Page::Widgets => (0..3).map(|v| (255, v)).collect(),
                        Page::Notifications => (0..5).map(|v| (255, v)).collect(),
                        _ => vec![(255, 0)],
                    };
                    for (tab, variant) in cases {
                        m.details.fill(false);
                        if page == Page::Settings {
                            m.dispatch(Action::SettingsTab(tab));
                            if tab == 6 && variant > 0 {
                                m.details[variant as usize - 1] = true;
                            }
                            if tab == 131 {
                                m.details[7] = variant > 0;
                            }
                            if tab == 132 {
                                m.native_state["settings"]["PlayerResponseMode"] =
                                    json!(if variant == 0 { "All" } else { "Selected" });
                                m.details[4] = variant > 0;
                            }
                            if tab == 135 {
                                m.dispatch(Action::LayoutOpen);
                                m.dispatch(Action::LayoutTab(variant));
                                if let Some(e) = &mut m.layout_editor {
                                    e.draft.thumb_enabled = true;
                                }
                            }
                        }
                        if page == Page::Focus {
                            m.mode = variant;
                        }
                        if page == Page::Notifications {
                            m.notice_filter = variant;
                        }
                        if page == Page::Widgets {
                            for size in &mut m.widget_sizes {
                                *size =
                                    dotisle_domain::desktop_widgets::Size::ALL[variant as usize];
                            }
                        }
                        let label = format!("{page:?}-{tab}-{variant}");
                        if page == Page::Home {
                            m.scroll = variant as f64 * crate::overview::STEP;
                            m.toast = None;
                            audit.check(&m, text, gpu, out, w, h, &label, capture)?;
                        } else {
                            audit.scroll(&mut m, text, gpu, out, w, h, &label, capture)?;
                        }
                    }
                }
                for (label, action) in [
                    ("todo-dialog", Action::EditTodo(None)),
                    ("rule-dialog", Action::EditRule(None)),
                    ("timer-dialog", Action::TimerPreferences),
                    ("stage-dialog", Action::WorkflowStageNew),
                    ("quick-dialog", Action::QuickReminder),
                    ("modules-dialog", Action::WorkflowModules),
                    (
                        "color-picker",
                        Action::PickColor("TodoDefaultColor", "新待办默认颜色"),
                    ),
                    ("font-picker", Action::FontPicker),
                    ("search", Action::Palette),
                ] {
                    m.dialog = None;
                    m.picker = None;
                    m.font_picker = None;
                    m.palette = false;
                    m.dispatch(action);
                    m.toast = None;
                    audit.check(&m, text, gpu, out, w, h, label, capture)?;
                }
                m.dialog = None;
                m.picker = None;
                m.font_picker = None;
                m.palette = false;
            }
        }
    }
    let report = json!({"success":audit.collisions.is_empty(),"checkedViews":audit.checked,"collisions":audit.collisions,"captures":audit.captures});
    std::fs::write(
        out.join("report.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    if report["success"] != true {
        return Err(format!(
            "layout controls overlap; see {}",
            out.join("report.json").display()
        ));
    }
    Ok(())
}
