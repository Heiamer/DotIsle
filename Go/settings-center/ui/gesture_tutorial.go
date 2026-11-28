package ui

import (
	"fmt"
	"strconv"
	"time"

	"dotisle/settings/island"
	"github.com/egoist/mygo/ui"
)

type gestureTutorial struct {
	id, step, error string
	index, total    int
	pending         bool
	observed        bool
	mandatory       bool
}

type practiceCopy struct{ title, detail string }

var practicePrompts = map[string]practiceCopy{
	"":              {"在你的灵动岛上试一试", "每一步都在实际灵动岛上完成。练习卡片会替代当前内容，不会操作音乐、计时器或待办。"},
	"clickExpand":   {"点击胶囊，展开卡片", "到屏幕上的「练习卡片 A」点击一下。展开成功后会自动进入下一步。"},
	"hoverExpand":   {"移到胶囊上，展开卡片", "把指针移到「练习卡片 A」上，不用点击。"},
	"move":          {"按住「拖动移动」，挪动灵动岛", "拖一小段距离再松开。练习结束后会恢复你原来的位置。"},
	"clickCollapse": {"点击卡片空白处，收起", "在展开的练习卡片上点击空白处，让它变回胶囊。"},
	"hoverCollapse": {"移开指针，收起卡片", "把指针移到卡片外，等它自动收回胶囊。"},
	"swipe":         {"左右拖动，切换活动", "按住练习胶囊，向左或向右拖一段再松手。看到另一张练习卡片就成功了。"},
	"hide":          {"右键点击，暂时隐藏", "在练习胶囊或展开卡片上点右键。下一步会按当前显示方式带你找回来。"},
	"recover":       {"从任务栏托盘找回点屿", "在任务栏右下角找到点屿图标（也可能在 ︿ 里），右键选择「显示 / 隐藏灵动岛」。"},
	"done":          {"实操完成，可以开始使用了", "你已完成当前启用手势的练习。结束后，灵动岛会恢复练习前的状态。"},
}

func (a *App) startGestureTutorial() {
	if a.gestureTutorial != nil {
		return
	}
	a.gestureTutorial = &gestureTutorial{id: strconv.FormatInt(time.Now().UnixNano(), 36)}
	a.navigationOpen, a.paletteOpen = false, false
}

func (a *App) forceGestureTutorial() {
	if a.leafBool("GesturePracticeSeen", false) {
		return
	}
	a.tutorialRequired = true
	a.startGestureTutorial()
	a.gestureTutorial.mandatory = true
}

func (t *gestureTutorial) applyPractice(raw any) {
	p, _ := raw.(map[string]any)
	if p == nil || p["id"] != t.id {
		return
	}
	step := stateString(p, "", "step")
	rawIndex, okIndex := stateFloat(p, "index")
	rawTotal, okTotal := stateFloat(p, "total")
	if !okIndex || !okTotal {
		return
	}
	index, total := int(rawIndex), int(rawTotal)
	if _, ok := practicePrompts[step]; !ok || step == "" || total < 1 || total > 7 || index < t.index || index > total || (step == "done") != (index == total) {
		return
	}
	t.step, t.index, t.total = step, index, total
}

func (a *App) syncGesturePractice(state map[string]any) {
	t := a.gestureTutorial
	if t == nil {
		return
	}
	if p, ok := state["gesturePractice"].(map[string]any); ok && p["id"] == t.id {
		t.applyPractice(p)
		t.observed = true
	} else if t.observed && !t.pending {
		// A native timeout or desktop lock ended the temporary lesson.
		t.step, t.index, t.total, t.observed = "", 0, 0, false
		t.error = "练习已结束，灵动岛已恢复。可以重新开始。"
	}
}

func (a *App) cancelGestureTutorial() {
	t := a.gestureTutorial
	if t == nil {
		return
	}
	a.gestureTutorial = nil
	if t.step != "" {
		a.command("gesturePractice", map[string]any{"action": "cancel", "id": t.id})
	}
}

func (a *App) gesturePracticeAction(action string) {
	t := a.gestureTutorial
	if t == nil || t.pending || (action == "finish" && t.step != "done") {
		return
	}
	if a.Win == nil || !a.Connected {
		t.error = "尚未连接到灵动岛，请稍后重试。"
		return
	}
	t.pending, t.error = true, ""
	a.startAsync(func() {
		reply, err := island.Request("gesturePractice", map[string]any{"action": action, "id": t.id})
		a.applyAsync(func() {
			if a.gestureTutorial != t {
				// The window can close while the native start is still on the pipe.
				if action == "start" && reply["success"] == true {
					a.startAsync(func() { _, _ = island.Request("gesturePractice", map[string]any{"action": "cancel", "id": t.id}) })
				}
				return
			}
			t.pending = false
			if err != nil {
				t.error = "未能连接灵动岛，请重试。"
			} else if reply["success"] != true {
				t.error = stateString(reply, "操作未成功，请重试。", "message")
			} else if action == "start" {
				t.applyPractice(reply["practice"])
			} else {
				a.tutorialRequired = false
				a.gestureTutorial = nil
			}
		})
	})
}

func (a *App) gestureTutorialView(c *ui.Context, w, h float32) {
	t := a.gestureTutorial
	if t == nil {
		return
	}
	p, s := &a.Palette, &a.Palette.Scheme
	pw, ph := min(float32(560), w-32), float32(292)
	copy := practicePrompts[t.step]
	switch t.step {
	case "recover":
		switch a.leafString("HiddenDisplay", "Strip") {
		case "None":
			copy = practiceCopy{"从任务栏托盘找回点屿", "在任务栏右下角找到点屿图标（也可能在 ︿ 里），右键选择「显示 / 隐藏灵动岛」。"}
		case "Mini":
			copy = practiceCopy{"点击顶部迷你岛恢复", "在屏幕顶部找到迷你灵动岛，点击它恢复完整灵动岛。"}
		default:
			copy = practiceCopy{"点击顶部蓝色边条恢复", "在屏幕顶部找到蓝色边条，点击它恢复完整灵动岛。"}
		}
	}
	ui.Overlay(c, func() {
		back := ui.Box(c).Key("gesture-tutorial-overlay").Absolute().Fill().Modal().Role(ui.RoleDialog).Label("灵动岛实操教程").Background(AlphaCol(0, .32))
		if back.OverlayShortcut(0, ui.KeyEscape) && !t.mandatory {
			a.cancelGestureTutorial()
		}
		back.Children(func() {
			panel := ui.Box(c).Absolute().Left((w - pw) / 2).Top(max(float32(12), h-ph-40)).Width(pw).Height(ph)
			overlayStyle(&panel, p)
			panel.Children(func() {
				heading := "灵动岛实操教程"
				if t.mandatory && t.step == "" {
					heading = "新版实操教程 · 只提示一次"
				}
				if t.step != "" && t.step != "done" {
					heading = fmt.Sprintf("灵动岛实操教程 · %d / %d", t.index+1, t.total)
				}
				Label(c, heading, 24, 24, 13, s.Primary, true, pw-48)
				Label(c, copy.title, 24, 62, 23, s.OnSurface, true, pw-48)
				ui.Box(c).Absolute().Left(24).Top(108).Width(pw - 48).Children(func() {
					ui.Text(c, copy.detail).Font(textFamily(copy.detail)).FontSize(14).FixedLineHeight(LineHeight(14)).TextColor(Col(s.OnSurfaceVariant))
				})
				if t.total > 0 {
					barWidth := (pw - 48 - float32(t.total-1)*6) / float32(t.total)
					for i := 0; i < t.total; i++ {
						ink := s.High
						if i <= t.index {
							ink = s.Primary
						}
						ui.Box(c).Absolute().Left(24 + float32(i)*(barWidth+6)).Top(167).Width(barWidth).Height(4).Radius(2).Background(Col(ink))
					}
				}
				if t.error != "" {
					Label(c, t.error, 24, 187, 12, s.Error, false, pw-48)
				} else if t.step != "" && t.step != "done" {
					Label(c, "等待你在灵动岛上完成操作…", 24, 187, 12, s.OnSurfaceVariant, false, pw-48)
				}
				Button(c, p, "跳过教程", 24, ph-62, 112, 40, "跳过教程", "", true, !t.pending, func() { a.gesturePracticeAction("skip") })
				if t.step == "" || t.step == "done" {
					label, action := "开始练习", "start"
					if t.step == "done" {
						label, action = "完成教程", "finish"
					}
					if t.pending {
						label = "请稍候…"
					}
					Button(c, p, label, pw-164, ph-62, 140, 40, label, "", false, !t.pending && a.Connected, func() { a.gesturePracticeAction(action) })
				}
			})
		})
	})
}
