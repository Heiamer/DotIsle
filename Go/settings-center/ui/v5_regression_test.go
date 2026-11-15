package ui

import (
	"dotisle/settings/prefs"
	"github.com/egoist/mygo/ui"
	"math"
	"path/filepath"
	"reflect"
	"testing"
	"time"
)

func TestScrollPageEditingPreservesSelectionAndStoresOrder(t *testing.T) {
	a := newTestApp(t)
	a.loadOverview()
	a.openOverviewCatalog("scroll")
	deckPos = 2 * deckStep
	a.moveCatalogCard("todos", -1)
	if a.scrollPages()[1] != "todos" || deckPos != deckStep {
		t.Fatal("reorder lost the selected page")
	}
	a.toggleOverviewCard("todos")
	if deckPos != 0 {
		t.Fatal("removing the current page must focus the first remaining page")
	}
	a.toggleOverviewCard("widget:calendar")
	saved, err := prefs.LoadOverview(filepath.Join(a.Data, "center-overview.json"))
	if err != nil || !reflect.DeepEqual(saved.ScrollPages, a.scrollPages()) {
		t.Fatal("scroll layout not persisted", err)
	}
	a.openOverviewCatalog("information")
	a.toggleOverviewCard("widget:calendar")
	if len(a.overview.config.Information) != 4 || !reflect.DeepEqual(saved.Cards, prefs.DefaultOverview().Cards) {
		t.Fatal("information editing leaked into the grid")
	}
	for _, count := range []int{1, 2, 3, 9} {
		for slot := -20; slot < 20; slot++ {
			index := deckSlotIndex(slot, count)
			if index < 0 || index >= count {
				t.Fatal("ring outside configured pages")
			}
			if math.Abs(deckPhase(float64(slot)*deckStep, count)-float64(index)) > .001 {
				t.Fatal("ring phase mismatch")
			}
		}
	}
}

func TestOneScrollPageAndHiddenNavigation(t *testing.T) {
	a := newTestApp(t)
	a.loadOverview()
	a.overview.config.View = "scroll"
	a.overview.config.ScrollPages = []string{"information"}
	deckPos, deckTarget, deckSpringIdle = 0, 0, true
	tt := ui.NewTester(a.View, 1200, 800)
	tt.Frame()
	tt.Scroll(700, 430, 0, 240)
	tt.Advance(400 * time.Millisecond)
	tt.Frame()
	if deckPos != 0 {
		t.Fatal("a one-page deck must remain still")
	}
	if !tt.HasText("等待天气数据") || !tt.HasText("信息组件") {
		t.Fatal("default information mosaic missing")
	}
	if a.sidebarWidth(1200) != 0 || a.navigationOpen {
		t.Fatal("navigation must start hidden")
	}
	if _, ok := tt.Access("切换到概览"); ok {
		t.Fatal("hidden menu left navigation on screen")
	}
	button, ok := tt.Access("打开导航")
	if !ok || button.Bounds.X != 12 || button.Bounds.Y != 8 {
		t.Fatal("menu button not in upper-left corner")
	}
	a.ToggleNavigation()
	tt.Frame()
	tt.Advance(250 * time.Millisecond)
	tt.Frame()
	if a.sidebarWidth(1200) != 0 {
		t.Fatal("menu consumed page width")
	}
	for _, name := range Pages {
		if _, ok := tt.Access("切换到" + name); !ok {
			t.Fatal("missing menu item", name)
		}
	}
	tt.Key(0, ui.KeyEscape)
	tt.Frame()
	if a.navigationOpen {
		t.Fatal("escape failed to close menu")
	}
	// Old layout files never force the new overlay to open at launch.
	a.layout.Sidebar = "expanded"
	if a.sidebarWidth(900) != 0 {
		t.Fatal("legacy expanded layout reserved space")
	}
	if _, ok := tt.Access("最大化窗口"); ok {
		t.Fatal("fixed window exposes maximize")
	}

}

func TestOverviewMusicRetainsSharedFrameWithoutStandaloneTab(t *testing.T) {
	a := newTestApp(t)
	a.Page = PageHome
	a.State["title"], a.State["artist"] = "同一首歌", "同一位歌手"
	a.State["musicPreview"] = map[string]any{"lyricFrame": map[string]any{
		"currentLine": "当前真实歌词", "previousLine": "上一句歌词", "nextLine": "下一句歌词",
		"currentTranslation": "真实译文", "currentLineIndex": float64(4), "wordScanProgress": float64(.5),
	}}
	tt := ui.NewTester(a.View, 1200, 800)
	tt.Frame()
	for _, text := range []string{"同一首歌", "同一位歌手", "当前真实歌词", "真实译文"} {
		if !tt.HasText(text) {
			t.Fatal("missing shared media state: " + text)
		}
	}
	if len(Pages) != 7 || Pages[PagePlugins] != "插件" {
		t.Fatal("plugin route missing")
	}
	for _, name := range Pages {
		if name == "音乐" || name == "音乐与歌词" {
			t.Fatal("removed standalone music route returned")
		}
	}
	frame := a.lyricFrame()
	frame["currentLine"], frame["currentLineIndex"] = "下一句歌词", float64(5)
	tt.Frame()
	tt.Advance(time.Second)
	tt.Frame()
	if !tt.HasText("下一句歌词") {
		t.Fatal("lyric did not follow island frame")
	}
	for _, phase := range []string{"checking", "downloading", "verifying", "ready", "applying", "restarting"} {
		if !updateChannelLocked(phase) {
			t.Fatal("channel switching allowed during " + phase)
		}
	}
}
