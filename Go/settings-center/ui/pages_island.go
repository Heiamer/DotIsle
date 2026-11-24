// The island settings body (island_settings.rs), tabs 128–134. Shared row builders
// mirror the Rust helpers: heading (+44), toggle panels (76 per row + 12), choice
// panels (108), range panels (96), color rows (74).
package ui

import (
	"math"
	"strings"

	"github.com/egoist/mygo/ui"
)

var materialOptions = [2][2]string{{"曜光玻璃", "LightField"}, {"亚克力", "Acrylic"}}

// islandSetting reads one island setting with its default. Motion/Spring keys are
// flat wire names but live nested in the state map (settings.json stores a Motion
// object); model.rs setting() does the same prefix demux for the Rust UI.
func (a *App) islandSetting(key string, def any) any {
	s := a.settings()
	if rest, ok := strings.CutPrefix(key, "Motion"); ok {
		if g, ok := s["Motion"].(map[string]any); ok {
			if v, ok := g[rest]; ok && v != nil {
				return v
			}
		}
		return def
	}
	if rest, ok := strings.CutPrefix(key, "Spring"); ok {
		if g, ok := s["Motion"].(map[string]any); ok {
			if sp, ok := g["Spring"].(map[string]any); ok {
				if v, ok := sp[rest]; ok && v != nil {
					return v
				}
			}
		}
		return def
	}
	if v, ok := s[key]; ok {
		return v
	}
	return def
}

func (a *App) islandBool(key string, def bool) bool {
	v, _ := a.islandSetting(key, def).(bool)
	return v
}

func (a *App) islandString(key, def string) string {
	v, _ := a.islandSetting(key, def).(string)
	return v
}

func (a *App) islandFloat(key string, def float64) float64 {
	v, _ := a.islandSetting(key, def).(float64)
	return v
}

// islandHeading is the 21px section title.
func (a *App) islandHeading(c *ui.Context, p *Palette, title string, x, y, w float32) float32 {
	Label(c, title, x+16, y+3, 14, p.Scheme.OnSurfaceVariant, true, w-32)
	return y + 31
}

// islandToggles is the toggle panel: rows of 76 DIP with the switch at the right.
func (a *App) islandToggles(c *ui.Context, p *Palette, x, y, w float32, rows []islandToggleRow) float32 {
	s := &p.Scheme
	settingCard(c, x, y, w, float32(len(rows))*settingToggleH, s.Container)
	for i, row := range rows {
		on := a.islandBool(row.key, row.def)
		top := y + float32(i)*settingToggleH
		if i != 0 {
			settingSeparator(c, p, x+16, top, w-32)
		}
		settingToggleRow(c, p, row.key, row.title, row.detail, x+16, top, w-32, on, func() {
			a.pushNative(row.key, !on)
		})
	}
	return y + float32(len(rows))*settingToggleH + 16
}

type islandToggleRow struct {
	title, detail, key string
	def                bool
}

// islandChoices is the choice panel: title plus full-width chips.
func (a *App) islandChoices(c *ui.Context, p *Palette, x, y, w float32, title, key, def string, options [][2]string) float32 {
	s := &p.Scheme
	settingCard(c, x, y, w, 108, s.Container)
	Label(c, title, x+24, y+15, 15, s.OnSurface, true, w-48)
	selected := a.islandString(key, def)
	labels := make([]string, len(options))
	index := -1
	for i, opt := range options {
		labels[i] = opt[0]
		if selected == opt[1] {
			index = i
		}
	}
	SegmentedKeyed(c, p, key, x+24, y+52, w-48, labels, index, func(i int) { a.pushNative(key, options[i][1]) })
	return y + 124
}

// islandRange is the slider row: title, live value at the right, slider below.
func (a *App) islandRange(c *ui.Context, p *Palette, x, y, w float32, title, key string, def, min, max float64, unit string) float32 {
	s := &p.Scheme
	settingCard(c, x, y, w, 96, s.Container)
	value := a.islandFloat(key, def)
	Label(c, title, x+24, y+14, 15, s.OnSurface, false, w-160)
	// While dragging, the live value mirrors the knob (settings_slider.rs trailing label).
	if st := sliderDrags[key]; st != nil && math.Abs(st.frac-clamp01((value-min)/(max-min))) > 1e-6 {
		value = min + st.frac*(max-min)
	}
	Label(c, trimFloat(value)+" "+unit, x+w-132, y+14, 13, s.Primary, true, 108)
	Slider(c, p, key, x+24, y+52, w-48, 28, (value-min)/(max-min), true, func(f float64) {
		a.pushNative(key, math.Round(min+f*(max-min)))
	})
	return y + 112
}

func trimFloat(v float64) string {
	whole := int(v)
	frac := int((v - float64(whole)) * 10)
	if frac < 0 {
		frac = -frac
	}
	return itoa(whole) + "." + itoa(frac)
}

// islandColorRow is the fixed-color row: swatch circle + hex button.
func (a *App) islandColorRow(c *ui.Context, p *Palette, x, y, w float32, title, key, def string) float32 {
	s := &p.Scheme
	settingCard(c, x, y, w, 74, s.Container)
	value := a.islandString(key, def)
	Label(c, title, x+24, y+24, 15, s.OnSurface, false, w-240)
	hex, ok := parseHexColor(value)
	if !ok {
		hex = s.Primary
	}
	ui.Box(c).Absolute().Left(x + w - 160).Top(y + 25).Width(24).Height(24).
		Radius(12).Background(Col(hex))
	entry := Button(c, p, key, x+w-138, y+16, 114, 42, value, "", true, true, func() {
		current := hexValue(value)
		a.openPicker(key, title, current, hexValue(def))
	})
	a.anchorPicker(entry, key)
	return y + 90
}

// islandSettingsBody renders one island tab (island_settings.rs render).
func (a *App) islandSettingsBody(c *ui.Context, p *Palette, x, y, w float32, tab uint8) float32 {
	switch tab {
	case 0:
		return a.islandTabSize(c, p, x, y, w)
	case 1:
		return a.islandTabMaterial(c, p, x, y, w)
	case 2:
		return a.islandTabGestures(c, p, x, y, w)
	case 3:
		return a.islandTabMotion(c, p, x, y, w)
	case 4:
		return a.islandTabPlayers(c, p, x, y, w)
	case 5:
		return a.islandTabProgress(c, p, x, y, w)
	default:
		return a.islandTabTransport(c, p, x, y, w)
	}
}

// islandTabSize is tab 128: 尺寸与排版.
func (a *App) islandTabSize(c *ui.Context, p *Palette, x, y, w float32) float32 {
	y = a.islandToggles(c, p, x, y, w, []islandToggleRow{{
		"随歌词调整灵动岛宽度", "根据当前歌词和译文自动伸缩；关闭后使用固定宽度。", "AutoCompactWidth", true}})
	y = a.islandHeading(c, p, "显示模式", x, y, w)
	y = a.islandChoices(c, p, x, y, w, "收起时样式 · 展开卡片共用", "IslandSizeMode", "Classic",
		[][2]string{{"经典", "Classic"}, {"迷你", "Mini"}})
	y = a.islandHeading(c, p, "音乐展开排版", x, y, w)
	Label(c, "选择一种布局；经典与 Mini 共用，同一首音乐会连续切换。", x, y, 12, p.Scheme.OnSurfaceVariant, false, w-8)
	y += 30
	y = a.musicLayoutChoices(c, p, x, y, w)
	y = a.islandHeading(c, p, "胶囊尺寸", x, y, w)
	y = a.islandRange(c, p, x, y, w, "基础宽度", "CompactWidth", 260, 140, 480, "px")
	y = a.islandRange(c, p, x, y, w, "自适应宽度上限", "AutoCompactMaxWidth", 420, 140, 480, "px")
	y = a.islandRange(c, p, x, y, w, "歌词字号", "FontSize", 14, 11, 20, "px")
	Button(c, p, "推荐字号", x, y, 220, 44, "应用 V3 推荐字号 · 14", "", true, true, func() {
		a.pushNative("FontSize", 14.0)
	})
	y += 64
	y = a.islandHeading(c, p, "字体", x, y, w)
	family := a.islandString("IslandFontFamily", defaultIslandFont)
	familyLabel := "默认字体"
	if family != defaultIslandFont {
		familyLabel = family
	}
	settingCard(c, x, y, w, 150, p.Scheme.Container)
	Label(c, "灵动岛字体", x+24, y+14, 15, p.Scheme.OnSurface, true, 200)
	Label(c, familyLabel, x+w-262, y+14, 12, p.Scheme.OnSurfaceVariant, false, 240)
	ui.Box(c).Absolute().Left(x + 24).Top(y + 42).Width(w - 48).Height(40).
		Radius(10).Background(Col(p.Scheme.High)).Children(func() {
		CenteredText(c, "The quick brown fox · 春夏秋冬", x, 0, w-48, 40, 18, p.Scheme.OnSurface, false)
	})
	Button(c, p, "更改字体", x+24, y+94, 150, 42, "更改字体", "search", true, true, func() {
		a.openFontPicker()
	})
	if family != defaultIslandFont {
		Button(c, p, "恢复默认字体", x+184, y+94, 150, 42, "恢复默认", "", true, true, func() {
			a.pushNative("IslandFontFamily", defaultIslandFont)
		})
	}
	y += 166
	y = a.islandHeading(c, p, "歌词显示", x, y, w)
	y = a.islandToggles(c, p, x, y, w, []islandToggleRow{
		{"逐字高亮", "跟随演唱进度点亮原文。", "WordHighlight", true},
		{"收起胶囊显示翻译", "有译文时显示较小的第二行。", "CompactTranslation", true},
		{"中文双行", "没有翻译时显示下一句。", "ChineseTwoLine", true},
		{"展开卡片显示翻译", "展开后优先显示可用译文。", "Translation", true},
		{"显示译按钮", "在展开卡片的歌词旁显示语言切换按钮。", "TranslationButton", true},
	})
	return y
}

// musicLayoutChoices draws the three redesigned layout preview cards.
func (a *App) musicLayoutChoices(c *ui.Context, p *Palette, x, y, w float32) float32 {
	s := &p.Scheme
	options := [][3]string{
		{"光屿画廊", "SideBySide", "左翼大唱片与中缝曜光"},
		{"流光分栏", "Separated", "电影顶眉与宽幕歌词"},
		{"星幕歌词", "LyricFocus", "极简中轴居中大歌词"},
	}
	selected := a.islandString("MusicExpandedLayout", "SideBySide")
	if selected == "FluidCloud" {
		selected = "SideBySide"
	}
	cw := (w - 24) / 3
	for i, opt := range options {
		cardX := x + float32(i)*(cw+12)
		on := selected == opt[1]
		card := ui.ButtonBase(c).Absolute().Left(cardX).Top(y).Width(cw).Height(156).
			Radius(16).Label("选择" + opt[0])
		if on {
			card.Background(Col(s.PrimaryContainer)).Border(2, Col(s.Primary))
		} else {
			card.Background(Col(s.Container)).Border(1, Col(s.OutlineVariant))
		}
		if card.Clicked() {
			a.pushNative("MusicExpandedLayout", opt[1])
		}
		// 呼应三套重制桌面端布局的结构示意图
		card.Children(func() {
			ink := s.OnSurface
			switch opt[1] {
			case "SideBySide":
				ui.Box(c).Absolute().Left(12).Top(14).Width(42).Height(42).Radius(9).Background(Col(s.High))
				ui.Box(c).Absolute().Left(12).Top(60).Width(30).Height(4).Radius(2).Background(Col(s.Primary))
				ui.Box(c).Absolute().Left(62).Top(16).Width(cw - 86).Height(7).Radius(3.5).Background(Col(ink))
				ui.Box(c).Absolute().Left(60).Top(34).Width(2).Height(18).Radius(1).Background(Col(s.Primary))
				ui.Box(c).Absolute().Left(66).Top(36).Width(cw - 80).Height(7).Radius(3.5).Background(Col(ink))
			case "Separated":
				ui.Box(c).Absolute().Left(14).Top(14).Width(24).Height(24).Radius(5).Background(Col(s.High))
				ui.Box(c).Absolute().Left(45).Top(17).Width(cw - 76).Height(6).Radius(3).Background(Col(ink))
				ui.Box(c).Absolute().Left(45).Top(28).Width(cw - 96).Height(5).Radius(2.5).Background(Col(s.OutlineVariant))
				ui.Box(c).Absolute().Left(14).Top(50).Width(cw - 28).Height(8).Radius(4).Background(Col(ink))
				ui.Box(c).Absolute().Left(14).Top(66).Width(cw - 64).Height(6).Radius(3).Background(Col(s.OutlineVariant))
			default:
				ui.Box(c).Absolute().Left(cw/2 - 38).Top(14).Width(22).Height(22).Radius(5).Background(Col(s.High))
				ui.Box(c).Absolute().Left(cw/2 - 10).Top(22).Width(48).Height(6).Radius(3).Background(Col(s.OutlineVariant))
				ui.Box(c).Absolute().Left(18).Top(50).Width(cw - 36).Height(9).Radius(4.5).Background(Col(ink))
				ui.Box(c).Absolute().Left(32).Top(68).Width(cw - 64).Height(6).Radius(3).Background(Col(s.OutlineVariant))
			}
			Label(c, opt[0], 16, 102, 13, s.OnSurface, true, cw-32)
			Label(c, opt[2], 16, 126, 11, s.OnSurfaceVariant, false, cw-32)
		})
	}
	return y + 172
}

// islandTabMaterial is tab 129: 材质与取色.
func (a *App) islandTabMaterial(c *ui.Context, p *Palette, x, y, w float32) float32 {
	y = a.islandHeading(c, p, "动态色彩", x, y, w)
	y = a.islandChoices(c, p, x, y, w, "灵动岛颜色来源", "IslandColorSource", "Content",
		[][2]string{{"专辑封面", "Content"}, {"系统强调色", "System"}, {"自选主色", "Custom"}})
	entry := Button(c, p, "选主色", x, y, 220, 44, "选择灵动岛主色", "", true, true, func() {
		current := hexValue(a.islandString("IslandDynamicSeedColor", "#A9E6CD"))
		a.openPicker("IslandDynamicSeedColor", "灵动岛动态主色", current, 0xA9E6CD)
	})
	a.anchorPicker(entry, "IslandDynamicSeedColor")
	y += 64
	y = a.islandToggles(c, p, x, y, w, []islandToggleRow{
		{"胶囊与迷你细条动态色彩", "收起形态的独立开关，与展开卡片互不影响。", "CoverBackground", false},
		{"展开卡片动态色彩", "从选定来源生成强调色与主题氛围。", "ExpandedMonetEnabled", true},
		{"歌词进度涂色", "跟随演唱进度填充原文歌词。", "WordHighlight", true},
		{"歌词填色动态色彩", "使用同一来源的强调色，与展开卡片背景独立；关闭后使用下方自定填色。", "LyricColorFromCover", true},
		{"封面磨砂叠层", "在亚克力与曜光玻璃展开卡片中叠加高级氛围色场封面。", "ExpandedCoverOverlay", false},
	})
	y = a.islandHeading(c, p, "固定颜色", x, y, w)
	y = a.islandColorRow(c, p, x, y, w, "展开卡片文字", "ExpandedTextColor", "#FFFFFF")
	y = a.islandColorRow(c, p, x, y, w, "展开卡片组件", "ExpandedComponentColor", "#FFFFFF")
	y = a.islandColorRow(c, p, x, y, w, "歌词进度高亮", "LyricProgressColor", "#DDF6FF")
	y = a.islandHeading(c, p, "卡片与弹窗材质", x, y, w)
	for _, row := range []struct{ title, key, def string }{
		{"经典胶囊", "ClassicCapsuleMaterial", "LightField"},
		{"展开卡片 · 两种模式共用", "IslandMaterial", "LightField"},
		{"迷你细条", "MiniMaterial", "LightField"},
		{"弹窗与提醒（通知 / 系统提示 / 验证码）", "PopupMaterial", "Acrylic"},
	} {
		y = a.islandChoices(c, p, x, y, w, row.title, row.key, row.def, materialOptions[:])
	}
	y = a.islandToggles(c, p, x, y, w, []islandToggleRow{})
	y = a.islandHeading(c, p, "灵动岛透明度", x, y, w)
	y = a.islandRange(c, p, x, y, w, "灵动岛整体透明度", "IslandOpacity", 100, 25, 100, "%")
	y = a.islandHeading(c, p, "亚克力叠层", x, y, w)
	y = a.islandRange(c, p, x, y, w, "展开卡片磨砂", "AcrylicCardFrost", 40, 0, 100, "%")
	y = a.islandRange(c, p, x, y, w, "展开卡片叠层强度", "AcrylicCardOpacity", 60, 0, 100, "%")
	y = a.islandRange(c, p, x, y, w, "胶囊磨砂", "AcrylicCapsuleFrost", 40, 0, 100, "%")
	y = a.islandRange(c, p, x, y, w, "胶囊叠层强度", "AcrylicCapsuleOpacity", 0, 0, 100, "%")
	return y
}

// islandTabGestures is tab 130: 交互与系统提示.
func (a *App) islandTabGestures(c *ui.Context, p *Palette, x, y, w float32) float32 {
	s := &p.Scheme
	settingCard(c, x, y, w, 104, s.PrimaryContainer)
	Label(c, "认识你的灵动岛", x+24, y+19, 18, s.OnPrimaryContainer, true, w-230)
	Label(c, "在实际灵动岛上练习展开、切换、隐藏与找回", x+24, y+54, 12, s.OnPrimaryContainer, false, w-230)
	Button(c, p, "练习手势", x+w-174, y+29, 150, 46, "练习手势", "play_arrow", false, true, a.startGestureTutorial)
	y += 128
	y = a.islandHeading(c, p, "出现与退场", x, y, w)
	on := a.islandBool("IslandKeepVisibleEnabled", false)
	settingCard(c, x, y, w, 88, s.Container)
	ToggleRow(c, p, "IslandKeepVisibleEnabled", x+24, y+12, w-48, "灵动岛常驻", "空闲时循环显示你选择的内容。", on, func() {
		a.pushNative("IslandKeepVisibleEnabled", !on)
	})
	y += 104
	if on {
		y = a.islandHeading(c, p, "常驻轮播内容", x, y, w)
		y = a.islandToggles(c, p, x, y, w, []islandToggleRow{
			{"时间", "显示本地时间。当没有可轮播的内容时，也会显示时间。", "IslandResidentShowTime", true},
			{"系统占用", "同时显示 CPU 与内存的当前占用率。", "IslandResidentShowSystem", false},
			{"设备电量", "逐个显示已连接的无线设备。没有连接时跳过。", "IslandResidentShowDevices", false},
		})
	}
	y = a.islandToggles(c, p, x, y, w, []islandToggleRow{
		{"暂停音乐后隐藏", "有计时器等活动或开启常驻时继续显示。", "HideWhenPaused", true},
		{"始终置顶", "不抢占键盘焦点。", "AlwaysOnTop", true},
	})
	y = a.islandHeading(c, p, "手势与位置", x, y, w)
	y = a.islandToggles(c, p, x, y, w, []islandToggleRow{
		{"悬停展开", "鼠标经过胶囊时展开当前活动。", "HoverExpand", false},
		{"点击任意区域展开", "点击胶囊主体展开当前活动。", "ClickAnywhereExpand", true},
		{"左右滑动切换", "在胶囊或卡片空白处拖动，切换音乐、计时与待办。", "SwipeActivities", true},
		{"向上拖动隐藏", "从空白处向上拖动；有通知仍会提醒，也可从托盘或边缘找回。", "SwipeUpHide", true},
		{"鼠标触碰边缘显示", "上滑隐藏后将鼠标移至屏幕顶部居中片刻浮现灵动岛（默认关闭）。", "EdgeHoverRevealEnabled", false},
		{"自由悬浮", "允许直接按住胶囊或卡片拖动到任意位置或屏幕，并自动保存。", "Floating", false},
		{"点击封面或歌名打开播放器", "只影响音乐卡片的封面和标题。", "MusicClickOpenApp", false},
	})
	y = a.islandHeading(c, p, "多显示器位置", x, y, w)
	displayOpts := [][2]string{{"主显示器（默认）", ""}}
	if raw, ok := a.State["displays"].([]any); ok {
		for _, item := range raw {
			if m, ok := item.(map[string]any); ok {
				id, _ := m["id"].(string)
				label, _ := m["label"].(string)
				if id != "" && label != "" {
					displayOpts = append(displayOpts, [2]string{label, id})
				}
			}
		}
	}
	y = a.islandChoices(c, p, x, y, w, "显示在哪个显示器", "Display", "", displayOpts)
	y = a.islandHeading(c, p, "任务栏模式", x, y, w)
	y = a.islandToggles(c, p, x, y, w, []islandToggleRow{
		{"任务栏模式", "在 Windows 任务栏显示实时逐字歌词与微控制条，点击向上展开完整卡片，通知仍照常在屏幕上方弹出。", "TaskbarModeEnabled", false},
		{"任务栏常驻显示", "未播放音乐或暂停时仍在任务栏常驻显示时间与系统状态，避免组件消失。", "TaskbarAlwaysShow", true},
	})
	y = a.islandChoices(c, p, x, y, w, "任务栏外观", "TaskbarStyle", "Transparent",
		[][2]string{{"透明融入", "Transparent"}, {"轻透胶囊", "Pill"}})
	y = a.islandChoices(c, p, x, y, w, "停靠基准", "TaskbarDockAlign", "Free",
		[][2]string{{"自由调节", "Free"}, {"智能自动", "Auto"}, {"任务栏左侧", "Left"}, {"托盘左侧", "Right"}})
	y = a.islandRange(c, p, x, y, w, "水平位置", "TaskbarPosition", 18, 0, 100, "%")
	y = a.islandRange(c, p, x, y, w, "歌词条宽度", "TaskbarWidth", 340, 220, 640, "px")
	y = a.islandHeading(c, p, "系统状态提示", x, y, w)
	y = a.islandToggles(c, p, x, y, w, []islandToggleRow{
		{"系统状态提示", "大小写锁定、数字锁定、麦克风静音与触控板状态。", "SystemStateHintsEnabled", true},
	})
	y = a.islandChoices(c, p, x, y, w, "提示位置", "SystemStateHintPlacement", "Island",
		[][2]string{{"灵动岛内", "Island"}, {"屏幕下方", "Bottom"}})
	Button(c, p, "切换显示", x, y, 152, 44, "显示 / 隐藏", "dialpad", true, true, func() {
		a.command("toggleVisibility", nil)
	})
	Button(c, p, "预览通知", x+168, y, 152, 44, "预览通知", "", true, true, func() {
		a.command("notificationPreview", nil)
	})
	return y + 64
}

// islandTabMotion is tab 131: 光效、动效与圆角.
func (a *App) islandTabMotion(c *ui.Context, p *Palette, x, y, w float32) float32 {
	y = a.islandHeading(c, p, "岛周围的光效", x, y, w)
	y = a.islandToggles(c, p, x, y, w, []islandToggleRow{
		{"音乐光晕", "随音乐呼吸或跟随播放时的输出强弱律动，默认关闭。", "MusicGlowEnabled", false},
		{"计时运行光晕", "经典岛计时运行时柔和呼吸，暂停后收敛。", "TimerGlowEnabled", false},
		{"更新彩虹流光", "经典岛更新时沿轮廓流动，完成后淡出。", "UpdateGlowEnabled", false},
	})
	y = a.islandChoices(c, p, x, y, w, "音乐光晕方式", "MusicGlowMode", "Breath",
		[][2]string{{"柔和呼吸", "Breath"}, {"音乐律动", "Rhythm"}})
	Label(c, "仅经典岛显示外围光效，Mini 不显示。", x, y, 13, p.Scheme.OnSurfaceVariant, false, w)
	y += 36
	y = a.islandHeading(c, p, "动效曲线与弹簧", x, y, w)
	y = a.motionCurveEditor(c, p, x, y, w)
	// def/min/max mirror island_settings.rs motion rows; the ranges must stay
	// inside config.rs validate_setting or the island silently rejects the commit
	// and the knob snaps back (MotionDuration 250–1200, Collapse 150–700, Gap 0–24,
	// Spring* 100–1000/10–80/0.3–2, CapsuleCornerRadius 0–48).
	y = a.islandRange(c, p, x, y, w, "动效时长", "MotionDuration", 480, 250, 1200, "ms")
	y = a.islandRange(c, p, x, y, w, "收起时长", "MotionCollapseDuration", 440, 150, 700, "ms")
	y = a.islandRange(c, p, x, y, w, "卡片间距", "MotionGap", 8, 0, 24, "px")
	y = a.islandRange(c, p, x, y, w, "弹簧质量", "SpringMass", 0.8, 0.3, 2, "")
	y = a.islandRange(c, p, x, y, w, "弹簧刚度", "SpringStiffness", 420, 100, 1000, "")
	y = a.islandRange(c, p, x, y, w, "弹簧阻尼", "SpringDamping", 30, 10, 80, "")
	y = a.islandRange(c, p, x, y, w, "胶囊圆角", "CapsuleCornerRadius", 20, 0, 48, "px")
	return y
}

var playerGroups = [][2]any{
	{"国内平台", [][2]string{{"网易云音乐", "Netease"}, {"QQ 音乐", "QQMusic"}, {"酷狗", "Kugou"}, {"酷我", "Kuwo"}, {"汽水音乐", "SodaMusic"}}},
	{"国外平台", [][2]string{{"Spotify", "Spotify"}, {"Apple Music", "AppleMusic"}, {"YouTube Music", "YouTubeMusic"}}},
	{"本地播放器", [][2]string{{"PotPlayer", "PotPlayer"}, {"Foobar2000", "Foobar2000"}, {"MusicBee", "MusicBee"}, {"AIMP", "AIMP"}, {"Salt Player", "SaltPlayer"}}},
	{"其他来源", [][2]string{{"洛雪音乐", "LXMusic"}, {"MusicFree", "MusicFree"}, {"Cider", "Cider"}, {"YesPlayMusic", "YesPlayMusic"}}},
}

// islandTabPlayers is tab 132: 播放器与同步.
func (a *App) islandTabPlayers(c *ui.Context, p *Palette, x, y, w float32) float32 {
	s := &p.Scheme
	y = a.islandHeading(c, p, "网易云进度拖动", x, y, w)
	settingCard(c, x, y, w, 104, s.PrimaryContainer)
	Label(c, "正常启动 · 自动连接", x+24, y+19, 18, s.OnPrimaryContainer, true, w-48)
	Label(c, "正常启动网易云即可自动接入，后台、最小化和托盘均可拖动进度。", x+24, y+55, 12, s.OnPrimaryContainer, false, w-48)
	y += 128
	y = a.islandHeading(c, p, "响应哪些播放器", x, y, w)
	Label(c, "选择响应范围；默认响应所有播放器，浏览器媒体单独控制。", x, y, 12, s.OnSurfaceVariant, false, w-8)
	y += 36
	mode := a.islandString("PlayerResponseMode", "All")
	settingCard(c, x, y, w, 108, s.Container)
	Label(c, "响应范围", x+24, y+15, 15, s.OnSurface, true, w-48)
	half := (w - 48 - 8) / 2
	Chip(c, p, "PlayerAll", x+24, y+52, half, 38, "所有播放器", mode == "All", func() {
		a.pushNative("PlayerResponseMode", "All")
	})
	Chip(c, p, "PlayerSelected", x+24+half+8, y+52, half, 38, "指定播放器", mode == "Selected", func() {
		a.pushNative("PlayerResponseMode", "Selected")
	})
	y += 124
	if mode == "Selected" {
		selected := strings.ToLower(a.islandString("SelectedSources", ""))
		for _, group := range playerGroups {
			title, _ := group[0].(string)
			players, _ := group[1].([][2]string)
			Label(c, title, x, y, 17, s.OnSurface, true, w)
			y += 36
			cw := (w - 16) / 3
			for i, player := range players {
				pid := player[1]
				on := strings.Contains(selected, strings.ToLower(pid))
				Chip(c, p, "player-"+pid, x+float32(i%3)*(cw+8), y+float32(i/3)*50, cw, 42,
					player[0], on, func() {
						a.togglePlayer(pid)
					})
			}
			y += float32((len(players)+2)/3)*50 + 20
		}
	}
	y = a.islandToggles(c, p, x, y, w, []islandToggleRow{
		{"浏览器媒体", "在浏览器中播放的音频也唤出灵动岛。", "BrowserMediaEnabled", false},
	})
	y = a.islandHeading(c, p, "歌词同步", x, y, w)
	y = a.islandRange(c, p, x, y, w, "同步偏移", "OffsetMilliseconds", 0, -5000, 5000, "ms")
	y = a.islandToggles(c, p, x, y, w, []islandToggleRow{
		{"在线补全歌曲信息", "信息缺失时补全封面、歌名与歌词。", "OnlineSongInfo", true},
	})
	return y
}

// togglePlayer flips one player in the SelectedSources list (model.rs player_selection).
func (a *App) togglePlayer(id string) {
	current := a.islandString("SelectedSources", "")
	var list []string
	for _, item := range strings.Split(current, ";") {
		if item = trimSpace(item); item != "" {
			list = append(list, item)
		}
	}
	found := false
	for i, item := range list {
		if equalFold(item, id) {
			list = append(list[:i], list[i+1:]...)
			found = true
			break
		}
	}
	if !found {
		list = append(list, id)
	}
	a.pushNative("SelectedSources", list)
}

func trimSpace(s string) string {
	start, end := 0, len(s)
	for start < end && (s[start] == ' ' || s[start] == '\t') {
		start++
	}
	for end > start && (s[end-1] == ' ' || s[end-1] == '\t') {
		end--
	}
	return s[start:end]
}

// Keys must match dotisle_render::progress::ThumbStyle; the old Dot/Ring/
// Rounded/Petal names were rejected by the native setting, so clicks did nothing.
var leafProgressThumbs = [][2]string{
	{"胶囊", "Pill"}, {"圆形", "Circle"}, {"方形", "Square"}, {"菱形", "Diamond"},
	{"心形", "Heart"}, {"花朵", "Flower"}, {"贝壳", "Clam"}, {"云朵", "Puffy"},
}

// islandTabProgress is tab 133: 播放进度条. The 8 bar styles and 16 thumb styles
// select through chips; the preview draws the current style.
func (a *App) islandTabProgress(c *ui.Context, p *Palette, x, y, w float32) float32 {
	s := &p.Scheme
	y = a.islandToggles(c, p, x, y, w, []islandToggleRow{{"进度条柔光", "只在已播放的进度周围显示柔光。", "ProgressBarGlow", true}})
	y = a.islandHeading(c, p, "进度条样式", x, y, w)
	barStyles := [][2]string{
		{"标准", "Normal"}, {"波浪", "Wavy"}, {"圆角", "Rounded"}, {"纤细", "Thin"},
		{"厚重", "Thick"}, {"渐变", "Gradient"}, {"分段", "Segmented"}, {"圆点", "Dots"},
	}
	y = a.styleChipGrid(c, p, x, y, w, barStyles, "ProgressBarStyle", "Normal")
	thumbOn := a.islandBool("ProgressBarThumbEnabled", false)
	y = a.islandToggles(c, p, x, y, w, []islandToggleRow{
		{"显示滑块", "进度末端显示当前主题的滑块。", "ProgressBarThumbEnabled", false},
		{"滑块自转", "滑块随播放缓慢旋转。", "ProgressBarThumbSpin", false},
	})
	if thumbOn {
		y = a.islandHeading(c, p, "滑块样式", x, y, w)
		y = a.styleChipGrid(c, p, x, y, w, leafProgressThumbs, "ProgressBarThumb", "Pill")
	}
	// Live preview of the current bar and thumb style at full width.
	settingCard(c, x, y, w, 72, s.Container)
	a.drawProgressPreview(c, p, x+16, y+22, w-32, thumbOn)
	return y + 88
}

// styleChipGrid lays out style chips four per row.
func (a *App) styleChipGrid(c *ui.Context, p *Palette, x, y, w float32, styles [][2]string, key, def string) float32 {
	selected := a.islandString(key, def)
	cw := (w - 48 - 3*8) / 4
	for i, style := range styles {
		value := style[1]
		Chip(c, p, key+value, x+24+float32(i%4)*(cw+8), y+float32(i/4)*50, cw, 38, style[0],
			selected == value, func() {
				a.pushNative(key, value)
			})
	}
	return y + float32((len(styles)+3)/4)*50 + 16
}

// islandTabTransport is tab 134: 播放按钮主题. Ten themes, each with a mini sample.
func (a *App) islandTabTransport(c *ui.Context, p *Palette, x, y, w float32) float32 {
	s := &p.Scheme
	y = a.islandHeading(c, p, "播放按钮主题", x, y, w)
	themes := [][2]string{
		{"礁石", "Reef"}, {"涟漪", "Ripple"}, {"细砂", "Sand"}, {"晨雾", "Mist"},
		{"琉璃", "Glaze"}, {"墨屿", "InkIsle"}, {"双潮", "Tide"}, {"留白", "Whitespace"},
		{"光环", "Halo"}, {"叠影", "Layered"},
	}
	selected := a.islandString("TransportTheme", "Reef")
	cw := (w - 48 - 2*12) / 3
	for i, theme := range themes {
		value := theme[1]
		cardX := x + 24 + float32(i%3)*(cw+12)
		cardY := y + float32(i/3)*120
		on := selected == value
		card := ui.ButtonBase(c).Absolute().Left(cardX).Top(cardY).Width(cw).Height(104).
			Radius(16).Label("选择" + theme[0] + "主题")
		if on {
			card.Background(Col(s.PrimaryContainer)).Border(2, Col(s.Primary))
		} else {
			card.Background(Col(s.Container)).Border(1, Col(s.OutlineVariant))
		}
		if card.Clicked() {
			a.pushNative("TransportTheme", value)
		}
		card.Children(func() {
			// Mini sample: accent play key plus two plain keys, per the theme fill.
			playBg, playInk := Col(s.Primary), Col(s.OnPrimary)
			playRadius, playBorder, playBorderW := float32(12), ui.Transparent, float32(0)
			sideBg := ui.Transparent
			sideRadius := float32(0)
			switch value {
			case "Ripple":
				playRadius = 20
			case "Sand":
				playBg, playInk = ui.Transparent, Col(s.Primary)
				playBorder, playBorderW = Col(s.Primary), 1.1
			case "Mist":
				playBg, playInk = AlphaCol(s.Primary, .20), Col(s.Primary)
				playBorder, playBorderW = AlphaCol(s.Primary, .38), 1
			case "Glaze":
				playBg, playInk = AlphaCol(s.Primary, .30), Col(s.Primary)
				playBorder, playBorderW = AlphaCol(s.Primary, .62), 1
			case "InkIsle":
				playBg, playInk = Col(s.OnSurface), Col(s.Surface)
			case "Tide":
				sideBg, sideRadius = AlphaCol(s.SecondaryContainer, .45), 14
			case "Whitespace":
				playBg, playInk = ui.Transparent, Col(s.Primary)
			case "Halo":
				playBg, playInk = ui.Transparent, Col(s.Primary)
				playBorder, playBorderW = Col(s.Primary), 2.2
			}
			// Row, not the default column: a column stacks the three keys and pushes
			// the play key onto the card's bottom edge.
			key := ui.Box(c).Row().Absolute().Left(cw/2 - 62).Top(14).Width(124).Height(40)
			key.Justify(ui.Center).AlignItems(ui.Center).Gap(8).Children(func() {
				prev := ui.Box(c).Width(30).Height(30).Radius(sideRadius).Background(sideBg).
					Justify(ui.Center).AlignItems(ui.Center)
				prev.Children(func() {
					ui.Icon(c, TransportSymbol(value, "prev")).FontSize(18).TextColor(Col(s.OnSurface))
				})
				play := ui.Box(c).Width(40).Height(40).Justify(ui.Center).AlignItems(ui.Center).
					Radius(playRadius).Background(playBg).Border(playBorderW, playBorder)
				play.Children(func() {
					ui.Icon(c, TransportSymbol(value, "play")).FontSize(20).TextColor(playInk)
				})
				playNext := ui.Box(c).Width(30).Height(30).Radius(sideRadius).Background(sideBg).
					Justify(ui.Center).AlignItems(ui.Center)
				playNext.Children(func() {
					ui.Icon(c, TransportSymbol(value, "next")).FontSize(18).TextColor(Col(s.OnSurface))
				})
			})
			CenteredText(c, theme[0], 0, 66, cw, 24, 12, s.OnSurface, on)
		})
	}
	return y + float32((len(themes)+2)/3)*120 + 16
}
