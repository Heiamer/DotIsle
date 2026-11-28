// Compact, live overview cards. Every value and action uses the existing core contract.
package ui

import (
	"fmt"
	"github.com/egoist/mygo/ui"
	"math"
	"sort"
	"strings"
)

const overviewCardHeight float32 = 264

func (a *App) renderOverviewCard(c *ui.Context, r overviewRect) {
	a.overviewCardSurface(c, r)
	a.renderOverviewCardBody(c, r)
}

// A scrolling page owns its shell; nested information tiles still use the
// complete renderer above, so their individual surfaces remain intact.
func (a *App) renderOverviewCardBody(c *ui.Context, r overviewRect) {
	if strings.HasPrefix(r.key, "plugin:") {
		a.pluginCard(c, r)
		return
	}
	if overviewIsMore(r.key) {
		a.overviewMore(c, r)
		return
	}
	if r.h < 180 {
		Label(c, overviewName(r.key), r.x+20, r.y+20, 14, a.Palette.Scheme.OnSurface, true, r.w-40)
		return
	}
	switch r.key {
	case "aiQuota":
		a.dashboardQuota(c, r)
	case "music":
		previous := a.Palette
		a.Palette = a.musicCardPalette()
		defer func() { a.Palette = previous }()
		a.dashboardMusic(c, r)
	case "widget:system":
		a.dashboardSystem(c, r)
	case "focus":
		a.dashboardFocus(c, r)
	case "todos":
		a.dashboardTodos(c, r)
	case "quick":
		a.dashboardQuick(c, r)
	case "clock":
		a.overviewClock(c, r)
	case "information":
		a.overviewInformation(c, r)
	default:
		a.overviewExtra(c, r)
	}
}

func (a *App) cardHeader(c *ui.Context, r overviewRect, title string) {
	s := &a.Palette.Scheme
	if r.w >= 620 {
		overviewText(c, title, r.x+overviewInset, r.y+20, r.w-160, overviewTitle, s.OnSurface, 1)
		return
	}
	Label(c, title, r.x+20, r.y+18, 15, s.OnSurface, true, r.w-80)
}

func cardBadge(c *ui.Context, p *Palette, text string, x, y, w float32, active bool) {
	bg := AlphaCol(0x111827, 0.05)
	fg := p.Scheme.OnSurfaceVariant
	if p.Dark {
		bg = AlphaCol(0xFFFFFF, 0.085)
	}
	if active {
		fg = p.Scheme.OnSurface
	}
	if p.HighContrast {
		bg = Col(p.Scheme.High)
		if active {
			bg = Col(mix(p.Scheme.Container, p.Scheme.Primary, .12))
			fg = p.Scheme.Primary
		}
	}
	ui.Box(c).Absolute().Left(x).Top(y).Width(w).Height(24).Radius(12).Background(bg).IgnoreInput(true)
	if active {
		ui.Box(c).Absolute().Left(x + 9).Top(y + 9.5).Width(5).Height(5).Radius(2.5).Background(Col(0x10B981)).IgnoreInput(true)
		CenteredText(c, text, x+6, y, w-6, 24, 10, fg, true).IgnoreInput(true)
		return
	}
	CenteredText(c, text, x, y, w, 24, 10, fg, true).IgnoreInput(true)
}

func (a *App) dashboardMusic(c *ui.Context, r overviewRect) {
	if expandedOverview(r) {
		a.expandedMusic(c, r)
		return
	}
	p, s := &a.Palette, &a.Palette.Scheme
	x, y, w := r.x, r.y, r.w
	a.cardHeader(c, r, "音乐")
	status := "未播放"
	if a.hasTrack() {
		status = "已暂停"
		if a.playing() {
			status = "播放中"
		}
	}
	cardBadge(c, p, status, x+w-84, y+14, 64, a.playing())
	cover, tx, titleSize := float32(64), float32(100), float32(18)
	if w < 360 {
		cover, tx, titleSize = 44, 76, 15
	}
	if bmp := a.coverBitmap(); bmp != nil {
		ui.Image(c, bmp).Absolute().Left(x + 20).Top(y + 52).Width(cover).Height(cover).Fit(ui.Cover).Radius(12)
	} else {
		coverBg := AlphaCol(0x111827, 0.045)
		coverBorder := AlphaCol(0x111827, 0.06)
		if p.Dark {
			coverBg = AlphaCol(0xFFFFFF, 0.07)
			coverBorder = AlphaCol(0xFFFFFF, 0.09)
		}
		ui.Box(c).Absolute().Left(x+20).Top(y+52).Width(cover).Height(cover).Radius(12).
			Background(coverBg).Border(1, coverBorder)
		Icon(c, "music_note", x+20+(cover-24)/2, y+52+(cover-24)/2, 24, s.OnSurfaceVariant)
	}
	title, artist := a.trackTitle(), a.trackArtist()
	if !a.hasTrack() {
		title, artist = "还没有播放音乐", "打开播放器后自动连接"
	}
	Label(c, title, x+tx, y+55, titleSize, s.OnSurface, true, w-tx-20)
	Label(c, artist, x+tx, y+84, 11, s.OnSurfaceVariant, false, w-tx-20)
	lyric := stateString(a.lyricFrame(), "", "currentLine")
	if lyric == "" {
		lyric = "等待歌词"
		if !a.hasTrack() {
			lyric = "播放器就绪后，歌曲与歌词会同步到这里"
		}
	}
	Label(c, lyric, x+20, y+127, 13, s.OnSurfaceVariant, false, w-40)
	Label(c, stateString(a.lyricFrame(), "", "currentTranslation"), x+20, y+148, 10, s.OnSurfaceVariant, false, w-40)
	fraction, duration := a.mediaFraction()
	Seekbar(c, p, "播放进度", x+20, y+r.h-98, w-40, 20, fraction, stateBool(a.State, "canSeek"), func(f float64) { a.seekTo(f, duration) })
	Label(c, formatTimerTime(int64(a.interpolatedPosition())), x+20, y+r.h-79, 10, s.OnSurfaceVariant, false, 48)
	Label(c, formatTimerTime(int64(duration)), x+w-62, y+r.h-79, 10, s.OnSurfaceVariant, false, 44)
	a.dashboardTransport(c, x+(w-172)/2, y+r.h-56)
}

func (a *App) dashboardTransport(c *ui.Context, x, y float32) {
	p := &a.Palette
	enabled := a.hasTrack()
	IconButton(c, p, "上一首", x, y+2, 40, 40, "skip_previous", false, enabled, func() { a.mediaCommand("previous") })
	icon, label := "play_arrow", "开始播放"
	if a.playing() {
		icon, label = "pause", "暂停播放"
	}
	IconButton(c, p, label, x+64, y, 44, 44, icon, true, enabled, func() { a.mediaCommand("toggle") })
	IconButton(c, p, "下一首", x+132, y+2, 40, 40, "skip_next", false, enabled, func() { a.mediaCommand("next") })
}

func (a *App) dashboardFocus(c *ui.Context, r overviewRect) {
	if expandedOverview(r) {
		a.expandedFocus(c, r)
		return
	}
	p, s := &a.Palette, &a.Palette.Scheme
	x, y, w := r.x, r.y, r.w
	a.cardHeader(c, r, "专注计时")
	IconButton(c, p, "打开全部计时", x+w-50, y+10, 32, 32, "timer", false, true, func() { a.navigate(PageFocus) })
	var timer *goTimer
	if items := a.timerItems(); len(items) > 0 {
		timer = &items[0]
	}
	clock, detail := fmt.Sprintf("%02d:00", deckMinutes), "选择时长，开始专注"
	if timer != nil {
		clock, detail = timer.clock(), timer.subtitle()
	}
	progress := float64(0)
	if timer != nil {
		progress = timer.progress()
	}
	ringInk, ringTrack := uint32(0x16181D), uint32(0xE5E7EB)
	ringWell := AlphaCol(0x111827, 0.035)
	if p.Dark {
		ringInk, ringTrack = 0xF3F4F6, 0x2B2E36
		ringWell = AlphaCol(0xFFFFFF, 0.045)
	}
	if p.HighContrast {
		ringInk, ringTrack = s.Primary, mix(s.Container, s.OnSurface, .08)
	}
	ui.Box(c).Absolute().Left(x + w - 76).Top(y + 56).Width(56).Height(56).Radius(28).Background(ringWell).IgnoreInput(true)
	widgetCircle(c, x+w-74, y+58, 52, progress, ringInk, ringTrack)
	ringLabel := "就绪"
	if timer != nil {
		if progress > 0 {
			ringLabel = fmt.Sprintf("%d%%", int(math.Round(progress*100)))
		} else if timer.Running {
			ringLabel = "进行"
		}
	}
	CenteredText(c, ringLabel, x+w-74, y+73, 52, 20, 10, s.OnSurfaceVariant, true).IgnoreInput(true)
	Label(c, clock, x+20, y+52, 42, s.OnSurface, true, w-106)
	Label(c, detail, x+22, y+110, 11.5, s.OnSurfaceVariant, false, w-44)
	if timer == nil {
		trayBg := AlphaCol(0x111827, 0.04)
		trayRim := AlphaCol(0x111827, 0.06)
		if p.Dark {
			trayBg = AlphaCol(0xFFFFFF, 0.055)
			trayRim = AlphaCol(0xFFFFFF, 0.08)
		}
		ui.Box(c).Absolute().Left(x+20).Top(y+144).Width(w-40).Height(38).Radius(19).
			Background(trayBg).Border(1, trayRim).IgnoreInput(true)
		chipW := (w - 48) / 3
		for i, minutes := range []int{15, 25, 45} {
			xx := x + 24 + float32(i)*chipW
			selected := deckMinutes == minutes
			button := ui.ButtonBase(c).Absolute().Left(xx).Top(y + 148).Width(chipW).Height(30).Radius(15).
				Label(fmt.Sprintf("概览专注 %d 分钟", minutes)).Cursor(ui.CursorPointer)
			bg, fg := ui.Transparent, s.OnSurfaceVariant
			if selected {
				if p.HighContrast {
					bg, fg = Col(mix(s.Container, s.Primary, .12)), s.Primary
				} else if p.Dark {
					bg, fg = AlphaCol(0xFFFFFF, 0.16), 0xFFFFFF
					button.Border(1, AlphaCol(0xFFFFFF, 0.18)).Shadow(0, 1, 3, 0, AlphaCol(0x000000, 0.25))
				} else {
					bg, fg = Col(0xFFFFFF), 0x16181D
					button.Border(1, AlphaCol(0x111827, 0.08)).Shadow(0, 1, 3, 0, AlphaCol(0x111827, 0.10))
				}
			}
			button.Background(bg)
			hoverLayer(&button, bg, s.OnSurface, true)
			if button.Clicked() {
				deckMinutes = minutes
			}
			button.Children(func() { CenteredText(c, fmt.Sprintf("%d 分钟", minutes), 0, 0, chipW, 30, 12, fg, selected) })
		}
	}
	label, icon := "开始专注", "play_arrow"
	if timer != nil {
		switch timer.primaryAction() {
		case "pause":
			label, icon = "暂停专注", "pause"
		case "resume":
			label = "继续专注"
		case "next":
			label, icon = "下一阶段", "skip_next"
		default:
			label = "再来一轮"
		}
	}
	Button(c, p, "专注主操作", x+20, y+r.h-56, w-40, 40, label, icon, false, a.Connected, func() {
		if timer == nil {
			a.command("timerCreate", a.quickFocusValue())
			return
		}
		a.command("timerAction", map[string]any{"id": timer.ID, "action": timer.primaryAction(), "revision": timer.Revision})
	})
}

func (a *App) dashboardTodos(c *ui.Context, r overviewRect) {
	if r.w >= 620 && a.pendingTodos() == 0 {
		a.overviewEmpty(c, r, false)
		return
	}
	if expandedOverview(r) {
		a.expandedTodos(c, r)
		return
	}
	p := &a.Palette
	x, y, w := r.x, r.y, r.w
	a.cardHeader(c, r, "待办事项")
	IconButton(c, p, "添加待办", x+w-50, y+10, 32, 32, "add", false, true, func() { a.openTodoDialog("") })
	today, _ := a.todoNow()
	items := a.todoItems()
	sort.SliceStable(items, func(i, j int) bool { return items[i].Date+items[i].Time < items[j].Date+items[j].Time })
	pending := []goTodo{}
	for _, item := range items {
		if !todoDone(item, today) {
			pending = append(pending, item)
		}
	}
	if len(pending) == 0 {
		a.todoAgendaEmpty(c, r)
	}
	rows := int((r.h - 100) / 64)
	for i, item := range pending {
		if i >= rows {
			break
		}
		a.todoAgendaRow(c, item, i, x+10, y+48+float32(i)*64, w-20, 58, i == 0, false)
	}
	widgetTextAction(c, p, "概览全部待办", "全部待办 · "+itoa(len(pending)), x+22, y+r.h-40, w-44, func() { a.navigate(PageTodos) })
}

func (a *App) dashboardQuick(c *ui.Context, r overviewRect) {
	if expandedOverview(r) {
		a.expandedQuick(c, r)
		return
	}
	p, s := &a.Palette, &a.Palette.Scheme
	x, y, w := r.x, r.y, r.w
	a.cardHeader(c, r, "快捷设置")
	rows := []struct {
		title, detail, icon string
		on                  bool
		action              func()
	}{
		{"灵动岛", "显示在桌面上方", "dialpad", a.Prefs.Island, func() { a.command("toggleVisibility", nil) }},
		{"勿扰模式", "暂停通知提醒", "volume_up", a.notificationsPaused(), func() { a.pushNative("PauseNotifications", !a.notificationsPaused()) }},
		{"深色外观", "控制中心显示主题", "dark_mode", a.Palette.Dark, a.toggleDark},
	}
	for i, row := range rows {
		top := y + 52 + float32(i)*66
		rowBg := AlphaCol(0x111827, 0.032)
		rowRim := AlphaCol(0x111827, 0.05)
		if p.Dark {
			rowBg = AlphaCol(0xFFFFFF, 0.045)
			rowRim = AlphaCol(0xFFFFFF, 0.07)
		}
		if row.on {
			rowBg = AlphaCol(s.Primary, 0.065)
			rowRim = AlphaCol(s.Primary, 0.16)
			if p.Dark {
				rowBg = AlphaCol(0xFFFFFF, 0.085)
				rowRim = AlphaCol(0xFFFFFF, 0.16)
			}
		}
		ui.Box(c).Absolute().Left(x+14).Top(top).Width(w-28).Height(58).Radius(16).
			Background(rowBg).Border(1, rowRim).IgnoreInput(true)
		bg := AlphaCol(0x111827, 0.055)
		fg := s.OnSurfaceVariant
		if p.Dark {
			bg = AlphaCol(0xFFFFFF, 0.08)
		}
		if row.on {
			if p.HighContrast {
				bg, fg = Col(s.Primary), s.OnPrimary
			} else if p.Dark {
				bg, fg = Col(0xF3F4F6), 0x111316
			} else {
				bg, fg = Col(0x16181D), 0xFFFFFF
			}
		}
		ui.Box(c).Absolute().Left(x + 24).Top(top + 12).Width(34).Height(34).Radius(12).Background(bg).IgnoreInput(true)
		Icon(c, row.icon, x+32, top+20, 18, fg)
		Label(c, row.title, x+68, top+10, 13.5, s.OnSurface, true, w-146)
		Label(c, row.detail, x+68, top+32, 10.5, s.OnSurfaceVariant, false, w-146)
		Switch(c, p, row.title, x+w-68, top+16, row.on, row.action)
	}
}
