package ui

import (
	"encoding/json"
	"image/png"
	"os"
	"path/filepath"
	"testing"

	"dotisle/settings/prefs"
	"github.com/egoist/mygo/ui"
)

// Legacy album/system colors must never recolor the default control center.
func TestDefaultPaletteIsIndependent(t *testing.T) {
	p := prefs.Default()
	for _, dark := range []bool{false, true} {
		p.Dark = dark
		for _, source := range []string{"Content", "Custom", "System"} {
			settings := map[string]any{"ControlCenterColorSource": source,
				"SettingsLightAccentColor": "#00ff00", "SettingsDarkAccentColor": "#00ff00"}
			state := map[string]any{"isPlaying": true, "systemAccent": float64(0x00ff00), "artworkSeed": float64(0x00ff00)}
			scheme := BuildPalette(&p, settings, state).Scheme
			if scheme.Primary != fallbackScheme(dark).Primary {
				t.Fatal("legacy green source changed the default theme")
			}
		}
	}
}

// Both sidebar forms must remain clickable after page tree sizes change.
func TestDefaultSidebarNavigation(t *testing.T) {
	for _, width := range []int{900, 1200} {
		a := newTestApp(t)
		a.State["pendingPage"] = ""
		tt := ui.NewTester(a.View, width, 800)
		tt.Frame()
		for _, page := range []int{PageTodos, PageFocus, PageNotifications, PageSettings, PageHome, PageHome} {
			if err := tt.Click("打开导航"); err != nil {
				t.Fatal("menu button unavailable")
			}
			tt.Frame()
			r, ok := tt.Find("切换到" + Pages[page])
			if !ok {
				t.Fatalf("%d: missing navigation %s", width, Pages[page])
			}
			tt.ClickAt(r.X+r.W/2, r.Y+r.H/2)
			tt.Frame()
			if a.Page != page || a.navigationOpen {
				t.Fatalf("%d: sidebar did not navigate to %s", width, Pages[page])
			}
			if page == PageSettings {
				if err := tt.Click("关闭设置"); err != nil {
					t.Fatal(err)
				}
				tt.Frame()
				if a.Page != PageNotifications {
					t.Fatal("settings did not return to the current page")
				}
			}
		}
	}
}

func TestDefaultOverviewUsesNativeTimerFields(t *testing.T) {
	a := newTestApp(t)
	a.State["timers"] = []any{map[string]any{
		"id": "timer-test", "mode": "pomodoro", "focus": float64(1500),
		"rest": float64(300), "phase": "focus", "elapsedMs": float64(60000),
		"round": float64(1), "rounds": float64(4), "running": true,
	}}
	tt := ui.NewTester(a.View, 1200, 800)
	tt.Frame()
	if !tt.HasText("24:00") || !tt.HasText("暂停专注") {
		t.Fatal("overview must render the actual native timer, including its action")
	}
}

func TestDarkToggleUsesResolvedAppearance(t *testing.T) {
	a := newTestApp(t)
	a.settings()["SettingsTheme"] = "Dark"
	a.Prefs.Dark = false
	a.toggleDark()
	if a.Prefs.Dark {
		t.Fatal("dark snapshot must toggle to light even with stale local preferences")
	}
}

// This optional capture uses the production view with an isolated core snapshot.
func TestDefaultDesignCaptures(t *testing.T) {
	output := os.Getenv("DOTISLE_DESIGN_CAPTURE")
	if output == "" {
		t.Skip("capture requested explicitly")
	}
	RegisterFonts()
	snapshot := map[string]any{}
	if path := os.Getenv("DOTISLE_DESIGN_SNAPSHOT"); path != "" {
		raw, err := os.ReadFile(path)
		if err != nil {
			t.Fatal(err)
		}
		if err = json.Unmarshal(raw, &snapshot); err != nil {
			t.Fatal(err)
		}
	}
	cases := []struct {
		name          string
		page          int
		tab           uint8
		width, height int
		dark          bool
	}{
		{"overview-light", PageHome, TabRoot, 1200, 800, false},
		{"overview-dark", PageHome, TabRoot, 1200, 800, true},
		{"overview-compact", PageHome, TabRoot, 900, 680, false},
		{"settings-light", PageSettings, TabRoot, 1200, 800, false},
		{"appearance-light", PageSettings, 0, 1200, 800, false},
		{"timers-light", PageFocus, TabRoot, 1200, 800, false},
		{"timers-compact", PageFocus, TabRoot, 900, 680, false},
		{"widgets-light", PageWidgets, TabRoot, 1200, 800, false},
		{"todos-light", PageTodos, TabRoot, 1200, 800, false},
		{"notifications-dark", PageNotifications, TabRoot, 1200, 800, true},
		{"island-light", PageSettings, 128, 1200, 800, false},
		{"modules-dark", PageSettings, 9, 1200, 800, true},
	}
	if err := os.MkdirAll(output, 0755); err != nil {
		t.Fatal(err)
	}
	for _, tc := range cases {
		a := newTestApp(t)
		if len(snapshot) > 0 {
			a.State = snapshot
		}
		// Copy settings so the shared captured snapshot stays immutable.
		settings := map[string]any{}
		for key, value := range a.settings() {
			settings[key] = value
		}
		if tc.dark {
			settings["SettingsTheme"] = "Dark"
		} else {
			settings["SettingsTheme"] = "Light"
		}
		a.State["settings"] = settings
		a.Prefs.Dark = tc.dark
		a.Page, a.SettingsTab = tc.page, tc.tab
		a.IslandTab = 0
		tt := ui.NewTester(a.View, tc.width, tc.height)
		tt.Frame()
		f, err := os.Create(filepath.Join(output, tc.name+".png"))
		if err != nil {
			t.Fatal(err)
		}
		err = png.Encode(f, tt.Image())
		f.Close()
		if err != nil {
			t.Fatal(err)
		}
		raw, _ := json.MarshalIndent(tt.Texts(), "", "  ")
		if err = os.WriteFile(filepath.Join(output, tc.name+".texts.json"), raw, 0644); err != nil {
			t.Fatal(err)
		}
	}
}
