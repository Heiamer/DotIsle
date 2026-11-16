package ui

import (
	"fmt"
	"strings"
	"time"

	"github.com/egoist/mygo/ui"
)

// Overview cards consume the core snapshot. Tiles configured in the overview own
// their content first; a desktop widget of the same kind is only a migration
// fallback, so adding a card never requires placing it on the desktop.
type overviewWidgetSource struct {
	id     string
	config map[string]any
	data   map[string]any
}

func (a *App) overviewWidgetSource(kind string) overviewWidgetSource {
	root, _ := a.State["widgetData"].(map[string]any)
	if items, ok := a.State["overviewWidgets"].([]any); ok {
		for _, item := range items {
			tile, ok := item.(map[string]any)
			if !ok || stateString(tile, "", "kind") != kind {
				continue
			}
			id := stateString(tile, "", "id")
			config, _ := tile["config"].(map[string]any)
			data, _ := root[id].(map[string]any)
			return overviewWidgetSource{id, config, data}
		}
	}
	items, _ := a.State["desktopWidgets"].([]any)
	for _, item := range items {
		widget, ok := item.(map[string]any)
		if !ok || stateString(widget, "", "kind") != kind {
			continue
		}
		config, _ := widget["config"].(map[string]any)
		id := stateString(widget, "", "id")
		data, _ := root[id].(map[string]any)
		return overviewWidgetSource{id, config, data}
	}
	return overviewWidgetSource{}
}

// overviewConfigurable lists the kinds that own user content. Automatic readings
// (clock, calendar, battery, system, screen time, today) have nothing to edit and
// therefore no "..." affordance.
func overviewConfigurable(kind string) bool {
	switch kind {
	case "weather", "storage", "worldClock", "note", "anniversary", "schedule", "launcher":
		return true
	}
	return false
}

func (a *App) overviewWidget(c *ui.Context, r overviewRect) {
	p, s := &a.Palette, &a.Palette.Scheme
	kind := strings.TrimPrefix(r.key, "widget:")
	src := a.overviewWidgetSource(kind)
	config := src.config
	if config == nil {
		config = map[string]any{}
	}
	data := src.data
	if overviewConfigurable(kind) {
		IconButton(c, p, "设置"+overviewName(r.key), r.x+r.w-50, r.y+10, 32, 32, "more_horiz", false, true, func() { a.openOverviewEditor(kind) })
	}
	x, y, w := r.x+20, r.y+54, r.w-40
	empty := func(title, detail, action string) {
		Label(c, title, x, y+25, 17, s.OnSurface, true, w)
		Label(c, detail, x, y+64, 11, s.OnSurfaceVariant, false, w)
		if action != "" {
			widgetTextAction(c, p, "设置"+overviewName(r.key)+"内容", action, x, r.y+r.h-44, w, func() { a.openOverviewEditor(kind) })
		}
	}
	value := func(text, detail string) {
		Label(c, text, x, y+8, 42, s.OnSurface, true, w)
		Label(c, detail, x, y+66, 12, s.OnSurfaceVariant, false, w)
	}
	switch kind {
	case "schedule":
		a.scheduleCard(c, config, x, y, w, r.h-70, false)
		if stateString(config, "", "deadline") == "" {
			widgetTextAction(c, p, "设置日程时间", "设置日期与时间", x, r.y+r.h-44, w, func() { a.openOverviewEditor("schedule") })
		}
	case "screenTime":
		a.screenTimeCard(c, x, y, w, r.h-70, false)
	case "calendar":
		now := time.Now()
		c.After(time.Minute)
		Label(c, now.Format("2006年1月"), x, y-2, 14, s.OnSurface, true, w)
		cell := w / 7
		for i, name := range []string{"一", "二", "三", "四", "五", "六", "日"} {
			CenteredText(c, name, x+float32(i)*cell, y+25, cell, 18, 10, s.OnSurfaceVariant, false)
		}
		first := time.Date(now.Year(), now.Month(), 1, 0, 0, 0, 0, now.Location())
		offset := (int(first.Weekday()) + 6) % 7
		for day := 1; day <= first.AddDate(0, 1, -1).Day(); day++ {
			index := day - 1 + offset
			xx, yy := x+float32(index%7)*cell, y+49+float32(index/7)*21
			fg := s.OnSurface
			if day == now.Day() {
				ui.Box(c).Absolute().Left(xx + (cell-22)/2).Top(yy).Width(22).Height(22).Radius(11).Background(Col(s.Primary)).IgnoreInput(true)
				fg = s.OnPrimary
			}
			CenteredText(c, itoa(day), xx, yy, cell, 22, 11, fg, day == now.Day())
		}
		return
	case "system":
		m := a.widgetSystemMetrics()
		for i, entry := range []struct {
			name      string
			amount    float64
			available bool
		}{{"CPU", m.cpu, m.hasCPU}, {"内存", m.memory, m.hasMemory}} {
			text := "暂时无读数"
			if entry.available {
				text = fmt.Sprintf("%.0f%%", entry.amount)
			}
			tw := (w - 12) / 2
			xx := x + float32(i)*(tw+12)
			if i == 1 {
				ui.Box(c).Absolute().Left(xx - 6).Top(y + 4).Width(1).Height(120).Background(AlphaCol(s.OnSurface, .07)).IgnoreInput(true)
			}
			Label(c, entry.name, xx, y+4, 12, s.OnSurfaceVariant, false, tw-8)
			if !entry.available {
				text = "—"
			}
			Label(c, text, xx, y+36, 32, s.OnSurface, true, tw-8)
			if entry.available {
				progressLine(c, p, xx, y+100, tw-12, entry.amount/100)
			} else {
				Label(c, "等待读数", xx, y+96, 10, s.OnSurfaceVariant, false, tw-8)
			}
		}
	case "battery":
		raw, _ := a.State["systemMetrics"].(map[string]any)
		if percent, ok := stateFloat(raw, "batteryPercent"); ok && percent >= 0 && percent <= 100 {
			status := "正在使用电池"
			if stateBool(raw, "externalPower") {
				status = "已连接电源"
			}
			value(fmt.Sprintf("%.0f%%", percent), status)
			progressLine(c, p, x, y+112, w, percent/100)
		} else {
			empty("未检测到电池", "等待设备提供电量读数", "")
		}
	case "today":
		for i, entry := range []struct {
			count int
			label string
		}{{a.pendingTodos(), "待办事项"}, {a.runningTimers(), "运行中计时"}} {
			tw := (w - 12) / 2
			xx := x + float32(i)*(tw+12)
			Label(c, itoa(entry.count), xx, y+15, 42, s.Primary, true, tw-12)
			Label(c, entry.label, xx+2, y+80, 11, s.OnSurfaceVariant, false, tw-12)
		}
		widgetTextAction(c, p, "今日安排打开待办", "查看待办事项", x, r.y+r.h-44, w, func() { a.navigate(PageTodos) })
		return
	case "focusStats":
		root, _ := a.State["widgetData"].(map[string]any)
		stats, _ := root["focusStats"].(map[string]any)
		if ms, ok := stateFloat(stats, "todayMs"); ok {
			Label(c, itoa(int(ms/60000)), x, y, 36, s.OnSurface, true, w)
			Label(c, "今日专注 · 分钟", x, y+48, 11, s.OnSurfaceVariant, false, w)
			week, _ := stats["week"].([]any)
			maximum := float64(1)
			for _, v := range week {
				if n, ok := v.(float64); ok && n > maximum {
					maximum = n
				}
			}
			for i := 0; i < 7; i++ {
				v := float64(0)
				if i < len(week) {
					v, _ = week[i].(float64)
				}
				barW, barH := (w-36)/7, float32(3+clamp01(v/maximum)*35)
				ui.Box(c).Absolute().Left(x + float32(i)*(barW+6)).Top(y + 115 - barH).Width(barW).Height(barH).Radius(3).Background(Col(mix(s.High, s.Primary, .3+.7*clamp01(v/maximum)))).IgnoreInput(true)
			}
		} else {
			empty("暂无专注记录", "完成专注计时后在这里查看", "")
		}
	case "note":
		if text := stateString(config, "", "text"); text != "" {
			for i, line := range strings.Split(text, "\n") {
				if i == 4 {
					break
				}
				Label(c, line, x, y+float32(i)*28, 14, s.OnSurface, false, w)
			}
		} else {
			empty("还没有便签", "点击右上角写下内容", "编辑内容")
		}
	case "anniversary":
		date, days, err := calendarDateDistance(stateString(config, "", "date"), time.Now())
		if err != nil {
			empty("设置一个重要日期", "点击右上角选择日期", "设置日期")
		} else {
			detail := stateString(config, date.Format("2006-01-02"), "title")
			prefix := "还有 "
			if days < 0 {
				prefix = "已经 "
				days = -days
			}
			value(itoa(days)+" 天", prefix+detail)
			Label(c, date.Format("2006年1月2日"), x, y+107, 11, s.OnSurfaceVariant, false, w)
		}
	case "weather":
		if temperature, ok := stateFloat(data, "temperature"); ok {
			detail := stateString(data, "", "city")
			if stateBool(data, "stale") {
				detail += " · 数据待更新"
			}
			Label(c, detail, x, y-2, 14, s.OnSurface, true, w)
			Label(c, fmt.Sprintf("%.0f°", temperature), x-2, y+22, 52, s.OnSurface, false, w)
			high, hok := stateFloat(data, "high")
			low, lok := stateFloat(data, "low")
			if hok && lok {
				label := fmt.Sprintf("最高 %.0f° · 最低 %.0f°", high, low)
				if stateString(data, "", "dailyRangeKind") == "available-hourly-samples" {
					label = fmt.Sprintf("预报范围 %.0f°–%.0f°", low, high)
				}
				Label(c, label, x, y+115, 11, s.OnSurfaceVariant, false, w)
			}
		} else if msg := stateString(data, "", "error"); msg != "" {
			empty("天气暂不可用", msg, "修改城市")
		} else {
			empty("等待天气数据", "点击右上角设置城市后同步", "设置城市")
		}
	case "storage":
		free, fok := stateFloat(data, "free")
		total, tok := stateFloat(data, "total")
		if fok && tok && total > 0 && free >= 0 && free <= total {
			value(fmt.Sprintf("%.0f GB", free/1e9), stateString(data, "", "drive")+" 可用 · 共 "+fmt.Sprintf("%.0f GB", total/1e9))
			progressLine(c, p, x, y+112, w, 1-free/total)
		} else {
			empty("暂无磁盘读数", "点击右上角选择磁盘", "选择磁盘")
		}
	case "worldClock":
		if clock := stateString(data, "", "clock"); clock != "" {
			value(clock, stateString(data, "", "date"))
			Label(c, stateString(config, stateString(data, "", "city"), "city"), x, y+107, 12, s.OnSurfaceVariant, false, w)
		} else {
			empty("等待世界时钟", "点击右上角选择城市", "选择城市")
		}
	case "launcher":
		target := stateString(config, "", "target")
		if target == "" {
			empty(stateString(config, "添加常用入口", "title"), "点击右上角设置目标", "设置目标")
			return
		}
		empty(stateString(config, "添加常用入口", "title"), target, "")
		Button(c, p, "打开概览快捷入口", x, r.y+r.h-46, w, 32, "打开", "", false, a.Connected, func() { a.command("widgetLaunch", map[string]any{"id": src.id}) })
		return
	}
}
