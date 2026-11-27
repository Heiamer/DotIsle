package ui

import (
	"strconv"
	"strings"

	"github.com/egoist/mygo/ui"
)

// Settings navigation is deliberately two levels. Single-page categories skip
// the subcategory strip. Legacy IPC destinations map to a real leaf on entry.
type settingSection struct {
	tab                uint8
	name, detail, tags string
}

type settingCategory struct {
	name, icon string
	sections   []settingSection
}

var settingsCategories = []settingCategory{
	{"侧边栏与组件", "widgets", []settingSection{
		{10, "效率侧边栏", "启用侧边栏、更改热键与展开方向，查看使用教程。", "侧边栏 快捷键 热键 教程 组件"},
	}},
	{"通用与窗口", "settings", []settingSection{
		{4, "启动与窗口", "启动行为、快捷键和字体加载。", "开机 托盘 关闭 隐藏 启动页 字体"},
	}},
	{"控制中心外观", "light_mode", []settingSection{
		{0, "主题与背景", "选择明暗主题、动态取色与窗口背景。", "颜色 深色 壁纸 背景 莫奈"},
	}},
	{"灵动岛", "dialpad", []settingSection{
		{128, "尺寸与排版", "调整胶囊大小、歌词字号和音乐卡片排版。", "宽度 歌词 迷你 mini 胶囊"},
		{129, "材质与取色", "设置胶囊和展开卡片的材质与颜色。", "曜光玻璃 亚克力 莫奈 动态取色 透明度"},
		{130, "交互与提示", "调整常驻、悬停、拖动、手势和系统状态提示。", "托盘 位置 教程 cpu 时间 电量 滑动"},
		{131, "光效与动效", "调整光效、形变曲线和圆角。", "动画 高光 过渡"},
		{135, "布局编辑器", "编辑音乐卡片元素，并预览应用到灵动岛的效果。", "播放按钮 自定义 控件 进度条 滑块"},
	}},
	{"音乐与歌词", "music_note", []settingSection{
		{132, "播放器与同步", "选择媒体来源，调整同步偏移。", "浏览器 媒体源 播放器 歌词延时"},
		{1, "歌词与本地文件", "设置翻译、高亮和本地歌词匹配。", "翻译 逐字 歌词路径 歌词柔光 LRC"},
		{133, "播放进度条", "设置进度条的显示方式。", "标准 播放进度 滑块 进度条柔光"},
		{134, "播放按钮", "设置播放控件主题。", "上一首 下一首 暂停"},
	}},
	{"通知与提醒", "volume_up", []settingSection{
		{2, "应用通知", "配置通知的接收、筛选与历史保留。", "微信 QQ 屏蔽 暂停 允许列表 清理"},
		{5, "设备连接", "设置耳机、手柄等设备的连接提醒。", "蓝牙 电量 键盘 鼠标"},
		{6, "提醒音效", "为通知、计时和设备连接选择提示音。", "声音 音量 铃声"},
		{7, "按应用提醒", "管理进入应用和使用时长规则。", "提醒规则 应用规则 限时"},
	}},
	{"计时与待办", "timer", []settingSection{
		{8, "计时与待办偏好", "设置锁屏暂停、到期提醒和待办默认颜色。", "专注 番茄钟 颜色"},
	}},
	{"关于与更新", "download", []settingSection{
		{3, "关于与更新", "查看版本、更新说明，管理更新源与渠道。", "检查更新 OTA 正式版 测试版 Gitee GitCode GitHub"},
	}},
}

func settingLeaf(tab uint8) uint8 {
	switch tab {
	case TabRoot, 244:
		return 4
	case 240:
		return 128
	case 241:
		return 132
	case 242:
		return 2
	case 243:
		return 8
	case 245:
		return 3
	case 246:
		return 9
	}
	return tab
}

func settingLocation(tab uint8) (int, settingSection) {
	for i, category := range settingsCategories {
		for _, section := range category.sections {
			if section.tab == tab {
				return i, section
			}
		}
	}
	return 0, settingsCategories[0].sections[0]
}

func searchSettings(query string, matches []uint8) []uint8 {
	for _, category := range settingsCategories {
		for _, section := range category.sections {
			haystack := category.name + " " + section.name + " " + section.detail + " " + section.tags
			if containsFold(haystack, query) {
				matches = append(matches, section.tab)
			}
		}
	}
	return matches
}

type settingsPanelState struct {
	returnPage        int
	query             string
	navScroll, scroll ui.ScrollState
	bodyHeight        float32
}

func (a *App) centerPage() int {
	if a.Page == PageSettings {
		return a.settingsPanel.returnPage
	}
	return a.Page
}

func (a *App) closeSettings() {
	if a.Page == PageSettings {
		a.navigate(a.settingsPanel.returnPage)
	}
}

func (a *App) settingsPanelView(c *ui.Context, w, h float32) {
	if a.Page != PageSettings {
		return
	}
	p := a.Palette
	s := &p.Scheme
	if !c.Preferences().HighContrast {
		// 瑞士建筑级分层白灰磨砂玻璃：外层略带温润浅灰（深色纯正炭黑），托出内部纯白卡片切片。
		if p.Dark {
			s.Surface, s.Container, s.High, s.SecondaryContainer = 0x121418, 0x1B1D23, 0x252830, 0x1F2229
			s.OnSurface, s.OnSurfaceVariant = 0xF3F4F6, 0x9CA3AF
		} else {
			s.Surface, s.Container, s.High, s.SecondaryContainer = 0xF3F4F6, 0xFFFFFF, 0xE9EBEE, 0xECEEF1
			s.OnSurface, s.OnSurfaceVariant = 0x14161A, 0x6B7280
		}
	}
	state := &a.settingsPanel
	category, section := settingLocation(settingLeaf(a.SettingsTab))
	group := settingsCategories[category]
	margin := min(float32(24), max(float32(8), (w-760)/4))
	pw, ph := min(w-margin*2, float32(1260)), h-32
	side := max(float32(154), min(float32(238), pw*.225))
	ui.Overlay(c, func() {
		backdrop := AlphaCol(0xDDE3ED, 0.72)
		if p.Dark {
			backdrop = AlphaCol(0x06080C, 0.72)
		}
		if opticalReduced(c, &p) {
			backdrop = AlphaCol(0, .38)
		}
		back := ui.Box(c).Key("settings-overlay").Absolute().Fill().Modal().Role(ui.RoleDialog).
			Label("点屿设置面板").Background(backdrop)
		if back.OverlayShortcut(0, ui.KeyEscape) {
			a.closeSettings()
			c.AnimationFrame()
			return
		}
		back.Children(func() {
			panel := ui.Box(c).Key("settings-panel").Absolute().Left((w - pw) / 2).Top(16).Width(pw).Height(ph)
			opticalOverlayStyle(c, &panel, &p, 24, false, a.opticalFlowAngle(315))
			panel.Children(func() {
				if !opticalReduced(c, &p) {
					sideFrost := AlphaCol(0xEDF0F5, 0.55)
					if p.Dark {
						sideFrost = AlphaCol(0x0E1015, 0.45)
					}
					ui.Box(c).Absolute().Fill().IgnoreInput(true).Draw(func(pc *ui.Painter, r ui.Rect) {
						pc.Clip(r, 24, func() {
							pc.Fill(ui.Rect{X: r.X, Y: r.Y, W: side, H: r.H}, sideFrost, 0)
						})
					})
				}
				divider := AlphaCol(0x111827, .06)
				if p.Dark {
					divider = AlphaCol(0xFFFFFF, .08)
				}
				ui.Box(c).Absolute().Left(side).Top(22).Width(1).Height(ph - 44).IgnoreInput(true).Background(divider)
				a.settingsSidebar(c, &p, side, ph, category)
				x := side + 26
				Label(c, group.name, x, 25, 29, s.OnSurface, true, pw-x-59)
				Label(c, section.detail, x, 68, 13, s.OnSurfaceVariant, false, pw-x-34)
				bodyW, top := pw-x-26, float32(99)
				if len(group.sections) > 1 {
					top = a.settingsSubcategories(c, &p, group.sections, section.tab, x, top, bodyW) + 10
				}
				ui.Scroll(c).Key("settings-body-" + strconv.Itoa(int(section.tab))).Absolute().Left(x - 3).Top(top).
					Width(bodyW + 14).Height(ph - top - 16).Scrollbars(false).TrackScroll(&state.scroll).
					Label("设置选项").Children(func() {
					ui.Box(c).Width(bodyW + 9).Height(state.bodyHeight).Children(func() {
						height := a.settingsBody(c, &p, section.tab, 3, 5, bodyW) + 12
						if state.bodyHeight != height {
							state.bodyHeight = height
							c.AnimationFrame()
						}
					})
				})
				close := ui.ButtonBase(c).Key("settings-close").Absolute().Left(pw - 46).Top(15).Width(32).Height(32).
					Radius(16).Label("关闭设置").Cursor(ui.CursorPointer)
				closeBg := AlphaCol(0x111827, 0.05)
				if p.Dark {
					closeBg = AlphaCol(0xFFFFFF, 0.08)
				}
				close.Background(closeBg)
				hoverLayer(&close, closeBg, s.OnSurface, true)
				if close.Clicked() {
					a.closeSettings()
				}
				close.Children(func() { Icon(c, "close", 8, 8, 16, s.OnSurfaceVariant).IgnoreInput(true) })
			})
		})
	})
}

func (a *App) settingsSidebar(c *ui.Context, p *Palette, w, h float32, active int) {
	state, s := &a.settingsPanel, &p.Scheme
	searchBox := ui.Box(c).Absolute().Left(14).Top(22).Width(w - 28).Height(37).Radius(10).IgnoreInput(true)
	if p.HighContrast {
		searchBox.Background(Col(s.SecondaryContainer))
	} else if p.Dark {
		searchBox.Background(AlphaCol(0xFFFFFF, 0.06)).Border(1, AlphaCol(0xFFFFFF, 0.08))
	} else {
		searchBox.Background(AlphaCol(0xFFFFFF, 0.85)).Border(1, AlphaCol(0x111827, 0.07)).
			InsetShadow(0, 1, 0, 0, AlphaCol(0xFFFFFF, 0.95))
	}
	Icon(c, "search", 25, 32, 17, s.OnSurfaceVariant).IgnoreInput(true)
	old := state.query
	ui.TextInput(c, &state.query).Absolute().Left(47).Top(22).Width(w-63).Height(37).
		Placeholder("搜索设置").Font(UIFont).FontSize(13).TextColor(Col(s.OnSurface)).Background(ui.Transparent).
		Border(0, ui.Transparent).Label("搜索设置").OnTextKey(func(k ui.TextKey) bool {
		if k.Mods != 0 || k.Key != ui.KeyEnter {
			return false
		}
		query := strings.TrimSpace(k.Text())
		if matches := searchSettings(query, nil); query != "" && len(matches) != 0 {
			a.openSettings(matches[0])
			return true
		}
		return false
	})
	if old != state.query {
		state.navScroll = ui.ScrollState{}
	}
	query := strings.TrimSpace(state.query)
	if query == "" {
		badge := ui.Box(c).Absolute().Left(17).Top(76).Width(37).Height(37).Radius(11).IgnoreInput(true)
		badgeInk := s.OnSurface
		if p.HighContrast {
			badge.Background(Col(s.SecondaryContainer))
			badgeInk = s.Primary
		} else if p.Dark {
			badge.Background(AlphaCol(0xFFFFFF, 0.08)).Border(1, AlphaCol(0xFFFFFF, 0.12))
			badgeInk = 0xF3F4F6
		} else {
			badge.Background(Col(0xFFFFFF)).Border(1, AlphaCol(0x111827, 0.07)).
				Shadow(0, 2, 6, -1, AlphaCol(0x111827, 0.06))
			badgeInk = 0x16181D
		}
		Icon(c, "dialpad", 24, 83, 23, badgeInk).IgnoreInput(true)
		Label(c, "点屿", 64, 73, 16, s.OnSurface, true, w-80)
		Label(c, "设置", 64, 96, 12, s.OnSurfaceVariant, false, w-80)
	}
	top := float32(131)
	if query != "" {
		top = 74
	}
	ui.Scroll(c).Key("settings-navigation").Absolute().Left(12).Top(top).Width(w - 24).Height(h - top - 19).
		Scrollbars(false).TrackScroll(&state.navScroll).Label("设置分类").Children(func() {
		if query == "" {
			ui.Box(c).Width(w - 24).Height(float32(len(settingsCategories)) * 43).Children(func() {
				for i, category := range settingsCategories {
					tab := category.sections[0].tab
					a.settingsNavItem(c, p, "设置分类 "+category.name, category.name, category.icon, w-24, float32(i)*43, i == active, func() {
						if i != active {
							a.openSettings(tab)
						}
					})
				}
			})
			return
		}
		var buffer [24]uint8
		matches := searchSettings(query, buffer[:0])
		if len(matches) == 0 {
			Label(c, "没有匹配的设置", 6, 6, 13, s.OnSurfaceVariant, false, w-36)
			return
		}
		ui.Box(c).Width(w - 24).Height(float32(len(matches)) * 43).Children(func() {
			for i, tab := range matches {
				category, section := settingLocation(tab)
				a.settingsNavItem(c, p, "设置搜索结果 "+section.name, section.name, settingsCategories[category].icon,
					w-24, float32(i)*43, false, func() { a.openSettings(tab) })
			}
		})
	})
}

func (a *App) settingsNavItem(c *ui.Context, p *Palette, label, name, icon string, w, y float32, selected bool, action func()) {
	s := &p.Scheme
	base, tile, ink := ui.Transparent, s.SecondaryContainer, s.OnSurfaceVariant
	if selected {
		base, tile, ink = Col(s.High), s.Primary, s.OnPrimary
	}
	e := ui.ButtonBase(c).Key(label).Absolute().Left(0).Top(y).Width(w).Height(39).Radius(11).Background(base).
		Label(label).AccessSelected(selected).Cursor(ui.CursorPointer)
	if !p.HighContrast && selected {
		if p.Dark {
			base = AlphaCol(0xFFFFFF, 0.10)
			e.Background(base).Border(1, AlphaCol(0xFFFFFF, 0.10)).
				InsetShadow(0, 1, 0, 0, AlphaCol(0xFFFFFF, 0.12))
		} else {
			base = Col(0xFFFFFF)
			e.Background(base).Border(1, AlphaCol(0x111827, 0.055)).
				InsetShadow(0, 1, 0, 0, AlphaCol(0xFFFFFF, 1.0)).
				Shadow(0, 2, 8, -2, AlphaCol(0x111827, 0.07))
		}
	}
	hoverLayer(&e, base, s.OnSurface, true)
	if e.Clicked() {
		action()
	}
	e.Children(func() {
		tileBox := ui.Box(c).Absolute().Left(8).Top(7).Width(25).Height(25).Radius(7).IgnoreInput(true)
		if selected && !p.HighContrast {
			if p.Dark {
				tileBox.Background(Col(0xF3F4F6))
				ink = 0x111316
			} else {
				tileBox.Background(Col(0x16181D))
				ink = 0xFFFFFF
			}
		} else if !p.HighContrast {
			if p.Dark {
				tileBox.Background(AlphaCol(0xFFFFFF, 0.07))
			} else {
				tileBox.Background(AlphaCol(0x111827, 0.05))
			}
		} else {
			tileBox.Background(Col(tile))
		}
		Icon(c, icon, 13, 12, 15, ink).IgnoreInput(true)
		Label(c, name, 42, 10, 14, s.OnSurface, selected, w-50).IgnoreInput(true)
	})
}

func (a *App) settingsSubcategories(c *ui.Context, p *Palette, sections []settingSection, active uint8, x, y, w float32) float32 {
	s := &p.Scheme
	left := x
	for _, section := range sections {
		bw := float32(len([]rune(section.name))*13 + 22)
		if left > x && left+bw > x+w {
			left, y = x, y+36
		}
		e := ui.ButtonBase(c).Key("settings-section-" + strconv.Itoa(int(section.tab))).Absolute().Left(left).Top(y).
			Width(bw).Height(31).Radius(10).Label("设置分组 " + section.name).AccessSelected(section.tab == active).
			Cursor(ui.CursorPointer)
		base, ink := Col(s.SecondaryContainer), s.OnSurfaceVariant
		if !p.HighContrast {
			if section.tab == active {
				if p.Dark {
					base, ink = AlphaCol(0xFFFFFF, 0.14), s.OnSurface
					e.Border(1, AlphaCol(0xFFFFFF, 0.14))
				} else {
					base, ink = Col(0xFFFFFF), s.OnSurface
					e.Border(1, AlphaCol(0x111827, 0.06)).Shadow(0, 2, 6, -2, AlphaCol(0x111827, 0.06))
				}
			} else {
				if p.Dark {
					base = AlphaCol(0xFFFFFF, 0.06)
				} else {
					base = AlphaCol(0x111827, 0.045)
				}
			}
		} else if section.tab == active {
			base, ink = Col(s.High), s.OnSurface
		}
		e.Background(base)
		hoverLayer(&e, base, s.OnSurface, true)
		if e.Clicked() {
			a.openSettings(section.tab)
		}
		e.Children(func() { CenteredText(c, section.name, 0, 0, bw, 31, 13, ink, section.tab == active).IgnoreInput(true) })
		left += bw + 6
	}
	return y + 31
}

func (a *App) settingsBody(c *ui.Context, p *Palette, tab uint8, x, y, w float32) float32 {
	switch {
	case tab == TabLayoutEditor:
		return a.musicLayoutEditor(c, p, x, y, w)
	case tab >= TabIslandFirst && tab <= TabIslandLast:
		return a.islandSettingsBody(c, p, x, y, w, tab-TabIslandFirst)
	default:
		return a.leafSettingsBody(c, p, x, y, w, tab)
	}
}

// settingCard 绘制洁净超白/黑曜连续曲率分组卡，托出建筑分层秩序。
func settingCard(c *ui.Context, x, y, w, h float32, ink uint32) ui.Element {
	dark := c.Theme().Dark
	highContrast := c.Preferences().HighContrast
	fill := AlphaCol(0xFFFFFF, 0.94)
	border := AlphaCol(0x1E293B, 0.08)
	topRim := AlphaCol(0xFFFFFF, 0.96)
	if dark {
		fill = AlphaCol(ink, 0.88)
		border = AlphaCol(0xFFFFFF, 0.085)
		topRim = AlphaCol(0xFFFFFF, 0.12)
	}
	if highContrast {
		fill = Col(ink)
		border = ui.Transparent
		topRim = ui.Transparent
	}
	card := ui.Box(c).Absolute().Left(x).Top(y).Width(w).Height(h)
	px, py, over := card.PointerPosition()
	return card.Draw(func(pc *ui.Painter, r ui.Rect) {
		if highContrast {
			pc.FillPath(continuousCardPath(r, 18), fill)
			return
		}
		paintContinuousCard(pc, r, 18, fill, border)
		inner := ui.Rect{X: r.X + 1, Y: r.Y + 1, W: r.W - 2, H: r.H - 2}
		if inner.W > 4 && inner.H > 4 {
			pc.Clip(inner, 16, func() {
				sheen := AlphaCol(0xFFFFFF, 0.18)
				if dark {
					sheen = AlphaCol(0xFFFFFF, 0.035)
				}
				// 覆盖完整内区避免 44px 短高度截断线
				pc.FillGradient(inner, ui.LinearGradient{
					From:  sheen,
					To:    AlphaCol(0xFFFFFF, 0),
					Angle: 165,
				}, 0)
				if over && r.W > 0 && r.H > 0 {
					spot := AlphaCol(0xFFFFFF, 0.28)
					if dark {
						spot = AlphaCol(0xFFFFFF, 0.06)
					}
					pc.FillRadialGradient(inner, ui.RadialGradient{
						Center: ui.Point{X: clamp01F(px / r.W), Y: clamp01F(py / r.H)},
						Radius: min(max(r.W, r.H)*0.40, 175),
						Stops: []ui.GradientStop{
							{Color: spot, At: 0},
							{Color: AlphaCol(0xFFFFFF, 0), At: 1},
						},
					}, 0)
				}
			})
			if r.W > 40 {
				pc.Line(r.X+18, r.Y+0.6, r.X+r.W-18, r.Y+0.6, 1, topRim)
			}
		}
	})
}

func settingSeparator(c *ui.Context, p *Palette, x, y, w float32) {
	ui.Box(c).Absolute().Left(x).Top(y).Width(w).Height(1).Background(AlphaCol(p.Scheme.OnSurface, .09)).IgnoreInput(true)
}

const settingToggleH float32 = 58

func settingToggleRow(c *ui.Context, p *Palette, label, title, detail string, x, y, w float32, on bool, toggle func()) {
	if detail == "" {
		Label(c, title, x, y+(settingToggleH-LineHeight(14))/2, 14, p.Scheme.OnSurface, false, w-66)
	} else {
		Label(c, title, x, y+10, 14, p.Scheme.OnSurface, false, w-66)
		Label(c, detail, x, y+31, 12, p.Scheme.OnSurfaceVariant, false, w-66)
	}
	Switch(c, p, label, x+w-51, y+(settingToggleH-26)/2, on, toggle)
}
