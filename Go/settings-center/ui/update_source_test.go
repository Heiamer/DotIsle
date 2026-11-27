package ui

import (
	"github.com/egoist/mygo/ui"
	"image/png"
	"os"
	"path/filepath"
	"testing"
)

func TestUpdateSourceDispatchRequiresAcknowledgementAndUnlockedConnection(t *testing.T) {
	a := newTestApp(t)
	a.Connected = true
	a.Palette = BuildPalette(&a.Prefs, a.settings(), a.State)
	a.Palette.ReducedMotion = true
	var commands []string
	a.onCommand = func(name string, value any) { commands = append(commands, name+":"+value.(string)) }
	tt := ui.NewTester(func(c *ui.Context) { a.aboutBody(c, &a.Palette, 20, 0, 600) }, 640, 780)
	tt.Frame()
	for _, label := range []string{"GITCODE", "GITEE", "GITHUB"} {
		if _, ok := tt.Access(label); !ok {
			t.Fatal("missing update source", label)
		}
	}
	if err := tt.Click("GITCODE"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if len(commands) != 0 {
		t.Fatal("default GitCode should not dispatch a mutation")
	}
	if err := tt.Click("GITEE"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if len(commands) != 1 || commands[0] != "updateSource:gitee" {
		t.Fatal("wrong source command", commands)
	}
	if _, ok := a.State["updateSource"]; ok {
		t.Fatal("UI must wait for native acknowledgement")
	}
	a.State["updateSource"] = "gitee"
	tt.Frame()
	if err := tt.Click("GITEE"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if len(commands) != 1 {
		t.Fatal("acknowledged source dispatched again")
	}
	for _, phase := range []string{"checking", "downloading", "switching", "verifying", "ready", "applying", "restarting"} {
		a.State["softwareUpdate"] = map[string]any{"phase": phase}
		tt.Frame()
		if err := tt.Click("GITHUB"); err != nil {
			t.Fatal(err)
		}
		tt.Frame()
		if len(commands) != 1 {
			t.Fatal("source switched during", phase)
		}
	}
	a.State["softwareUpdate"] = map[string]any{"phase": "idle"}
	a.Connected = false
	tt.Frame()
	_ = tt.Click("GITHUB")
	tt.Frame()
	if len(commands) != 1 {
		t.Fatal("disconnected source mutation")
	}
}

func TestUpdateSourceProductionPageCapture(t *testing.T) {
	output := os.Getenv("DOTISLE_UPDATE_CAPTURE")
	if output == "" {
		t.Skip("explicit visual capture")
	}
	RegisterFonts()
	for _, theme := range []string{"Light", "Dark"} {
		a := newTestApp(t)
		a.Page = PageSettings
		a.SettingsTab = 3
		a.Connected = true
		a.settings()["SettingsTheme"] = theme
		a.settings()["ReduceMotionEnabled"] = true
		a.State["version"] = "5.2.7"
		a.State["updateSource"] = "gitcode"
		a.State["softwareUpdate"] = map[string]any{"phase": "idle", "message": "检查此渠道可用的新版本"}
		tt := ui.NewTester(a.View, 900, 780)
		tt.Frame()
		tt.Scroll(780, 430, 0, 390)
		tt.Frame()
		tt.Frame()
		if !tt.HasText("GITCODE") || !tt.HasText("GITEE") || !tt.HasText("GITHUB") {
			t.Fatal("source selector missing from production page")
		}
		if err := os.MkdirAll(output, 0755); err != nil {
			t.Fatal(err)
		}
		f, err := os.Create(filepath.Join(output, "update-sources-"+theme+".png"))
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
