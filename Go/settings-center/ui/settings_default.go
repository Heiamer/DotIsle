package ui

import (
	"fmt"
	"github.com/egoist/mygo/ui"
)

// Theme previews use the same neutral tokens as the full control center.
func (a *App) appearanceBody(c *ui.Context, p *Palette, x, y, w float32) float32 {
	s := &p.Scheme
	Label(c, "外观模式", x+4, y, 14, s.OnSurface, true, w-8)
	y += 34
	current := a.leafString("SettingsTheme", "System")
	tw := (w - 24) / 3
	for i, item := range [][2]string{{"跟随系统", "System"}, {"浅色", "Light"}, {"深色", "Dark"}} {
		xx := x + float32(i)*(tw+12)
		button := ui.ButtonBase(c).Absolute().Left(xx).Top(y).Width(tw).Height(160).
			Radius(20).Label("外观" + item[0]).Cursor(ui.CursorPointer)
		surfaceStyle(&button, p, s.Container, 20)
		if current == item[1] {
			button.Border(2, Col(s.Primary))
		}
		if button.Clicked() {
			if item[1] != "System" {
				a.Prefs.Dark = item[1] == "Dark"
			}
			a.savePrefs()
			a.pushNative("SettingsTheme", item[1])
		}
		button.Children(func() {
			preview := ui.Box(c).Absolute().Left(12).Top(12).Width(tw - 24).Height(102).Radius(12).Clip()
			preview.Children(func() {
				parts := 1
				if i == 0 {
					parts = 2
				}
				for part := 0; part < parts; part++ {
					dark := i == 2 || i == 0 && part == 1
					palette := fallbackScheme(dark)
					pw := (tw - 24) / float32(parts)
					ui.Box(c).Absolute().Left(float32(part) * pw).Top(0).Width(pw).Height(102).Background(Col(palette.Surface)).Clip().Children(func() {
						ui.Box(c).Absolute().Left(0).Top(0).Width(pw * .25).Height(102).Background(Col(palette.SecondaryContainer))
						for row := 0; row < 3; row++ {
							ui.Box(c).Absolute().Left(7).Top(20 + float32(row)*15).Width(pw*.25 - 14).Height(4).Radius(2).Background(Col(palette.OnSurfaceVariant))
						}
						ui.Box(c).Absolute().Left(pw*.25 + 9).Top(14).Width(pw * .52).Height(5).Radius(2).Background(Col(palette.OnSurface))
						for row := 0; row < 2; row++ {
							ui.Box(c).Absolute().Left(pw*.25 + 8).Top(31 + float32(row)*31).Width(pw*.75 - 16).Height(25).Radius(6).Background(Col(palette.Container))
						}
					})
				}
			})
			Label(c, item[0], 16, 128, 13, s.OnSurface, current == item[1], tw-50)
			if current == item[1] {
				Icon(c, "check", tw-34, 127, 18, s.Primary)
			}
		})
	}
	y += 180
	y = a.pastelSettings(c, p, x, y, w)
	Panel(c, p, x, y, w, 164, s.Container)
	contrast := a.leafBool("DynamicColorHighContrast", false)
	appearanceToggleRow(c, p, "增强颜色对比", x, y, w, 82, "增强颜色对比", "提高文字和边界的辨识度", contrast, func() { a.pushNative("DynamicColorHighContrast", !contrast) })
	animationOn := a.leafBool("Animation", !a.Prefs.ReducedMotion)
	appearanceToggleRow(c, p, "减少动态效果", x, y+82, w, 82, "减少动态效果", "减少弹性与过渡，保留状态反馈", !animationOn, func() {
		a.Prefs.ReducedMotion = animationOn
		a.savePrefs()
		a.pushNative("Animation", !animationOn)
	})
	y += 184
	return a.backgroundSettingsBody(c, p, x, y, w)
}

// Appearance rows share one trailing guide and center the two-line text block.
func appearanceToggleRow(c *ui.Context, p *Palette, label string, x, y, w, h float32, title, detail string, on bool, onClick func()) {
	textY := y + (h-LineHeight(14)-4-LineHeight(12))/2
	Label(c, title, x+24, textY, 14, p.Scheme.OnSurface, false, w-124)
	Label(c, detail, x+24, textY+LineHeight(14)+4, 12, p.Scheme.OnSurfaceVariant, false, w-124)
	Switch(c, p, label, x+w-64, y+(h-20)/2-3, on, onClick)
}

func updateChannelLocked(phase string) bool {
	switch phase {
	case "checking", "downloading", "switching", "verifying", "ready", "applying", "restarting":
		return true
	}
	return false
}

// Channel state is acknowledged by the native updater, never guessed locally.
func (a *App) aboutBody(c *ui.Context, p *Palette, x, y, w float32) float32 {
	s := &p.Scheme
	version := stateString(a.State, "5.0.3", "bundleVersion")
	if version == "5.0.3" {
		version = stateString(a.State, "5.0.3", "version")
	}
	u, _ := a.State["softwareUpdate"].(map[string]any)
	phase := leafStr(u["phase"], "idle")
	channel := stateString(a.State, "stable", "updateChannel")
	locked := updateChannelLocked(phase)
	Panel(c, p, x, y, w, 236, s.Container).Label("关于点屿品牌卡片")
	wordW := min(float32(220), w-80)
	drawWordmark(c, x+(w-wordW)/2, y+26, wordW, 112, s.OnSurface)
	CenteredText(c, "版本 "+version, x+24, y+151, w-48, 24, 14, s.OnSurfaceVariant, false)
	CenteredText(c, "界面字体 · HarmonyOS Sans", x+24, y+192, w-48, 20, 12, s.OnSurfaceVariant, false)
	y += 252
	Panel(c, p, x, y, w, 140, s.Container)
	Label(c, "更新渠道", x+24, y+17, 14, s.OnSurface, true, w-48)
	options := []struct{ title, value string }{{"正式版", "stable"}, {"测试版", "beta"}}
	for i, option := range options {
		cw := (w - 56) / 2
		bg, fg := s.High, s.OnSurfaceVariant
		if channel == option.value {
			bg, fg = s.SecondaryContainer, s.OnSurface
		}
		button := ui.ButtonBase(c).Absolute().Left(x + 24 + float32(i)*(cw+8)).Top(y + 48).Width(cw).Height(40).
			Radius(12).Background(Col(bg)).Label("更新渠道" + option.title).Disabled(locked).Cursor(ui.CursorPointer)
		if channel == option.value {
			button.Border(1, Col(s.Primary))
		}
		hoverLayer(&button, Col(bg), s.OnSurface, !locked)
		if !locked && channel != option.value && button.Clicked() {
			a.command("updateChannel", option.value)
		}
		button.Children(func() { CenteredText(c, option.title, 0, 0, cw, 40, 13, fg, channel == option.value) })
	}
	detail := "默认渠道 · 获取稳定的正式版本"
	if channel == "beta" {
		detail = "提前获取测试版本，也接收后续正式版"
	}
	if locked {
		detail = "当前更新任务完成或取消后可切换渠道"
	}
	Label(c, detail, x+24, y+106, 12, s.OnSurfaceVariant, false, w-48)
	y += 156
	Panel(c, p, x, y, w, 204, s.Container)
	title, ink := leafUpdateStatus(phase), s.OnSurface
	if v := leafStr(u["version"], ""); v != "" && phase != "idle" {
		title += "  " + v
	}
	if phase == "failed" {
		ink = s.Error
	}
	Label(c, title, x+24, y+20, 18, ink, true, w-48)
	message := leafStr(u["message"], "检查此渠道可用的新版本")
	if message == "" {
		message = "检查此渠道可用的新版本"
	}
	progressY := y + 103
	if phase == "downloading" {
		received, _ := stateFloat(u, "receivedBytes")
		total, _ := stateFloat(u, "totalBytes")
		message = fmt.Sprintf("已下载 %.1f MB", received/1048576)
		if total > 0 {
			message += fmt.Sprintf(" / %.1f MB", total/1048576)
			progressLine(c, p, x+24, progressY, w-48, received/total)
		} else {
			Label(c, "总大小未知", x+24, progressY-4, 11, s.OnSurfaceVariant, false, w-48)
		}
	} else if phase == "ready" || phase == "completed" {
		progressLine(c, p, x+24, progressY, w-48, 1)
	}
	Label(c, message, x+24, y+58, 12, s.OnSurfaceVariant, false, w-48)
	if label, command := leafUpdateAction(phase); command != "" {
		Button(c, p, "更新"+command, x+24, y+140, 164, 40, label, "", false, a.Connected, func() { a.command(command, nil) })
		if phase == "ready" {
			Button(c, p, "取消就绪的更新", x+200, y+140, 100, 40, "取消", "", true, true, func() { a.command("updateCancel", nil) })
		}
	} else {
		Label(c, "正在处理，请稍候…", x+24, y+152, 12, s.OnSurfaceVariant, false, w-48)
	}
	y += 220
	Panel(c, p, x, y, w, 88, s.Container)
	on := a.leafBool("AutoCheckUpdatesEnabled", true)
	ToggleRow(c, p, "自动检查更新", x+24, y+8, w-48, "自动检查更新", "后台检查，下载和安装由你确认", on, func() { a.pushNative("AutoCheckUpdatesEnabled", !on) })
	return y + 104
}
