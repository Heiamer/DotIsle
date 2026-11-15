package ui

import (
	"fmt"
	"math"
	"time"

	"github.com/egoist/mygo/ui"
)

func scheduleText(config map[string]any, now time.Time) (string, string, string) {
	raw := stateString(config, "", "deadline")
	at, err := time.ParseInLocation("2006-01-02 15:04", raw, now.Location())
	if err != nil {
		return "设置日程", "填写日期与时间", ""
	}
	minutes := int(math.Ceil(at.Sub(now).Minutes()))
	if minutes <= 0 {
		return "已到时间", stateString(config, "重要日程", "title"), at.Format("1月2日 15:04")
	}
	text := fmt.Sprintf("%d分", minutes)
	if minutes >= 1440 {
		text = fmt.Sprintf("%d天 %d小时", minutes/1440, minutes%1440/60)
	} else if minutes >= 60 {
		text = fmt.Sprintf("%d小时 %d分", minutes/60, minutes%60)
	}
	return text, "距离 " + stateString(config, "重要日程", "title"), at.Format("1月2日 15:04")
}

func (a *App) scheduleCard(c *ui.Context, config map[string]any, x, y, w, h float32, small bool) {
	s := &a.Palette.Scheme
	c.After(time.Minute)
	text, detail, date := scheduleText(config, time.Now())
	size := float32(32)
	if small || w < 220 {
		size = 23
	}
	Label(c, text, x, y+4, size, s.OnSurface, true, w)
	detailY := float32(50)
	if small {
		detailY = 39
	}
	Label(c, detail, x, y+detailY, 12, s.OnSurfaceVariant, false, w)
	if date != "" {
		Label(c, date, x, y+detailY+25, 11, s.Primary, false, w)
	}
}

func usageDuration(ms float64) string {
	m := int(math.Max(0, ms) / 60000)
	if m >= 60 {
		return fmt.Sprintf("%d小时 %d分", m/60, m%60)
	}
	return fmt.Sprintf("%d分钟", m)
}

func (a *App) screenTimeCard(c *ui.Context, x, y, w, h float32, small bool) {
	s := &a.Palette.Scheme
	data, _ := a.State["screenTime"].(map[string]any)
	today, known := stateFloat(data, "todayMs")
	text := "正在连接"
	if known {
		text = usageDuration(today)
		if today > 0 && today < 60000 {
			text = "不足1分钟"
		}
	}
	size := float32(32)
	if small || w < 220 {
		size = 25
	}
	Label(c, text, x, y+4, size, s.OnSurface, true, w)
	Label(c, "今日活跃用机", x, y+46, 11, s.OnSurfaceVariant, false, w)
	yesterday := "昨日暂无记录"
	if v, ok := stateFloat(data, "yesterdayMs"); ok {
		yesterday = "昨日 " + usageDuration(v)
	}
	chartX, chartW := x, w
	split := w >= 380 && h >= 160
	if split {
		chartX, chartW = x+w*.47, w*.53
		Label(c, yesterday, x, y+82, 12, s.OnSurfaceVariant, false, w*.45)
		Label(c, "仅统计点屿运行期间", x, y+113, 10, s.OnSurfaceVariant, false, w*.45)
	}
	if h < 190 {
		Label(c, yesterday, x, y+74, 11, s.OnSurfaceVariant, false, w)
	}
	if !small && h >= 135 {
		week, _ := data["week"].([]any)
		peak := float64(60000)
		for _, value := range week {
			if v, ok := value.(float64); ok && v > peak {
				peak = v
			}
		}
		chartH := float32(28)
		if h > 190 || split {
			chartH = 74
			if !split {
				Label(c, yesterday, x, y+76, 12, s.OnSurfaceVariant, false, w)
			}
		}
		base := y + h - 23
		bw := (chartW - 36) / 7
		for i := 0; i < 7; i++ {
			v, ok := float64(0), false
			if i < len(week) {
				v, ok = week[i].(float64)
			}
			barH := float32(3)
			color := s.High
			if ok {
				barH += float32(clamp01(v/peak)) * chartH
				color = mix(s.High, s.Primary, .4)
				if i == 6 {
					color = s.Primary
				}
			}
			ui.Box(c).Absolute().Left(chartX + float32(i)*(bw+6)).Top(base - barH).Width(bw).Height(barH).Radius(3).Background(Col(color)).IgnoreInput(true)
		}
		Label(c, "近7天", chartX, base+6, 10, s.OnSurfaceVariant, false, chartW)
	}
}
