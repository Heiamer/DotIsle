package ui

import (
	"dotisle/settings/prefs"
	"github.com/egoist/mygo/ui"
	"image/png"
	"math"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestPluginSurfaceCaptures(t *testing.T) {
	out := os.Getenv("DOTISLE_DESIGN_CAPTURE")
	if out == "" {
		t.Skip("capture requested explicitly")
	}
	RegisterFonts()
	a := newTestApp(t)
	a.State["pendingPage"] = ""
	a.State["plugins"] = []any{map[string]any{"id": "example.hello", "name": "你好，积木", "version": "1.0.0", "description": "一个插件，连接灵动岛、概览和桌面。", "enabled": true, "views": map[string]any{
		"island":   []any{map[string]any{"kind": "text", "text": "你好，点屿"}},
		"overview": []any{map[string]any{"kind": "metric", "text": "42", "label": "已运行秒数"}, map[string]any{"kind": "progress", "label": "今日进度", "value": float64(.42)}},
		"desktop":  []any{map[string]any{"kind": "text", "text": "桌面上的一块积木"}},
	}}}
	a.addPluginCard("example.hello")
	if err := os.MkdirAll(out, 0755); err != nil {
		t.Fatal(err)
	}
	for _, tc := range []struct {
		name string
		page int
	}{{"plugin-manager", PagePlugins}, {"plugin-overview", PageHome}} {
		a.Page = tc.page
		if tc.page == PageHome {
			a.overview.config.Cards = []string{"plugin:example.hello"}
		}
		tt := ui.NewTester(a.View, 1200, 800)
		tt.Frame()
		tt.Advance(time.Second)
		f, err := os.Create(filepath.Join(out, tc.name+".png"))
		if err != nil {
			t.Fatal(err)
		}
		err = png.Encode(f, tt.Image())
		closeErr := f.Close()
		if err != nil {
			t.Fatal(err)
		}
		if closeErr != nil {
			t.Fatal(closeErr)
		}
	}
}

func TestPluginCardsPersistAndRemovedPluginsStayInert(t *testing.T) {
	a := newTestApp(t)
	a.State["pendingPage"] = ""
	a.State["plugins"] = []any{map[string]any{"id": "example.hello", "name": "测试插件", "enabled": true, "views": map[string]any{"overview": []any{map[string]any{"kind": "metric", "text": "42", "label": "本日记录"}}}}}
	a.addPluginCard("example.hello")
	p, err := prefs.LoadOverview(filepath.Join(a.Data, "center-overview.json"))
	if err != nil {
		t.Fatal(err)
	}
	if p.Cards[len(p.Cards)-1] != "plugin:example.hello" {
		t.Fatal("plugin ID lost on save")
	}
	a.overview.config.Cards = []string{"plugin:example.hello"}
	tt := ui.NewTester(a.View, 1200, 800)
	tt.Frame()
	if !tt.HasText("42") {
		t.Fatal("plugin blocks were not rendered")
	}
	a.State["plugins"] = []any{}
	tt.Frame()
	if !tt.HasText("插件未启用或已移除") {
		t.Fatal("removed plugin is still live")
	}
	a.Page = PagePlugins
	tt.Frame()
	if _, ok := tt.Find("安装插件"); !ok {
		t.Fatal("plugin manager missing")
	}
	for _, key := range []string{"plugin:../../bad", "plugin:EXAMPLE", "plugin:", "plugin:a/b"} {
		if prefs.ValidPluginKey(key) {
			t.Fatal("unsafe key accepted", key)
		}
	}
}

func TestDeckMotionUsesApplicationSwitchEvenWhenWindowsDisablesMotion(t *testing.T) {
	defer func() { deckPos, deckTarget, deckSpringIdle = 0, 0, true }()
	for _, enabled := range []bool{true, false} {
		a := newTestApp(t)
		a.State["pendingPage"] = ""
		a.settings()["Animation"] = enabled
		a.Prefs.ReducedMotion = !enabled
		a.overview.loaded = true
		a.overview.config = prefs.DefaultOverview()
		a.overview.config.View = "scroll"
		deckPos, deckTarget, deckSpringIdle = 0, 0, true
		deckLastInput = time.Time{}
		tt := ui.NewTester(a.View, 1200, 800)
		tt.SetPreferences(ui.Preferences{ReduceMotion: true, TextScale: 1})
		tt.Frame()
		a.StepOverview(1)
		tt.Frame()
		tt.Advance(60 * time.Millisecond)
		if enabled {
			if deckPos <= 0 || deckPos >= deckStep {
				t.Fatalf("Windows disabled app spring: %f", deckPos)
			}
			before := deckPos
			a.StepOverview(-1)
			tt.Frame()
			if math.Abs(deckPos-before) > deckStep*.2 {
				t.Fatal("reversal snapped position")
			}
			tt.Advance(time.Second)
			if math.Abs(deckPos) > 0.01 {
				t.Fatal("reversal failed to settle")
			}
		} else if math.Abs(deckPos-deckStep) > .01 {
			t.Fatal("application animation switch ignored")
		}
	}
}

func TestAppSpringReversalKeepsPositionAndVelocity(t *testing.T) {
	now := time.Unix(1, 0)
	s := appSpring{}
	s.jump(0, now)
	s.sample(now, 1, 380, .82, false)
	now = now.Add(60 * time.Millisecond)
	s.sample(now, 1, 380, .82, false)
	before, velocity := s.value, s.velocity
	s.sample(now, 0, 380, .82, false)
	if s.value != before || s.velocity != velocity {
		t.Fatal("retarget discarded physical state")
	}
	s.sample(now.Add(time.Second), 0, 380, .82, false)
	if !s.settled() {
		t.Fatal("reversal kept scheduling frames")
	}
}
