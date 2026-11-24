package ui

import (
	"fmt"
	"github.com/egoist/mygo/ui"
)

const timerStudioHeight = 356

func (a *App) timerCards(c *ui.Context, p *Palette, t *goTimer, lx, ly, w float32) {
	s := &p.Scheme
	left := (w - 16) * .54
	right := w - left - 16
	rx := lx + left + 16
	appleWidgetSurface(c, p, overviewRect{"", lx, ly, left, timerStudioHeight})
	appleWidgetSurface(c, p, overviewRect{"", rx, ly, right, timerStudioHeight})
	mode := []string{"番茄钟", "倒计时", "秒表"}[max(0, min(focusUI.mode, 2))]
	title := mode
	if t != nil && t.Name != "" {
		title = t.Name
	}
	Label(c, title, lx+24, ly+20, 15, s.OnSurface, true, left-136)
	status := "准备就绪"
	pillBg := AlphaCol(s.OnSurface, .06)
	pillFg := s.OnSurfaceVariant
	dotCol := s.Outline
	if t != nil {
		status = "已暂停"
		if t.Running {
			status = "进行中"
			pillBg = AlphaCol(s.Primary, .14)
			pillFg = s.Primary
			dotCol = s.Primary
		} else if t.Finished {
			status = "已完成"
			pillBg = AlphaCol(s.Primary, .12)
			pillFg = s.Primary
			dotCol = s.Primary
		}
	}
	pill := ui.Box(c).Absolute().Left(lx + left - 102).Top(ly + 17).Width(78).Height(26).Radius(13).Background(pillBg).IgnoreInput(true)
	if !p.HighContrast {
		pill.Border(1, AlphaCol(pillFg, .18))
	}
	pill.Children(func() {
		ui.Box(c).Absolute().Left(10).Top(10).Width(6).Height(6).Radius(3).Background(Col(dotCol)).IgnoreInput(true)
		Label(c, status, 22, 5, 11, pillFg, true, 52)
	})
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
	ringSize := float32(194)
	cx, cy := lx+(left-ringSize)/2, ly+56
	ink := s.Primary
	if focusUI.mode == 2 {
		ink = s.OnSurface
	}
	// 内环微光垫底，增强计时舞台空间层次
	ui.Box(c).Absolute().Left(cx + 12).Top(cy + 12).Width(ringSize - 24).Height(ringSize - 24).
		Radius((ringSize - 24) / 2).Background(AlphaCol(ink, .04)).IgnoreInput(true)
	widgetCircle(c, cx, cy, ringSize, fraction, ink, mix(s.Container, s.OnSurface, .06))
	size := float32(42)
	if len(clock) > 5 {
		size = 32
	}
	CenteredText(c, clock, cx-24, cy+56, ringSize+48, 64, size, s.OnSurface, true)
	subtitle := []string{"专注当下", "到时提醒", "正向计时"}[max(0, min(focusUI.mode, 2))]
	if t != nil {
		subtitle = t.subtitle()
	}
	CenteredText(c, subtitle, lx+24, cy+121, left-48, 22, 11.5, s.OnSurfaceVariant, false)
	if focusUI.mode == 0 {
		rounds, round := int64(max(focusUI.rounds, 1)), int64(0)
		if t != nil {
			rounds, round = max64(t.Rounds, 1), max64(t.Round, 0)
		}
		if rounds > 8 {
			CenteredText(c, fmt.Sprintf("第 %d / %d 轮", round, rounds), lx+24, ly+258, left-48, 22, 11, s.Primary, true)
		}
		dots := int(min(rounds, 8))
		for i := 0; i < dots && rounds <= 8; i++ {
			color := mix(s.Container, s.OnSurface, .12)
			dotW := float32(7)
			if int64(i) < round {
				color = s.Primary
			} else if int64(i) == round && t != nil && t.Running {
				color = s.Primary
				dotW = 14
			}
			ui.Box(c).Absolute().Left(lx + left/2 - float32(dots*15)/2 + float32(i*15) + 2).Top(ly + 264).Width(dotW).Height(7).Radius(3.5).Background(Col(color)).IgnoreInput(true)
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
	Button(c, p, label, lx+24, ly+292, left-104, 44, label, icon, false, a.Connected, func() {
		if t != nil {
			a.timerAction(t.ID, t.primaryAction())
		} else {
			a.startTimer("")
		}
	})
	IconButton(c, p, "重置计时", lx+left-68, ly+292, 44, 44, "restart_alt", false, t != nil, func() {
		if t != nil {
			a.timerAction(t.ID, "reset")
		}
	})
	if focusUI.mode == 2 {
		Label(c, "分段记录", rx+24, ly+22, 15, s.OnSurface, true, right-48)
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
		statBox := ui.Box(c).Absolute().Left(rx + 24).Top(ly + 64).Width(right - 48).Height(196).Radius(16).Background(AlphaCol(s.OnSurface, .035)).IgnoreInput(true)
		if !p.HighContrast {
			statBox.Border(1, AlphaCol(s.OnSurface, .06))
		}
		Label(c, fmt.Sprintf("%02d", count), rx+42, ly+82, 46, s.OnSurface, true, right-84)
		Label(c, "已记录分段", rx+42, ly+138, 12, s.OnSurfaceVariant, false, right-84)
		ui.Box(c).Absolute().Left(rx + 42).Top(ly + 172).Width(right - 84).Height(1).Background(AlphaCol(s.OnSurface, .08)).IgnoreInput(true)
		Label(c, "最近一段", rx+42, ly+186, 12, s.OnSurfaceVariant, false, right-84)
		Label(c, formatTimerTime(latest/1000), rx+42, ly+210, 26, s.Primary, true, right-84)
		Button(c, p, "记录分段", rx+24, ly+292, right-48, 44, "记录分段", "add", true, t != nil && t.Running && a.Connected, func() {
			if t != nil {
				a.timerAction(t.ID, "lap")
			}
		})
		return
	}
	Label(c, "设定时长", rx+24, ly+22, 15, s.OnSurface, true, right-120)
	widgetTextAction(c, p, "调整参数", "自定义", rx+right-94, ly+14, 72, a.openTimerPrefs)
	dialBox := ui.Box(c).Absolute().Left(rx + 24).Top(ly + 58).Width(right - 48).Height(104).Radius(18).Background(AlphaCol(s.OnSurface, .035)).IgnoreInput(true)
	if !p.HighContrast {
		dialBox.Border(1, AlphaCol(s.OnSurface, .06))
	}
	display := fmt.Sprintf("%d", focusUI.minutes)
	unit := "分钟"
	if focusUI.seconds > 0 && focusUI.seconds%60 != 0 {
		display = formatTimerTime(int64(focusUI.seconds))
		unit = "分 : 秒"
	}
	CenteredText(c, display, rx+72, ly+68, right-144, 62, 44, s.OnSurface, true)
	CenteredText(c, unit, rx+72, ly+128, right-144, 22, 11.5, s.OnSurfaceVariant, false)
	IconButton(c, p, "减少一分钟", rx+34, ly+90, 40, 40, "arrow_back", false, focusUI.minutes > 1, func() { focusUI.minutes = max(1, focusUI.minutes-1); focusUI.seconds = 0; focusUI.selected = "" })
	IconButton(c, p, "增加一分钟", rx+right-74, ly+90, 40, 40, "add", false, focusUI.minutes < 240, func() { focusUI.minutes = min(240, focusUI.minutes+1); focusUI.seconds = 0; focusUI.selected = "" })
	cw := (right - 56) / 2
	for i, minutes := range []int{5, 15, 25, 45} {
		Chip(c, p, fmt.Sprintf("%d 分钟", minutes), rx+24+float32(i%2)*(cw+8), ly+180+float32(i/2)*48, cw, 38, fmt.Sprintf("%d 分钟", minutes), focusUI.minutes == minutes && focusUI.seconds == 0, func() { focusUI.minutes = minutes; focusUI.seconds = 0; focusUI.selected = "" })
	}
	detail := "计时结束后将在灵动岛提醒你"
	if focusUI.mode == 0 {
		detail = fmt.Sprintf("%d 轮专注  ·  每轮休息 %d 分钟", focusUI.rounds, focusUI.rest/60)
	}
	summaryPill := ui.Box(c).Absolute().Left(rx + 24).Top(ly + 292).Width(right - 48).Height(40).Radius(12).Background(AlphaCol(s.OnSurface, .04)).IgnoreInput(true)
	summaryPill.Children(func() {
		CenteredText(c, detail, 12, 9, right-72, 22, 12, s.OnSurfaceVariant, false)
	})
}
