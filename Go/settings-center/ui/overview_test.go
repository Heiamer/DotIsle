package ui

import (
	"path/filepath"
	"testing"
	"time"

	"dotisle/settings/prefs"
	"github.com/egoist/mygo/ui"
)

func TestOverviewSwitchAndCardSelectionPersist(t *testing.T) {
	a := newTestApp(t)
	tt := ui.NewTester(a.View, 1200, 800)
	tt.Frame()
	r, ok := tt.Find("滚动视图")
	if !ok {
		t.Fatal("missing view selector")
	}
	tt.ClickAt(r.X+r.W/2, r.Y+r.H/2)
	tt.Frame()
	saved, err := prefs.LoadOverview(filepath.Join(a.Data, "center-overview.json"))
	if err != nil || saved.View != "scroll" {
		t.Fatalf("view not saved: %+v %v", saved, err)
	}
	before := deckPos
	tt.Scroll(700, 430, 0, 240)
	tt.Advance(32 * time.Millisecond)
	tt.Frame()
	if deckPos == before {
		t.Fatal("scroll view swallowed wheel input")
	}
	r, _ = tt.Find("卡片视图")
	tt.ClickAt(r.X+r.W/2, r.Y+r.H/2)
	tt.Frame()
	r, _ = tt.Find("自定义概览组件")
	tt.ClickAt(r.X+r.W/2, r.Y+r.H/2)
	tt.Frame()
	r, ok = tt.Find("显示音乐")
	if !ok {
		t.Fatal("missing card choice")
	}
	tt.ClickAt(r.X+r.W/2, r.Y+r.H/2)
	tt.Frame()
	saved, err = prefs.LoadOverview(filepath.Join(a.Data, "center-overview.json"))
	if err != nil {
		t.Fatal(err)
	}
	for _, key := range saved.Cards {
		if key == "music" {
			t.Fatal("disabled card not saved")
		}
	}
}

func TestOverviewLongPressReorderPersists(t *testing.T) {
	for _, name := range []string{"音乐", "时钟与日历"} {
		a := newTestApp(t)
		a.loadOverview()
		a.overview.config.Cards = []string{"music", "clock", "todos", "focus", "quick"}
		tt := ui.NewTester(a.View, 1200, 800)
		tt.Frame()
		r, ok := tt.Find("长按排列" + name)
		if !ok {
			t.Fatal("missing card handle")
		}
		tt.Press(r.X+40, r.Y+24)
		tt.Advance(500 * time.Millisecond)
		tt.Frame()
		if !a.overview.edit || a.overview.dragging == "" {
			t.Fatalf("%s did not enter rearranging", name)
		}
		target := "音乐"
		if name == "音乐" {
			target = "时钟与日历"
		}
		dest, ok := tt.Find("长按排列" + target)
		if !ok {
			t.Fatal("missing destination")
		}
		tt.Move(dest.X+dest.W/2, dest.Y+80)
		tt.Frame()
		tt.Release(dest.X+dest.W/2, dest.Y+80)
		tt.Frame()
		saved, err := prefs.LoadOverview(filepath.Join(a.Data, "center-overview.json"))
		if err != nil {
			t.Fatal(err)
		}
		if saved.Cards[0] != "clock" || saved.Cards[1] != "music" {
			t.Fatalf("%s drop not restored: %v", name, saved.Cards)
		}
	}
}

func TestOverviewShortPressKeepsLayout(t *testing.T) {
	a := newTestApp(t)
	tt := ui.NewTester(a.View, 1200, 800)
	tt.Frame()
	r, _ := tt.Find("长按排列音乐")
	tt.ClickAt(r.X+40, r.Y+20)
	tt.Frame()
	if a.overview.edit {
		t.Fatal("a short press entered edit mode")
	}
	r, _ = tt.Find("添加待办")
	tt.ClickAt(r.X+r.W/2, r.Y+r.H/2)
	tt.Frame()
	if !a.todoDialog {
		t.Fatal("card handle swallowed the add action")
	}
}

func TestOverviewCrossRowReorderAndEscape(t *testing.T) {
	for _, width := range []int{900, 1200} {
		a := newTestApp(t)
		a.loadOverview()
		a.overview.config.Cards = []string{"music", "clock", "todos", "focus", "quick"}
		tt := ui.NewTester(a.View, width, 800)
		tt.Frame()
		r, _ := tt.Find("长按排列音乐")
		tt.Press(r.X+30, r.Y+20)
		tt.Advance(500 * time.Millisecond)
		tt.Frame()
		dest, _ := tt.Find("长按排列待办事项")
		tt.Move(dest.X+dest.W/2, dest.Y+100)
		tt.Release(dest.X+dest.W/2, dest.Y+100)
		tt.Frame()
		if a.overview.config.Cards[2] != "music" {
			t.Fatalf("%d cross-row drop: %v", width, a.overview.config.Cards)
		}
		tt.Key(0, ui.KeyEscape)
		tt.Frame()
		if a.overview.edit {
			t.Fatal("Escape did not leave layout editing")
		}
	}
}

func TestOverviewInformationRequiresRealReadings(t *testing.T) {
	a := newTestApp(t)
	a.overview.loaded = true
	a.overview.config = prefs.Overview{View: "cards", Cards: []string{"widget:weather", "widget:battery", "widget:storage"}}
	tt := ui.NewTester(a.View, 1200, 800)
	tt.Frame()
	for _, text := range []string{"等待天气数据", "未检测到电池", "暂无磁盘读数"} {
		if !tt.HasText(text) {
			t.Fatal("missing unavailable state: " + text)
		}
	}
	a.State["systemMetrics"] = map[string]any{"batteryPercent": float64(42), "externalPower": false}
	a.State["desktopWidgets"] = []any{map[string]any{"id": "weather-real", "kind": "weather"}, map[string]any{"id": "disk-real", "kind": "storage"}}
	a.State["widgetData"] = map[string]any{"weather-real": map[string]any{"temperature": float64(23), "city": "上海"}, "disk-real": map[string]any{"free": float64(64000000000), "total": float64(256000000000), "drive": "D:"}}
	tt.Frame()
	for _, text := range []string{"42%", "23°", "上海", "64 GB"} {
		if !tt.HasText(text) {
			t.Fatal("missing core reading: " + text)
		}
	}
}

// Overview tiles own their content: no desktop placement is needed for the card
// to show a configured city, disk or world clock.
func TestOverviewTilesReadWithoutDesktopWidgets(t *testing.T) {
	a := newTestApp(t)
	a.overview.loaded = true
	a.overview.config = prefs.Overview{View: "cards", Cards: []string{"widget:weather", "widget:storage", "widget:worldClock"}}
	a.State["desktopWidgets"] = []any{}
	a.State["overviewWidgets"] = []any{
		map[string]any{"id": "overview-weather", "kind": "weather", "config": map[string]any{"city": "厦门"}},
		map[string]any{"id": "overview-storage", "kind": "storage", "config": map[string]any{"drive": "D:"}},
		map[string]any{"id": "overview-worldClock", "kind": "worldClock", "config": map[string]any{"city": "东京"}},
	}
	a.State["widgetData"] = map[string]any{
		"overview-weather":    map[string]any{"temperature": float64(26), "city": "厦门 · 晴"},
		"overview-storage":    map[string]any{"free": float64(128e9), "total": float64(512e9), "drive": "D:\\"},
		"overview-worldClock": map[string]any{"clock": "09:30", "date": "11月16日", "city": "东京"},
	}
	tt := ui.NewTester(a.View, 1200, 800)
	tt.Frame()
	for _, text := range []string{"26°", "厦门", "128 GB", "09:30", "东京"} {
		if !tt.HasText(text) {
			t.Fatal("missing overview tile reading: " + text)
		}
	}
}

func TestOverviewCardEllipsisOpensItsOwnSettings(t *testing.T) {
	a := newTestApp(t)
	a.overview.loaded = true
	a.overview.config = prefs.Overview{View: "cards", Cards: []string{"widget:weather"}}
	tt := ui.NewTester(a.View, 1200, 800)
	tt.Frame()
	if err := tt.Click("设置天气"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if a.overviewEditor == nil || a.overviewEditor.kind != "weather" {
		t.Fatal("ellipsis did not open the weather settings")
	}
	if !tt.HasText("设置天气组件") {
		t.Fatal("settings dialog is missing")
	}
}
