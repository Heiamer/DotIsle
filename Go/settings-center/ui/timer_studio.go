package ui

import (
	"fmt"
	"github.com/egoist/mygo/ui"
)

const timerStudioHeight = 340

func (a *App) timerCards(c *ui.Context, p *Palette, t *goTimer, lx, ly, w float32) {
	s := &p.Scheme
	left := (w - 16) * .52
	right := w - left - 16
	rx := lx + left + 16
	appleWidgetSurface(c, p, overviewRect{"", lx, ly, left, timerStudioHeight})
	appleWidgetSurface(c, p, overviewRect{"", rx, ly, right, timerStudioHeight})
	mode := []string{"番茄钟", "倒计时", "秒表"}[max(0, min(focusUI.mode, 2))]
	title := mode
	if t != nil && t.Name != "" {
		title = t.Name
	}
	Label(c, title, lx+22, ly+20, 15, s.OnSurface, true, left-120)
	status := "准备就绪"
	if t != nil {
		status = "已暂停"
		if t.Running {
			status = "进行中"
		}
		if t.Finished {
			status = "已完成"
		}
	}
	Label(c, status, lx+left-88, ly+23, 11, s.OnSurfaceVariant, false, 68)
	clock := formatTimerTime(int64(focusUI.minutes * 60))
	if focusUI.seconds > 0 {
		clock = formatTimerTime(int64(focusUI.seconds))
	}
	if focusUI.mode == 2 {
		clock = "00:00"
	}
	if t != nil {
		clock = t.clock()
	}
	fraction := float64(0)
	if t != nil {
		fraction = t.progress()
	}
	cx, cy := lx+(left-184)/2, ly+57
	ink := s.Primary
	if focusUI.mode == 2 {
		ink = s.OnSurface
	}
	widgetCircle(c, cx, cy, 184, fraction, ink, mix(s.Container, s.OnSurface, .055))
	size := float32(42)
	if len(clock) > 5 {
		size = 32
	}
	CenteredText(c, clock, cx-28, cy+52, 240, 64, size, s.OnSurface, true)
	subtitle := []string{"专注当下", "到时提醒", "正向计时"}[max(0, min(focusUI.mode, 2))]
	if t != nil {
		subtitle = t.subtitle()
	}
	CenteredText(c, subtitle, lx+24, cy+117, left-48, 24, 11, s.OnSurfaceVariant, false)
	if focusUI.mode == 0 {
		rounds, round := int64(max(focusUI.rounds, 1)), int64(0)
		if t != nil {
			rounds, round = max64(t.Rounds, 1), max64(t.Round, 0)
		}
		if rounds > 8 {
			CenteredText(c, fmt.Sprintf("第 %d / %d 轮", round, rounds), lx+24, ly+245, left-48, 22, 11, s.Primary, true)
		}
		dots := int(min(rounds, 8))
		for i := 0; i < dots && rounds <= 8; i++ {
			color := mix(s.Container, s.OnSurface, .1)
			if int64(i) < round {
				color = s.Primary
			}
			ui.Box(c).Absolute().Left(lx + left/2 - float32(dots*14)/2 + float32(i*14) + 3).Top(ly + 252).Width(7).Height(7).Radius(3.5).Background(Col(color)).IgnoreInput(true)
		}
	}
	label, icon := "开始计时", "play_arrow"
	if t != nil {
		switch t.primaryAction() {
		case "pause":
			label, icon = "暂停", "pause"
		case "resume":
			label = "继续"
		default:
			label = "重新开始"
		}
	}
	Button(c, p, label, lx+22, ly+278, left-96, 42, label, icon, false, a.Connected, func() {
		if t != nil {
			a.timerAction(t.ID, t.primaryAction())
		} else {
			a.startTimer("")
		}
	})
	IconButton(c, p, "重置计时", lx+left-64, ly+277, 44, 44, "restart_alt", false, t != nil, func() {
		if t != nil {
			a.timerAction(t.ID, "reset")
		}
	})
	if focusUI.mode == 2 {
		Label(c, "分段记录", rx+22, ly+22, 15, s.OnSurface, true, right-44)
		count := 0
		latest := int64(0)
		if t != nil {
			count = len(t.Laps)
			if count > 0 {
				latest = t.Laps[count-1]
				if count > 1 {
					latest -= t.Laps[count-2]
				}
			}
		}
		Label(c, fmt.Sprintf("%02d", count), rx+24, ly+66, 48, s.OnSurface, true, right-48)
		Label(c, "已记录分段", rx+24, ly+128, 12, s.OnSurfaceVariant, false, right-48)
		ui.Box(c).Absolute().Left(rx + 24).Top(ly + 172).Width(right - 48).Height(1).Background(AlphaCol(s.OnSurface, .07)).IgnoreInput(true)
		Label(c, "最近一段", rx+24, ly+194, 12, s.OnSurfaceVariant, false, right-48)
		Label(c, formatTimerTime(latest/1000), rx+24, ly+219, 26, s.OnSurface, true, right-48)
		Button(c, p, "记录分段", rx+22, ly+278, right-44, 42, "记录分段", "add", true, t != nil && t.Running && a.Connected, func() {
			if t != nil {
				a.timerAction(t.ID, "lap")
			}
		})
		return
	}
	Label(c, "设定时长", rx+22, ly+22, 15, s.OnSurface, true, right-120)
	widgetTextAction(c, p, "调整参数", "自定义", rx+right-92, ly+14, 72, a.openTimerPrefs)
	display := fmt.Sprintf("%d", focusUI.minutes)
	unit := "分钟"
	if focusUI.seconds > 0 && focusUI.seconds%60 != 0 {
		display = formatTimerTime(int64(focusUI.seconds))
		unit = "分 : 秒"
	}
	CenteredText(c, display, rx+62, ly+62, right-124, 72, 48, s.OnSurface, true)
	CenteredText(c, unit, rx+62, ly+132, right-124, 22, 11, s.OnSurfaceVariant, false)
	IconButton(c, p, "减少一分钟", rx+20, ly+80, 40, 40, "arrow_back", false, focusUI.minutes > 1, func() { focusUI.minutes = max(1, focusUI.minutes-1); focusUI.seconds = 0; focusUI.selected = "" })
	IconButton(c, p, "增加一分钟", rx+right-60, ly+80, 40, 40, "add", false, focusUI.minutes < 240, func() { focusUI.minutes = min(240, focusUI.minutes+1); focusUI.seconds = 0; focusUI.selected = "" })
	cw := (right - 56) / 2
	for i, minutes := range []int{5, 15, 25, 45} {
		Chip(c, p, fmt.Sprintf("%d 分钟", minutes), rx+24+float32(i%2)*(cw+8), ly+180+float32(i/2)*46, cw, 36, fmt.Sprintf("%d 分钟", minutes), focusUI.minutes == minutes && focusUI.seconds == 0, func() { focusUI.minutes = minutes; focusUI.seconds = 0; focusUI.selected = "" })
	}
	detail := "计时结束后提醒你"
	if focusUI.mode == 0 {
		detail = fmt.Sprintf("%d 轮专注  ·  休息 %d 分钟", focusUI.rounds, focusUI.rest/60)
	}
	Label(c, detail, rx+24, ly+292, 12, s.OnSurfaceVariant, false, right-48)
}
