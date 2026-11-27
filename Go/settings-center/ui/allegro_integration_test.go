package ui

import (
	"encoding/json"
	"github.com/egoist/mygo/ui"
	"image/png"
	"os"
	"path/filepath"
	"testing"
)

func TestAllegroSettingsDeltaAndBoardPersistence(t *testing.T) {
	a := newTestApp(t)
	a.allegroSettings = defaultAllegroSettings()
	a.allegro = &allegroController{queue: make(chan allegroMutation, 8), live: make(chan []byte, 1)}
	a.allegroChange(func(s *allegroSettings) { s.Enabled = false; s.Hotkey = "alt_q" })
	m := <-a.allegro.queue
	raw, err := m.change([]byte(`{"enabled":true,"edge":"left","hotkey":"alt_grave","onboarding_done":true}`))
	if err != nil {
		t.Fatal(err)
	}
	var settings allegroSettings
	if err = json.Unmarshal(raw, &settings); err != nil {
		t.Fatal(err)
	}
	if settings.Enabled || settings.Hotkey != "alt_q" || settings.Edge != "left" || !settings.OnboardingDone {
		t.Fatalf("lost concurrent settings: %+v", settings)
	}
	path := filepath.Join(t.TempDir(), m.file)
	if err = atomicAllegro(path, raw); err != nil {
		t.Fatal(err)
	}
	if err = atomicAllegro(path, raw); err != nil {
		t.Fatal(err)
	}
	loaded, err := os.ReadFile(path)
	if err != nil || string(loaded) != string(raw) {
		t.Fatal("atomic replacement failed", err)
	}
	a.allegroBoardChange(func(b *allegroBoard) { b.Cards = append(b.Cards, allegroCard{ID: 99, Kind: "music", Size: "Medium"}) })
	m = <-a.allegro.queue
	raw, err = m.change([]byte(`{"cards":[]}`))
	if err != nil {
		t.Fatal(err)
	}
	var b allegroBoard
	if err = json.Unmarshal(raw, &b); err != nil || len(b.Cards) != 1 || b.Cards[0].ID != 99 {
		t.Fatal("board mutation failed", err)
	}
	for i := 0; i < 100; i++ {
		a.publishAllegroLive(map[string]any{"position": i})
	}
	var snapshot map[string]any
	if err = json.Unmarshal(<-a.allegro.live, &snapshot); err != nil || snapshot["position"] != float64(99) {
		t.Fatal("latest state was lost", err)
	}
	if len(a.allegro.queue) != 0 {
		t.Fatal("live traffic entered settings queue")
	}
}
func TestAllegroProductionCapture(t *testing.T) {
	out := os.Getenv("DOTISLE_ALLEGRO_CAPTURE")
	if out == "" {
		t.Skip("explicit production view capture")
	}
	RegisterFonts()
	os.MkdirAll(out, 0755)
	for _, page := range []string{"settings", "city", "tutorial", "widgets"} {
		a := cityTestApp(t)
        if os.Getenv("DOTISLE_CAPTURE_DARK")=="1" {a.Prefs.Dark=true;a.settings()["SettingsTheme"]="Dark"}
		a.AllegroAvailable = true
		a.allegroSettings = defaultAllegroSettings()
		a.allegroSettings.OnboardingDone = true
		a.allegroPreviews = map[string]*ui.Bitmap{}
		files, _ := filepath.Glob(filepath.Join(os.Getenv("DOTISLE_ALLEGRO_ASSETS"), "*.png"))
		for _, p := range files {
			raw, _ := os.ReadFile(p)
			b, err := ui.DecodeBitmap(raw)
			if err == nil {
				a.allegroPreviews[filepath.Base(p)] = b
			}
		}
		a.cityPromptAsked = true
		switch page {
		case "settings":
			a.Page = PageSettings
			a.SettingsTab = 10
		case "city":
			a.cityPrompt = &cityPromptState{text: "上海"}
		case "tutorial":
			a.allegroTutorialOpen = true
		case "widgets":
			a.Page = PageWidgets
		}
		tt := ui.NewTester(a.View, 1200, 800)
		tt.Frame()
		name:=page+".png";if os.Getenv("DOTISLE_CAPTURE_DARK")=="1"{name=page+"-dark.png"}
        f, err := os.Create(filepath.Join(out, name))
		if err != nil {
			t.Fatal(err)
		}
		err = png.Encode(f, tt.Image())
		f.Close()
		if err != nil {
			t.Fatal(err)
		}
		if page == "city" {
			r, ok := tt.Find("稍后填写")
			if !ok {
				t.Fatal("dismiss missing")
			}
			tt.ClickAt(r.X+r.W/2, r.Y+r.H/2)
			tt.Frame()
			if a.cityPrompt != nil {
				t.Fatal("dismiss did not close")
			}
		}
	}
}
func TestSidebarCustomHotkeyRecording(t *testing.T) {
 a:=cityTestApp(t);a.AllegroAvailable=true;a.allegroSettings=defaultAllegroSettings();a.allegroSettings.OnboardingDone=true;a.cityPromptAsked=true;a.Page=PageSettings;a.SettingsTab=10
 a.allegro=&allegroController{queue:make(chan allegroMutation,8),live:make(chan []byte,1)}
 tt:=ui.NewTester(a.View,1200,800);tt.Frame();r,ok:=tt.Find("录入侧边栏热键");if !ok{t.Fatal("record button missing")}
 tt.ClickAt(r.X+r.W/2,r.Y+r.H/2);tt.Frame();if !a.allegroRecording{t.Fatal("recording did not start")}
 tt.Key(ui.Ctrl|ui.Alt|ui.Shift,ui.KeyX);tt.Frame()
 if a.allegroRecording||a.allegroSettings.CustomKey!=0x58||a.allegroSettings.CustomModifiers!=7{t.Fatalf("recording failed: %+v, %s",a.allegroSettings,a.allegroCaptureError)}
 m:=<-a.allegro.queue;raw,err:=m.change(nil);if err!=nil{t.Fatal(err)};var s allegroSettings;json.Unmarshal(raw,&s)
 if s.hotkeyLabel()!="Ctrl + Alt + Shift + X"{t.Fatal(s.hotkeyLabel())}
 a.allegroRecording=true;a.recordSidebarHotkey(ui.InputEvent{Kind:ui.InputKeyDown,Key:ui.KeyE,Mods:ui.Alt});if a.allegroCaptureError==""{t.Fatal("reserved editor key accepted")}
 a.recordSidebarHotkey(ui.InputEvent{Kind:ui.InputKeyDown,Key:ui.KeyEscape});if a.allegroRecording{t.Fatal("escape did not cancel")}
 if Symbol("widgets")==nil{t.Fatal("sidebar icon missing")}
}
