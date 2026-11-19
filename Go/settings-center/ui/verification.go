package ui

import (
	"encoding/json"
	"fmt"
	"image"
	"image/color"
	"image/png"
	"os"
	"path/filepath"

	"dotisle/settings/prefs"
	"github.com/egoist/mygo/ui"
)

// Verify renders the shipped views without opening a window or contacting a core.
// The same executable used by users runs this check after packaging.
func Verify(output string) (err error) {
	if err = os.MkdirAll(output, 0755); err != nil {
		return err
	}
	RegisterFonts()
	data, err := os.MkdirTemp(output, "fixture-")
	if err != nil {
		return err
	}
	defer os.RemoveAll(data)
	defer func() {
		if r := recover(); r != nil {
			err = fmt.Errorf("control center verification: %v", r)
		}
		report := map[string]any{"success": err == nil, "renderer": "MyGo software / production App.View"}
		if err != nil {
			report["error"] = err.Error()
		}
		raw, _ := json.MarshalIndent(report, "", "  ")
		if e := os.WriteFile(filepath.Join(output, "report.json"), raw, 0644); err == nil {
			err = e
		}
	}()
	cover := image.NewNRGBA(image.Rect(0, 0, 96, 96))
	for y := 0; y < 96; y++ {
		for x := 0; x < 96; x++ {
			cover.SetNRGBA(x, y, color.NRGBA{uint8(30 + x), uint8(60 + y), 130, 255})
		}
	}
	coverPath := filepath.Join(data, "cover.png")
	f, err := os.Create(coverPath)
	if err != nil {
		return err
	}
	err = png.Encode(f, cover)
	f.Close()
	if err != nil {
		return err
	}
	type route struct {
		name   string
		page   int
		tab    uint8
		scroll bool
	}
	routes := []route{}
	for page, name := range Pages {
		routes = append(routes, route{name, page, TabRoot, false})
	}
	for _, tab := range []uint8{0, 1, 2, 3, 4, 5, 6, 7, 8, 128, 129, 130, 131, 132, 133, 134, 135, 240, 241, 242, 243, 244, 245} {
		routes = append(routes, route{fmt.Sprintf("settings-%d", tab), PageSettings, tab, false})
	}
	routes = append(routes, route{"overview-scroll", PageHome, TabRoot, true})
	routes = append(routes, route{"gesture-tutorial", PageSettings, 130, false}, route{"gesture-recovery", PageSettings, 130, false}, route{"motion-curve-editor", PageSettings, 131, false})
	coverage := []map[string]any{}
	for _, dark := range []bool{false, true} {
		for i, route := range routes {
			a := &App{Prefs: prefs.Default(), Data: filepath.Join(data, fmt.Sprint(i, dark)), Page: route.page, SettingsTab: route.tab,
				State: map[string]any{"pendingPage": "", "title": "此刻，正在聆听", "artist": "验收示例", "position": float64(66), "duration": float64(240),
					"settings":     map[string]any{"SettingsTheme": map[bool]string{false: "Light", true: "Dark"}[dark]},
					"musicPreview": map[string]any{"coverPath": coverPath, "lyricFrame": map[string]any{"currentLine": "把这一刻留给自己"}}}}
			a.Prefs.Dark = dark
			if route.tab >= TabIslandFirst && route.tab <= TabIslandLast {
				a.IslandTab = route.tab - TabIslandFirst
			}
			a.overview.loaded = true
			a.overview.config = prefs.DefaultOverview()
			if route.scroll {
				a.overview.config.View = "scroll"
				deckPos, deckTarget, deckSpringIdle = 0, 0, true
			}
			if route.name == "gesture-tutorial" || route.name == "gesture-recovery" {
				a.startGestureTutorial()
				if route.name == "gesture-recovery" {
					a.gestureTutorial.step, a.gestureTutorial.index, a.gestureTutorial.total = "recover", 4, 5
				}
			}
			view := a.View
			if route.name == "motion-curve-editor" {
				a.Palette = BuildPalette(&a.Prefs, a.settings(), a.State)
				view = func(c *ui.Context) {
					ui.Box(c).Absolute().Fill().Background(Col(a.Palette.Scheme.Surface))
					a.motionCurveEditor(c, &a.Palette, 32, 32, WindowW-64)
				}
			}
			tt := ui.NewTester(view, WindowW, WindowH)
			tt.Frame()
			tt.Frame()
			if len(tt.Texts()) < 5 {
				return fmt.Errorf("empty route %s", route.name)
			}
			if a.musicEditor != nil && a.musicEditor.Error != "" {
				return fmt.Errorf("layout editor: %s", a.musicEditor.Error)
			}
			name := fmt.Sprintf("%s-%t.png", route.name, dark)
			f, e := os.Create(filepath.Join(output, name))
			if e != nil {
				return e
			}
			e = png.Encode(f, tt.Image())
			f.Close()
			if e != nil {
				return e
			}
			coverage = append(coverage, map[string]any{"file": name, "page": route.page, "tab": route.tab, "dark": dark, "textCount": len(tt.Texts())})
		}
	}
	if err := verifyDesign(output, data, coverPath, &coverage); err != nil {
		return err
	}
	raw, err := json.MarshalIndent(coverage, "", "  ")
	if err != nil {
		return err
	}
	return os.WriteFile(filepath.Join(output, "coverage.json"), raw, 0644)
}
