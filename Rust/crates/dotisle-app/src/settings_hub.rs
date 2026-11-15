//! One settings hierarchy. Legacy page bodies remain reusable behind named destinations.
use crate::{
    draw::{self, rect, Icon},
    model::{Action, Model},
    views::ViewState,
    widgets::Ui,
};
const GROUPS: [(&str, &str); 6] = [
    ("外观与灵动岛", "主题、排版、材质、字体与动效"),
    ("音乐与歌词", "播放器、歌词、进度与播放按钮"),
    ("通知与提醒", "应用通知、设备、音效与应用规则"),
    ("计时与待办", "专注习惯、到期提醒与默认颜色"),
    ("通用与窗口", "开机启动、窗口行为与快捷键"),
    ("关于与更新", "版本信息与自动更新"),
];
pub fn destination(tab: u8) -> u8 {
    match tab {
        243 => 8,
        244 => 4,
        245 => 3,
        246 => 9,
        _ => tab,
    }
}
pub fn parent(tab: u8) -> u8 {
    match tab {
        0 | 128..=131 | 135 => 240,
        1 | 132..=134 => 241,
        2 | 5..=7 => 242,
        8 => 243,
        4 => 244,
        3 => 245,
        _ => 255,
    }
}
fn name(tab: u8) -> &'static str {
    match tab {
        9 => "模块管理",
        135 => "灵动岛布局编辑器",
        0 => "控制中心外观",
        1 => "歌词与本地文件",
        2 => "应用通知与历史",
        3 => "关于与更新",
        4 => "启动、窗口与字体",
        5 => "设备连接提醒",
        6 => "提醒音效",
        7 => "按应用提醒",
        8 => "计时与待办偏好",
        128 => "灵动岛尺寸与排版",
        129 => "材质与取色",
        130 => "交互与系统提示",
        131 => "光效、动效与圆角",
        132 => "播放器与同步",
        133 => "播放进度条",
        134 => "播放按钮主题",
        _ => "设置",
    }
}
fn link(
    ui: &mut Ui,
    x: f64,
    y: f64,
    w: f64,
    title: &str,
    detail: &str,
    tab: u8,
    backdrop: &vello::Scene,
) {
    let s = ui.s;
    let r = rect(x, y, w, 84.);
    crate::acrylic::surface(
        ui,
        r,
        24.,
        backdrop,
        vello::kurbo::Affine::IDENTITY,
        s.surface < 0x808080,
    );
    let hit = ui.hit(r, title, Action::SettingsTab(destination(tab)), true);
    ui.state(r, 24., hit, s.on_surface);
    let group = if (240..246).contains(&tab) {
        tab
    } else {
        parent(tab)
    };
    let index = group.saturating_sub(240).min(5) as usize;
    let icon = [
        Icon::Island,
        Icon::Music,
        Icon::Volume,
        Icon::Focus,
        Icon::Settings,
        Icon::Import,
    ][index];
    let classic = !ui.miuix && !ui.terminal && !ui.pixel && !ui.swiss;
    let color = if classic {
        [0x5877d5, 0xd45a6c, 0xb88833, 0x467e67, 0x72727e, 0x5381b4][index]
    } else {
        s.primary
    };
    draw::fill(ui.scene, rect(x + 20., y + 23., 38., 38.), 12., color);
    ui.icon(
        icon,
        x + 29.,
        y + 32.,
        20.,
        if classic { 0xffffff } else { s.on_primary },
    );
    ui.label(title, x + 74., y + 17., 16., s.on_surface, true, w - 118.);
    ui.label(
        detail,
        x + 74.,
        y + 47.,
        12.,
        s.on_surface_variant,
        false,
        w - 118.,
    );
    let mut arrow = vello::kurbo::BezPath::new();
    arrow.move_to((x + w - 30., y + 37.));
    arrow.line_to((x + w - 25., y + 42.));
    arrow.line_to((x + w - 30., y + 47.));
    ui.scene.stroke(
        &vello::kurbo::Stroke::new(1.5),
        vello::kurbo::Affine::IDENTITY,
        draw::color(s.on_surface_variant),
        None,
        &arrow,
    );
}
pub fn render(
    ui: &mut Ui,
    m: &Model,
    x: f64,
    mut y: f64,
    w: f64,
    state: &ViewState,
    backdrop: &vello::Scene,
) -> f64 {
    let tab = destination(m.settings_tab);
    let s = ui.s;
    if tab == 135 {
        return crate::layout_editor::render(ui, m, x, y, w, state);
    }
    if tab == 255 {
        link(ui,x,y,w,"模块管理","CORE 与 13 个功能 DLL · 替换、校验和回滚",9,backdrop);y+=100.;
        let cw = (w - 16.) / 2.;
        for (i, (title, detail)) in GROUPS.iter().enumerate() {
            link(
                ui,
                x + (i % 2) as f64 * (cw + 16.),
                y + (i / 2) as f64 * 100.,
                cw,
                title,
                detail,
                240 + i as u8,
                backdrop,
            );
        }
        return y + 284.;
    }
    let is_group = (240..243).contains(&tab);
    let direct = matches!(tab, 3 | 4 | 8 | 9);
    ui.button(
        rect(x, y, 128., 40.),
        if is_group || direct {
            "全部设置"
        } else {
            "返回分类"
        },
        Some(Icon::Back),
        Action::SettingsTab(if is_group || direct { 255 } else { parent(tab) }),
        true,
        true,
    );
    ui.label(
        if is_group {
            GROUPS[(tab - 240) as usize].0
        } else {
            name(tab)
        },
        x + 152.,
        y + 7.,
        21.,
        s.on_surface,
        true,
        w - 168.,
    );
    y += 64.;
    if is_group {
        let entries: &[(u8, &str)] = match tab {
            240 => &[
                (135, "组合歌曲信息、歌词、播放按钮与进度条，预览并应用"),
                (0, "控制中心主题与背景"),
                (128, "模式、大小、字体与音乐排版"),
                (129, "胶囊、卡片与 Mini 的材质和配色"),
                (130, "悬停、手势、位置与系统状态提示"),
                (131, "音乐、计时与更新光效开关，形变曲线和圆角"),
            ],
            241 => &[
                (135, "10 套播放按钮主题、8 种进度条与 16 种滑块"),
                (1, "翻译、高亮与本地歌词文件"),
                (132, "播放器响应范围、浏览器媒体与同步偏移"),
            ],
            242 => &[
                (2, "接收、暂停、允许与屏蔽列表、历史保留"),
                (5, "耳机、音响、手柄与输入设备"),
                (6, "通知、待办计时与设备提醒音"),
                (7, "进入应用与使用时长规则"),
            ],
            243 => &[(8, "到期提醒、锁屏暂停与默认颜色")],
            244 => &[(4, "启动页面、开机启动、快捷键与字体范围")],
            _ => &[(3, "手写字标、版本信息与双源更新")],
        };
        for (leaf, detail) in entries {
            link(ui, x, y, w, name(*leaf), detail, *leaf, backdrop);
            y += 100.;
        }
        return y;
    }
    if tab==9{return crate::module_management::render(ui,m,x,y,w);}
    if (128..135).contains(&tab) {
        return crate::island_settings::render(ui, m, x, y, w, state);
    }
    crate::views::settings(ui, m, x, y, w)
}
