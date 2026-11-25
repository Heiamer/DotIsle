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

func TestOpeningPureTypographyCascadeAndNoPill(t *testing.T) {
	RegisterFonts()
	absDiff := func(a, b uint8) int {
		if a > b {
			return int(a - b)
		}
		return int(b - a)
	}
	for _, theme := range []string{"Light", "Dark"} {
		t.Run(theme, func(t *testing.T) {
			a := newTestApp(t)
			a.settings()["SettingsTheme"] = theme
			a.StartSignature()
			tt := ui.NewTester(a.View, WindowW, WindowH)

			wordW := min(float32(328), float32(WindowW)-96)
			wordH := wordW * signatureHeight / signatureWidth
			wordX := (float32(WindowW) - wordW) / 2
			wordY := (float32(WindowH) - wordH - 36) / 2

			measureInk := func() int {
				img := tt.Image()
				ink := 0
				for y := int(wordY) - 24; y <= int(wordY+wordH)+24; y++ {
					for x := int(wordX) - 24; x <= int(wordX+wordW)+24; x++ {
						p := img.RGBAAt(x, y)
						luma := (int(p.R)*299 + int(p.G)*587 + int(p.B)*114) / 1000
						if (theme == "Light" && luma < 165) || (theme == "Dark" && luma > 130) {
							ink++
						}
					}
				}
				return ink
			}
			checkNoPill := func(secs float64) {
				img := tt.Image()
				y0 := max(TitlebarH+10, int(wordY)-64)
				y1 := max(y0+1, int(wordY)-16)
				x0, x1 := WindowW/2-70, WindowW/2+70
				for y := y0; y < y1; y++ {
					for x := x0; x < x1; x++ {
						p := img.RGBAAt(x, y)
						if theme == "Light" && (p.R < 210 || p.G < 210 || p.B < 210) {
							t.Fatalf("%s t=%.2fs: unexpected dark element above wordmark at (%d,%d): %+v", theme, secs, x, y, p)
						}
						if theme == "Dark" && (p.R > 95 || p.G > 95 || p.B > 95) {
							t.Fatalf("%s t=%.2fs: unexpected bright pill/ring element above wordmark at (%d,%d): %+v", theme, secs, x, y, p)
						}
						if x+1 < x1 {
							pn := img.RGBAAt(x+1, y)
							if absDiff(p.R, pn.R) > 12 || absDiff(p.G, pn.G) > 12 || absDiff(p.B, pn.B) > 12 {
								t.Fatalf("%s t=%.2fs: sharp edge above wordmark at (%d,%d): %+v -> %+v", theme, secs, x, y, p, pn)
							}
						}
					}
				}
				if a.openingCanvas == nil {
					t.Fatalf("%s t=%.2fs: expected optical light-field canvas during opening", theme, secs)
				}
			}

			tt.Advance(120 * time.Millisecond)
			tt.Frame()
			c0 := measureInk()

			tt.Advance(300 * time.Millisecond) // 0.42s
			tt.Frame()
			c1 := measureInk()

			tt.Advance(60 * time.Millisecond) // 0.48s
			tt.Frame()
			checkNoPill(0.48)

			tt.Advance(420 * time.Millisecond) // 0.90s
			tt.Frame()
			c2 := measureInk()

			tt.Advance(100 * time.Millisecond) // 1.00s
			tt.Frame()
			checkNoPill(1.00)

			if c0 <= 0 || !(c0 < c1 && c1 < c2) {
				t.Fatalf("%s: expected strictly increasing wordmark ink across cascade (0.12s, 0.42s, 0.90s), got [%d, %d, %d]", theme, c0, c1, c2)
			}

			tt.Advance(600 * time.Millisecond) // 1.60s
			tt.Frame()
			if a.openingCanvas != nil {
				t.Fatalf("%s: openingCanvas leaked after splash settled", theme)
			}
		})
	}
}

// Explicit production-view captures, including the navigation overlay.
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
