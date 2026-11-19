package ui

import (
	"testing"

	"github.com/egoist/mygo/ui"
)

func TestSettingsSingletonSearchAndTwoLevels(t *testing.T) {
	a := newTestApp(t)
	a.State["pendingPage"] = ""
	a.Page = PageNotifications
	a.openSettings(TabRoot)
	tt := ui.NewTester(a.View, 900, 680)
	tt.Frame()
	click := func(label string) {
		t.Helper()
		if err := tt.Click(label); err != nil {
			t.Fatal(err)
		}
		tt.Frame()
	}
	click("设置分类 音乐与歌词")
	click("设置分组 播放进度条")
	if a.SettingsTab != 133 || !tt.HasText("进度条柔光") {
		t.Fatal("progress leaf not reached directly")
	}
	click("搜索设置")
	tt.Type("歌词柔光")
	tt.Frame()
	// A generic open request reuses the panel, including an active query.
	a.openSettings(TabRoot)
	if a.settingsPanel.query != "歌词柔光" || a.SettingsTab != 133 {
		t.Fatal("reopen reset the active panel")
	}
	click("设置搜索结果 歌词与本地文件")
	if a.SettingsTab != 1 || !tt.HasText("当前歌词柔光") {
		t.Fatal("search did not open the leaf")
	}
	a.openSettings(130)
	tt.Frame()
	if a.SettingsTab != 130 || a.centerPage() != PageNotifications {
		t.Fatal("deep link did not reuse the panel")
	}
	tt.Key(0, ui.KeyEscape)
	tt.Frame()
	if a.Page != PageNotifications {
		t.Fatal("Escape closed a nested panel instead of returning")
	}
	if _, found := tt.Find("关闭设置"); found {
		t.Fatal("another settings panel remained open")
	}
}

func TestSettingsRoutesAreUniqueAndReachable(t *testing.T) {
	seen := map[uint8]bool{}
	for _, category := range settingsCategories {
		for _, section := range category.sections {
			if seen[section.tab] {
				t.Fatalf("tab %d belongs to multiple categories", section.tab)
			}
			seen[section.tab] = true
		}
	}
	for tab := uint8(0); tab <= 8; tab++ {
		if !seen[tab] {
			t.Fatalf("missing settings tab %d", tab)
		}
	}
	if seen[9] {
		t.Fatal("module management tab must stay hidden")
	}
	for tab := uint8(128); tab <= 135; tab++ {
		if !seen[tab] {
			t.Fatalf("missing island tab %d", tab)
		}
	}
}
