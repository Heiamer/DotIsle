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
	p, s := &a.Palette, &a.Palette.Scheme
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
		size = 24
	}
	Label(c, text, x, y+2, size, s.OnSurface, true, w)
	Label(c, "今日活跃用机", x, y+44, 11, s.OnSurfaceVariant, false, w)
	yesterday := "昨日暂无记录"
	if v, ok := stateFloat(data, "yesterdayMs"); ok {
		yesterday = "昨日 " + usageDuration(v)
	}
	chartX, chartW := x, w
	split := w >= 380 && h >= 160
	if split {
		chartX, chartW = x+w*.46, w*.54
		pillBg := AlphaCol(0x111827, 0.045)
		if p.Dark {
			pillBg = AlphaCol(0xFFFFFF, 0.065)
		}
		ui.Box(c).Absolute().Left(x).Top(y + 72).Width(w*.42 - 8).Height(28).Radius(10).Background(pillBg).IgnoreInput(true)
		Label(c, yesterday, x+10, y+79, 11, s.OnSurfaceVariant, false, w*.42-24)
		ui.Box(c).Absolute().Left(x + 2).Top(y + h - 20).Width(5).Height(5).Radius(2.5).Background(AlphaCol(s.Primary, 0.72)).IgnoreInput(true)
		Label(c, "仅统计点屿运行期间", x+12, y+h-23, 10, s.OnSurfaceVariant, false, w*.45)
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
		chartH := float32(34)
		if h > 190 || split {
			chartH = 88
			if !split {
				Label(c, yesterday, x, y+76, 12, s.OnSurfaceVariant, false, w)
			}
		}
		base := y + h - 25
		// 两条极淡水平刻度线提升数据图表参考精度
		for _, frac := range []float32{0.5, 1.0} {
			gy := base - chartH*frac
			ui.Box(c).Absolute().Left(chartX).Top(gy).Width(chartW).Height(1).Background(AlphaCol(s.OnSurface, 0.055)).IgnoreInput(true)
		}
		bw := ((chartW - 36) / 7)
		if bw < 6 {
			bw = 6
		}
		gap := (chartW - bw*7) / 6
		if gap < 3 {
			gap = 3
		}
		trackBg := AlphaCol(0x111827, 0.045)
		if p.Dark {
			trackBg = AlphaCol(0xFFFFFF, 0.055)
		}
		now := time.Now()
		dayNames := []string{"日", "一", "二", "三", "四", "五", "六"}
		for i := 0; i < 7; i++ {
			bx := chartX + float32(i)*(bw+gap)
			// 满高圆角背景轨道槽，避免无记录的日子完全隐形
			ui.Box(c).Absolute().Left(bx).Top(base - chartH).Width(bw).Height(chartH).Radius(4).Background(trackBg).IgnoreInput(true)
			v, ok := float64(0), false
			if i < len(week) {
				v, ok = week[i].(float64)
			}
			barH := float32(4)
			barCol := mix(s.High, s.Primary, 0.16)
			if ok && v > 0 {
				barH += float32(clamp01(v/peak)) * (chartH - 4)
				barCol = mix(s.High, s.Primary, 0.45)
			}
			if i == 6 {
				barCol = s.Primary
			}
			ui.Box(c).Absolute().Left(bx).Top(base - barH).Width(bw).Height(barH).Radius(4).Background(Col(barCol)).IgnoreInput(true)
			if i == 6 {
				ui.Box(c).Absolute().Left(bx + (bw-4)/2).Top(base - chartH - 8).Width(4).Height(4).Radius(2).Background(Col(s.Primary)).IgnoreInput(true)
			}
			wd := now.AddDate(0, 0, i-6).Weekday()
			lbl := dayNames[wd]
			if i == 6 {
				lbl = "今"
			}
			CenteredText(c, lbl, bx-2, base+4, bw+4, 16, 9.5, s.OnSurfaceVariant, i == 6)
		}
		Label(c, "近7天", chartX, y+4, 10, s.OnSurfaceVariant, false, chartW)
	}
}
