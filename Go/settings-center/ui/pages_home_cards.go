// The five deck card bodies (overview.rs fn content), each at Original geometry.
package ui

import (
	"github.com/egoist/mygo/ui"
)

// deckMusicCard is music_card::render on the Grid: cover column left, lyric column
// right, transport centered over the progress line.
func (a *App) deckMusicCard(c *ui.Context, p *Palette, w, h float32) {
	s := &p.Scheme
	inset := float32(24)
	column := float32(math8Round(float64(w) * 0.24 / 8))
	if column < 192 {
		column = 192
	}
	if column > 224 {
		column = 224
	}
	right := inset + column + 32
	coverSize := clampF32(h-222, 64, 112)
	titleY := 64 + coverSize + 12
	controlsW := float32(164)

	// Cover column.
	if bmp := a.coverBitmap(); bmp != nil {
		ui.Image(c, bmp).Absolute().Left(inset + (column-coverSize)/2).Top(64).
			Width(coverSize).Height(coverSize).Fit(ui.Cover).Radius(15)
	} else {
		ui.Box(c).Absolute().Left(inset + (column-coverSize)/2).Top(64).Width(coverSize).Height(coverSize).
			Radius(15).Background(Col(s.SecondaryContainer)).Children(func() {
			size := coverSize * 0.32
			if svg := Symbol("music_note"); svg != nil {
				ui.Icon(c, svg).FontSize(size).TextColor(Col(s.OnSurfaceVariant)).
					Absolute().Left((coverSize - size) / 2).Top((coverSize - size) / 2)
			}
		})
	}
	CenteredText(c, a.trackTitle(), inset, titleY, column, 28, 20, s.OnSurface, true)
	CenteredText(c, a.trackArtist(), inset, titleY+34, column, 18, 12, s.OnSurfaceVariant, false)

	// Lyrics: three lines centered vertically in the lyrics area (Grid lyrics rect).
	a.deckLyrics(c, p, right, 64, w-right-inset, h-156)

	// Transport: under cover, y = height - 78 (Grid.controls).
	a.playerControls(c, inset+(column-controlsW)/2, h-78, false)
	// Progress: right column, y = height - 64 (Grid.progress).
	fraction, duration := a.mediaFraction()
	Seekbar(c, p, "播放进度", right, h-64, w-right-inset, 24, fraction,
		stateBool(a.State, "canSeek"), func(f float64) { a.seekTo(f, duration) })
}

// deckLyrics shows previous/current/next at the music-card sizes.
func (a *App) deckLyrics(c *ui.Context, p *Palette, x, y, w, h float32) {
	s := &p.Scheme
	previous := stateString(a.lyricFrame(), "", "previousLine")
	current := stateString(a.lyricFrame(), "把耳朵交给音乐，把此刻留给自己。", "currentLine")
	next := stateString(a.lyricFrame(), "", "nextLine")
	translated := stateString(a.lyricFrame(), "", "currentTranslation")

	rows := []struct {
		text  string
		size  float32
		color uint32
		bold  bool
	}{
		{previous, 17, s.OnSurfaceVariant, false},
		{current, 25, s.OnSurface, true},
		{next, 17, s.OnSurfaceVariant, false},
	}
	total := float32(0)
	for _, row := range rows {
		if row.text != "" {
			total += LineHeight(row.size)
		}
	}
	if translated != "" {
		total += 23
	}
	cursor := y + (h-total)/2
	for _, row := range rows {
		if row.text == "" {
			continue
		}
		Label(c, row.text, x, cursor, row.size, row.color, row.bold, w)
		cursor += LineHeight(row.size)
	}
	if translated != "" {
		Label(c, translated, x, cursor, 13, s.OnSurfaceVariant, false, w)
	}
}

// deckFocusCard mirrors overview.rs content case 1.
func (a *App) deckFocusCard(c *ui.Context, p *Palette, w, h float32) {
	s := &p.Scheme
	var timer *goTimer
	if timers := a.timerItems(); len(timers) > 0 {
		timer = &timers[0]
	}
	clock := "25:00"
	subtitle := "先选一段时间，再专心做一件事。"
	if timer != nil {
		clock = timer.clock()
		subtitle = timer.subtitle()
	}
	Label(c, clock, 26, 64, 62, s.OnSurface, true, w-310)
	Label(c, subtitle, 30, 151, 13, s.OnSurfaceVariant, false, w-320)
	bx := w - 246
	label, icon := "开始专注", "play_arrow"
	if timer != nil {
		switch timer.primaryAction() {
		case "pause":
			label, icon = "暂停专注", "pause"
		case "resume":
			label, icon = "继续专注", "play_arrow"
		case "next":
			label, icon = "下一阶段", "skip_next"
		default:
			label = "再来一轮"
		}
	}
	if timer != nil {
		Button(c, p, "专注主操作", bx, 72, 216, 48, label, icon, false, true, func() {
			a.command("timerAction", map[string]any{
				"id": timer.ID, "action": timer.primaryAction(), "revision": timer.Revision})
		})
		Button(c, p, "重置计时", bx, 132, 102, 38, "重置", "restart_alt", true, true, func() {
			a.command("timerAction", map[string]any{
				"id": timer.ID, "action": "reset", "revision": timer.Revision})
		})
	} else {
		Button(c, p, "开始专注", bx, 72, 216, 48, label, icon, false, true, func() {
			a.command("timerCreate", a.quickFocusValue())
		})
		Button(c, p, "重置时长", bx, 132, 102, 38, "重置", "restart_alt", true, true, func() {
			a.command("timerCreate", a.quickFocusValue())
		})
	}
	Button(c, p, "打开计时页", bx+114, 132, 102, 38, "全部计时", "", true, true, func() {
		a.navigate(PageFocus)
	})
	if timer != nil {
		trackY := h - 83
		ui.Box(c).Absolute().Left(30).Top(trackY).Width(w - 60).Height(6).
			Radius(3).Background(Col(s.OutlineVariant))
		ui.Box(c).Absolute().Left(30).Top(trackY).
			Width(float32(float64(w-60) * timer.progress())).Height(6).
			Radius(3).Background(Col(s.Primary))
		Label(c, timer.subtitle(), 30, h-57, 12, s.OnSurfaceVariant, false, w-60)
	} else {
		Label(c, "专注时长", 30, h-84, 11, s.OnSurfaceVariant, false, 100)
		minutes := [3]int{15, 25, 45}
		for j, min := range minutes {
			Chip(c, p, "专注预设", 28+float32(j)*110, h-57, 100, 34,
				itoa(min)+" 分钟", deckMinutes == min, func() {
					deckMinutes = min
					a.savePrefs()
				})
		}
		Label(c, "结束后，记得休息一下。", w-246, h-47, 11, s.OnSurfaceVariant, false, 216)
	}
}

// deckTodosCard mirrors overview.rs content case 2.
func (a *App) deckTodosCard(c *ui.Context, p *Palette, w, h float32) {
	s := &p.Scheme
	Label(c, itoa(a.pendingTodos())+" 件小事，慢慢完成", 28, 65, 12, s.OnSurfaceVariant, false, w-360)
	Button(c, p, "快速提醒", w-278, 60, 116, 34, "快速提醒", "", true, true, func() {
		a.showQuickReminder = true
	})
	Button(c, p, "添加待办", w-150, 60, 124, 34, "添加待办", "add", false, true, func() {
		a.todoDialog = true
	})
	now, _ := a.todoNow()
	all := a.todoItems()
	var items []goTodo
	for _, t := range all {
		if !todoDone(t, now) {
			items = append(items, t)
		}
	}
	if len(items) == 0 {
		if svg := Symbol("eco"); svg != nil {
			Icon(c, "eco", 32, 127, 36, s.Primary)
		}
		Label(c, "清爽的一天", 90, 119, 25, s.OnSurface, true, w-122)
		Label(c, "记下一件小事，然后安心去做。", 92, 162, 13, s.OnSurfaceVariant, false, w-124)
	}
	for j, item := range items {
		if j >= 2 {
			break
		}
		y := 108 + float32(j)*64
		ui.Box(c).Absolute().Left(22).Top(y).Width(w - 44).Height(56).
			Radius(16).Background(AlphaCol(s.High, 0.6))
		IconButton(c, p, "完成待办", 28, y+7, 40, 40, "check", false, true, func() {
			a.command("todoComplete", map[string]any{"id": item.ID, "completed": true})
		})
		Label(c, item.Title, 82, y+6, 15, s.OnSurface, true, w-266)
		meta := item.Date + "  " + item.Time
		if item.Daily {
			meta += " · 每天"
		}
		Label(c, meta, 82, y+32, 11, s.OnSurfaceVariant, false, w-266)
		id := item.ID
		Button(c, p, "编辑待办", w-174, y+10, 84, 36, "编辑", "", true, true, func() {
			a.openTodoDialog(id)
		})
		IconButton(c, p, "移除待办", w-78, y+10, 36, 36, "delete", false, true, func() {
			a.command("todoDelete", map[string]any{"id": id})
		})
	}
	Button(c, p, "打开待办页", 26, h-45, 124, 32, "全部待办", "", true, true, func() {
		a.navigate(PageTodos)
	})
}

// deckNotificationsCard mirrors overview.rs content case 3.
func (a *App) deckNotificationsCard(c *ui.Context, p *Palette, w, h float32) {
	s := &p.Scheme
	paused := stateBool(a.settings(), "PauseNotifications")
	events, _ := a.State["notificationHistory"].([]any)
	count := len(events)
	text := "最近收到 " + itoa(count) + " 条提醒"
	if paused {
		text = "通知已暂停"
	}
	Label(c, text, 28, 65, 12, s.OnSurfaceVariant, false, w-360)
	Button(c, p, "暂停或恢复通知", w-294, 60, 130, 34,
		map[bool]string{true: "恢复通知", false: "暂停通知"}[paused],
		map[bool]string{true: "play_arrow", false: "pause"}[paused], true, true, func() {
			a.pushNative("PauseNotifications", !paused)
		})
	Button(c, p, "打开通知页", w-152, 60, 126, 34, "查看全部", "", true, true, func() {
		a.navigate(PageNotifications)
	})
	if count == 0 {
		Icon(c, "volume_up", 32, 135, 34, s.Primary)
		Label(c, "此刻很安静", 90, 121, 25, s.OnSurface, true, w-122)
		Label(c, "收到的通知与到期提醒，会收拢在这里。", 92, 165, 13, s.OnSurfaceVariant, false, w-124)
	}
	for j, raw := range events {
		if j >= 2 {
			break
		}
		event, _ := raw.(map[string]any)
		y := 108 + float32(j)*64
		ui.Box(c).Absolute().Left(22).Top(y).Width(w - 44).Height(56).
			Radius(16).Background(AlphaCol(s.High, 0.6))
		Icon(c, "volume_up", 36, y+17, 20, s.OnSurfaceVariant)
		Label(c, stateString(event, "通知", "title"), 74, y+5, 14, s.OnSurface, true, w-270)
		Label(c, stateString(event, "", "body"), 74, y+31, 11, s.OnSurfaceVariant, false, w-270)
		Label(c, stateString(event, "", "clock"), w-174, y+8, 10, s.OnSurfaceVariant, false, 132)
		id := event["id"]
		kind, _ := event["kind"].(string)
		if kind == "notice" {
			Button(c, p, "打开来源", w-174, y+24, 132, 28, "打开来源", "", true, true, func() {
				a.command("historyOpen", id)
			})
		} else if todoID, ok := event["todoId"].(string); ok {
			Button(c, p, "查看待办", w-174, y+24, 132, 28, "查看待办", "", true, true, func() {
				a.openTodoDialog(todoID)
			})
		}
	}
	Button(c, p, "通知偏好", 26, h-45, 132, 32, "通知偏好", "settings", true, true, func() {
		a.openSettings(242)
	})
}

// deckWidgetsCard mirrors overview.rs content case 4: six add entries.
func (a *App) deckWidgetsCard(c *ui.Context, p *Palette, w, h float32) {
	s := &p.Scheme
	Label(c, "把常用的小事，轻轻放到桌面。", 28, 65, 13, s.OnSurfaceVariant, false, w-56)
	kinds := []struct {
		label, detail, icon string
	}{
		{"时钟", "一眼知晓时间", "timer"},
		{"日历", "今天，也值得期待", "timer"},
		{"待办", "记下重要小事", "checklist"},
		{"专注计时", "留一段时间给自己", "play_arrow"},
		{"音乐", "喜欢的歌近一点", "music_note"},
		{"系统状态", "状态，一眼知晓", "settings"},
	}
	cw := (w - 76) / 3
	for j, kind := range kinds {
		x := 26 + float32(j%3)*(cw+12)
		y := 102 + float32(j/3)*64
		canAdd := a.widgetCount() < 16
		row := ui.ButtonBase(c).Absolute().Left(x).Top(y).Width(cw).Height(54).
			Radius(16).Background(AlphaCol(s.High, 0.6)).
			Label("添加" + kind.label + "桌面组件")
		if !canAdd {
			row.Disabled(true)
		}
		hoverLayer(&row, AlphaCol(s.High, 0.6), s.OnSurface, canAdd)
		if canAdd && row.Clicked() {
			a.command("widgetAdd", map[string]any{
				"kind": homeWidgetKind(j), "size": homeWidgetDefaultSize(j)})
		}
		row.Children(func() {
			Icon(c, kind.icon, x+14, y+16, 22, s.Primary)
			Label(c, kind.label, x+48, y+8, 14, s.OnSurface, true, cw-82)
			Label(c, kind.detail, x+48, y+31, 10, s.OnSurfaceVariant, false, cw-76)
			Icon(c, "add", x+cw-28, y+19, 16, s.OnSurfaceVariant)
		})
	}
	Button(c, p, "打开组件页", 26, h-45, 180, 32, "进入组件页", "", true, true, func() {
		a.navigate(PageWidgets)
	})
}
