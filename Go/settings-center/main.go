// DotIsle control center, MyGo Native UI front-end (formal instruction: native UI
// only, no WebView). The Go layer renders and routes; every semantic goes through
// the island's settings pipe or the local JSON stores, exactly like the Rust UI.
package main

import (
	"flag"
	"log"

	"dotisle/settings/island"
	"dotisle/settings/prefs"
	"dotisle/settings/ui"
	"github.com/egoist/mygo"
	mygoui "github.com/egoist/mygo/ui"
)

func main() {
	dataFlag := flag.String("data-dir", "", "island data directory")
	flag.Parse()

	data := island.DataDir(*dataFlag)
	if !acquireSingleInstance(data) {
		return // a center window is already up; the island wakes it via pendingPage
	}
	prefsPath, _, _ := prefs.Paths(data)
	p, err := prefs.Load(prefsPath)
	if err != nil {
		log.Printf("preferences: %v", err)
		p = prefs.Default()
	}

	app := &ui.App{Prefs: p, Data: data, SettingsTab: ui.TabRoot}
	app.LoadStartupSettings()

	mygo.App.WhenReady(func() {
		ui.RegisterFonts()
		win := mygo.NewWindow(mygo.WindowOptions{
			Title:             "点屿 · 控制中心",
			Width:             ui.WindowW,
			Height:            ui.WindowH,
			MinWidth:          ui.WindowW,
			MinHeight:         ui.WindowH,
			MaxWidth:          ui.WindowW,
			MaxHeight:         ui.WindowH,
			DisableResize:     true,
			DisableMaximize:   true,
			DisableFullScreen: true,
			Frameless:         true,
			Vibrancy:          mygo.VibrancyMica,
			Content:           mygoui.View(app.View),
		})
		if err := win.SetIcon(ui.AppIcon); err != nil {
			log.Printf("window icon: %v", err)
		}
		app.Win = win
		app.StartSignature()
		win.SetBackgroundColor("transparent")

		// The island pump: every snapshot lands on the main thread through Update.
		go func() {
			for st := range island.Run(data) {
				island.PumpTrace("update enter err=%q", st.Err)
				win.Update(func() {
					if st.Err != "" {
						app.ApplyPacket(nil, false, st.Err)
					} else {
						app.ApplyPacket(st.Raw, true, "")
					}
					island.PumpTrace("update applied")
				})
			}
		}()
	})
	if err := mygo.App.Run(); err != nil {
		log.Fatal(err)
	}
}
