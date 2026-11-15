package ui

import "time"

// Gallery examples have their own snapshot and never enter the live app state
// or the widgetAdd command. Cache them by day instead of allocating every frame.
func (a *App) widgetGalleryExamples(now time.Time) *App {
	day := now.Format("2006-01-02")
	if a.widgetExamples == nil || stateString(a.widgetExamples.State, "", "today") != day {
		a.widgetExamples = &App{State: widgetExampleState(now)}
	}
	a.widgetExamples.Palette = a.Palette
	return a.widgetExamples
}

func widgetExampleState(now time.Time) map[string]any {
	day := now.Format("2006-01-02")
	widgets := []any{}
	data := map[string]any{}
	add := func(kind string, config, readings map[string]any) {
		id := "gallery-example-" + kind
		widgets = append(widgets, map[string]any{"id": id, "kind": kind, "config": config})
		data[id] = readings
	}
	add("note", map[string]any{"text": "把今天过得从容一点\n读几页书，散一会儿步\n记得给自己留点时间"}, nil)
	add("anniversary", map[string]any{"title": "下一次旅行", "date": now.AddDate(0, 0, 28).Format("2006-01-02")}, nil)
	add("schedule", map[string]any{"title": "周末出游", "deadline": now.AddDate(0, 0, 3).Format("2006-01-02") + " 10:00"}, nil)
	add("weather", map[string]any{"city": "上海"}, map[string]any{"city": "上海 · 多云", "temperature": float64(24), "code": float64(2)})
	add("storage", map[string]any{"drive": "C:"}, map[string]any{"free": float64(256e9), "total": float64(512e9)})
	add("worldClock", map[string]any{"city": "东京"}, map[string]any{"city": "东京 · 日本", "clock": now.In(time.FixedZone("JST", 9*60*60)).Format("15:04")})
	add("launcher", map[string]any{"title": "打开工作文件夹", "target": ""}, nil)
	data["focusStats"] = map[string]any{"todayMs": float64(95 * 60000)}
	return map[string]any{
		"today": day, "now": now.Format("15:04"),
		"title": "午后漫游", "artist": "点屿 · 轻音乐精选", "isPlaying": true,
		"todos": []any{
			map[string]any{"id": "gallery-example-read", "title": "读完一章书", "date": day, "time": "15:00", "color": "#57BFFF"},
			map[string]any{"id": "gallery-example-walk", "title": "傍晚散步", "date": day, "time": "18:30", "color": "#9A8BEB"},
			map[string]any{"id": "gallery-example-plan", "title": "整理明天的计划", "date": day, "time": "20:00", "color": "#EAB36A"},
		},
		"timers": []any{map[string]any{
			"id": "gallery-example-focus", "name": "阅读时间", "mode": "pomodoro",
			"focus": float64(1500), "rest": float64(300), "duration": float64(1500),
			"phase": "focus", "elapsedMs": float64(378000), "running": true,
			"round": float64(1), "rounds": float64(4),
		}},
		"systemMetrics": map[string]any{
			"cpuPercent": float64(18), "memoryPercent": float64(42),
			"memoryUsedGiB": float64(6.7), "memoryTotalGiB": float64(16),
			"batteryPercent": float64(86), "externalPower": false,
			"wirelessDevices": []any{
				map[string]any{"id": "gallery-example-headphones", "name": "蓝牙耳机", "kind": "headphones", "connected": true, "batteryPercent": float64(84)},
				map[string]any{"id": "gallery-example-mouse", "name": "无线鼠标", "kind": "mouse", "connected": true, "batteryPercent": float64(76)},
				map[string]any{"id": "gallery-example-keyboard", "name": "无线键盘", "kind": "keyboard", "connected": true, "batteryPercent": float64(92)},
				map[string]any{"id": "gallery-example-gamepad", "name": "游戏手柄", "kind": "gamepad", "connected": true, "batteryPercent": float64(52)},
			},
		},
		"screenTime": map[string]any{
			"todayMs": float64(266 * 60000), "yesterdayMs": float64(312 * 60000),
			"week": []any{float64(185 * 60000), float64(242 * 60000), float64(205 * 60000), float64(320 * 60000), float64(228 * 60000), float64(312 * 60000), float64(266 * 60000)},
		},
		"desktopWidgets": widgets, "widgetData": data,
	}
}
