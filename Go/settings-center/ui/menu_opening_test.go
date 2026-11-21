package ui

import (
	"github.com/egoist/mygo/ui"
	"image/png"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestMenuIsModalAndClosesForExternalRoutes(t *testing.T) {
	a := newTestApp(t)
	tt := ui.NewTester(a.View, 1200, 800)
	if err := tt.Click("打开导航"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	tt.Advance(250 * time.Millisecond)
	tt.Frame()
	if _, ok := tt.Access("自定义概览组件"); ok {
		t.Fatal("background is still accessible")
	}
	tt.Key(ui.Ctrl, ui.KeyK)
	tt.Frame()
	if a.paletteOpen {
		t.Fatal("background shortcut escaped menu scope")
	}
	tt.Key(0, ui.KeyTab)
	tt.Frame()
	a.State["pendingPage"] = "todos"
	a.State["pendingRequestId"] = float64(987)
	a.applyPendingPage()
	if a.navigationOpen || a.Page != PageTodos {
		t.Fatal("external route retained overlay")
	}
}

func TestMenuFollowsThemeAndLeavesRightUnchanged(t *testing.T) {
	RegisterFonts()
	for _, theme := range []string{"Light", "Dark"} {
		a := newTestApp(t)
		a.settings()["SettingsTheme"] = theme
		tt := ui.NewTester(a.View, 1200, 800)
		before := append([]byte{}, tt.Image().Pix...)
		a.ToggleNavigation()
		tt.Frame()
		// Text must be legible from the first open frame, including on hover.
		label, ok := tt.Find("组件")
		if !ok {
			t.Fatal("navigation text missing")
		}
		for _, hover := range []bool{false, true} {
			if hover {
				tt.Move(label.X+label.W/2, label.Y+label.H/2)
				tt.Frame()
			}
			img := tt.Image()
			ink := 0
			for y := int(label.Y); y < int(label.Y+label.H); y++ {
				for x := int(label.X); x < int(label.X+label.W); x++ {
					p := img.RGBAAt(x, y)
					if (theme == "Light" && p.R < 60 && p.G < 60 && p.B < 60) || (theme == "Dark" && p.R > 195 && p.G > 195 && p.B > 195) {
						ink++
					}
				}
			}
			if ink < 20 {
				t.Fatalf("%s hover=%v: navigation text faded, contrast pixels=%d rect=%v", theme, hover, ink, label)
			}
		}
		tt.Advance(400 * time.Millisecond)
		tt.Frame()
		panel, ok := tt.Find("侧边导航栏")
		if !ok || panel.X != 0 || panel.Y != 0 || panel.H != 800 || panel.W > 360 {
			t.Fatal("navigation covered the page", panel)
		}
		text, ok := tt.Find("组件")
		if !ok || text.X > 72 {
			t.Fatal("navigation text is not left aligned", text)
		}
		img := tt.Image()
		for y := 70; y < 790; y++ {
			for x := 400; x < 1190; x++ {
				o := img.PixOffset(x, y)
				for i := 0; i < 4; i++ {
					if img.Pix[o+i] != before[o+i] {
						t.Fatalf("%s: right content changed at %d,%d", theme, x, y)
					}
				}
			}
		}
		v := img.RGBAAt(15, 700)
		if (theme == "Light" && (v.R < 200 || v.G < 200 || v.B < 200)) || (theme == "Dark" && (v.R > 70 || v.G > 70 || v.B > 70)) {
			t.Fatal("sidebar stopped following the theme", v)
		}
		if err := tt.Click("收起导航"); err != nil {
			t.Fatal(err)
		}
		tt.Frame()
		if a.navigationOpen {
			t.Fatal("outside click did not dismiss sidebar")
		}
	}
}

func TestOpeningExitAndOpaquePageCulling(t *testing.T) {
	if signatureDuration(false) > 1.8 {
		t.Fatal("opening delays the interface")
	}
	_, alpha := sigInk(sigWrite+sigHold+sigExit/2, false)
	if alpha <= 0 || alpha >= 1 {
		t.Fatal("exit did not crossfade")
	}
	a := newTestApp(t)
	a.StartSignature()
	tt := ui.NewTester(a.View, 1200, 800)
	if tt.HasText("音乐、计时与待办事项") {
		t.Fatal("opaque cover still builds background page")
	}
	if !tt.HasText("5.0.0") {
		t.Fatal("opening lost current runtime version")
	}
	tt.Advance(1400 * time.Millisecond)
	tt.Frame()
	if !tt.HasText("音乐、计时与待办事项") {
		t.Fatal("fade must composite over the real page")
	}
}

// Explicit production-view captures, including real Gaussian blur in the scene.
func TestMenuOpeningCaptures(t *testing.T) {
	out := os.Getenv("DOTISLE_MENU_CAPTURE")
	if out == "" {
		t.Skip("explicit capture")
	}
	if err := os.MkdirAll(out, 0755); err != nil {
		t.Fatal(err)
	}
	RegisterFonts()
	for _, dark := range []bool{false, true} {
		a := newTestApp(t)
		suffix := "light"
		a.settings()["SettingsTheme"] = "Light"
		if dark {
			suffix = "dark"
			a.settings()["SettingsTheme"] = "Dark"
		}
		tt := ui.NewTester(a.View, 1200, 800)
		save := func(name string) {
			f, err := os.Create(filepath.Join(out, name+"-"+suffix+".png"))
			if err != nil {
				t.Fatal(err)
			}
			defer f.Close()
			if err = png.Encode(f, tt.Image()); err != nil {
				t.Fatal(err)
			}
		}
		save("overview")
		a.ToggleNavigation()
		tt.Frame()
		tt.Advance(250 * time.Millisecond)
		tt.Frame()
		save("menu")
		a.navigate(PageHome)
		a.StartSignature()
		tt.Frame()
		tt.Advance(600 * time.Millisecond)
		tt.Frame()
		save("opening-060")
		tt.Advance(760 * time.Millisecond)
		tt.Frame()
		save("opening-136")
	}
}
