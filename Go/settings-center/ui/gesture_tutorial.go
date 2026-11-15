package ui

import (
	"fmt"
	"math"

	"dotisle/settings/island"
	"github.com/egoist/mygo/ui"
)

type gestureStep struct{ kind, title, detail string }
type gestureTutorial struct {
	steps       []gestureStep
	index, demo int
	dx, dy      float32
	pending     bool
	error       string
}

func (a *App) startGestureTutorial() {
	t := &gestureTutorial{}
	mini := a.islandString("IslandSizeMode", "Classic") == "Mini"
	if mini || a.islandBool("ClickAnywhereExpand", true) {
		t.steps = append(t.steps, gestureStep{"expand", "轻点展开，再点收起", "点击胶囊空白处展开，点击卡片外收起。按钮与进度条保留各自的操作。"})
	} else if a.islandBool("HoverExpand", false) {
		t.steps = append(t.steps, gestureStep{"hover", "悬停展开", "将鼠标移到胶囊上展开，移开后收起。按钮与进度条保留各自的操作。"})
	}
	if a.islandBool("SwipeActivities", true) {
		t.steps = append(t.steps, gestureStep{"swipe", "左右拖动，切换活动", "按住空白处左右拖动，在音乐、计时器与待办之间切换。有多个活动时可用。"})
	}
	if !mini && a.islandBool("Floating", false) {
		t.steps = append(t.steps, gestureStep{"move", "拖动把手，调整位置", "使用展开卡片左下角的移动按钮，再拖到你喜欢的位置。位置会自动保存。"})
	}
	if a.islandBool("SwipeUpHide", true) {
		t.steps = append(t.steps, gestureStep{"hide", "向上拖动，暂时隐藏", "从胶囊或卡片空白处向上拖动，松手隐藏。短距离会回弹，音乐与计时继续运行。"})
	}
	t.steps = append(t.steps, gestureStep{"recover", "随时找回点屿", "右键任务栏托盘中的点屿图标，选择「显示 / 隐藏灵动岛」。点击下方模拟托盘试一试。"})
	a.gestureTutorial = t
	a.navigationOpen, a.paletteOpen = false, false
}

func (t *gestureTutorial) navigate(delta int) {
	if !t.pending {
		t.index = max(0, min(len(t.steps)-1, t.index+delta))
		t.demo, t.dx, t.dy = 0, 0, 0
	}
}
func (a *App) finishGestureTutorial() {
	t := a.gestureTutorial
	if t == nil || t.pending {
		return
	}
	if a.Win == nil || !a.Connected {
		t.error = "尚未连接到灵动岛，请稍后重试。"
		return
	}
	t.pending, t.error = true, ""
	go func() {
		reply, err := island.Request("update", map[string]any{"key": "GestureTutorialSeen", "value": true})
		a.Win.Update(func() { a.applyGestureTutorialReply(t, reply, err) })
	}()
}
func (a *App) applyGestureTutorialReply(t *gestureTutorial, reply map[string]any, err error) {
	if a.gestureTutorial != t {
		return
	}
	t.pending = false
	if err != nil {
		t.error = "保存失败：" + err.Error()
	} else if reply["success"] != true {
		t.error = stateString(reply, "教程状态保存失败，请重试。", "message")
	} else {
		a.gestureTutorial = nil
	}
}

func (a *App) gestureTutorialView(c *ui.Context, w, h float32) {
	t := a.gestureTutorial
	if t == nil {
		return
	}
	p := &a.Palette
	s := &p.Scheme
	const pw, ph = float32(640), float32(528)
	ui.Overlay(c, func() {
		back := ui.Box(c).Key("gesture-tutorial-overlay").Absolute().Fill().Modal().Role(ui.RoleDialog).Label("手势教程").Background(AlphaCol(0, .32))
		if back.OverlayShortcut(0, ui.KeyEscape) {
			a.finishGestureTutorial()
		}
		back.Children(func() {
			panel := ui.Box(c).Absolute().Left((w - pw) / 2).Top((h - ph) / 2).Width(pw).Height(ph)
			overlayStyle(&panel, p)
			panel.Children(func() {
				step := t.steps[t.index]
				Label(c, fmt.Sprintf("认识你的灵动岛 · %02d / %02d", t.index+1, len(t.steps)), 28, 24, 13, s.Primary, true, pw-190)
				Button(c, p, "跳过教程", pw-112, 16, 84, 36, "跳过", "", true, !t.pending, a.finishGestureTutorial)
				Label(c, step.title, 28, 76, 24, s.OnSurface, true, pw-56)
				stage := ui.Box(c).Key("gesture-stage").Absolute().Left(24).Top(124).Width(pw - 48).Height(208).Radius(24).Background(Col(s.High)).Clip()
				stage.Children(func() {
					Label(c, "交互演示 · 仅用于练习", 18, 16, 11, s.OnSurfaceVariant, false, pw-84)
					if step.kind == "recover" {
						Button(c, p, "模拟托盘图标", pw-132, 162, 60, 32, "", "dialpad", true, !t.pending, func() { t.demo = 1 })
						if t.demo == 1 {
							Button(c, p, "模拟显示灵动岛", pw-300, 112, 220, 38, "显示 / 隐藏灵动岛", "dialpad", true, !t.pending, func() { t.demo = 2 })
						}
						if t.demo != 2 {
							CenteredText(c, "点击右下角的模拟托盘图标", 24, 60, pw-96, 38, 14, s.OnSurfaceVariant, false).IgnoreInput(true)
							return
						}
					}
					if step.kind == "hide" && t.demo == 1 {
						CenteredText(c, "已暂时隐藏 · 音乐与计时继续运行", 24, 72, pw-96, 48, 14, s.OnSurfaceVariant, false)
						Button(c, p, "恢复隐藏演示", (pw-192)/2, 142, 144, 36, "再试一次", "refresh", true, true, func() { t.demo, t.dy = 0, 0 })
						return
					}
					open := t.demo == 1 && (step.kind == "expand" || step.kind == "hover")
					ch := float32(56)
					if open || step.kind == "move" {
						ch = 132
					}
					cx, cy := (pw-48-300)/2, float32(64)
					if step.kind == "move" {
						cx += t.dx
						cy += t.dy
					}
					card := ui.ButtonBase(c).Key("gesture-demo-island").Absolute().Left(cx).Top(cy).Width(300).Height(ch).Radius(24).Background(Col(s.PrimaryContainer)).Label("教程演示灵动岛").Disabled(t.pending)
					if card.Clicked() && step.kind == "expand" {
						t.demo = 1 - t.demo
					}
					if step.kind == "hover" {
						t.demo = boolInt(card.Hovered())
					}
					if dx, dy, active := card.Dragged(); active {
						switch step.kind {
						case "swipe":
							t.dx += dx
							if math.Abs(float64(t.dx)) >= 60 {
								t.demo = 1
							}
						case "hide":
							t.dy += dy
							if t.dy <= -48 {
								t.demo = 1
							}
						case "move":
							t.dx = max(-90, min(90, t.dx+dx))
							t.dy = max(-18, min(6, t.dy+dy))
						}
					}
					card.Children(func() {
						icon, text := "music_note", "让好音乐，陪你度过好时光"
						if step.kind == "swipe" && t.demo == 1 {
							icon, text = "timer", "专注时刻 · 25:00"
						}
						Icon(c, icon, 18, 17, 22, s.OnPrimaryContainer).IgnoreInput(true)
						Label(c, text, 52, 18, 13, s.OnPrimaryContainer, true, 228).IgnoreInput(true)
						if ch > 60 {
							Label(c, "海边来信 · 点屿", 22, 55, 11, s.OnPrimaryContainer, false, 256).IgnoreInput(true)
							Icon(c, "play_arrow", 139, 90, 24, s.OnPrimaryContainer).IgnoreInput(true)
							if step.kind == "move" {
								Icon(c, "open_with", 18, 94, 18, s.OnPrimaryContainer).IgnoreInput(true)
							}
						}
					})
				})
				overviewText(c, step.detail, 28, 352, pw-56, overviewCaption, s.OnSurfaceVariant, 3)
				if t.error != "" {
					Label(c, t.error, 28, 421, 12, s.Error, false, pw-56)
				}
				Button(c, p, "教程上一步", 28, ph-66, 108, 40, "上一步", "", true, t.index > 0 && !t.pending, func() { t.navigate(-1) })
				Button(c, p, "重置教程演示", 148, ph-66, 108, 40, "重试演示", "refresh", true, !t.pending, func() { t.demo, t.dx, t.dy = 0, 0, 0 })
				label := "下一步"
				if t.index == len(t.steps)-1 {
					label = "开始使用"
				}
				if t.pending {
					label = "正在保存…"
				}
				Button(c, p, "教程下一步", pw-168, ph-66, 140, 40, label, "", false, !t.pending, func() {
					if t.index == len(t.steps)-1 {
						a.finishGestureTutorial()
					} else {
						t.navigate(1)
					}
				})
			})
		})
	})
}
