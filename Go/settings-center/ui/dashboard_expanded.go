package ui

import (
	"fmt"
	"sort"

	"github.com/egoist/mygo/ui"
)

// Large cards use a 24-DIP inset, a 60-DIP header and a 56-DIP action
// zone. Their content has its own layout, rather than stretching a small card.
func expandedOverview(r overviewRect) bool { return r.w >= 620 && r.h >= 340 }

func (a *App) expandedMusic(c *ui.Context, r overviewRect) {
	p, s := &a.Palette, &a.Palette.Scheme
	g := expandedGuides(r)
	a.cardHeader(c, r, "音乐")
	status := "未播放"
	if a.hasTrack() {
		status = "已暂停"
		if a.playing() {
			status = "播放中"
		}
	}
	cardBadge(c, p, status, r.x+r.w-overviewInset-64, r.y+20, 64, a.playing())
	cover := min(g.visualW, r.h-204)
	cx, cy := g.left, g.top+4
	if bmp := a.coverBitmap(); bmp != nil {
		ui.Image(c, bmp).Absolute().Left(cx).Top(cy).Width(cover).Height(cover).Fit(ui.Cover).Radius(18)
	} else {
		ui.Box(c).Absolute().Left(cx).Top(cy).Width(cover).Height(cover).Radius(18).Background(Col(s.High)).IgnoreInput(true)
		Icon(c, "music_note", cx+(cover-56)/2, cy+(cover-56)/2, 56, s.OnSurfaceVariant)
	}
	tx, tw := g.bodyX, g.bodyW
	title, artist := a.trackTitle(), a.trackArtist()
	if !a.hasTrack() {
		title, artist = "还没有播放音乐", "打开播放器后自动连接"
	}
	overviewText(c, title, tx, cy, tw, overviewLead, s.OnSurface, 1)
	overviewText(c, artist, tx, cy+40, tw, overviewCaption, s.OnSurfaceVariant, 1)
	lyric := stateString(a.lyricFrame(), "等待歌词", "currentLine")
	if !a.hasTrack() {
		lyric = "播放器就绪后，歌曲与歌词会同步到这里"
	}
	lines := 1
	if cover >= 176 {
		lines = 2
	}
	overviewText(c, lyric, tx, cy+88, tw, overviewBody, s.OnSurface, lines)
	if cover >= 208 {
		overviewText(c, stateString(a.lyricFrame(), "", "currentTranslation"), tx, cy+144, tw, overviewCaption, s.OnSurfaceVariant, 2)
	}
	fraction, duration := a.mediaFraction()
	seekY := r.y + r.h - 124
	Seekbar(c, p, "播放进度", r.x+24, seekY, r.w-48, 20, fraction, stateBool(a.State, "canSeek"), func(f float64) { a.seekTo(f, duration) })
	overviewText(c, formatTimerTime(int64(a.interpolatedPosition())), g.left, seekY+24, 64, overviewCaption, s.OnSurfaceVariant, 1)
	overviewText(c, formatTimerTime(int64(duration)), r.x+r.w-80, seekY+24, 56, overviewCaption, s.OnSurfaceVariant, 1)
	a.dashboardTransport(c, r.x+(r.w-172)/2, r.y+r.h-68)
}

func (a *App) expandedFocus(c *ui.Context, r overviewRect) {
	p, s := &a.Palette, &a.Palette.Scheme
	g := expandedGuides(r)
	a.cardHeader(c, r, "专注计时")
	IconButton(c, p, "打开全部计时", r.x+r.w-56, r.y+16, 32, 32, "timer", false, true, func() { a.navigate(PageFocus) })
	var timer *goTimer
	if items := a.timerItems(); len(items) > 0 {
		timer = &items[0]
	}
	clock, detail, progress := fmt.Sprintf("%02d:00", deckMinutes), "选择时长，开始专注", float64(0)
	if timer != nil {
		clock, detail, progress = timer.clock(), timer.subtitle(), timer.progress()
	}
	ring := min(g.visualW, g.bottom-g.top-48)
	rx, ry := g.left+(g.visualW-ring)/2, (g.top+g.bottom-ring)/2
	widgetCircle(c, rx, ry, ring, progress, s.Primary, mix(s.Container, s.OnSurface, .07))
	CenteredText(c, clock, rx, ry+(ring-64)/2, ring, 64, min(float32(44), ring/4.5), s.OnSurface, true)
	x, w := g.bodyX, min(g.bodyW, float32(512))
	stackH := float32(180)
	if timer != nil {
		stackH = 112
	}
	y := (g.top + g.bottom - stackH) / 2
	overviewText(c, detail, x, y, w, overviewType{20, 28, 600}, s.OnSurface, 1)
	if timer == nil {
		chipW := (w - 24) / 3
		for i, minutes := range []int{15, 25, 45} {
			Chip(c, p, fmt.Sprintf("概览专注 %d 分钟", minutes), x+float32(i)*(chipW+12), y+48, chipW, 40, fmt.Sprintf("%d 分钟", minutes), deckMinutes == minutes, func() { deckMinutes = minutes })
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
	Button(c, p, "专注主操作", x, y+stackH-48, w, 48, label, icon, false, a.Connected, func() {
		if timer == nil {
			a.command("timerCreate", a.quickFocusValue())
			return
		}
		a.command("timerAction", map[string]any{"id": timer.ID, "action": timer.primaryAction(), "revision": timer.Revision})
	})
}

func (a *App) expandedTodos(c *ui.Context, r overviewRect) {
	p, s := &a.Palette, &a.Palette.Scheme
	g := expandedGuides(r)
	a.cardHeader(c, r, "待办事项")
	IconButton(c, p, "添加待办", r.x+r.w-56, r.y+16, 32, 32, "add", false, true, func() { a.openTodoDialog("") })
	today, _ := a.todoNow()
	pending := []goTodo{}
	for _, item := range a.todoItems() {
		if !todoDone(item, today) {
			pending = append(pending, item)
		}
	}
	sort.SliceStable(pending, func(i, j int) bool { return pending[i].Date+pending[i].Time < pending[j].Date+pending[j].Time })
	a.overviewCount(c, g, len(pending), "件待完成")
	x, w := g.bodyX, g.bodyW
	if len(pending) == 0 {
		y := (g.top + g.bottom - 136) / 2
		overviewText(c, "今天，轻松一点。", x, y, w, overviewLead, s.OnSurface, 1)
		overviewText(c, "添加下一件要做的事，按自己的节奏完成。", x, y+44, w, overviewCaption, s.OnSurfaceVariant, 1)
		Button(c, p, "概览空态添加待办", x, y+88, 160, 44, "添加待办", "add", false, true, func() { a.openTodoDialog("") })
		return
	}
	rows := int((g.bottom - g.top - 44) / 80)
	for i, item := range pending {
		if i >= rows {
			break
		}
		a.todoAgendaRow(c, item, i, x, g.top+float32(i)*80, w, 72, i == 0, false)
	}
	widgetTextAction(c, p, "概览全部待办", "全部待办 · "+itoa(len(pending)), x, r.y+r.h-48, w, func() { a.navigate(PageTodos) })
}

func (a *App) expandedQuick(c *ui.Context, r overviewRect) {
	p, s := &a.Palette, &a.Palette.Scheme
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
	rowH := (r.h - 96) / 3
	for i, row := range rows {
		y := r.y + 64 + float32(i)*rowH
		bg, fg := s.High, s.OnSurfaceVariant
		if row.on {
			bg, fg = s.Primary, s.OnPrimary
		}
		ui.Box(c).Absolute().Left(r.x + 24).Top(y + 14).Width(48).Height(48).Radius(24).Background(Col(bg)).IgnoreInput(true)
		Icon(c, row.icon, r.x+37, y+27, 22, fg)
		Label(c, row.title, r.x+92, y+13, 18, s.OnSurface, true, r.w-200)
		Label(c, row.detail, r.x+92, y+46, 13, s.OnSurfaceVariant, false, r.w-200)
		Switch(c, p, row.title, r.x+r.w-68, y+24, row.on, row.action)
		if i < 2 {
			ui.Box(c).Absolute().Left(r.x + 92).Top(y + rowH - 8).Width(r.w - 116).Height(1).Background(AlphaCol(s.OnSurface, .06)).IgnoreInput(true)
		}
	}
}

func (a *App) expandedNotifications(c *ui.Context, r overviewRect) {
	p, s := &a.Palette, &a.Palette.Scheme
	g := expandedGuides(r)
	a.cardHeader(c, r, "通知记录")
	events := a.historyEvents()
	a.overviewCount(c, g, len(events), "条通知")
	x, w := g.bodyX, g.bodyW
	if len(events) == 0 {
		y := (g.top + g.bottom - 104) / 2
		overviewText(c, "暂时没有通知", x, y, w, overviewLead, s.OnSurface, 1)
		overviewText(c, "新收到的通知与到期提醒会显示在这里", x, y+44, w, overviewCaption, s.OnSurfaceVariant, 1)
		widgetTextAction(c, p, "概览全部通知", "查看全部通知", x, y+80, w, func() { a.navigate(PageNotifications) })
		return
	}
	rows := int((g.bottom - g.top - 44) / 88)
	for i, event := range events {
		if i >= rows {
			break
		}
		y := g.top + float32(i)*88
		overviewText(c, event.Title, x, y, w-88, overviewTitle, s.OnSurface, 1)
		overviewText(c, event.Clock, x+w-72, y+2, 72, overviewCaption, s.OnSurfaceVariant, 1)
		overviewText(c, event.AppName+" · "+event.Body, x, y+32, w, overviewCaption, s.OnSurfaceVariant, 2)
		if i < min(len(events), rows)-1 {
			ui.Box(c).Absolute().Left(x).Top(y + 80).Width(w).Height(1).Background(AlphaCol(s.OnSurface, .06)).IgnoreInput(true)
		}
	}
	widgetTextAction(c, p, "概览全部通知", "查看全部通知", x, r.y+r.h-48, w, func() { a.navigate(PageNotifications) })
}
