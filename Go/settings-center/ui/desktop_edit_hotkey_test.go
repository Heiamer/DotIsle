package ui

import (
	"encoding/json"
	"image/png"
	"os"
	"path/filepath"
	"testing"

	"github.com/egoist/mygo/ui"
)

func TestDesktopEditHotkeyRecording(t *testing.T) {
	a := cityTestApp(t)
	a.AllegroAvailable = true
	a.allegroSettings = defaultAllegroSettings()
	a.allegroSettings.OnboardingDone = true
	a.cityPromptAsked = true
	a.Page, a.SettingsTab = PageSettings, 10
	a.allegro = &allegroController{queue: make(chan allegroMutation, 8), live: make(chan []byte, 1)}
	tt := ui.NewTester(a.View, 1200, 900)
	tt.Frame()
	r, ok := tt.Find("录入桌面编辑热键")
	if !ok { t.Fatal("desktop editor recording button missing") }
	tt.ClickAt(r.X+r.W/2, r.Y+r.H/2)
	tt.Frame()
	if !a.allegroRecording || !a.allegroRecordingEdit { t.Fatal("editor recording did not start") }
	tt.Key(ui.Ctrl|ui.Alt|ui.Shift, ui.KeyY)
	tt.Frame()
	if a.allegroRecording || a.allegroSettings.EditKey != 0x59 || a.allegroSettings.EditModifiers != 7 {
		t.Fatalf("editor recording failed: %+v, %s", a.allegroSettings, a.allegroCaptureError)
	}
	m := <-a.allegro.queue
	raw, err := m.change([]byte(`{"enabled":false,"edge":"left","custom_modifiers":6,"custom_key":88}`))
	if err != nil { t.Fatal(err) }
	var s allegroSettings
	if err = json.Unmarshal(raw, &s); err != nil { t.Fatal(err) }
	if s.editHotkeyLabel() != "Ctrl + Alt + Shift + Y" || s.CustomKey != 88 || s.Edge != "left" || s.Enabled {
		t.Fatalf("editor save lost concurrent settings: %+v", s)
	}
	a.allegroRecording = true
	a.recordSidebarHotkey(ui.InputEvent{Kind: ui.InputKeyDown, Key: ui.KeyY, Mods: ui.Ctrl|ui.Alt|ui.Shift})
	if a.allegroCaptureError == "" { t.Fatal("sidebar accepted editor chord") }
	a.allegroSettings.CustomKey, a.allegroSettings.CustomModifiers = 0x58, 7
	a.allegroCaptureError = ""
	a.recordEditHotkey(ui.InputEvent{Kind: ui.InputKeyDown, Key: ui.KeyX, Mods: ui.Ctrl|ui.Alt|ui.Shift})
	if a.allegroCaptureError == "" { t.Fatal("editor accepted sidebar chord") }
	a.recordEditHotkey(ui.InputEvent{Kind: ui.InputKeyDown, Key: ui.KeyEscape})
	if a.allegroRecording { t.Fatal("escape did not cancel editor recording") }
	if defaultAllegroSettings().editHotkeyLabel() != "Alt + E" { t.Fatal("legacy default changed") }
	if out := os.Getenv("DOTISLE_ALLEGRO_CAPTURE"); out != "" {
		os.MkdirAll(out, 0755)
		tt.Frame()
		f, err := os.Create(filepath.Join(out, "desktop-edit-hotkey.png"))
		if err != nil { t.Fatal(err) }
		err = png.Encode(f, tt.Image()); f.Close()
		if err != nil { t.Fatal(err) }
		a.allegroTutorialOpen, a.allegroTutorial = true, 2
		tt.Frame()
		f, err = os.Create(filepath.Join(out, "desktop-edit-tutorial.png"))
		if err != nil { t.Fatal(err) }
		err = png.Encode(f, tt.Image()); f.Close()
		if err != nil { t.Fatal(err) }
	}
}
