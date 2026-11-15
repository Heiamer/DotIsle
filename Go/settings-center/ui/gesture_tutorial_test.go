package ui

import (
	"errors"
	"testing"

	"github.com/egoist/mygo/ui"
)

func TestGestureTutorialEntryAndPractice(t *testing.T) {
	a := newTestApp(t)
	a.State["pendingPage"] = ""
	a.Page, a.SettingsTab, a.IslandTab = PageSettings, 130, 2
	tt := ui.NewTester(a.View, 1200, 800)
	tt.Frame()
	if err := tt.Click("再看一次"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if a.gestureTutorial == nil || !tt.HasText("轻点展开，再点收起") {
		t.Fatal("tutorial did not open")
	}
	if err := tt.Click("教程演示灵动岛"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if a.gestureTutorial.demo != 1 || !tt.HasText("海边来信 · 点屿") {
		t.Fatal("expand practice did not respond")
	}
	for a.gestureTutorial.index < len(a.gestureTutorial.steps)-1 {
		a.gestureTutorial.navigate(1)
	}
	tt.Frame()
	if err := tt.Click("模拟托盘图标"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if err := tt.Click("模拟显示灵动岛"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if a.gestureTutorial.demo != 2 {
		t.Fatal("tray recovery practice failed")
	}
	guide := a.gestureTutorial
	a.applyGestureTutorialReply(guide, nil, errors.New("fixture failure"))
	if a.gestureTutorial == nil || guide.error == "" {
		t.Fatal("failed save silently closed guide")
	}
	a.applyGestureTutorialReply(guide, map[string]any{"success": true}, nil)
	if a.gestureTutorial != nil {
		t.Fatal("successful save did not close guide")
	}
}

func TestGestureTutorialUsesEnabledGestures(t *testing.T) {
	a := newTestApp(t)
	s := a.settings()
	s["ClickAnywhereExpand"], s["HoverExpand"], s["SwipeActivities"], s["SwipeUpHide"], s["Floating"] = false, false, false, false, true
	a.startGestureTutorial()
	if len(a.gestureTutorial.steps) != 2 || a.gestureTutorial.steps[0].kind != "move" {
		t.Fatal("disabled gestures still shown", a.gestureTutorial.steps)
	}
	s["IslandSizeMode"] = "Mini"
	a.startGestureTutorial()
	if len(a.gestureTutorial.steps) != 2 || a.gestureTutorial.steps[0].kind != "expand" {
		t.Fatal("mini guide offered floating drag or omitted expand", a.gestureTutorial.steps)
	}
}
