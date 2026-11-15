package ui

import (
	"fmt"
	"github.com/egoist/mygo/ui"
	"sort"
	"strings"
	"time"
)

func (a *App) appleWidgetsGallery(c *ui.Context, x, y, w float32) float32 {
	p, s := &a.Palette, &a.Palette.Scheme
	Label(c, "桌面组件", x, y, 18, s.OnSurface, true, w)
	Label(c, "以下为示例预览。挑选尺寸，添加到桌面后可编辑内容。", x, y+34, 12, s.OnSurfaceVariant, false, w)
	examples := a.widgetGalleryExamples(time.Now())
	y += 82
	cw := (w - 20) / 2
	cardH := float32(306)
	for i, entry := range widgetKindLabels {
		xx, yy := x+float32(i%2)*(cw+20), y+float32(i/2)*(cardH+20)
		if yy+cardH < a.scrollY() || yy > a.scrollY()+float32(clipH) {
			continue
		}
		appleWidgetSurface(c, p, overviewRect{"", xx, yy, cw, cardH})
		// The gallery stage is quiet; the preview itself keeps native proportions.
		stage := ui.Box(c).Absolute().Left(xx + 12).Top(yy + 12).Width(cw - 24).Height(184).Radius(17).Background(Col(s.High)).IgnoreInput(true)
		size := a.widgetSizeOf(entry.kind)
		dims := widgetDims(size)
		scale := float32(1)
		if v := (cw - 48) / float32(dims[0]); v < scale {
			scale = v
		}
		if v := float32(168) / float32(dims[1]); v < scale {
			scale = v
		}
		stage.Children(func() {
			examples.widgetPreview(c, p, entry.kind, size, 0, (cw-24-float32(dims[0])*scale)/2, (184-float32(dims[1])*scale)/2, float32(dims[0])*scale, float32(dims[1])*scale)
		})
		Label(c, entry.label, xx+20, yy+208, 15, s.OnSurface, true, cw-40)
		chipW := float32(42)
		for j, sz := range widgetSizes {
			Chip(c, p, "size-"+entry.kind+sz.key, xx+18+float32(j)*(chipW+5), yy+252, chipW, 32, sz.label, size == sz.key, func() { a.setWidgetSize(entry.kind, sz.key) })
		}
		Button(c, p, "add-"+entry.kind+"Original", xx+cw-126, yy+250, 108, 36, "添加到桌面", "add", false, a.Connected && a.widgetCount() < 16, func() { a.command("widgetAdd", map[string]any{"kind": entry.kind, "size": size, "style": "original"}) })
	}
	y += float32((len(widgetKindLabels)+1)/2)*(cardH+20) + 12
	if msg := stateString(a.State, "", "desktopWidgetError"); msg != "" {
		Label(c, msg, x, y, 12, s.Error, false, w)
		y += 36
	}
	Label(c, "桌面上的组件 · "+itoa(a.widgetCount()), x, y, 18, s.OnSurface, true, w)
	y += 48
	return a.widgetRows(c, p, x, y, w)
}

func (a *App) appleDesktopPreview(c *ui.Context, p *Palette, kind, size string, x, y, w, h float32) {
	dims := widgetDims(size)
	pw, ph := float32(dims[0]), float32(dims[1])
	scale := w / pw
	ui.Box(c).Absolute().Left(x).Top(y).Width(pw).Height(ph).TransformOrigin(0, 0).Scale(scale, scale).Children(func() {
		r := overviewRect{"", 8, 8, pw - 16, ph - 16}
		appleWidgetSurface(c, p, r)
		s := &p.Scheme
		xx, yy, ww := float32(24), float32(24), pw-48
		small, large := size == "small", size == "large"
		if kind != "calendar" || small {
			Label(c, kindLabel(kind), xx, yy, 12, s.OnSurfaceVariant, true, ww)
		}
		widget, data := a.overviewWidgetSource(kind)
		config, _ := widget["config"].(map[string]any)
		value := func(text, detail string) {
			font := float32(46)
			if small {
				font = 36
			}
			Label(c, text, xx, yy+30, font, s.OnSurface, true, ww)
			Label(c, detail, xx, yy+95, 11, s.OnSurfaceVariant, false, ww)
		}
		switch kind {
		case "schedule":
			a.scheduleCard(c, config, xx, yy+30, ww, ph-yy-50, small)
		case "screenTime":
			a.screenTimeCard(c, xx, yy+30, ww, ph-yy-50, small)
		case "clock":
			now := time.Now()
			value(now.Format("15:04"), now.Format("1月2日")+" · "+[]string{"星期日", "星期一", "星期二", "星期三", "星期四", "星期五", "星期六"}[now.Weekday()])
		case "calendar":
			now := time.Now()
			if small {
				Label(c, []string{"星期日", "星期一", "星期二", "星期三", "星期四", "星期五", "星期六"}[now.Weekday()], xx, yy+28, 13, s.Primary, true, ww)
				Label(c, itoa(now.Day()), xx, yy+48, 58, s.OnSurface, false, ww)
			} else {
				ws := &widgetScheme{colors: p.Scheme}
				a.widgetCalendar(c, ws, xx, yy, ww, ph-48, large)
			}
		case "todos":
			today, _ := a.todoNow()
			items := []goTodo{}
			for _, t := range a.todoItems() {
				if !todoDone(t, today) {
					items = append(items, t)
				}
			}
			sort.SliceStable(items, func(i, j int) bool { return items[i].Date+items[i].Time < items[j].Date+items[j].Time })
			if len(items) == 0 {
				Label(c, "今天，轻松一点。", xx, yy+45, 17, s.OnSurface, true, ww)
			}
			rows := 2
			if small {
				rows = 1
			}
			if large {
				rows = 4
			}
			for i, t := range items {
				if i >= rows {
					break
				}
				if small {
					ui.Box(c).Absolute().Left(xx).Top(yy + 30).Width(4).Height(54).Radius(2).Background(Col(todoAccent(t, i))).IgnoreInput(true)
					Label(c, t.Title, xx+12, yy+28, 15, s.OnSurface, true, ww-12)
					Label(c, todoTimeLabel(t, today), xx+12, yy+63, 11, s.OnSurfaceVariant, false, ww-12)
					continue
				}
				a.todoAgendaRow(c, t, i, xx-6, yy+27+float32(i)*55, ww+12, 49, i == 0, false)
			}
		case "focus":
			clock, detail, progress := "25:00", "准备开始", float64(0)
			if ts := a.timerItems(); len(ts) > 0 {
				clock, detail, progress = ts[0].clock(), ts[0].subtitle(), ts[0].progress()
			}
			ring := float32(64)
			if small {
				ring = 38
			}
			if large {
				ring = 156
			}
			cx, cy := pw-32-ring, yy+24
			if large {
				cx = (pw - ring) / 2
				cy = yy + 42
			} else if small {
				cx, cy = pw-24-ring, yy-6
			}
			widgetCircle(c, cx, cy, ring, progress, s.Primary, s.High)
			if large {
				CenteredText(c, clock, cx, cy+(ring-60)/2, ring, 60, 40, s.OnSurface, true)
				CenteredText(c, detail, xx, cy+ring+12, ww, 24, 11, s.OnSurfaceVariant, false)
			} else {
				font := float32(38)
				textW := ww - ring - 12
				if small {
					font, textW = 30, ww
				}
				clockY := yy + 23
				Label(c, clock, xx, clockY, font, s.OnSurface, true, textW)
				Label(c, detail, xx, clockY+LineHeight(font)+5, 10, s.OnSurfaceVariant, false, textW)
			}
			label, icon := "开始专注", "play_arrow"
			if ts := a.timerItems(); len(ts) > 0 && ts[0].Running {
				label, icon = "暂停专注", "pause"
			}
			wButton(c, &widgetScheme{colors: p.Scheme}, xx, ph-52, ww, 30, label, icon, false)
		case "music":
			cover := float32(60)
			if small {
				cover = 38
			}
			if large {
				cover = 100
			}
			if bmp := a.coverBitmap(); bmp != nil {
				ui.Image(c, bmp).Absolute().Left(xx).Top(yy + 27).Width(cover).Height(cover).Fit(ui.Cover).Radius(12)
			} else {
				ui.Box(c).Absolute().Left(xx).Top(yy + 27).Width(cover).Height(cover).Radius(12).Background(Col(s.High))
				Icon(c, "music_note", xx+cover/2-10, yy+27+cover/2-10, 20, s.OnSurfaceVariant)
			}
			tx, ty := xx+cover+12, yy+29
			if small {
				tx, ty = xx, yy+71
			}
			if large {
				tx, ty = xx, yy+143
			}
			title := a.trackTitle()
			if !a.hasTrack() {
				title = "还没有播放音乐"
			}
			Label(c, title, tx, ty, 12, s.OnSurface, true, pw-tx-24)
			if !small {
				Label(c, a.trackArtist(), tx, ty+26, 11, s.OnSurfaceVariant, false, pw-tx-24)
			}
			for i, icon := range []string{"skip_previous", "play_arrow", "skip_next"} {
				Icon(c, icon, pw/2-58+float32(i)*46, ph-43, 20, s.OnSurface)
			}
		case "system":
			m := a.widgetSystemMetrics()
			for i, row := range []struct {
				name string
				v    float64
				ok   bool
			}{{"CPU", m.cpu, m.hasCPU}, {"内存", m.memory, m.hasMemory}} {
				x, y, cw := xx+float32(i)*ww/2, yy+32, ww/2-10
				if small {
					x, y, cw = xx, yy+28+float32(i)*50, ww
				}
				Label(c, row.name, x, y, 11, s.OnSurfaceVariant, false, cw)
				text := "—"
				if row.ok {
					text = fmt.Sprintf("%.0f%%", row.v)
				}
				Label(c, text, x, y+19, 25, s.OnSurface, true, cw)
			}
		case "wireless":
			devices := a.wirelessBatteries()
			if len(devices) == 0 {
				Icon(c, "headphones", xx, yy+34, 28, s.OnSurfaceVariant)
				Label(c, "暂无连接的设备", xx, yy+78, 14, s.OnSurface, true, ww)
			}
			cols := 2
			if small {
				cols = 1
			}
			cw := ww / float32(cols)
			limit := cols
			if large {
				limit = 4
			}
			for i, d := range devices {
				if i >= limit {
					break
				}
				x, y := xx+float32(i%cols)*cw, yy+28+float32(i/cols)*137
				ring := float32(60)
				if small {
					ring = 50
				}
				fraction := float64(0)
				if d.known {
					fraction = d.percent / 100
				}
				widgetCircle(c, x+(cw-ring)/2, y, ring, fraction, s.Primary, s.High)
				icon := "headphones"
				if d.kind == "mouse" {
					icon = "mouse"
				}
				if d.kind == "keyboard" {
					icon = "keyboard"
				}
				if d.kind == "gamepad" {
					icon = "gamepad"
				}
				Icon(c, icon, x+(cw-24)/2, y+(ring-24)/2, 24, s.OnSurface)
				text := "—"
				if d.known {
					text = fmt.Sprintf("%.0f%%", d.percent)
				}
				CenteredText(c, text, x, y+ring+7, cw, 27, 20, s.OnSurface, true)
				CenteredText(c, d.name, x, y+ring+34, cw, 18, 10, s.OnSurfaceVariant, false)
			}
		case "note":
			ui.Box(c).Absolute().Left(xx).Top(yy + 27).Width(ww).Height(3).Radius(1.5).Background(Col(0xE9BD51))
			text := stateString(config, "记下一点想法\n点击编辑便签", "text")
			for i, line := range strings.Split(text, "\n") {
				if i >= 3 && !large {
					break
				}
				Label(c, line, xx, yy+43+float32(i)*23, 14, s.OnSurface, false, ww)
			}
		case "anniversary":
			date, days, err := calendarDateDistance(stateString(config, "", "date"), time.Now())
			if err != nil {
				Label(c, "设置重要日期", xx, yy+49, 16, s.OnSurface, true, ww)
			} else {
				if days < 0 {
					days = -days
				}
				value(itoa(days), "天 · "+date.Format("2006-01-02"))
			}
		case "weather":
			if t, ok := stateFloat(data, "temperature"); ok {
				value(fmt.Sprintf("%.0f°", t), stateString(data, "天气", "city"))
			} else {
				Icon(c, "light_mode", xx, yy+32, 32, s.Primary)
				Label(c, "等待天气数据", xx, yy+83, 14, s.OnSurface, true, ww)
			}
		case "battery":
			raw, _ := a.State["systemMetrics"].(map[string]any)
			if v, ok := stateFloat(raw, "batteryPercent"); ok {
				value(fmt.Sprintf("%.0f%%", v), "电池电量")
			} else {
				value("—", "未检测到电池")
			}
		case "storage":
			if free, ok := stateFloat(data, "free"); ok {
				value(fmt.Sprintf("%.0f", free/1e9), "GB 可用")
			} else {
				Label(c, "暂无磁盘读数", xx, yy+57, 16, s.OnSurface, true, ww)
			}
		case "worldClock":
			value(stateString(data, "—", "clock"), stateString(data, "选择城市", "city"))
		case "launcher":
			ui.Box(c).Absolute().Left(xx).Top(yy + 33).Width(50).Height(50).Radius(25).Background(Col(s.High))
			Icon(c, "arrow_forward", xx+12, yy+45, 26, s.Primary)
			Label(c, stateString(config, "常用入口", "title"), xx, yy+95, 16, s.OnSurface, true, ww)
		case "today":
			for i, row := range []struct {
				n     int
				label string
			}{{a.pendingTodos(), "待办事项"}, {a.runningTimers(), "运行中计时"}} {
				x, y, cw := xx+float32(i)*ww/2, yy+30, ww/2
				if small {
					x, y, cw = xx, yy+26+float32(i)*48, ww
				}
				Label(c, itoa(row.n), x, y, 27, s.Primary, true, cw)
				Label(c, row.label, x, y+38, 10, s.OnSurfaceVariant, false, cw)
			}
		case "focusStats":
			root, _ := a.State["widgetData"].(map[string]any)
			stats, _ := root["focusStats"].(map[string]any)
			ms, _ := stateFloat(stats, "todayMs")
			value(itoa(int(ms/60000)), "今日专注 · 分钟")
		}
	})
}
