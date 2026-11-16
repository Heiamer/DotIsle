package ui

import (
	"fmt"
	"image/png"
	"os"
	"path/filepath"
	"time"

	"dotisle/settings/prefs"
	"github.com/egoist/mygo/ui"
)

// These captures use shipped geometry, view code and embedded fonts. The clock
// is explicit so every intermediate pose is reproducible without desktop input.
func verifyDesign(output, data, cover string, coverage *[]map[string]any) error {
	fixture := func(dark bool) *App {
		theme := "Light"
		if dark {
			theme = "Dark"
		}
		return &App{Prefs: prefs.Default(), Data: data, State: map[string]any{
			"pendingPage": "", "version": "5.0.3", "title": "此刻，正在聆听", "artist": "验收示例",
			"settings":     map[string]any{"SettingsTheme": theme, "Animation": true},
			"musicPreview": map[string]any{"coverPath": cover, "lyricFrame": map[string]any{"currentLine": "把这一刻留给自己"}},
		}}
	}
	capture := func(name string, tt *ui.Tester) error {
		f, err := os.Create(filepath.Join(output, name+".png"))
		if err != nil {
			return err
		}
		err = png.Encode(f, tt.Image())
		f.Close()
		if err == nil {
			*coverage = append(*coverage, map[string]any{"file": name + ".png", "textCount": len(tt.Texts()), "design": true})
		}
		return err
	}
	defer func() { deckPos, deckTarget, deckSpringIdle = 0, 0, true }()
	for _, dark := range []bool{false, true} {
		a := fixture(dark)
		a.overview.loaded, a.overview.config = true, prefs.DefaultOverview()
		a.overview.config.View = "scroll"
		deckPos, deckTarget, deckSpringIdle = 0, 0, true
		tt := ui.NewTester(func(c *ui.Context) { deckLastInput = c.Now(); a.View(c) }, WindowW, WindowH)
		var bounds ui.Rect
		for i, key := range a.scrollPages() {
			a.deckTo(float64(i)*deckStep, true)
			tt.Frame()
			r, ok := tt.Find("浏览" + overviewName(key) + "卡片")
			if !ok {
				return fmt.Errorf("missing scroll page %s", key)
			}
			if i == 0 {
				bounds = r
			} else if r.W != bounds.W || r.H != bounds.H || r.X != bounds.X || r.Y != bounds.Y {
				return fmt.Errorf("scroll page %s changed dimensions: %v != %v", key, r, bounds)
			}
			if err := capture(fmt.Sprintf("scroll-%s-%t", key, dark), tt); err != nil {
				return err
			}
		}
		for _, phase := range []float64{.25, .5, .75} {
			a.deckTo(phase*deckStep, true)
			tt.Frame()
			if err := capture(fmt.Sprintf("scroll-transition-%.2f-%t", phase, dark), tt); err != nil {
				return err
			}
		}
		a = fixture(dark)
		a.Page, a.SettingsTab = PageSettings, TabLayoutEditor
		tt = ui.NewTester(a.View, WindowW, WindowH)
		tt.Frame()
		if a.musicEditor == nil || a.musicEditor.Error != "" {
			return fmt.Errorf("editor initialization failed")
		}
		for tab := 0; tab < 5; tab++ {
			a.musicEditor.Tab = tab
			a.musicEditor.Scroll = ui.ScrollState{}
			tt.Frame()
			tt.Frame()
			left, lok := tt.Find("布局预览面板")
			right, rok := tt.Find("布局调整面板")
			if !lok || !rok || left.Y != right.Y || left.H != right.H || left.Y+left.H > WindowH-24 {
				return fmt.Errorf("editor panes escaped the viewport: %v %v", left, right)
			}
			if tab == 0 {
				for _, label := range []string{"布局横向", "布局纵向", "布局宽度"} {
					r, ok := tt.Find(label)
					if !ok || r.Y+r.H > WindowH-24 {
						return fmt.Errorf("editor slider off screen: %s %v", label, r)
					}
				}
			}
			if err := capture(fmt.Sprintf("layout-editor-tab-%d-%t", tab, dark), tt); err != nil {
				return err
			}
		}
		a = fixture(dark)
		a.StartSignature()
		tt = ui.NewTester(a.View, WindowW, WindowH)
		previous := 0.
		for _, seconds := range []float64{.18, .48, 1.10, 1.38, 1.55} {
			tt.Advance(time.Duration((seconds - previous) * float64(time.Second)))
			tt.Frame()
			previous = seconds
			if err := capture(fmt.Sprintf("opening-%.2f-%t", seconds, dark), tt); err != nil {
				return err
			}
		}
	}
	return nil
}
