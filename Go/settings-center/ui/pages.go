// Page bodies. The settings hub is fully routed; the remaining pages land page by
// page, each ported from its Vello renderer at the same geometry.
package ui

import (
	"github.com/egoist/mygo/ui"
)

var groupCards = [6][2]string{
	{"外观与灵动岛", "主题、排版、材质、字体与动效"},
	{"音乐与歌词", "播放器、歌词、进度与播放按钮"},
	{"通知与提醒", "应用通知、设备、音效与应用规则"},
	{"计时与待办", "专注习惯、到期提醒与默认颜色"},
	{"通用与窗口", "开机启动、窗口行为与快捷键"},
	{"关于与更新", "版本信息与自动更新"},
}

var groupIcons = [6]string{"dialpad", "music_note", "volume_up", "timer", "settings", "download"}

var groupColors = [6]uint32{0x5877d5, 0xd45a6c, 0xb88833, 0x6e59c4, 0x72727e, 0x5381b4}

// settingsHubName mirrors settings_hub::name.
func settingsHubName(tab uint8) string {
	switch tab {
	case 9:
		return "模块管理"
	case 135:
		return "灵动岛布局编辑器"
	case 0:
		return "控制中心外观"
	case 1:
		return "歌词与本地文件"
	case 2:
		return "应用通知与历史"
	case 3:
		return "关于与更新"
	case 4:
		return "启动、窗口与字体"
	case 5:
		return "设备连接提醒"
	case 6:
		return "提醒音效"
	case 7:
		return "按应用提醒"
	case 8:
		return "计时与待办偏好"
	case 128:
		return "灵动岛尺寸与排版"
	case 129:
		return "材质与取色"
	case 130:
		return "交互与系统提示"
	case 131:
		return "光效、动效与圆角"
	case 132:
		return "播放器与同步"
	case 133:
		return "播放进度条"
	case 134:
		return "播放按钮主题"
	}
	return "设置"
}

func settingsHubParent(tab uint8) uint8 {
	switch {
	case tab == 0 || (tab >= 128 && tab <= 131) || tab == 135:
		return 240
	case tab == 1 || (tab >= 132 && tab <= 134):
		return 241
	case tab == 2 || (tab >= 5 && tab <= 7):
		return 242
	case tab == 8:
		return 243
	case tab == 4:
		return 244
	case tab == 3:
		return 245
	}
	return 255
}

// linkCard is settings_hub::link: an 84px acrylic card with the colored icon tile.
func (a *App) linkCard(c *ui.Context, p *Palette, x, y, w float32, title, detail string, tab uint8) {
	s := &p.Scheme
	card := ui.ButtonBase(c).Absolute().Left(x).Top(y).Width(w).Height(84).
		Label("打开" + title).Cursor(ui.CursorPointer)
	surfaceStyle(&card, p, s.Container, CardRadius)
	if card.Hovered() {
		card.Background(Col(s.High))
	}
	if card.Clicked() {
		a.openSettings(tab)
	}
	// settings_hub.rs: the card's own group tab uses itself; leaves use their parent.
	group := int(settingsHubParent(tab))
	if tab >= 240 && tab <= 245 {
		group = int(tab)
	}
	index := group - 240
	if index < 0 || index > 5 {
		index = 5
	}
	card.Children(func() {
		ui.Box(c).Absolute().Left(20).Top(23).Width(38).Height(38).
			Radius(10).Background(Col(s.High))
		ui.Icon(c, Symbol(groupIcons[index])).FontSize(20).TextColor(Col(s.OnSurfaceVariant)).
			Absolute().Left(29).Top(32)
		Label(c, title, 74, 19, 14, s.OnSurface, true, w-118)
		Label(c, detail, 74, 45, 12, s.OnSurfaceVariant, false, w-118)
		// The trailing chevron, stroked 1.5 like the Vello arrow path.
		ui.Box(c).Absolute().Left(w - 32).Top(37).Width(8).Height(10).Draw(func(pc *ui.Painter, r ui.Rect) {
			var path ui.Path
			path.MoveTo(r.X, r.Y)
			path.LineTo(r.X+5, r.Y+5)
			path.LineTo(r.X, r.Y+10)
			pc.StrokePath(&path, 1.5, Col(s.OnSurfaceVariant))
		})
	})
}

// settingsHeaderExtra draws the search affordance next to the header on hub pages.
func (a *App) settingsHeaderExtra(c *ui.Context, w float32) {
	// The search icon button is part of the titlebar row; nothing extra yet.
	_ = c
	_ = w
}

func (a *App) searchButton(c *ui.Context, w float32) {
	IconButton(c, &a.Palette, "搜索与跳转", w-378, 10, 36, 36, "search", false, true,
		a.togglePalette)
}

// settingsPage is settings_hub::render: root cards, group lists and leaf routes.
func (a *App) settingsPage(c *ui.Context, x, y, w float32) float32 {
	p := &a.Palette
	s := &p.Scheme
	tab := a.SettingsTab
	switch tab {
	case TabLayoutEditor:
		return a.musicLayoutEditor(c, p, x, y, w)
	case TabRoot:
		cw := (w - 16) / 2
		for i, card := range groupCards {
			a.linkCard(c, p, x+float32(i%2)*(cw+16), y+float32(i/2)*100, cw, card[0], card[1], uint8(240+i))
		}
		return y + 300 + 28
	}
	isGroup := tab >= 240 && tab <= 242
	direct := tab == 3 || tab == 4 || tab == 8 || tab == 9
	backLabel, backTab := "返回分类", settingsHubParent(tab)
	if isGroup || direct {
		backLabel, backTab = "全部设置", TabRoot
	}
	Button(c, p, "返回", x, y, 128, 40, backLabel, "arrow_back", true, true, func() {
		a.openSettings(backTab)
	})
	title := backLabelGroup(tab)
	if !isGroup {
		title = settingsHubName(tab)
	}
	Label(c, title, x+152, y+9, 16, s.OnSurface, true, w-168)
	y += 64
	if isGroup {
		for _, entry := range groupEntries(tab) {
			a.linkCard(c, p, x, y, w, settingsHubName(entry.tab), entry.detail, entry.tab)
			y += 100
		}
		return y + 28
	}
	switch {
	case tab >= TabIslandFirst && tab <= TabIslandLast:
		return a.islandSettingsBody(c, p, x, y, w) + 28
	default:
		return a.leafSettingsPage(c, p, x, y, w, tab) + 28
	}
}

func backLabelGroup(tab uint8) string {
	if tab >= 240 && tab <= 245 {
		return groupCards[tab-240][0]
	}
	return "设置"
}

type hubEntry struct {
	tab    uint8
	detail string
}

// groupEntries mirrors the settings_hub group lists.
func groupEntries(tab uint8) []hubEntry {
	switch tab {
	case 240:
		return []hubEntry{
			{135, "组合歌曲信息、歌词、播放按钮与进度条，预览并应用"},
			{0, "控制中心主题与背景"},
			{128, "模式、大小、字体与音乐排版"},
			{129, "胶囊、卡片与 Mini 的材质和配色"},
			{130, "悬停、手势、位置与系统状态提示"},
			{131, "音乐、计时与更新光效开关，形变曲线和圆角"},
		}
	case 241:
		return []hubEntry{
			{135, "10 套播放按钮主题、8 种进度条与 16 种滑块"},
			{1, "翻译、高亮与本地歌词文件"},
			{132, "播放器响应范围、浏览器媒体与同步偏移"},
		}
	case 242:
		return []hubEntry{
			{2, "接收、暂停、允许与屏蔽列表、历史保留"},
			{5, "耳机、音响、手柄与输入设备"},
			{6, "通知、待办计时与设备提醒音"},
			{7, "进入应用与使用时长规则"},
		}
	case 243:
		return []hubEntry{{8, "到期提醒、锁屏暂停与默认颜色"}}
	case 244:
		return []hubEntry{{4, "启动页面、开机启动、快捷键与字体范围"}}
	}
	return []hubEntry{{3, "手写字标、版本信息与双源更新"}}
}

// leafSettingsPage hosts the leaf tabs 0..8; each lands with its own port.
func (a *App) leafSettingsPage(c *ui.Context, p *Palette, x, y, w float32, tab uint8) float32 {
	return a.leafSettingsBody(c, p, x, y, w, tab)
}

func (a *App) placeholderPage(c *ui.Context, p *Palette, x, y, w float32, text string) {
	Label(c, text, x, y+24, 14, p.Scheme.OnSurfaceVariant, false, w-40)
}
