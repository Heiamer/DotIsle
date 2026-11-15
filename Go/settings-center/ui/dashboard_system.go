package ui

import (
	"fmt"
	"github.com/egoist/mygo/ui"
)

func (a *App) dashboardSystem(c *ui.Context, r overviewRect) {
	p, s := &a.Palette, &a.Palette.Scheme
	a.cardHeader(c, r, "电脑状态")
	m := a.widgetSystemMetrics()
	status := "等待连接"
	if a.Connected {
		status = "实时"
	}
	cardBadge(c, p, status, r.x+r.w-84, r.y+14, 64, a.Connected)
	w := (r.w - 56) / 2
	for i, metric := range []struct {
		name  string
		value float64
		known bool
	}{
		{"处理器", m.cpu, m.hasCPU}, {"内存", m.memory, m.hasMemory},
	} {
		x, y := r.x+20+float32(i)*(w+16), r.y+60
		Label(c, metric.name, x, y, 12, s.OnSurfaceVariant, false, w)
		value := "—"
		if metric.known {
			value = fmt.Sprintf("%.0f%%", metric.value)
		}
		Label(c, value, x, y+28, 38, s.OnSurface, true, w)
		if r.w >= 480 {
			widgetCircle(c, x+w-76, y+6, 70, metric.value/100, s.Primary, mix(s.Container, s.OnSurface, .06))
		}
		if metric.known {
			progressLine(c, p, x, y+102, w, metric.value/100)
		} else {
			Label(c, "等待读数", x, y+98, 11, s.OnSurfaceVariant, false, w)
		}
	}
	ui.Box(c).Absolute().Left(r.x + 20).Top(r.y + r.h - 66).Width(r.w - 40).Height(1).
		Background(AlphaCol(s.OnSurface, .06)).IgnoreInput(true)
	detail := "等待内存容量"
	if m.hasSizes {
		detail = fmt.Sprintf("已用 %.1f / %.1f GiB", m.usedGiB, m.totalGiB)
	}
	Label(c, detail, r.x+20, r.y+r.h-46, 12, s.OnSurfaceVariant, false, r.w-40)
	if r.w >= 480 {
		raw, _ := a.State["systemMetrics"].(map[string]any)
		power := ""
		if connected, ok := raw["externalPower"].(bool); ok {
			if connected {
				power = "已连接电源"
			} else if m.hasBattery {
				power = fmt.Sprintf("电量 %.0f%%", m.battery)
			}
		}
		Label(c, power, r.x+r.w-140, r.y+r.h-46, 12, s.OnSurfaceVariant, false, 120)
	}
}
