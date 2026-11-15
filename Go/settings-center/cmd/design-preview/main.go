// Isolated native D3D preview of the production view. No core is started.
package main

import (
	"encoding/json"
	"flag"
	"log"
	"math"
	"os"
	"path/filepath"
	"strconv"
	"time"

	"dotisle/settings/prefs"
	center "dotisle/settings/ui"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

func main() {
	audit := flag.String("frame-audit", "", "isolated motion audit: menu or opening")
	view := flag.String("overview", "cards", "initial isolated overview: cards or scroll")
	log.SetFlags(log.Ldate | log.Ltime | log.Lmicroseconds)
	snapshot := flag.String("snapshot", "", "isolated core JSON fixture")
	flag.Parse()
	data, err := os.MkdirTemp("", "dotisle-design-preview-")
	if err != nil {
		log.Fatal(err)
	}
	defer os.RemoveAll(data)
	os.Setenv("LYRICISLAND_TEST_PIPE", "LyricIsland.Settings.test.design-preview."+strconv.Itoa(os.Getpid()))
	state := map[string]any{"settings": map[string]any{"SettingsTheme": "Light"}, "pendingPage": ""}
	if *snapshot != "" {
		raw, err := os.ReadFile(*snapshot)
		if err != nil {
			log.Fatal(err)
		}
		if err = json.Unmarshal(raw, &state); err != nil {
			log.Fatal(err)
		}
	}
	state["pendingPage"] = ""
	a := &center.App{Prefs: prefs.Default(), Data: filepath.Clean(data), State: state, SettingsTab: center.TabRoot}
	if *view == "scroll" || *audit == "deck" || *audit == "deck-drag" {
		overview := prefs.DefaultOverview()
		overview.View = "scroll"
		if err := prefs.SaveOverview(filepath.Join(data, "center-overview.json"), overview); err != nil {
			log.Fatal(err)
		}
	}
	auditStart := time.Time{}
	auditSlot := -1
	mygo.App.WhenReady(func() {
		center.RegisterFonts()
		win := mygo.NewWindow(mygo.WindowOptions{Title: "点屿 · 控制中心预览", Width: center.WindowW, Height: center.WindowH, MinWidth: center.WindowW, MinHeight: center.WindowH, MaxWidth: center.WindowW, MaxHeight: center.WindowH, DisableResize: true, DisableMaximize: true, DisableFullScreen: true, Frameless: true, Vibrancy: mygo.VibrancyMica, Content: ui.View(func(c *ui.Context) {
			if *audit != "" {
				if auditStart.IsZero() {
					auditStart = c.Now()
					log.Printf("audit scene=%s start", *audit)
				}
				elapsed := c.Now().Sub(auditStart)
				if *audit == "deck-drag" && elapsed < 6*time.Second {
					// Continuous changing frames; no dependency on the OS motion switch.
					progress := 2.5 - 2.5*math.Cos(elapsed.Seconds()*math.Pi/3)
					a.PreviewOverviewProgress(progress, c.Now())
				}
				slot := int(elapsed / (400 * time.Millisecond))
				if *audit == "menu" && slot != auditSlot && elapsed < 6*time.Second {
					a.ToggleNavigation()
					log.Printf("audit menu slot=%d", slot)
					auditSlot = slot
				}
				if *audit == "deck" && slot != auditSlot && elapsed < 6*time.Second {
					a.StepOverview(1)
					log.Printf("audit deck slot=%d systemReducedMotion=%t appReducedMotion=%t", slot, c.Preferences().ReduceMotion, a.Prefs.ReducedMotion)
					auditSlot = slot
				}
				a.View(c)
				if elapsed < 6*time.Second {
					c.AnimationFrame()
				}
			} else {
				a.View(c)
			}
		})})
		if err := win.SetIcon(center.AppIcon); err != nil {
			log.Printf("window icon: %v", err)
		}
		a.Win = win
		if os.Getenv("DOTISLE_PREVIEW_OPENING") != "" || *audit == "opening" {
			a.StartSignature()
		}
		win.SetBackgroundColor("transparent")
	})
	if err := mygo.App.Run(); err != nil {
		log.Fatal(err)
	}
}
