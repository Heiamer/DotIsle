use serde_json::{json, Value};
pub fn defaults() -> Value {
    let mut defaults = json!({"AutoCheckUpdatesEnabled":true,"LyricColorFromCover":true,"DeviceRemindersEnabled":true,"DeviceSoundEnabled":true,"DeviceSound":"Connect","DeviceSoundPath":"","DeviceVolume":55.,"HeadphoneConnectionRemindersEnabled":true,"SpeakerConnectionRemindersEnabled":true,"GamepadConnectionRemindersEnabled":true,"MouseConnectionRemindersEnabled":false,"KeyboardConnectionRemindersEnabled":false,"DeviceDisconnectRemindersEnabled":true,"SystemStateHintsEnabled":true,"SystemStateHintPlacement":"Island","CapsLockHintEnabled":true,"NumLockHintEnabled":true,"MicrophoneMuteHintEnabled":true,"TouchpadHintEnabled":true,"SwipeActivities":true,"AutoCompactMaxWidth":420.,"TodoDefaultColor":"#F3D79B","TodoRemindersEnabled":true,"TodoIslandEnabled":true,"AutoCompactWidth":true,"CompactTranslation":true,"ChineseTwoLine":true,"NotificationSoundEnabled":true,"TodoSoundEnabled":true,"NotificationSound":"Soft","TodoSound":"Bell","NotificationSoundPath":"","TodoSoundPath":"","QuickReminderHotkey":"","FocusCountUp":false,"FocusPauseOnLock":false,"AppEntryRemindersEnabled":false,"AppUsageRemindersEnabled":false,"NotificationVolume":35.,"TodoVolume":45.,"CompactWidth":260.,"FontSize":14.,"Floating":false,"Translation":true,"TranslationButton":true,"Animation":true,"HoverExpand":false,"ClickAnywhereExpand":true,"NotificationsEnabled":true,"PauseNotifications":false,"NotificationClickOpenApp":true,"MusicClickOpenApp":false,"MusicExpandedLayout":"SideBySide","NotificationWhitelistEnabled":false,"NotificationBlacklistEnabled":false,"NotificationBlockedApps":[],"NotificationAllowedApps":["QQNT","QQ"],"NotificationHistoryEnabled":true,"NotificationHistoryDays":7,"WordHighlight":true,"AlwaysOnTop":true,"AutoStart":true,"OnlineSongInfo":true,"CoverBackground":false,"IslandMonetPreferenceVersion":1,"HideWhenPaused":true,"IgnoredSources":["Browser"],"PlayerFilterVersion":2,"DefaultsOnboardingVersion":0,"ExpandedTextColor":"#FFFFFF","ExpandedComponentColor":"#FFFFFF","LyricProgressColor":"#DDF6FF","SettingsTheme":"System","ControlCenterStartPage":"overview","WorkflowModules":[{"id":"builtin-thinking","name":"整理思路","minutes":5,"kind":0},{"id":"builtin-focus","name":"专注","minutes":25,"kind":1},{"id":"builtin-rest","name":"休息","minutes":5,"kind":2},{"id":"builtin-review","name":"复盘","minutes":10,"kind":3},{"id":"builtin-move","name":"活动一下","minutes":3,"kind":4},{"id":"builtin-custom","name":"自定义","minutes":10,"kind":5}],"CenterX":0.5,"TopFraction":0.08,"Display":"","OffsetMilliseconds":0,"NeteaseCdpPort":"9223","LocalFolders":[],"Motion":crate::motion::Options::default()});
    defaults["TimerMonoDigits"] = json!(false);
    defaults["HiddenEdgeStripEnabled"] = json!(true);
    defaults["HideTrigger"] = json!("RightClick");
    defaults["HiddenDisplay"] = json!("Strip");
    defaults["IslandKeepVisibleEnabled"] = json!(false);
    defaults[crate::resident::TIME] = json!(true);
    defaults[crate::resident::SYSTEM] = json!(false);
    defaults[crate::resident::DEVICES] = json!(false);
    defaults["MusicGlowEnabled"] = json!(false);
    defaults["TimerGlowEnabled"] = json!(false);
    defaults["UpdateGlowEnabled"] = json!(false);
    defaults["MusicGlowMode"] = json!("Breath");
    defaults["StartupSignatureEnabled"] = json!(true);
    defaults["StartupGreetingEnabled"] = json!(true);
    defaults["VerificationCodeEnabled"] = json!(true);
    defaults["WeChatMessageHintsEnabled"]=json!(false);
    for key in ["FeishuMessageHintsEnabled","DingTalkMessageHintsEnabled","WeComMessageHintsEnabled"] { defaults[key]=json!(true); }
    defaults["WeatherAlertsEnabled"] = json!(true);
    defaults["ReminderPlugins"] = json!({});
    defaults["WeatherCity"] = json!("");
    defaults["ControlCenterColorSource"] = json!("Content");
    defaults["IslandColorSource"] = json!("Content");
    defaults["IslandDynamicSeedColor"] = json!("#B1E8D1");
    defaults["DynamicColorHighContrast"] = json!(false);
    defaults["ControlCenterMonetEnabled"] = json!(true);
    defaults["ControlCenterDynamicColorEnabled"] = json!(false);
    defaults["ControlCenterCustomColor"] = json!("#9ABFE8");
    defaults["ProgressBarGlow"] = json!(true);
    defaults["MusicLyricGlow"] = json!(true);
    defaults["ProgressBarStyle"] = json!("Normal");
    defaults["ProgressBarThumb"] = json!("Pill");
    defaults["ProgressBarThumbEnabled"] = json!(false);
    defaults["ProgressBarThumbSpin"] = json!(false);
    defaults["TransportTheme"] = json!("Reef");
    defaults["MusicCustomLayout"] =
        serde_json::to_value(lyric_core::custom_layout::Layout::default()).unwrap();
    defaults["PlayerResponseMode"] = json!("All");
    defaults["BrowserMediaEnabled"] = json!(false);
    defaults["SelectedSources"] = json!(dotisle_domain::players::ALL);
    defaults["IslandFontFamily"] = json!("Noto Sans SC");
    defaults["SettingsLightAccentColor"] = json!("#2563EB");
    defaults["SettingsDarkAccentColor"] = json!("#6EA8FF");
    defaults["IslandMaterial"] = json!("LightField");
    defaults["IslandSizeMode"] = json!("Classic");
    defaults["ClassicCapsuleMaterial"] = json!("LightField");
    defaults["MiniMaterial"] = json!("LightField");
    defaults["MiniExpandedMaterial"] = json!("LightField");
    defaults["PopupMaterial"] = json!("Acrylic");
    // 亚克力叠层参数各表面独立；凝光保持洁净底面，仅在开启动态色彩时用专辑色点缀强调与歌词高亮。
    defaults["AcrylicCardFrost"] = json!(40.);
    defaults["AcrylicCardOpacity"] = json!(60.);
    defaults["AcrylicCapsuleFrost"] = json!(40.);
    defaults["AcrylicCapsuleOpacity"] = json!(0.);
    defaults["LiquidGlassRefraction"] = json!(76.);
    defaults["LiquidGlassDispersion"] = json!(46.);
    defaults["LiquidGlassBlur"] = json!(8.);
    defaults["LiquidGlassTint"] = json!(6.);
    defaults["LiquidGlassSource"] = json!("Live");
    defaults["ExpandedMonetEnabled"] = json!(true);
    defaults["ExpandedCoverOverlay"] = json!(false);
    defaults["AcrylicAdjustTarget"] = json!("Card");
    defaults["SoundLibraryVersion"] = json!(2);
    for p in ["Notification", "Todo", "Device"] {
        defaults[format!("{p}SoundSource")] = json!("Builtin");
    }
    defaults["CapsuleCornerRadius"] = Value::Null;
    defaults["SwipeUpHide"] = json!(true);
    defaults["EdgeHoverRevealEnabled"] = json!(false);
    defaults["TaskbarModeEnabled"] = json!(false);
    defaults["TaskbarAlwaysShow"] = json!(true);
    defaults["TaskbarStyle"] = json!("Transparent");
    defaults["TaskbarDockAlign"] = json!("Free");
    defaults["TaskbarPosition"] = json!(18.);
    defaults["TaskbarWidth"] = json!(340.);
    defaults["IslandOpacity"] = json!(100.);
    defaults["GestureTutorialSeen"] = json!(false);
    defaults["GesturePracticeSeen"] = json!(false);
    defaults
}
/// The face the island actually paints with. `fontScope == "Center"` keeps the island on the
/// shipped face while the stored `IslandFontFamily` remains the user's choice for the centre.
pub fn resolve_render_font(s: &mut Value, path: &std::path::Path) {
    let family = s["IslandFontFamily"]
        .as_str()
        .unwrap_or("Noto Sans SC")
        .to_string();
    let mut render = family;
    let prefs = path.with_file_name("preferences.json");
    if let Ok(raw) = std::fs::read(&prefs) {
        if let Ok(p) = serde_json::from_slice::<Value>(&raw) {
            if p["fontScope"].as_str() == Some("Center") {
                render = "Noto Sans SC".into();
            }
        }
    }
    s["IslandFontFamilyRender"] = json!(render);
}
pub fn load(path: &std::path::Path) -> Value {
    let stored = lyric_backend::io::read_settings(path);
    let mut s = defaults();
    let legacy_material = stored.get("IslandGlassMaterial").and_then(Value::as_bool);
    let has_material = stored
        .get("IslandMaterial")
        .and_then(Value::as_str)
        .is_some();
    if let Some(fields) = stored.as_object() {
        for (k, v) in fields {
            if k == "Motion" {
                if let Ok(o) = serde_json::from_value::<crate::motion::Options>(v.clone()) {
                    s[k] = serde_json::to_value(o).unwrap();
                }
            } else {
                s[k] = v.clone();
            }
        }
    }
    if s["WeChatTaskbarWarningAccepted"] != true {
        s["WeChatMessageHintsEnabled"] = json!(false);
    }
    if let Some(fields) = s.as_object_mut() {
        fields.remove("IslandFluidMotion");
        fields.remove("MotionPreferenceVersion");
        fields.remove("MotionPhysicsVersion");
        fields.remove("FollowWindowsDnd");
        fields.remove("NotificationCompatibility");
        fields.remove("CardMaterial");
        fields.remove("IslandGlassMaterial");
        fields.remove("GlassRefractionEnabled");
        fields.remove("MiniAutoContrast");
        fields.remove("MusicControlTheme");
        fields.remove("AlcoveThemeUnlocked");
        fields.remove("IslandResidentEnabled");
    }
    // 2.0.x stored a boolean for the one optional frosted material. Carry that choice into the
    // material setting once; a file that already has IslandMaterial wins over the legacy key.
    if !has_material {
        if let Some(glass) = legacy_material {
            s["IslandMaterial"] = json!(if glass { "Acrylic" } else { "LightField" });
        }
    }
    // 1.5 ships three previously opt-in features as defaults. An upgrade persists every key, so a
    // stored `false` is indistinguishable from a deliberate opt-out; migrate once, then respect it.
    if s["DefaultsOnboardingVersion"].as_u64().unwrap_or(0) < 1 {
        for k in [
            "SwipeActivities",
            "NotificationsEnabled",
            "DeviceRemindersEnabled",
        ] {
            s[k] = json!(true);
        }
        s["DefaultsOnboardingVersion"] = json!(1);
    }
    // 2.0.0 shipped mouse and keyboard reminders inside the device bundle, so every install that
    // ran it persisted `true` without anyone choosing it. A mouse that sleeps and wakes, or a
    // keyboard behind a KVM, reconnects many times a day — far past the point where a banner is
    // still information. 2.0.1 defaults them off and carries existing installs over once; a user
    // who wants them turns them back on and the marker keeps that choice.
    if s["DeviceDefaultsVersion"].as_u64().unwrap_or(0) < 1 {
        for k in [
            "MouseConnectionRemindersEnabled",
            "KeyboardConnectionRemindersEnabled",
        ] {
            s[k] = json!(false);
        }
        s["DeviceDefaultsVersion"] = json!(1);
    }
    // 2.0.2 and earlier asked "which players may respond?" and shipped six of ten switched on, so a
    // player nobody had heard of — 汽水音乐, PotPlayer, 洛雪 — stayed silent until the user found the
    // list. The filter is now expressed the other way round: everything responds, and the entries
    // here are the ones that do not.
    // 5.2.0 统一采用可变字重 Noto Sans SC 替换多套鸿蒙静态字体包以大幅瘦身；
    // 若用户未在本机安装 HarmonyOS Sans SC，则自动平滑迁移至 Noto Sans SC。
    if s.get("FontFaceVersion").and_then(Value::as_u64).unwrap_or(0) < 2 {
        if s["IslandFontFamily"].as_str().is_none_or(|name| {
            name == "HarmonyOS Sans SC"
                && dotisle_render::draw::Text::system_family(name).is_none()
        }) {
            s["IslandFontFamily"] = json!("Noto Sans SC");
        }
        s["FontFaceVersion"] = json!(2);
    }
    let stored_font = s["IslandFontFamily"]
        .as_str()
        .unwrap_or("Noto Sans SC")
        .to_owned();
    let family = if stored_font == "Noto Sans SC"
        || dotisle_render::draw::Text::bundled_family(&stored_font)
    {
        stored_font
    } else {
        dotisle_render::draw::Text::system_family(&stored_font)
            .unwrap_or_else(|| "Noto Sans SC".into())
    };
    // Keep the user's choice in IslandFontFamily: the control centre reads it to decide its own face.
    s["IslandFontFamily"] = json!(family.clone());
    // Font scope lives with the control centre, but the island must honour it too: resolve the
    // face the island renders with, leaving the stored family untouched for the centre.
    resolve_render_font(&mut s, path);
    migrate_player_filter(&mut s);
    // Four legacy per-kind flags are aliases of the single system-hint preference.
    let enabled = s["SystemStateHintsEnabled"].as_bool().unwrap_or(true);
    for key in [
        "SystemStateHintsEnabled",
        "CapsLockHintEnabled",
        "NumLockHintEnabled",
        "MicrophoneMuteHintEnabled",
        "TouchpadHintEnabled",
    ] {
        s[key] = json!(enabled);
    }
    // Upgrade only the old factory timings; keep deliberate custom values and startup opt-outs.
    if s["ThrowMotionVersion"].as_u64().unwrap_or(0) < 1 {
        if s["Motion"]["Duration"] == json!(360.) {
            s["Motion"]["Duration"] = json!(480.);
        }
        if s["Motion"]["CollapseDuration"] == json!(300.) {
            s["Motion"]["CollapseDuration"] = json!(440.);
        }
        s["ThrowMotionVersion"] = json!(1);
    }
    // Retire the implicit cover tint once; subsequent explicit opt-ins survive restarts.
    if stored["IslandMonetPreferenceVersion"].as_u64().unwrap_or(0) < 1 {
        s["CoverBackground"] = json!(false);
    }
    if !s["CoverBackground"].is_boolean() {
        s["CoverBackground"] = json!(false);
    }
    s["IslandMonetPreferenceVersion"] = json!(1);
    if !s["ExpandedMonetEnabled"].is_boolean() {
        s["ExpandedMonetEnabled"] = json!(true);
    }
    if !s["ExpandedCoverOverlay"].is_boolean() {
        s["ExpandedCoverOverlay"] = json!(false);
    }
    migrate_materials(&stored, &mut s);
    if !["LightField", "Acrylic", "LiquidGlass"].contains(&s["PopupMaterial"].as_str().unwrap_or("")) {
        s["PopupMaterial"] = json!("Acrylic");
    }
    if !s["TranslationButton"].is_boolean() {
        s["TranslationButton"] = json!(true);
    }
    for (key, fallback) in [
        ("AcrylicCardFrost", 40.),
        ("AcrylicCardOpacity", 60.),
        ("AcrylicCapsuleFrost", 40.),
        ("AcrylicCapsuleOpacity", 0.),
        ("LiquidGlassRefraction", 76.),
        ("LiquidGlassDispersion", 46.),
        ("LiquidGlassBlur", 8.),
        ("LiquidGlassTint", 6.),
    ] {
        s[key] = json!(s[key]
            .as_f64()
            .filter(|v| v.is_finite())
            .unwrap_or(fallback)
            .clamp(0., 100.));
    }
    if !["Live", "Wallpaper"].contains(&s["LiquidGlassSource"].as_str().unwrap_or("")) {
        s["LiquidGlassSource"] = json!("Live");
    }
    s["IslandOpacity"] = json!(s["IslandOpacity"]
        .as_f64()
        .filter(|v| v.is_finite())
        .unwrap_or(100.)
        .clamp(25., 100.));
    if !s["EdgeHoverRevealEnabled"].is_boolean() {
        s["EdgeHoverRevealEnabled"] = json!(false);
    }
    if !s["TaskbarModeEnabled"].is_boolean() {
        s["TaskbarModeEnabled"] = json!(false);
    }
    if !s["TaskbarAlwaysShow"].is_boolean() {
        s["TaskbarAlwaysShow"] = json!(true);
    }
    if !["Transparent", "Pill"].contains(&s["TaskbarStyle"].as_str().unwrap_or("")) {
        s["TaskbarStyle"] = json!("Transparent");
    }
    if !["Free", "Auto", "Left", "Right"].contains(&s["TaskbarDockAlign"].as_str().unwrap_or("")) {
        s["TaskbarDockAlign"] = json!("Free");
    }
    if !s["Display"].is_string() {
        s["Display"] = json!("");
    }
    for prefix in ["Notification", "Todo", "Device"] {
        if stored[format!("{prefix}SoundSource")].is_null() {
            s[format!("{prefix}SoundSource")] = json!(if stored[format!("{prefix}SoundPath")]
                .as_str()
                .is_some_and(|p| !p.is_empty())
            {
                "Custom"
            } else {
                "Builtin"
            });
        }
    }
    if stored["SoundLibraryVersion"].as_u64().unwrap_or(0) < 2 {
        s = crate::sound::reset_builtin(&s);
    }
    if !["LightField", "Acrylic", "LiquidGlass"].contains(&s["ClassicCapsuleMaterial"].as_str().unwrap_or("")) {
        s["ClassicCapsuleMaterial"] = json!("LightField");
    }
    if !["LightField", "Acrylic", "LiquidGlass"].contains(&s["MiniMaterial"].as_str().unwrap_or("")) {
        s["MiniMaterial"] = json!("LightField");
    }
    if !["LightField", "Acrylic", "LiquidGlass"].contains(&s["MiniExpandedMaterial"].as_str().unwrap_or("")) {
        s["MiniExpandedMaterial"] = json!("LightField");
    }
    if !["Classic", "Mini"].contains(&s["IslandSizeMode"].as_str().unwrap_or("")) {
        s["IslandSizeMode"] = json!("Classic");
    }
    if !["LightField", "Acrylic", "LiquidGlass"].contains(&s["IslandMaterial"].as_str().unwrap_or("")) {
        s["IslandMaterial"] = json!("LightField");
    }
    for (key, fallback) in [
        ("SettingsLightAccentColor", "#2563EB"),
        ("SettingsDarkAccentColor", "#6EA8FF"),
    ] {
        let valid = s[key].as_str().is_some_and(|v| {
            v.len() == 7 && v.starts_with('#') && v[1..].bytes().all(|b| b.is_ascii_hexdigit())
        });
        if !valid {
            s[key] = json!(fallback);
        }
    }
    // Validate persisted numbers before they enter geometry or a spring denominator.
    for (k, min, max, fallback) in [
        ("AutoCompactMaxWidth", 140., 480., 420.),
        ("CompactWidth", 140., 480., 260.),
        ("TaskbarWidth", 220., 640., 340.),
        ("TaskbarPosition", 0., 100., 18.),
        ("FontSize", 11., 20., 14.),
        ("CenterX", 0., 1., 0.5),
        ("TopFraction", 0., 1., 0.08),
    ] {
        s[k] = json!(s[k]
            .as_f64()
            .filter(|v| v.is_finite())
            .unwrap_or(fallback)
            .clamp(min, max));
    }
    for (k, min, max, fallback) in [
        ("Gap", 0., 24., 8.),
        ("Duration", 250., 1200., 480.),
        ("CollapseDuration", 150., 700., 440.),
        ("Radius", 8., 48., 30.),
    ] {
        s["Motion"][k] = json!(s["Motion"][k].as_f64().unwrap_or(fallback).clamp(min, max));
    }
    for (k, min, max, fallback) in [
        ("Stiffness", 100., 1000., 420.),
        ("Damping", 10., 80., 30.),
        ("Mass", 0.3, 2., 0.8),
    ] {
        s["Motion"]["Spring"][k] = json!(s["Motion"]["Spring"][k]
            .as_f64()
            .unwrap_or(fallback)
            .clamp(min, max));
    }
    if lyric_core::custom_layout::Layout::from_value(&s["MusicCustomLayout"]).is_err() {
        s["MusicCustomLayout"] =
            serde_json::to_value(lyric_core::custom_layout::Layout::default()).unwrap();
    }
    if s["MusicExpandedLayout"].as_str() == Some("FluidCloud") {
        s["MusicExpandedLayout"] = json!("SideBySide");
    }
    if !["Separated", "SideBySide", "LyricFocus"]
        .contains(&s["MusicExpandedLayout"].as_str().unwrap_or(""))
    {
        s["MusicExpandedLayout"] = json!("SideBySide");
    }
    if !dotisle_render::progress::BarStyle::ALL
        .iter()
        .any(|v| v.key() == s["ProgressBarStyle"].as_str().unwrap_or(""))
    {
        s["ProgressBarStyle"] = json!("Normal");
    }
    if !dotisle_render::progress::ThumbStyle::ALL
        .iter()
        .any(|v| v.key() == s["ProgressBarThumb"].as_str().unwrap_or(""))
    {
        s["ProgressBarThumb"] = json!("Pill");
    }
    if !s["ProgressBarThumbEnabled"].is_boolean() {
        s["ProgressBarThumbEnabled"] = json!(false);
    }
    if !s["ProgressBarThumbSpin"].is_boolean() {
        s["ProgressBarThumbSpin"] = json!(false);
    }
    if !dotisle_render::transport::TransportTheme::ALL
        .iter()
        .any(|t| t.key() == s["TransportTheme"].as_str().unwrap_or(""))
    {
        s["TransportTheme"] = json!("Reef");
    }
    if !["All", "Selected"].contains(&s["PlayerResponseMode"].as_str().unwrap_or("")) {
        s["PlayerResponseMode"] = json!("All");
    }
    if !s["BrowserMediaEnabled"].is_boolean() {
        s["BrowserMediaEnabled"] = json!(false);
    }
    if !s["SelectedSources"].is_array() {
        s["SelectedSources"] = json!(dotisle_domain::players::ALL);
    }
    if !["Island", "Bottom"].contains(&s["SystemStateHintPlacement"].as_str().unwrap_or("")) {
        s["SystemStateHintPlacement"] = json!("Island");
    }
    s
}
/// Upgrade stored preferences once; the removed renderer is never selected at runtime.
fn migrate_materials(stored: &Value, s: &mut Value) {
    // Some older versions shared the mini strip and expanded preference.
    if stored.get("MiniExpandedMaterial").is_none() {
        if let Some(material) = stored.get("MiniMaterial").and_then(Value::as_str) {
            s["MiniExpandedMaterial"] = json!(material);
        }
    }
    for key in [
        "IslandMaterial",
        "MiniMaterial",
        "MiniExpandedMaterial",
        "ClassicCapsuleMaterial",
        "PopupMaterial",
    ] {
        // 仅迁移已废弃的初代 Glass，保留独立回归的液态玻璃 Beta（LiquidGlass）
        if matches!(s[key].as_str(), Some("Glass")) {
            s[key] = json!("LightField");
        }
    }
}
/// Convert a 2.0.2 allow-list into the exclusion list that replaced it.
///
/// Only the ten players that list ever offered are considered, so a player introduced since (汽水,
/// PotPlayer, 洛雪…) starts responding instead of being silently excluded by an old file. Anything
/// the user had switched off stays off, which is the one part of the old choice worth carrying over.
fn migrate_player_filter(s: &mut Value) {
    let Some(allowed) = s.get("AllowedSources").and_then(Value::as_array).cloned() else {
        return;
    };
    let legacy = [
        "Netease",
        "QQMusic",
        "Kugou",
        "Kuwo",
        "Spotify",
        "Foobar2000",
        "MusicBee",
        "AIMP",
        "Browser",
    ];
    let ignored = legacy
        .iter()
        .copied()
        .filter(|id| {
            !allowed.iter().any(|value| {
                value
                    .as_str()
                    .is_some_and(|value| value.eq_ignore_ascii_case(id))
            })
        })
        .collect::<Vec<_>>();
    s["IgnoredSources"] = json!(ignored);
    s["PlayerFilterVersion"] = json!(2);
    if let Some(fields) = s.as_object_mut() {
        fields.remove("AllowedSources");
    }
}

pub fn is_acrylic_value(key: &str) -> bool {
    [
        "AcrylicCardFrost",
        "AcrylicCardOpacity",
        "AcrylicCapsuleFrost",
        "AcrylicCapsuleOpacity",
        "IslandOpacity",
    ]
    .contains(&key)
}
pub fn is_paint_setting(key: &str) -> bool {
    is_acrylic_value(key)
        || [
            "LiquidGlassRefraction",
            "LiquidGlassDispersion",
            "LiquidGlassBlur",
            "LiquidGlassTint",
            "LiquidGlassSource",
            "CoverBackground",
            "ExpandedMonetEnabled",
            "ExpandedCoverOverlay",
            "LyricColorFromCover",
            "ExpandedTextColor",
            "ExpandedComponentColor",
            "LyricProgressColor",
            "AcrylicAdjustTarget",
            "IslandOpacity",
        ]
        .contains(&key)
}
pub fn validate_setting(v: Value) -> Result<Value, String> {
    let key = v["key"].as_str().ok_or("missing key")?;
    let value = &v["value"];
    let mut settings = v["settings"].clone();
    if !settings.is_object() {
        return Err("invalid settings".into());
    }
    let range = match key {
        "CapsuleCornerRadius" => Some((0., 48.)),
        "DeviceVolume" | "NotificationVolume" | "TodoVolume" => Some((0., 100.)),
        "AutoCompactMaxWidth" | "CompactWidth" => Some((140., 480.)),
        "TaskbarPosition" => Some((0., 100.)),
        "TaskbarWidth" => Some((220., 640.)),
        "IslandOpacity" => Some((25., 100.)),
        "FontSize" => Some((11., 20.)),
        "OffsetMilliseconds" => Some((-5000., 5000.)),
        "NotificationHistoryDays" => Some((1., 30.)),
        "MotionGap" => Some((0., 24.)),
        "MotionDuration" => Some((250., 1200.)),
        "MotionCollapseDuration" => Some((150., 700.)),
        "MotionRadius" => Some((8., 48.)),
        "SpringStiffness" => Some((100., 1000.)),
        "SpringDamping" => Some((10., 80.)),
        "SpringMass" => Some((0.3, 2.)),
        "AcrylicCardFrost"
        | "AcrylicCardOpacity"
        | "AcrylicCapsuleFrost"
        | "AcrylicCapsuleOpacity"
        | "LiquidGlassRefraction"
        | "LiquidGlassDispersion"
        | "LiquidGlassBlur"
        | "LiquidGlassTint" => Some((0., 100.)),
        _ => None,
    };
    let value = if key == "IslandDynamicSeedColor" || key == "ControlCenterCustomColor" {
        let c = value.as_str().ok_or("invalid colour")?;
        if c.len() != 7 || !c.starts_with('#') || !c[1..].bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("invalid colour".into());
        }
        json!(c.to_uppercase())
    } else if key == "ControlCenterColorSource" || key == "IslandColorSource" {
        let source = value.as_str().ok_or("invalid colour source")?;
        if !["Content", "System", "Custom"].contains(&source) {
            return Err("invalid colour source".into());
        }
        if key == "ControlCenterColorSource" {
            settings["ControlCenterMonetEnabled"] = json!(true);
        } else {
            settings["ExpandedMonetEnabled"] = json!(true);
            settings["LyricColorFromCover"] = json!(true);
        }
        json!(source)
    } else if key == "IslandFontFamily" {
        let name = value.as_str().ok_or("invalid font")?;
        if name == "Noto Sans SC" || dotisle_render::draw::Text::bundled_family(name) {
            json!(name)
        } else {
            dotisle_render::draw::Text::system_family(name)
                .map(|v| json!(v))
                .ok_or_else(|| "系统里找不到这个字体，请重新选择".to_string())?
        }
    } else if key == "MotionCurve" {
        if value.is_null() {
            Value::Null
        } else {
            let c: [f64; 4] = serde_json::from_value(value.clone()).map_err(|_| "invalid curve")?;
            if !lyric_core::motion_curve::valid(c) {
                return Err("curve control points outside range".into());
            }
            json!(c)
        }
    } else if let Some((min, max)) = range {
        let n = value.as_f64().ok_or("invalid number")?;
        if !n.is_finite() || n < min || n > max {
            return Err("number outside range".into());
        }
        if key == "OffsetMilliseconds" {
            json!(n.round() as i32)
        } else if key == "NotificationHistoryDays" {
            json!(n.round() as u32)
        } else {
            json!(n)
        }
    } else if key == "IslandSizeMode" {
        let s = value.as_str().ok_or("invalid size mode")?;
        if !["Classic", "Mini"].contains(&s) {
            return Err("invalid size mode".into());
        }
        json!(s)
    } else if key == "HideTrigger" {
        let s = value.as_str().ok_or("invalid hide trigger")?;
        if !["RightClick", "SwipeUp"].contains(&s) {
            return Err("invalid hide trigger".into());
        }
        json!(s)
    } else if key == "HiddenDisplay" {
        let s = value.as_str().ok_or("invalid hidden display")?;
        if !["None", "Strip", "Mini"].contains(&s) {
            return Err("invalid hidden display".into());
        }
        json!(s)
    } else if key == "MiniMaterial" {
        let s = value.as_str().ok_or("invalid mini material")?;
        if !["LightField", "Acrylic", "LiquidGlass"].contains(&s) {
            return Err("invalid mini material".into());
        }
        json!(s)
    } else if key == "ClassicCapsuleMaterial" {
        let s = value.as_str().ok_or("invalid capsule material")?;
        if !["LightField", "Acrylic", "LiquidGlass"].contains(&s) {
            return Err("invalid capsule material".into());
        }
        json!(s)
    } else if key == "IslandMaterial" || key == "MiniExpandedMaterial" || key == "PopupMaterial" {
        let s = value.as_str().ok_or("invalid material")?;
        if !["LightField", "Acrylic", "LiquidGlass"].contains(&s) {
            return Err("invalid material".into());
        }
        json!(s)
    } else if key == "LiquidGlassSource" {
        let s = value.as_str().ok_or("invalid liquid glass source")?;
        if !["Live", "Wallpaper"].contains(&s) {
            return Err("invalid liquid glass source".into());
        }
        json!(s)
    }
    // Which surface the acrylic sliders edit; the two value pairs are stored independently.
    else if key == "AcrylicAdjustTarget" {
        let s = value.as_str().ok_or("invalid acrylic target")?;
        if !["Card", "Capsule"].contains(&s) {
            return Err("invalid acrylic target".into());
        }
        json!(s)
    } else if key == "Display" {
        json!(value.as_str().ok_or("invalid display")?.trim())
    } else if key == "TaskbarStyle" {
        let s = value.as_str().ok_or("invalid taskbar style")?;
        if !["Transparent", "Pill"].contains(&s) {
            return Err("invalid taskbar style".into());
        }
        json!(s)
    } else if key == "TaskbarDockAlign" {
        let s = value.as_str().ok_or("invalid taskbar dock align")?;
        if !["Free", "Auto", "Left", "Right"].contains(&s) {
            return Err("invalid taskbar dock align".into());
        }
        json!(s)
    } else if key == "MusicGlowMode" {
        let mode = value.as_str().ok_or("invalid glow mode")?;
        if !["Breath", "Rhythm"].contains(&mode) {
            return Err("invalid glow mode".into());
        }
        json!(mode)
    } else if [
        "MusicGlowEnabled",
        "TimerGlowEnabled",
        "UpdateGlowEnabled",
        "DynamicColorHighContrast",
        "ControlCenterMonetEnabled",
        "ControlCenterDynamicColorEnabled",
        "LyricColorFromCover",
        "AutoCheckUpdatesEnabled",
        "DeviceRemindersEnabled",
        "DeviceSoundEnabled",
        "SwipeActivities",
        "SwipeUpHide",
        "EdgeHoverRevealEnabled",
        "TaskbarModeEnabled",
        "TaskbarAlwaysShow",
        "GestureTutorialSeen",
        "GesturePracticeSeen",
        "TodoRemindersEnabled",
        "TodoIslandEnabled",
        "AutoCompactWidth",
        "CompactTranslation",
        "ChineseTwoLine",
        "NotificationSoundEnabled",
        "TodoSoundEnabled",
        "Floating",
        "Translation",
        "TranslationButton",
        "Animation",
        "StartupSignatureEnabled",
        "StartupGreetingEnabled",
        "VerificationCodeEnabled",
        "WeChatMessageHintsEnabled",
        "FeishuMessageHintsEnabled",
        "DingTalkMessageHintsEnabled",
        "WeComMessageHintsEnabled",
        "WeatherAlertsEnabled",
        "HoverExpand",
        "ClickAnywhereExpand",
        "WordHighlight",
        "TimerMonoDigits",
        "HiddenEdgeStripEnabled",
        "AlwaysOnTop",
        "AutoStart",
        "OnlineSongInfo",
        "HideWhenPaused",
        "IslandKeepVisibleEnabled",
        crate::resident::TIME,
        crate::resident::SYSTEM,
        crate::resident::DEVICES,
        "NotificationsEnabled",
        "PauseNotifications",
        "NotificationClickOpenApp",
        "MusicClickOpenApp",
        "NotificationBlacklistEnabled",
        "NotificationWhitelistEnabled",
        "NotificationHistoryEnabled",
        "ProgressBarGlow",
        "MusicLyricGlow",
        "ProgressBarThumbEnabled",
        "ProgressBarThumbSpin",
        "BrowserMediaEnabled",
        "CoverBackground",
        "ExpandedMonetEnabled",
        "ExpandedCoverOverlay",
        "FocusCountUp",
        "FocusPauseOnLock",
        "AppEntryRemindersEnabled",
        "AppUsageRemindersEnabled",
        "HeadphoneConnectionRemindersEnabled",
        "SpeakerConnectionRemindersEnabled",
        "GamepadConnectionRemindersEnabled",
        "MouseConnectionRemindersEnabled",
        "KeyboardConnectionRemindersEnabled",
        "DeviceDisconnectRemindersEnabled",
        "SystemStateHintsEnabled",
        "CapsLockHintEnabled",
        "NumLockHintEnabled",
        "MicrophoneMuteHintEnabled",
        "TouchpadHintEnabled",
    ]
    .contains(&key)
    {
        json!(value.as_bool().ok_or("invalid bool")?)
    } else if key == "DeviceSound" || key == "NotificationSound" || key == "TodoSound" {
        let name = value.as_str().ok_or("invalid sound")?;
        if dotisle_domain::sound_catalog::find(name).is_none() {
            return Err("invalid sound".into());
        }
        json!(name)
    } else if key == "DeviceSoundPath" || key == "NotificationSoundPath" || key == "TodoSoundPath" {
        let path = value.as_str().ok_or("invalid path")?;
        crate::sound::validate_file(path)?;
        json!(path)
    }
    // The settings window's own appearance. The capsule is dark by design, so this never reaches
    // the renderer — it is persisted here purely so the choice survives a restart.
    else if key == "WeatherCity" {
        let city=value.as_str().ok_or("城市名称无效")?.trim();
        if city.chars().count()>100||city.chars().any(char::is_control){return Err("城市名称过长或包含无效字符".into())}
        json!(city)
    }
    else if key == "SettingsTheme" {
        let s = value.as_str().ok_or("invalid theme")?;
        if !["System", "Light", "Dark"].contains(&s) {
            return Err("invalid theme".into());
        }
        value.clone()
    }
    // Any modifier+key combination RegisterHotKey can take, not just the three presets: the
    // recorder in 设置 · 常规 writes whatever the user pressed. Stored in canonical spelling so
    // two runs of the same binding cannot produce two different strings.
    else if key == "QuickReminderHotkey" {
        let s = value.as_str().ok_or("invalid hotkey")?;
        if s.is_empty() {
            json!("")
        } else {
            match lyric_core::hotkey::normalize(s){Some(canonical)=>json!(canonical),None=>return Err("这个组合不能用作全局快捷键：需要至少一个修饰键（Ctrl / Alt / Shift / Win）加一个字母、数字或 F1–F24".into())}
        }
    }
    // The 流程 module library is user data, not a preset list. Every entry carries a stable id so
    // renames and duplicates cannot collide, and an explicit kind so a renamed module keeps its icon.
    else if key == "WorkflowModules" {
        let list = value.as_array().ok_or("invalid modules")?;
        if list.len() > 24 {
            return Err("模块库最多 24 个，请先删除不用的模块".into());
        }
        let mut out = Vec::with_capacity(list.len());
        let mut ids = std::collections::HashSet::new();
        for item in list {
            let id = item["id"].as_str().ok_or("invalid module id")?;
            if id.is_empty()
                || id.len() > 64
                || !id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Err("模块标识无效".into());
            }
            if !ids.insert(id.to_owned()) {
                return Err("模块标识重复".into());
            }
            let name = item["name"]
                .as_str()
                .map(str::trim)
                .ok_or("invalid module name")?;
            let count = name.chars().count();
            if count == 0 || count > 40 || name.chars().any(char::is_control) {
                return Err("模块名称需要 1–40 个字".into());
            }
            let minutes = item["minutes"].as_u64().ok_or("invalid module minutes")?;
            if !(1..=240).contains(&minutes) {
                return Err("模块时长需要 1–240 分钟".into());
            }
            let kind = item["kind"].as_u64().ok_or("invalid module kind")?;
            if kind > 5 {
                return Err("模块外观类型无效".into());
            }
            out.push(json!({"id":id,"name":name,"minutes":minutes,"kind":kind}));
        }
        json!(out)
    } else if key == "ControlCenterStartPage" {
        let s = value.as_str().ok_or("invalid start page")?;
        if ![
            "overview",
            "notifications",
            "todos",
            "timers",
            "island",
            "settings",
            "about",
        ]
        .contains(&s)
        {
            return Err("invalid start page".into());
        }
        json!(s)
    }
    // Layout values stay typed and are validated before they enter live geometry.
    else if key == "SystemStateHintPlacement" {
        let s = value.as_str().ok_or("invalid placement")?;
        if !["Island", "Bottom"].contains(&s) {
            return Err("invalid placement".into());
        }
        json!(s)
    } else if key == "MusicCustomLayout" {
        serde_json::to_value(lyric_core::custom_layout::Layout::from_value(&value)?)
            .map_err(|e| e.to_string())?
    } else if key == "MusicExpandedLayout" {
        let s = value.as_str().ok_or("invalid layout")?;
        if !["Separated", "SideBySide", "LyricFocus"].contains(&s) {
            return Err("invalid layout".into());
        }
        json!(s)
    }
    // The seek bar owns both choices; passive progress indicators keep their fixed drawing.
    else if key == "ProgressBarStyle" {
        let v = value.as_str().ok_or("invalid progress style")?;
        if !dotisle_render::progress::BarStyle::ALL
            .iter()
            .any(|s| s.key() == v)
        {
            return Err("invalid progress style".into());
        }
        json!(v)
    } else if key == "ProgressBarThumb" {
        let v = value.as_str().ok_or("invalid progress thumb")?;
        if !dotisle_render::progress::ThumbStyle::ALL
            .iter()
            .any(|s| s.key() == v)
        {
            return Err("invalid progress thumb".into());
        }
        json!(v)
    } else if key == "PlayerResponseMode" {
        let v = value.as_str().ok_or("invalid player mode")?;
        if !["All", "Selected"].contains(&v) {
            return Err("invalid player mode".into());
        }
        json!(v)
    } else if key == "TransportTheme" {
        let v = value.as_str().ok_or("invalid transport theme")?;
        if !dotisle_render::transport::TransportTheme::ALL
            .iter()
            .any(|t| t.key() == v)
        {
            return Err("invalid transport theme".into());
        }
        json!(v)
    } else if key == "SelectedSources" {
        let a = value.as_array().ok_or("invalid player list")?;
        if a.len() > 32 {
            return Err("player list too long".into());
        }
        let mut out = vec![];
        for s in a {
            let s = s.as_str().ok_or("invalid player")?.trim();
            if s.is_empty() || s.encode_utf16().count() > 64 {
                return Err("invalid player".into());
            }
            if !out.iter().any(|v: &String| v.eq_ignore_ascii_case(s)) {
                out.push(s.to_string())
            }
        }
        json!(out)
    } else if key.ends_with("Color") {
        let s = value.as_str().ok_or("invalid color")?;
        if ![
            "TodoDefaultColor",
            "ExpandedTextColor",
            "ExpandedComponentColor",
            "LyricProgressColor",
            "SettingsLightAccentColor",
            "SettingsDarkAccentColor",
        ]
        .contains(&key)
            || !s.starts_with('#')
            || s.len() != 7
            || !s[1..].bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("invalid color".into());
        }
        json!(s.to_uppercase())
    } else if key == "NeteaseCdpPort" {
        let raw = value.as_str().ok_or("端口需要 1024–65535 的数字")?;
        let port = raw
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|p| (1024..=65535).contains(p))
            .ok_or("端口需要 1024–65535 的数字")?;
        json!(port.to_string())
    } else if key == "IgnoredSources"
        || key == "AllowedSources"
        || key == "NotificationAllowedApps"
        || key == "NotificationBlockedApps"
    {
        let a = value.as_array().ok_or("invalid source list")?;
        if a.len() > 32 {
            return Err("source list too long".into());
        }
        let mut out = vec![];
        for s in a {
            let s = s.as_str().ok_or("invalid source")?.trim();
            if s.is_empty() || s.encode_utf16().count() > 256 {
                return Err("invalid source".into());
            }
            if !out.iter().any(|v: &String| v.eq_ignore_ascii_case(s)) {
                out.push(s.to_string())
            }
        }
        json!(out)
    } else {
        return Err("unknown setting".into());
    };
    if [
        "SystemStateHintsEnabled",
        "CapsLockHintEnabled",
        "NumLockHintEnabled",
        "MicrophoneMuteHintEnabled",
        "TouchpadHintEnabled",
    ]
    .contains(&key)
    {
        for alias in [
            "SystemStateHintsEnabled",
            "CapsLockHintEnabled",
            "NumLockHintEnabled",
            "MicrophoneMuteHintEnabled",
            "TouchpadHintEnabled",
        ] {
            settings[alias] = value.clone();
        }
        return Ok(settings);
    }
    if ["IslandMaterial", "MiniExpandedMaterial"].contains(&key) {
        settings["IslandMaterial"] = value.clone();
        settings["MiniExpandedMaterial"] = value;
        return Ok(settings);
    }
    if ["DeviceSoundPath", "NotificationSoundPath", "TodoSoundPath"].contains(&key) {
        let prefix = key.trim_end_matches("SoundPath");
        settings[format!("{prefix}SoundSource")] =
            json!(if value.as_str().is_none_or(str::is_empty) {
                "Builtin"
            } else {
                "Custom"
            });
    }
    if key == "MusicExpandedLayout" {
        settings["MusicCustomLayout"]["enabled"] = json!(false);
    }
    for (dst, src) in [
        ("TransportTheme", "transportTheme"),
        ("ProgressBarStyle", "barStyle"),
        ("ProgressBarThumb", "thumbStyle"),
        ("ProgressBarThumbEnabled", "thumbEnabled"),
        ("ProgressBarThumbSpin", "thumbSpin"),
    ] {
        if key == dst {
            settings["MusicCustomLayout"][src] = value.clone();
        }
    }
    if key == "MusicCustomLayout" && value["enabled"] == true {
        for (dst, src) in [
            ("TransportTheme", "transportTheme"),
            ("ProgressBarStyle", "barStyle"),
            ("ProgressBarThumb", "thumbStyle"),
            ("ProgressBarThumbEnabled", "thumbEnabled"),
            ("ProgressBarThumbSpin", "thumbSpin"),
        ] {
            settings[dst] = value[src].clone();
        }
    }
    if let Some(name) = key.strip_prefix("Motion") {
        settings["Motion"][name] = value;
    } else if let Some(name) = key.strip_prefix("Spring") {
        settings["Motion"]["Spring"][name] = value;
    } else {
        settings[key] = value;
    }
    Ok(settings)
}

#[cfg(test)]
mod unified_osd_tests {
    use super::*;
    #[test]
    fn mini_material_has_an_independent_clear_default_and_three_choices() {
        let d = defaults();
        assert_eq!(d["IslandSizeMode"], "Classic");
        assert_eq!(d["IslandMaterial"], "LightField");
        assert_eq!(d["MiniMaterial"], "LightField");
        for value in ["LightField", "Acrylic", "LiquidGlass"] {
            let s =
                validate_setting(json!({"key":"MiniMaterial","value":value,"settings":d})).unwrap();
            assert_eq!(s["MiniMaterial"], value);
            assert_eq!(s["IslandMaterial"], "LightField");
        }
        for value in [json!(true), json!("glass"), json!("Glass")] {
            assert!(
                validate_setting(json!({"key":"MiniMaterial","value":value,"settings":d})).is_err()
            );
        }
    }
    #[test]
    fn one_osd_switch_controls_all_legacy_aliases() {
        let s = validate_setting(
            json!({"key":"SystemStateHintsEnabled","value":false,"settings":defaults()}),
        )
        .unwrap();
        for key in [
            "SystemStateHintsEnabled",
            "CapsLockHintEnabled",
            "NumLockHintEnabled",
            "MicrophoneMuteHintEnabled",
            "TouchpadHintEnabled",
        ] {
            assert_eq!(s[key], false);
        }
        let s = validate_setting(json!({"key":"CapsLockHintEnabled","value":true,"settings":s}))
            .unwrap();
        assert_eq!(s["SystemStateHintsEnabled"], true);
        assert_eq!(s["TouchpadHintEnabled"], true);
        assert!(validate_setting(
            json!({"key":"SystemStateHintsEnabled","value":"true","settings":s})
        )
        .is_err());
    }
}

#[cfg(test)]
mod workflow_module_tests {
    use super::*;
    fn accept(value: Value) -> Result<Value, String> {
        validate_setting(json!({"key":"WorkflowModules","value":value,"settings":defaults()}))
    }
    #[test]
    fn the_library_ships_the_six_built_in_modules() {
        let modules = defaults()["WorkflowModules"].clone();
        let list = modules.as_array().unwrap();
        assert_eq!(list.len(), 6);
        assert_eq!(list[1]["name"], "专注");
        assert_eq!(list[1]["minutes"], 25);
        let ids = list
            .iter()
            .map(|m| m["id"].as_str().unwrap())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(ids.len(), 6);
    }
    #[test]
    fn modules_are_normalized_to_id_name_minutes_and_kind() {
        let s = accept(
            json!([{"id":"user-1","name":" 专注 ","minutes":25,"kind":1,"extra":"dropped"}]),
        )
        .unwrap();
        assert_eq!(
            s["WorkflowModules"],
            json!([{"id":"user-1","name":"专注","minutes":25,"kind":1}])
        );
    }
    #[test]
    fn an_empty_library_is_valid_because_every_module_can_be_deleted() {
        assert_eq!(accept(json!([])).unwrap()["WorkflowModules"], json!([]));
    }
    #[test]
    fn duplicate_ids_are_rejected_even_when_names_differ() {
        assert!(accept(json!([{"id":"same","name":"一","minutes":5,"kind":0},{"id":"same","name":"二","minutes":5,"kind":0}])).is_err());
    }
    #[test]
    fn names_minutes_and_kinds_are_bounded() {
        for module in [
            json!({"id":"a","name":"","minutes":5,"kind":0}),
            json!({"id":"a","name":"   ","minutes":5,"kind":0}),
            json!({"id":"a","name":"名","minutes":0,"kind":0}),
            json!({"id":"a","name":"名","minutes":241,"kind":0}),
            json!({"id":"a","name":"名","minutes":5,"kind":6}),
            json!({"id":"bad id","name":"名","minutes":5,"kind":0}),
            json!({"id":"a","name":"名".repeat(41),"minutes":5,"kind":0}),
        ] {
            assert!(accept(json!([module])).is_err());
        }
    }
    #[test]
    fn the_capacity_limit_is_twenty_four() {
        let modules = (0..25)
            .map(|i| json!({"id":format!("m-{i}"),"name":format!("模块{i}"),"minutes":10,"kind":5}))
            .collect::<Vec<_>>();
        assert!(accept(json!(modules)).is_err());
    }
}

#[cfg(test)]
mod player_filter_tests {
    use super::*;
    /// A default 2.0.2 install allowed six of ten. Upgrading keeps exactly that behaviour: the four it
    /// did not allow stay silent, and the six it did keep responding.
    #[test]
    fn an_upgraded_install_keeps_what_it_had_switched_off() {
        let mut s = defaults();
        s["AllowedSources"] = json!([
            "Netease",
            "QQMusic",
            "Kugou",
            "Kuwo",
            "Spotify",
            "TwilightEcho"
        ]);
        migrate_player_filter(&mut s);
        assert_eq!(
            s["IgnoredSources"],
            json!(["Foobar2000", "MusicBee", "AIMP", "Browser"])
        );
        assert!(
            s.get("AllowedSources").is_none(),
            "the old key must not survive to fight the new one"
        );
        assert_eq!(s["PlayerFilterVersion"], 2);
        // TwilightEcho is no longer a source. An old file that mentions it must not put a name the app
        // cannot recognise back into the list.
        assert!(!s["IgnoredSources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == "TwilightEcho"));
    }
    /// The whole point of the change: a player that list never mentioned responds without being added.
    #[test]
    fn a_fresh_install_only_silences_the_browser() {
        let s = defaults();
        assert_eq!(s["IgnoredSources"], json!(["Browser"]));
        assert!(s.get("AllowedSources").is_none());
    }
    /// The verification harnesses pin the island to a fake session by writing the old key; the migration
    /// has to silence every real player for that pin to still mean what it did.
    #[test]
    fn a_pinned_verification_install_silences_every_built_in_player() {
        let mut s = defaults();
        s["AllowedSources"] = json!(["LyricIsland.Verify13"]);
        migrate_player_filter(&mut s);
        assert_eq!(s["IgnoredSources"].as_array().map(Vec::len), Some(9));
        assert!(!s["IgnoredSources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == "LyricIsland.Verify13"));
    }
    /// Idempotent: the conversion runs on every load until a save drops the old key, and a second pass
    /// must not reinterpret its own output.
    #[test]
    fn running_the_conversion_twice_changes_nothing() {
        let mut s = defaults();
        s["AllowedSources"] = json!(["Netease"]);
        migrate_player_filter(&mut s);
        let once = s["IgnoredSources"].clone();
        migrate_player_filter(&mut s);
        assert_eq!(s["IgnoredSources"], once);
    }
}

#[cfg(test)]
mod material_tests {
    use super::*;
    #[test]
    fn material_defaults_to_lightfield_and_accepts_three_values() {
        let s = defaults();
        assert_eq!(s["AutoStart"], true);
        assert_eq!(s["IslandMaterial"], "LightField");
        for value in ["LightField", "Acrylic"] {
            let updated = validate_setting(
                json!({"key":"IslandMaterial","value":value,"settings":s.clone()}),
            )
            .unwrap();
            assert_eq!(updated["IslandMaterial"], value);
        }
        assert!(
            validate_setting(json!({"key":"IslandMaterial","value":"acrylic","settings":s}))
                .is_err()
        );
    }
    #[test]
    fn legacy_glass_flag_migrates_once() {
        let p = std::env::temp_dir().join(format!(
            "dotisle-material-upgrade-{}.json",
            std::process::id()
        ));
        std::fs::write(&p,r#"{"AutoStart":false,"IslandGlassMaterial":true,"Motion":{"Duration":600,"CollapseDuration":550}}"#).unwrap();
        let s = load(&p);
        assert_eq!(s["IslandMaterial"], "Acrylic");
        assert_eq!(s["AutoStart"], false);
        assert_eq!(s["Motion"]["Duration"], 600.);
        std::fs::write(&p, r#"{"IslandGlassMaterial":false}"#).unwrap();
        let s = load(&p);
        assert_eq!(s["IslandMaterial"], "LightField");
        std::fs::write(
            &p,
            r#"{"IslandMaterial":"Glass","IslandGlassMaterial":false}"#,
        )
        .unwrap();
        let s = load(&p);
        assert_eq!(s["IslandMaterial"], "LightField");
        let _ = std::fs::remove_file(p);
    }
}

#[cfg(test)]
mod classic_capsule_tests {
    use super::*;
    #[test]
    fn capsule_material_is_independent_and_validated() {
        let d = defaults();
        assert_eq!(d["ClassicCapsuleMaterial"], "LightField");
        for material in ["LightField", "Acrylic", "LiquidGlass"] {
            let s = validate_setting(
                json!({"key":"ClassicCapsuleMaterial","value":material,"settings":d}),
            )
            .unwrap();
            assert_eq!(s["ClassicCapsuleMaterial"], material);
            assert_eq!(s["MiniMaterial"], "LightField");
            assert_eq!(s["IslandMaterial"], "LightField");
        }
        for value in [
            json!(true),
            json!("Black"),
            json!("glass"),
            json!("Glass"),
        ] {
            assert!(validate_setting(
                json!({"key":"ClassicCapsuleMaterial","value":value,"settings":d})
            )
            .is_err());
        }
    }
}

#[cfg(test)]
mod expanded_material_tests {
    use super::*;
    use crate::renderer::Material;
    #[test]
    fn fresh_install_starts_in_classic_and_hides_paused_music() {
        let path = std::env::temp_dir().join(format!(
            "dotisle-fresh-material-defaults-{}.json",
            std::process::id()
        ));
        let fresh = load(&path);
        assert_eq!(fresh["HideWhenPaused"], true);
        assert_eq!(fresh["IslandKeepVisibleEnabled"], false);
        assert_eq!(fresh["IslandSizeMode"], "Classic");
        assert_eq!(fresh["Motion"]["Radius"], 30.);
        assert_eq!(fresh["CoverBackground"], false);
        assert_eq!(fresh["SystemStateHintsEnabled"], true);
        assert_eq!(fresh["NotificationsEnabled"], true);
        std::fs::write(&path, r#"{"HideWhenPaused":false,"IslandSizeMode":"Mini"}"#).unwrap();
        let existing = load(&path);
        assert_eq!(existing["HideWhenPaused"], false);
        assert_eq!(existing["IslandKeepVisibleEnabled"], false);
        assert_eq!(existing["IslandSizeMode"], "Mini");
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn expanded_material_aliases_share_one_choice_and_survive_reload() {
        let path =
            std::env::temp_dir().join(format!("dotisle-mini-card-{}.json", std::process::id()));
        let mut settings = defaults();
        assert_eq!(settings["MiniExpandedMaterial"], "LightField");
        for classic in ["LightField", "Acrylic"] {
            for mini in ["LightField", "Acrylic"] {
                settings = validate_setting(
                    json!({"key":"IslandMaterial","value":classic,"settings":settings}),
                )
                .unwrap();
                settings = validate_setting(
                    json!({"key":"MiniExpandedMaterial","value":mini,"settings":settings}),
                )
                .unwrap();
                std::fs::write(&path, serde_json::to_vec(&settings).unwrap()).unwrap();
                settings = load(&path);
                assert_eq!(settings["IslandMaterial"], mini);
                assert_eq!(settings["MiniExpandedMaterial"], mini);
                assert_eq!(settings["MiniMaterial"], "LightField");
                assert_eq!(settings["ClassicCapsuleMaterial"], "LightField");
                assert_eq!(
                    Material::expanded(&settings, false),
                    Material::from_setting(mini)
                );
                assert_eq!(
                    Material::expanded(&settings, true),
                    Material::from_setting(mini)
                );
            }
        }
        for invalid in [json!(true), json!("glass"), json!("Glass"), json!("")] {
            assert!(validate_setting(
                json!({"key":"MiniExpandedMaterial","value":invalid,"settings":settings})
            )
            .is_err());
        }
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn retired_material_migrates_and_does_not_override_new_card_preference() {
        let path = std::env::temp_dir().join(format!(
            "dotisle-retired-material-{}.json",
            std::process::id()
        ));
        for (stored, mini) in [
            (
                json!({"IslandMaterial":"LiquidGlass","MiniMaterial":"LiquidGlass","ClassicCapsuleMaterial":"LiquidGlass","PopupMaterial":"Glass","GlassRefractionEnabled":true,"MiniAutoContrast":true}),
                "LiquidGlass",
            ),
            (json!({"MiniMaterial":"Classic"}), "LightField"),
            (
                json!({"MiniMaterial":"LiquidGlass","MiniExpandedMaterial":"Acrylic"}),
                "Acrylic",
            ),
            (
                json!({"MiniMaterial":"Glass","MiniExpandedMaterial":"invalid"}),
                "LightField",
            ),
            (json!({}), "LightField"),
        ] {
            std::fs::write(&path, serde_json::to_vec(&stored).unwrap()).unwrap();
            let updated = load(&path);
            assert_eq!(updated["MiniExpandedMaterial"], mini);
            assert!(updated.get("GlassRefractionEnabled").is_none());
            assert!(updated.get("MiniAutoContrast").is_none());
            for key in ["IslandMaterial", "MiniMaterial", "ClassicCapsuleMaterial", "PopupMaterial"] {
                match stored[key].as_str() {
                    Some("Glass") => assert_eq!(updated[key], "LightField"),
                    Some("LiquidGlass") => assert_eq!(updated[key], "LiquidGlass"),
                    _ => {}
                }
            }
            std::fs::write(&path, serde_json::to_vec(&updated).unwrap()).unwrap();
            assert_eq!(load(&path), updated);
        }
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn both_capsules_accept_three_materials_and_liquid_glass_tuning() {
        let mut settings = defaults();
        assert_eq!(settings["ClassicCapsuleMaterial"], "LightField");
        assert_eq!(settings["MiniMaterial"], "LightField");
        for key in ["ClassicCapsuleMaterial", "MiniMaterial", "IslandMaterial"] {
            for value in ["LightField", "Acrylic", "LiquidGlass"] {
                settings =
                    validate_setting(json!({"key":key,"value":value,"settings":settings})).unwrap();
                assert_eq!(settings[key], value);
            }
            for invalid in [json!(true), json!("glass"), json!("Glass"), json!("")] {
                assert!(
                    validate_setting(json!({"key":key,"value":invalid,"settings":settings}))
                        .is_err()
                );
            }
        }
        for k in ["LiquidGlassRefraction", "LiquidGlassDispersion", "LiquidGlassBlur", "LiquidGlassTint"] {
            let ok = validate_setting(json!({"key":k,"value":55.,"settings":settings})).unwrap();
            assert_eq!(ok[k], 55.);
            assert!(validate_setting(json!({"key":k,"value":105.,"settings":settings})).is_err());
        }
        let src = validate_setting(json!({"key":"LiquidGlassSource","value":"Wallpaper","settings":settings})).unwrap();
        assert_eq!(src["LiquidGlassSource"], "Wallpaper");
        assert!(validate_setting(json!({"key":"LiquidGlassSource","value":"Invalid","settings":settings})).is_err());
    }
}

#[cfg(test)]
mod monet_preference_tests {
    use super::*;
    #[test]
    fn all_material_defaults_are_lightfield_without_monet() {
        let s = defaults();
        for key in [
            "IslandMaterial",
            "MiniExpandedMaterial",
            "ClassicCapsuleMaterial",
            "MiniMaterial",
        ] {
            assert_eq!(s[key], "LightField");
        }
        assert_eq!(s["PopupMaterial"], "Acrylic");
        assert_eq!(s["CoverBackground"], false);
        assert_eq!(s["ExpandedMonetEnabled"], true);
        assert_eq!(s["LyricColorFromCover"], true);
        assert_eq!(s["ExpandedCoverOverlay"], false);
        assert!(s.get("IslandFluidMotion").is_none());
        assert!(
            validate_setting(json!({"key":"IslandFluidMotion","value":false,"settings":s}))
                .is_err()
        );
    }
    #[test]
    fn legacy_implicit_tint_is_disabled_once_and_explicit_opt_in_persists() {
        let path = std::env::temp_dir().join(format!(
            "dotisle-monet-migration-{}.json",
            std::process::id()
        ));
        std::fs::write(&path,r#"{"CoverBackground":true,"IslandFluidMotion":false,"IslandMaterial":"Glass","MiniMaterial":"Classic"}"#).unwrap();
        let migrated = load(&path);
        assert_eq!(migrated["CoverBackground"], false);
        assert_eq!(migrated["IslandMaterial"], "LightField");
        assert_eq!(migrated["MiniMaterial"], "LightField");
        assert!(migrated.get("IslandFluidMotion").is_none());
        let enabled =
            validate_setting(json!({"key":"CoverBackground","value":true,"settings":migrated}))
                .unwrap();
        std::fs::write(&path, serde_json::to_vec(&enabled).unwrap()).unwrap();
        assert_eq!(load(&path)["CoverBackground"], true);
        let disabled =
            validate_setting(json!({"key":"CoverBackground","value":false,"settings":enabled}))
                .unwrap();
        std::fs::write(&path, serde_json::to_vec(&disabled).unwrap()).unwrap();
        assert_eq!(load(&path)["CoverBackground"], false);
        assert!(validate_setting(
            json!({"key":"CoverBackground","value":"true","settings":disabled})
        )
        .is_err());
        let _ = std::fs::remove_file(path);
    }
}
#[cfg(test)]
mod acrylic_slider_tests {
    use super::*;
    #[test]
    fn the_card_starts_frosted_and_the_strip_starts_untouched() {
        let d = defaults();
        assert_eq!(
            (
                d["AcrylicCardFrost"].as_f64(),
                d["AcrylicCardOpacity"].as_f64()
            ),
            (Some(40.), Some(60.))
        );
        assert_eq!(
            (
                d["AcrylicCapsuleFrost"].as_f64(),
                d["AcrylicCapsuleOpacity"].as_f64()
            ),
            (Some(40.), Some(0.))
        );
        assert_eq!(d["AcrylicAdjustTarget"], "Card");
    }
    #[test]
    fn both_sliders_are_clamped_to_a_percentage_and_the_target_is_a_closed_set() {
        let d = defaults();
        for key in [
            "AcrylicCardFrost",
            "AcrylicCardOpacity",
            "AcrylicCapsuleFrost",
            "AcrylicCapsuleOpacity",
        ] {
            for value in [json!(0.), json!(55.), json!(100.)] {
                let s = validate_setting(json!({"key":key,"value":value,"settings":d})).unwrap();
                assert_eq!(s[key], value);
            }
            for value in [json!(-1.), json!(101.), json!("50")] {
                assert!(validate_setting(json!({"key":key,"value":value,"settings":d})).is_err());
            }
        }
        let s =
            validate_setting(json!({"key":"AcrylicAdjustTarget","value":"Capsule","settings":d}))
                .unwrap();
        assert_eq!(s["AcrylicAdjustTarget"], "Capsule");
        assert!(
            validate_setting(json!({"key":"AcrylicAdjustTarget","value":"Body","settings":d}))
                .is_err()
        );
    }
}

#[cfg(test)]
mod expanded_appearance_tests {
    use super::*;
    #[test]
    fn new_defaults_and_old_preferences_round_trip_independently() {
        let file = std::env::temp_dir().join(format!(
            "dotisle-expanded-preferences-{}.json",
            std::process::id()
        ));
        std::fs::write(
            &file,
            r#"{"IslandMonetPreferenceVersion":1,"CoverBackground":false,"AcrylicCardOpacity":75}"#,
        )
        .unwrap();
        let mut s = load(&file);
        assert_eq!(s["PopupMaterial"], "Acrylic");
        assert_eq!(s["CoverBackground"], false);
        assert_eq!(s["ExpandedMonetEnabled"], true);
        assert_eq!(s["LyricColorFromCover"], true);
        assert_eq!(s["ExpandedCoverOverlay"], false);
        assert_eq!(s["AcrylicCardOpacity"], 75.);
        for key in ["ExpandedMonetEnabled", "ExpandedCoverOverlay"] {
            s = validate_setting(json!({"key":key,"value":true,"settings":s})).unwrap();
        }
        s = validate_setting(json!({"key":"PopupMaterial","value":"LightField","settings":s})).unwrap();
        std::fs::write(&file, serde_json::to_vec(&s).unwrap()).unwrap();
        let restored = load(&file);
        assert_eq!(restored["CoverBackground"], false);
        assert_eq!(restored["PopupMaterial"], "LightField");
        for key in ["ExpandedMonetEnabled", "ExpandedCoverOverlay"] {
            assert_eq!(restored[key], true);
            assert!(
                validate_setting(json!({"key":key,"value":"false","settings":restored})).is_err()
            );
        }
        let _ = std::fs::remove_file(file);
    }
}

#[cfg(test)]
mod retired_theme_tests {
    use super::*;
    #[test]
    fn the_hidden_theme_keys_are_dropped_and_the_classic_layout_survives() {
        let file =
            std::env::temp_dir().join(format!("dotisle-retired-theme-{}.json", std::process::id()));
        std::fs::write(&file,r#"{"MusicControlTheme":"Alcove","AlcoveThemeUnlocked":true,"MusicExpandedLayout":"LyricRight"}"#).unwrap();
        let loaded = load(&file);
        assert!(loaded.get("MusicControlTheme").is_none());
        assert!(loaded.get("AlcoveThemeUnlocked").is_none());
        assert_eq!(loaded["MusicExpandedLayout"], "SideBySide");
        assert!(validate_setting(
            json!({"key":"MusicControlTheme","value":"Alcove","settings":loaded.clone()})
        )
        .is_err());
        assert!(validate_setting(
            json!({"key":"AlcoveThemeUnlocked","value":true,"settings":loaded.clone()})
        )
        .is_err());
        let _ = std::fs::remove_file(file);
    }
}

#[cfg(test)]
mod taskbar_layout_setting_tests {
    use super::*;
    #[test]
    fn free_dock_sliders_are_accepted() {
        let mut s = defaults();
        s = validate_setting(json!({"key":"TaskbarPosition","value":64.,"settings":s})).unwrap();
        assert_eq!(s["TaskbarPosition"], 64.);
        s = validate_setting(json!({"key":"TaskbarWidth","value":420.,"settings":s})).unwrap();
        assert_eq!(s["TaskbarWidth"], 420.);
        assert!(validate_setting(json!({"key":"TaskbarWidth","value":900.,"settings":s.clone()})).is_err());
        assert!(validate_setting(json!({"key":"TaskbarPosition","value":-1.,"settings":s})).is_err());
    }
}

#[cfg(test)]
mod translation_button_tests {
    use super::*;
    /// One switch hides the card's 译 button; the translation text preference is a different
    /// decision and neither may silently rewrite the other.
    #[test]
    fn the_translation_button_switch_defaults_on_and_round_trips() {
        let d = defaults();
        assert_eq!(d["TranslationButton"], true);
        assert_eq!(d["Translation"], true);
        let missing = load(std::path::Path::new(
            "no-such-settings-for-translation-button.json",
        ));
        assert_eq!(missing["TranslationButton"], true);
        let mut s = validate_setting(json!({"key":"TranslationButton","value":false,"settings":d}))
            .unwrap();
        assert_eq!(s["TranslationButton"], false);
        assert_eq!(s["Translation"], true);
        s = validate_setting(json!({"key":"Translation","value":false,"settings":s})).unwrap();
        assert_eq!(s["Translation"], false);
        assert_eq!(s["TranslationButton"], false);
        for invalid in [json!("false"), json!(0), json!(null)] {
            assert!(validate_setting(
                json!({"key":"TranslationButton","value":invalid,"settings":s.clone()})
            )
            .is_err());
        }
        let file = std::env::temp_dir().join(format!(
            "dotisle-translation-button-{}.json",
            std::process::id()
        ));
        std::fs::write(&file, serde_json::to_vec(&s).unwrap()).unwrap();
        let restored = load(&file);
        assert_eq!(restored["TranslationButton"], false);
        assert_eq!(restored["Translation"], false);
        std::fs::write(&file, r#"{"Translation":true,"TranslationButton":"yes"}"#).unwrap();
        assert_eq!(load(&file)["TranslationButton"], true);
        let _ = std::fs::remove_file(file);
    }
}

#[cfg(test)]
mod expressive_preferences {
    use super::*;
    #[test]
    fn pastel_beta_is_explicit_and_message_preferences_round_trip() {
        let path=std::env::temp_dir().join(format!("dotisle-pastel-message-{}.json",std::process::id()));
        std::fs::write(&path,br#"{"ControlCenterMonetEnabled":true}"#).unwrap();
        let mut s=load(&path);
        assert_eq!(s["ControlCenterDynamicColorEnabled"],false);
        s=validate_setting(json!({"key":"ControlCenterDynamicColorEnabled","value":true,"settings":s})).unwrap();
        s=validate_setting(json!({"key":"ControlCenterCustomColor","value":"#A9D8C2","settings":s})).unwrap();
        assert_eq!(s["WeChatMessageHintsEnabled"],false);
        for key in ["FeishuMessageHintsEnabled","DingTalkMessageHintsEnabled","WeComMessageHintsEnabled"] {
            assert_eq!(s[key],true);
            s=validate_setting(json!({"key":key,"value":false,"settings":s})).unwrap();
        }
        std::fs::write(&path,serde_json::to_vec(&s).unwrap()).unwrap();
        let restored=load(&path);
        assert_eq!(restored["ControlCenterDynamicColorEnabled"],true);
        assert_eq!(restored["ControlCenterCustomColor"],"#A9D8C2");
        for key in ["WeChatMessageHintsEnabled","FeishuMessageHintsEnabled","DingTalkMessageHintsEnabled","WeComMessageHintsEnabled"] {assert_eq!(restored[key],false);}
        assert!(validate_setting(json!({"key":"ControlCenterCustomColor","value":"green","settings":restored})).is_err());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn centre_monet_and_lyric_colour_are_independent_and_persist_false() {
        let d = defaults();
        let s =
            validate_setting(json!({"key":"ControlCenterMonetEnabled","value":false,"settings":d}))
                .unwrap();
        assert_eq!(s["ExpandedMonetEnabled"], true);
        assert_eq!(s["LyricColorFromCover"], true);
        let s = validate_setting(json!({"key":"ExpandedMonetEnabled","value":false,"settings":s}))
            .unwrap();
        let s = validate_setting(json!({"key":"LyricColorFromCover","value":false,"settings":s}))
            .unwrap();
        assert_eq!(s["ControlCenterMonetEnabled"], false);
        assert_eq!(s["ExpandedMonetEnabled"], false);
        assert_eq!(s["LyricColorFromCover"], false);
    }
    #[test]
    fn right_layout_and_shared_expanded_material_are_persisted() {
        let path =
            std::env::temp_dir().join(format!("dotisle-immersive-{}.json", std::process::id()));
        let s = validate_setting(
            json!({"key":"MusicExpandedLayout","value":"LyricFocus","settings":defaults()}),
        )
        .unwrap();
        let s =
            validate_setting(json!({"key":"MiniExpandedMaterial","value":"Acrylic","settings":s}))
                .unwrap();
        assert_eq!(s["IslandMaterial"], "Acrylic");
        assert_eq!(s["MiniExpandedMaterial"], "Acrylic");
        std::fs::write(&path, serde_json::to_vec(&s).unwrap()).unwrap();
        let loaded = load(&path);
        let _ = std::fs::remove_file(path);
        assert_eq!(loaded["MusicExpandedLayout"], "LyricFocus");
        assert_eq!(loaded["IslandMaterial"], "Acrylic");
    }
    #[test]
    fn the_default_font_survives_and_unknown_system_families_are_rejected() {
        let s = validate_setting(
            json!({"key":"IslandFontFamily","value":"Noto Sans SC","settings":defaults()}),
        )
        .unwrap();
        assert_eq!(s["IslandFontFamily"], "Noto Sans SC");
        for name in ["Space Mono", "Doto"] {
            assert_eq!(
                validate_setting(json!({"key":"IslandFontFamily","value":name,"settings":defaults()}))
                    .unwrap()["IslandFontFamily"],
                name
            );
        }
        for name in ["Definitely Missing Font 123", ""] {
            assert!(validate_setting(
                json!({"key":"IslandFontFamily","value":name,"settings":defaults()})
            )
            .is_err());
        }
        let file =
            std::env::temp_dir().join(format!("dotisle-font-fallback-{}.json", std::process::id()));
        std::fs::write(
            &file,
            r#"{"IslandFontFamily":"Definitely Missing Font 123"}"#,
        )
        .unwrap();
        assert_eq!(load(&file)["IslandFontFamily"], "Noto Sans SC");
        let _ = std::fs::remove_file(file);
    }
}

#[cfg(test)]
mod player_filter_ui_tests {
    use super::*;
    #[test]
    fn the_response_profile_defaults_to_every_player_with_browser_off() {
        let d = defaults();
        assert_eq!(d["PlayerResponseMode"], "All");
        assert_eq!(d["BrowserMediaEnabled"], false);
        assert_eq!(
            d["SelectedSources"].as_array().map(Vec::len),
            Some(dotisle_domain::players::ALL.len())
        );
        assert_eq!(d["IgnoredSources"], json!(["Browser"]));
    }
    #[test]
    fn the_mode_browser_and_selection_are_closed_sets() {
        let d = defaults();
        for mode in ["All", "Selected"] {
            let s = validate_setting(
                json!({"key":"PlayerResponseMode","value":mode,"settings":d.clone()}),
            )
            .unwrap();
            assert_eq!(s["PlayerResponseMode"], mode);
        }
        assert!(validate_setting(
            json!({"key":"PlayerResponseMode","value":"Some","settings":d.clone()})
        )
        .is_err());
        for v in [json!(true), json!(false)] {
            let s = validate_setting(
                json!({"key":"BrowserMediaEnabled","value":v,"settings":d.clone()}),
            )
            .unwrap();
            assert_eq!(s["BrowserMediaEnabled"], v);
        }
        let s = validate_setting(
            json!({"key":"SelectedSources","value":["Netease","Netease","QQMusic"],"settings":d}),
        )
        .unwrap();
        assert_eq!(s["SelectedSources"], json!(["Netease", "QQMusic"]));
    }
}
#[cfg(test)]
mod progress_style_tests {
    use super::*;
    #[test]
    fn both_seek_bar_choices_default_to_the_shipped_look_and_reject_unknown_values() {
        let d = defaults();
        assert_eq!(d["ProgressBarStyle"], "Normal");
        assert_eq!(d["ProgressBarThumb"], "Pill");
        assert_eq!(d["ProgressBarThumbEnabled"], false);
        for style in dotisle_render::progress::BarStyle::ALL {
            let s = validate_setting(
                json!({"key":"ProgressBarStyle","value":style.key(),"settings":d.clone()}),
            )
            .unwrap();
            assert_eq!(s["ProgressBarStyle"], style.key());
        }
        for style in dotisle_render::progress::ThumbStyle::ALL {
            let s = validate_setting(
                json!({"key":"ProgressBarThumb","value":style.key(),"settings":d.clone()}),
            )
            .unwrap();
            assert_eq!(s["ProgressBarThumb"], style.key());
        }
        for bad in [json!("Bouncy"), json!(true), json!(null)] {
            assert!(validate_setting(
                json!({"key":"ProgressBarStyle","value":bad,"settings":d.clone()})
            )
            .is_err());
            assert!(validate_setting(
                json!({"key":"ProgressBarThumb","value":bad,"settings":d.clone()})
            )
            .is_err());
        }
        for value in [json!(true), json!(false)] {
            let s = validate_setting(
                json!({"key":"ProgressBarThumbEnabled","value":value,"settings":d.clone()}),
            )
            .unwrap();
            assert_eq!(s["ProgressBarThumbEnabled"], value);
        }
        for bad in [json!("true"), json!(1), json!(null)] {
            assert!(validate_setting(
                json!({"key":"ProgressBarThumbEnabled","value":bad,"settings":d.clone()})
            )
            .is_err());
        }
    }
    #[test]
    fn an_unknown_stored_style_falls_back_once_and_a_known_one_survives_reload() {
        let file = std::env::temp_dir().join(format!(
            "dotisle-progress-style-{}.json",
            std::process::id()
        ));
        std::fs::write(&file,r#"{"ProgressBarStyle":"Bouncy","ProgressBarThumb":"Star","ProgressBarThumbEnabled":"yes"}"#).unwrap();
        let loaded = load(&file);
        assert_eq!(loaded["ProgressBarStyle"], "Normal");
        assert_eq!(loaded["ProgressBarThumb"], "Pill");
        assert_eq!(loaded["ProgressBarThumbEnabled"], false);
        std::fs::write(&file, serde_json::to_vec(&loaded).unwrap()).unwrap();
        assert_eq!(load(&file), loaded);
        std::fs::write(&file,r#"{"ProgressBarStyle":"Segmented","ProgressBarThumb":"Clover8","ProgressBarThumbEnabled":true}"#).unwrap();
        let restored = load(&file);
        assert_eq!(restored["ProgressBarStyle"], "Segmented");
        assert_eq!(restored["ProgressBarThumb"], "Clover8");
        assert_eq!(restored["ProgressBarThumbEnabled"], true);
        let _ = std::fs::remove_file(file);
    }
}

#[cfg(test)]
mod custom_layout_settings_tests {
    use super::*;
    #[test]
    fn custom_layout_styles_are_one_transaction_and_survive_reload() {
        let mut layout = lyric_core::custom_layout::Layout {
            enabled: true,
            ..Default::default()
        };
        layout.bar_style = "Dots".into();
        layout.transport_theme = "Halo".into();
        layout.thumb_enabled = true;
        layout.move_module(5, 0);
        let s = validate_setting(
            json!({"key":"MusicCustomLayout","value":layout,"settings":defaults()}),
        )
        .unwrap();
        assert_eq!(s["ProgressBarStyle"], "Dots");
        assert_eq!(s["TransportTheme"], "Halo");
        let path = std::env::temp_dir().join(format!("custom-layout-{}.json", std::process::id()));
        std::fs::write(&path, serde_json::to_vec(&s).unwrap()).unwrap();
        let loaded = load(&path);
        let _ = std::fs::remove_file(path);
        assert_eq!(
            loaded["MusicCustomLayout"],
            serde_json::to_value(layout).unwrap()
        );
    }
    #[test]
    fn malformed_layout_cannot_replace_settings_and_builtin_selection_can_disable_it() {
        let before = defaults();
        let mut bad = before["MusicCustomLayout"].clone();
        bad["modules"][0] = bad["modules"][1].clone();
        assert!(
            validate_setting(json!({"key":"MusicCustomLayout","value":bad,"settings":before}))
                .is_err()
        );
        let mut s = defaults();
        s["MusicCustomLayout"]["enabled"] = json!(true);
        let next =
            validate_setting(json!({"key":"MusicExpandedLayout","value":"Separated","settings":s}))
                .unwrap();
        assert_eq!(next["MusicCustomLayout"]["enabled"], false);
    }
}

#[cfg(test)]
mod resident_setting_tests {
    use super::*;
    #[test]
    fn resident_is_opt_in_validated_and_persisted() {
        let path = std::env::temp_dir().join(format!("dotisle-resident-{}.json", std::process::id()));
        let mut settings = defaults();
        assert_eq!(settings["IslandKeepVisibleEnabled"], false);
        for invalid in [json!("true"), json!(1), Value::Null] {
            assert!(validate_setting(json!({"key":"IslandKeepVisibleEnabled","value":invalid,"settings":settings})).is_err());
        }
        settings = validate_setting(json!({"key":"IslandKeepVisibleEnabled","value":true,"settings":settings})).unwrap();
        std::fs::write(&path, serde_json::to_vec(&settings).unwrap()).unwrap();
        assert_eq!(load(&path)["IslandKeepVisibleEnabled"], true);
        let _ = std::fs::remove_file(path);
    }
}
