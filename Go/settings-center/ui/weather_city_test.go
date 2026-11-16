package ui

import (
	"errors"
	"image/png"
	"os"
	"path/filepath"
	"testing"

	"github.com/egoist/mygo/ui"
)

func TestCityPromptCapture(t *testing.T) {
	out := os.Getenv("DOTISLE_CITY_CAPTURE")
	if out == "" {
		t.Skip("explicit production-view render")
	}
	if err := os.MkdirAll(out, 0755); err != nil {
		t.Fatal(err)
	}
	RegisterFonts()
	for _, dark := range []bool{false, true} {
		a := cityTestApp(t)
		a.Prefs.Dark = dark
		a.settings()["SettingsTheme"] = map[bool]string{true: "Dark", false: "Light"}[dark]
		tt := ui.NewTester(a.View, 1200, 800)
		tt.Frame()
		name := "city-light.png"
		if dark {
			name = "city-dark.png"
		}
		f, err := os.Create(filepath.Join(out, name))
		if err != nil {
			t.Fatal(err)
		}
		err = png.Encode(f, tt.Image())
		f.Close()
		if err != nil {
			t.Fatal(err)
		}
	}
}

func cityTestApp(t *testing.T) *App {
	a := newTestApp(t)
	a.EnableCityPrompt = true
	a.Connected = true
	a.State = map[string]any{"settings": map[string]any{"WeatherCity": "", "ControlCenterSignatureEnabled": false}}
	a.BeginCenterVisit()
	return a
}

func TestCityPromptWaitsForVisibleConnectedSnapshotAndAvoidsOtherDialogs(t *testing.T) {
	a := cityTestApp(t)
	a.EndCenterVisit()
	a.maybePromptCity()
	if a.cityPrompt != nil {
		t.Fatal("background startup prompted")
	}
	a.BeginCenterVisit()
	a.Connected = false
	a.maybePromptCity()
	if a.cityPrompt != nil {
		t.Fatal("disconnected prompted")
	}
	a.Connected = true
	a.showQuickReminder = true
	a.maybePromptCity()
	if a.cityPrompt != nil {
		t.Fatal("prompt covered another dialog")
	}
	a.showQuickReminder = false
	a.maybePromptCity()
	if a.cityPrompt == nil {
		t.Fatal("missing city not prompted")
	}
	a.cityPrompt = nil
	a.maybePromptCity()
	if a.cityPrompt != nil {
		t.Fatal("dismissal prompted again during same visit")
	}
	a.BeginCenterVisit()
	a.maybePromptCity()
	if a.cityPrompt == nil {
		t.Fatal("next visit failed to remind")
	}
}

func TestExistingWeatherCitiesAreReused(t *testing.T) {
	for _, key := range []string{"WeatherCity", "overviewWidgets", "desktopWidgets"} {
		a := cityTestApp(t)
		if key == "WeatherCity" {
			a.settings()[key] = " 北京 "
		} else {
			a.State[key] = []any{map[string]any{"kind": "weather", "config": map[string]any{"city": "上海"}}}
		}
		a.maybePromptCity()
		if a.cityPrompt != nil {
			t.Fatalf("existing city in %s prompted", key)
		}
	}
}

func TestCitySaveNeedsExplicitAcknowledgementAndKeepsFailedDraft(t *testing.T) {
	a := cityTestApp(t)
	a.maybePromptCity()
	draft := a.cityPrompt
	draft.text = "上海"
	draft.sent = "上海"
	draft.saving = true
	a.applyCitySaveReply(draft, nil, errors.New("offline"))
	if a.cityPrompt != draft || draft.error == "" || draft.saving {
		t.Fatal("failed save lost draft")
	}
	a.applyCitySaveReply(draft, map[string]any{}, nil)
	if a.cityPrompt != draft {
		t.Fatal("missing acknowledgement accepted")
	}
	a.applyCitySaveReply(draft, map[string]any{"success": true}, nil)
	if a.cityPrompt != nil || a.configuredWeatherCity() != "上海" {
		t.Fatal("acknowledged city not applied")
	}
}

func TestCityPromptRendersAndDismisses(t *testing.T) {
	a := cityTestApp(t)
	tt := ui.NewTester(a.View, 1200, 800)
	tt.Frame()
	if _, ok := tt.Find("保存城市"); !ok {
		t.Fatal("save button missing")
	}
	r, ok := tt.Find("稍后填写")
	if !ok {
		t.Fatal("defer button missing")
	}
	tt.ClickAt(r.X+r.W/2, r.Y+r.H/2)
	tt.Frame()
	if a.cityPrompt != nil {
		t.Fatal("defer did not dismiss")
	}
}
