package ui

import (
	"testing"

	"github.com/egoist/mygo/ui"
)

func TestGestureTutorialEntryAndPractice(t *testing.T) {
	a := newTestApp(t)
	a.State["pendingPage"] = ""
	a.Page, a.SettingsTab, a.IslandTab = PageSettings, 130, 2
	tt := ui.NewTester(a.View, 1200, 800)
	tt.Frame()
	if err := tt.Click("练习手势"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if a.gestureTutorial == nil || !tt.HasText("在你的灵动岛上试一试") {
		t.Fatal("tutorial welcome did not open")
	}
	guide := a.gestureTutorial
	packet := func(id, step string, index int) map[string]any {
		return map[string]any{"gesturePractice": map[string]any{"id": id, "step": step, "index": float64(index), "total": float64(5)}}
	}
	a.syncGesturePractice(packet("other", "recover", 4))
	if guide.step != "" {
		t.Fatal("another session changed the instructions")
	}
	a.syncGesturePractice(packet(guide.id, "clickExpand", 0))
	tt.Frame()
	if !tt.HasText("点击胶囊，展开卡片") || tt.HasText("教程演示灵动岛") {
		t.Fatal("tutorial is not tied to native outcomes")
	}
	if err := tt.Click("完成教程"); err == nil {
		t.Fatal("unfinished tutorial offered completion")
	}
	a.settings()["HiddenDisplay"] = "None"
	a.syncGesturePractice(packet(guide.id, "recover", 4))
	a.syncGesturePractice(packet(guide.id, "hide", 3))
	tt.Frame()
	if !tt.HasText("从任务栏托盘找回点屿") {
		t.Fatal("tray instruction was not retained")
	}
	a.syncGesturePractice(packet(guide.id, "done", 5))
	tt.Frame()
	if !tt.HasText("实操完成，可以开始使用了") {
		t.Fatal("native completion was not reflected")
	}
	a.syncGesturePractice(map[string]any{})
	if guide.step != "" || guide.error == "" {
		t.Fatal("expired session did not recover")
	}
}

func TestFirstLaunchRoutesToTutorialWelcome(t *testing.T) {
	a := newTestApp(t)
	a.State["pendingPage"] = "gestureTutorial"
	a.applyPendingPage()
	if a.Page != PageHome || a.gestureTutorial == nil || a.gestureTutorial.step != "" {
		t.Fatal("first launch must ask before starting practice")
	}
	a.EndCenterVisit()
	if a.gestureTutorial != nil {
		t.Fatal("closing the center left the lesson open")
	}
}
