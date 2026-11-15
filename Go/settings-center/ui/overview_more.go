package ui

import (
	"fmt"
	"github.com/egoist/mygo/ui"
	"sort"
	"strings"
	"time"
)

type wirelessBattery struct {
	id, name, kind string
	percent        float64
	known          bool
}

func (a *App) wirelessBatteries() []wirelessBattery {
	metrics, _ := a.State["systemMetrics"].(map[string]any)
	raw, _ := metrics["wirelessDevices"].([]any)
	out := []wirelessBattery{}
	seen := map[string]bool{}
	for _, entry := range raw {
		v, ok := entry.(map[string]any)
		if !ok || !stateBool(v, "connected") {
			continue
		}
		id := stateString(v, "", "id")
		if id == "" || seen[id] {
			continue
		}
		seen[id] = true
		percent, known := stateFloat(v, "batteryPercent")
		known = known && percent >= 0 && percent <= 100
		out = append(out, wirelessBattery{id, stateString(v, "无线设备", "name"), stateString(v, "device", "kind"), percent, known})
	}
	return out
}
func (a *App) wirelessBatteryBody(c *ui.Context, r overviewRect) {
	s := &a.Palette.Scheme
	devices := a.wirelessBatteries()
	if len(devices) == 0 {
		Icon(c, "headphones", r.x+22, r.y+66, 32, s.OnSurfaceVariant)
		Label(c, "暂无连接的无线设备", r.x+22, r.y+117, 16, s.OnSurface, true, r.w-44)
		Label(c, "连接蓝牙耳机、鼠标或键盘后显示", r.x+22, r.y+151, 11, s.OnSurfaceVariant, false, r.w-44)
		return
	}
	columns := 2
	if r.w >= 400 {
		columns = 3
	}
	if len(devices) < columns {
		columns = len(devices)
	}
	cell := (r.w - 32) / float32(columns)
	maxRows := max(1, int((r.h-65)/156))
	for i, device := range devices {
		if i >= columns*maxRows {
			break
		}
		x, y := r.x+16+float32(i%columns)*cell, r.y+58+float32(i/columns)*156
		ring := float32(66)
		icon := "headphones"
		if device.kind == "mouse" {
			icon = "mouse"
		}
		if device.kind == "keyboard" {
			icon = "keyboard"
		}
		if device.kind == "gamepad" {
			icon = "gamepad"
		}
		if device.kind == "device" {
			icon = "bluetooth"
		}
		ink := s.Primary
		if device.known && device.percent <= 20 {
			ink = 0xF29B49
		}
		fraction := float64(0)
		if device.known {
			fraction = device.percent / 100
		}
		widgetCircle(c, x+(cell-ring)/2, y, ring, fraction, ink, mix(s.Container, s.OnSurface, .07))
		Icon(c, icon, x+(cell-26)/2, y+20, 26, s.OnSurface)
		text := "—"
		if device.known {
			text = fmt.Sprintf("%.0f%%", device.percent)
		}
		CenteredText(c, text, x, y+78, cell, 28, 22, s.OnSurface, true)
		CenteredText(c, device.name, x+3, y+111, cell-6, 20, 11, s.OnSurfaceVariant, false)
		if !device.known {
			CenteredText(c, "未提供电量", x, y+132, cell, 16, 9, s.OnSurfaceVariant, false)
		}
	}
}

func (a *App) overviewMore(c *ui.Context, r overviewRect) {
	s := &a.Palette.Scheme
	a.cardHeader(c, r, overviewName(r.key))
	x, y, w := r.x+20, r.y+54, r.w-40
	switch r.key {
	case "wireless":
		a.wirelessBatteryBody(c, r)
	case "cpu", "memory":
		m := a.widgetSystemMetrics()
		v, known, label := m.cpu, m.hasCPU, "实时使用率"
		if r.key == "memory" {
			v, known = m.memory, m.hasMemory
		}
		text := "—"
		if known {
			text = fmt.Sprintf("%.0f%%", v)
		}
		widgetCircle(c, x+w-104, y+12, 100, v/100, s.Primary, mix(s.Container, s.OnSurface, .06))
		Label(c, text, x, y+20, 38, s.OnSurface, true, w-106)
		Label(c, label, x, y+78, 11, s.OnSurfaceVariant, false, w-108)
		if !known {
			Label(c, "等待读数", x, r.y+r.h-52, 12, s.OnSurfaceVariant, false, w)
		} else {
			detail := "由系统实时更新"
			if r.key == "memory" {
				raw, _ := a.State["systemMetrics"].(map[string]any)
				used, ok := stateFloat(raw, "memoryUsedGib")
				total, tok := stateFloat(raw, "memoryTotalGib")
				if ok && tok {
					detail = fmt.Sprintf("%.1f / %.1f GB", used, total)
				}
			}
			Label(c, detail, x, r.y+r.h-52, 12, s.OnSurfaceVariant, false, w)
		}
	case "week":
		now := time.Now()
		c.After(time.Minute)
		weekday := (int(now.Weekday()) + 6) % 7
		progress := (float64(weekday*86400 + now.Hour()*3600 + now.Minute()*60)) / 604800
		Label(c, fmt.Sprintf("%.0f%%", progress*100), x, y+2, 42, s.OnSurface, true, w)
		Label(c, "这一周，走到这里", x, y+66, 12, s.OnSurfaceVariant, false, w)
		cell := (w - 36) / 7
		for i, name := range []string{"一", "二", "三", "四", "五", "六", "日"} {
			bg := mix(s.Container, s.OnSurface, .06)
			if i <= weekday {
				bg = s.Primary
			}
			ui.Box(c).Absolute().Left(x + float32(i)*(cell+6)).Top(r.y + r.h - 82).Width(cell).Height(10).Radius(5).Background(Col(bg)).IgnoreInput(true)
			CenteredText(c, name, x+float32(i)*(cell+6), r.y+r.h-58, cell, 20, 10, s.OnSurfaceVariant, false)
		}
	case "agenda":
		today, _ := a.todoNow()
		items := []goTodo{}
		for _, t := range a.todoItems() {
			if !todoDone(t, today) {
				items = append(items, t)
			}
		}
		sort.SliceStable(items, func(i, j int) bool { return items[i].Date+items[i].Time < items[j].Date+items[j].Time })
		if len(items) == 0 {
			Label(c, "接下来，留给自己", x, y+30, 18, s.OnSurface, true, w)
			Label(c, "新建待办后，安排会显示在这里", x, y+70, 11, s.OnSurfaceVariant, false, w)
			return
		}
		for i, item := range items {
			if i >= int((r.h-68)/64) {
				break
			}
			a.todoAgendaRow(c, item, i, r.x+10, y-4+float32(i)*64, r.w-20, 58, i == 0, false)
		}
	}
}

func overviewIsMore(key string) bool {
	return strings.Contains("|wireless|agenda|cpu|memory|week|", "|"+key+"|")
}
