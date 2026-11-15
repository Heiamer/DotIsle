package ui

import (
	"image/png"
	"os"
	"path/filepath"
	"testing"

	"dotisle/settings/prefs"
	"github.com/egoist/mygo/ui"
)

// Explicit production-view captures for the independent overview tiles.
func TestOverviewTileCaptures(t *testing.T) {
	out := os.Getenv("DOTISLE_OVERVIEW_CAPTURE")
	if out == "" {
		t.Skip("explicit capture")
	}
	if err := os.MkdirAll(out, 0755); err != nil {
		t.Fatal(err)
	}
	RegisterFonts()
	a := newTestApp(t)
	a.overview.loaded = true
	a.overview.config = prefs.Overview{View: "cards", Cards: []string{"widget:weather", "widget:storage", "widget:note", "widget:worldClock"}}
	a.State["overviewWidgets"] = []any{
		map[string]any{"id": "overview-weather", "kind": "weather", "config": map[string]any{"city": "厦门"}},
		map[string]any{"id": "overview-storage", "kind": "storage", "config": map[string]any{"drive": "C:"}},
		map[string]any{"id": "overview-note", "kind": "note", "config": map[string]any{"text": "今天把概览组件都设置好\n不再依赖桌面组件"}},
		map[string]any{"id": "overview-worldClock", "kind": "worldClock", "config": map[string]any{"city": "东京"}},
	}
	a.State["widgetData"] = map[string]any{
		"overview-weather":    map[string]any{"temperature": float64(26), "city": "厦门 · 晴", "high": float64(29), "low": float64(22)},
		"overview-storage":    map[string]any{"free": float64(128e9), "total": float64(512e9), "drive": "C:\\"},
		"overview-worldClock": map[string]any{"clock": "09:30", "date": "11月16日", "city": "东京"},
	}
	tt := ui.NewTester(a.View, 1200, 800)
	tt.Frame()
	save := func(name string) {
		f, err := os.Create(filepath.Join(out, name+".png"))
		if err != nil {
			t.Fatal(err)
		}
		defer f.Close()
		if err := png.Encode(f, tt.Image()); err != nil {
			t.Fatal(err)
		}
	}
	save("overview-tiles")
	if err := tt.Click("设置天气"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	save("weather-editor")
}
