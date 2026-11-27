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
		src := a.overviewWidgetSource(kind)
		config, data := src.config, src.data
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
			text := stateString(config, "", "text")
			if text == "" {
				break
			}
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
		case "analogClock":
			now := time.Now()
			dial := float32(84)
			if small {
				dial = 76
			} else if large {
				dial = 156
			}
			dx, dy := xx+(ww-dial)/2, yy+26
			if !small && !large {
				dx, dy = xx+8, yy+24
			} else if large {
				dy = yy + 48
			}
			ui.Box(c).Absolute().Left(dx).Top(dy).Width(dial).Height(dial).Radius(dial / 2).Background(Col(s.High))
			ui.Box(c).Absolute().Left(dx + dial/2 - 2).Top(dy + dial*0.18).Width(4).Height(dial * 0.34).Radius(2).Background(Col(s.OnSurface))
			ui.Box(c).Absolute().Left(dx + dial/2).Top(dy + dial/2 - 1).Width(dial * 0.30).Height(3).Radius(1.5).Background(Col(0xFF9F0A))
			ui.Box(c).Absolute().Left(dx + dial/2 - 4).Top(dy + dial/2 - 4).Width(8).Height(8).Radius(4).Background(Col(0xFF9F0A))
			if !small && !large {
				rx := dx + dial + 20
				rw := pw - 24 - rx
				Label(c, []string{"星期日", "星期一", "星期二", "星期三", "星期四", "星期五", "星期六"}[now.Weekday()], rx, yy+34, 13, 0xFF9F0A, true, rw)
				Label(c, now.Format("1月2日"), rx, yy+58, 24, s.OnSurface, true, rw)
				Label(c, now.Format("15:04:05"), rx, yy+94, 12, s.OnSurfaceVariant, false, rw)
			}
		case "activityRings":
			outer := float32(82)
			if large {
				outer = 140
			}
			rx := xx + (ww-outer)/2
			ry := yy + 28
			if !small && !large {
				rx = xx + 10
			}
			widgetCircle(c, rx, ry, outer, 0.72, 0xFA114F, s.High)
			widgetCircle(c, rx+11, ry+11, outer-22, 0.58, 0x30D158, s.High)
			widgetCircle(c, rx+22, ry+22, outer-44, 0.84, 0x32ADE6, s.High)
			if !small {
				lx, ly, lw := rx+outer+18, yy+32, pw-24-(rx+outer+18)
				if large {
					lx, ly, lw = xx, ry+outer+22, ww
				}
				for i, row := range []struct {
					lbl string
					col uint32
				}{{"专注  72%", 0xFA114F}, {"待办  58%", 0x30D158}, {"活跃  84%", 0x32ADE6}} {
					ui.Box(c).Absolute().Left(lx).Top(ly + float32(i)*30 + 5).Width(8).Height(8).Radius(4).Background(Col(row.col))
					Label(c, row.lbl, lx+14, ly+float32(i)*30, 12, s.OnSurface, true, lw-14)
				}
			}
		case "quickDock":
			tiles := []struct {
				label, icon string
				tint        uint32
			}{
				{"快速专注", "light_mode", 0xFF7B61},
				{"播放音乐", "play_arrow", 0x0A84FF},
				{"下一曲", "skip_next", 0x5E5CE6},
				{"待办清单", "arrow_forward", 0x30D158},
			}
			gap := float32(8)
			tw, th := (ww-gap)/2, (ph-yy-42-gap)/2
			for i, tile := range tiles {
				tx := xx + float32(i%2)*(tw+gap)
				ty := yy + 26 + float32(i/2)*(th+gap)
				ui.Box(c).Absolute().Left(tx).Top(ty).Width(tw).Height(th).Radius(14).Background(Col(s.High))
				Icon(c, tile.icon, tx+12, ty+th/2-9, 18, tile.tint)
				if !small {
					Label(c, tile.label, tx+36, ty+th/2-8, 12, s.OnSurface, true, tw-42)
				}
			}
		case "quote":
			Label(c, "“", xx+2, yy+22, 26, s.Primary, true, 24)
			quoteText := stateString(config, "少即是多，让桌面留出呼吸的秩序。", "text")
			Label(c, quoteText, xx+22, yy+34, 14, s.OnSurface, false, ww-26)
			Label(c, "Apple Design · 点击切换", xx+2, ph-40, 10, s.OnSurfaceVariant, false, ww)
		case "photoFrame":
			ui.Box(c).Absolute().Left(xx).Top(yy + 26).Width(ww).Height(ph - yy - 68).Radius(14).Background(Col(s.High))
			Icon(c, "light_mode", xx+ww/2-14, yy+(ph-yy-68)/2+4, 28, 0xF59E0B)
			CenteredText(c, "拖入照片或目录", xx, yy+(ph-yy-68)/2+36, ww, 20, 11, s.OnSurface, true)
			Label(c, stateString(config, "桌面相框 · 点击切换", "caption"), xx+2, ph-36, 11, s.OnSurfaceVariant, false, ww)
		case "dropzone":
			ui.Box(c).Absolute().Left(xx).Top(yy + 26).Width(ww).Height(ph - yy - 56).Radius(16).Background(Col(s.High))
			Icon(c, "arrow_forward", xx+ww/2-12, yy+(ph-yy-56)/2, 24, 0x06B6D4)
			CenteredText(c, "拖放文件至此暂存", xx, yy+(ph-yy-56)/2+28, ww, 20, 12, s.OnSurface, true)
		case "systemPro":
			m := a.widgetSystemMetrics()
			cols := 3
			if small {
				cols = 2
			}
			cw := (ww - float32(cols-1)*6) / float32(cols)
			cards := []struct {
				name string
				val  string
			}{
				{"CPU", fmt.Sprintf("%.0f%%", m.cpu)},
				{"GPU", "24%"},
				{"RAM", fmt.Sprintf("%.0f%%", m.memory)},
			}
			for i := 0; i < cols; i++ {
				cx := xx + float32(i)*(cw+6)
				ui.Box(c).Absolute().Left(cx).Top(yy + 26).Width(cw).Height(52).Radius(12).Background(Col(s.High))
				Label(c, cards[i].name, cx+8, yy+31, 9, s.OnSurfaceVariant, true, cw-16)
				Label(c, cards[i].val, cx+8, yy+46, 17, s.OnSurface, true, cw-16)
			}
			wMeter(c, xx, ph-56, ww, 6, float32(m.cpu/100), 0x10B981, s.High)
			Label(c, "↓ 512 KB/s   ↑ 48 KB/s", xx+2, ph-40, 10, s.OnSurfaceVariant, true, ww)
		case "appFolder":
			colors := []uint32{0x007AFF, 0x2C2C2E, 0x0EA5E9, 0x636366, 0xF59E0B, 0x374151, 0x9333EA, 0x059669, 0x4F46E5}
			if large {
				gap := float32(10)
				cell := float32(72)
				ox, oy := xx+(ww-cell*3-gap*2)/2, yy+26
				for i := 0; i < 9; i++ {
					cx := ox + float32(i%3)*(cell+gap)
					cy := oy + float32(i/3)*(cell+gap)
					ui.Box(c).Absolute().Left(cx).Top(cy).Width(cell).Height(cell).Radius(16).Background(Col(colors[i]))
				}
			} else {
				cols := 4
				if small {
					cols = 2
				}
				gap := float32(10)
				iconS := float32(58)
				if small {
					iconS = 54
				}
				ox := xx + (ww-float32(cols)*iconS-float32(cols-1)*gap)/2
				oy := yy + (ph-yy*2-iconS)/2
				if small {
					oy = yy + 6
				}
				for i := 0; i < 4; i++ {
					cx, cy := ox+float32(i%cols)*(iconS+gap), oy
					if small {
						cy = oy + float32(i/2)*(iconS+gap)
					}
					ui.Box(c).Absolute().Left(cx).Top(cy).Width(iconS).Height(iconS).Radius(14).Background(Col(colors[i]))
				}
			}
		}
	})
}
