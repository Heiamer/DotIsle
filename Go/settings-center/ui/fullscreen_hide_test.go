package ui

import (
	"image/png"
	"os"
	"testing"

	"github.com/egoist/mygo/ui"
)

func TestFullscreenAutoHideSwitch(t *testing.T) {
	RegisterFonts()
	a := newTestApp(t)
	a.State["pendingPage"] = ""
	a.Page, a.SettingsTab, a.IslandTab = PageSettings, 130, 2
	const key = "HideIslandWhenFullscreen"
	var values []bool
	a.onCommand = func(name string, value any) {
		update, ok := value.(map[string]any)
		if !ok || name != "update" || update["key"] != key {
			t.Fatalf("unexpected fullscreen command: %s, %v", name, value)
		}
		checked := update["value"].(bool)
		values = append(values, checked)
		a.settings()[key] = checked
	}
	tt := ui.NewTester(a.View, 1060, 760)
	tt.Frame()
	tt.Scroll(820, 460, 0, 550)
	tt.Frame()
	if err := tt.Click(key); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if dest := os.Getenv("DOTISLE_FULLSCREEN_CAPTURE"); dest != "" {
		f, err := os.Create(dest)
		if err != nil {
			t.Fatal(err)
		}
		defer f.Close()
		if err := png.Encode(f, tt.Image()); err != nil {
			t.Fatal(err)
		}
	}
	if err := tt.Click(key); err != nil {
		t.Fatal(err)
	}
	if len(values) != 2 || !values[0] || values[1] {
		t.Fatalf("switch should default off and toggle independently: %v", values)
	}
}
