// DotIsle control center, MyGo Native UI front-end (formal instruction: native UI
// only, no WebView). The Go layer renders and routes; every semantic goes through
// the island's settings pipe or the local JSON stores, exactly like the Rust UI.
package main

import (
	"flag"
	"log"
	"os"
	"runtime/debug"
	"sync/atomic"
	"time"

	"dotisle/settings/desktop"
	"dotisle/settings/island"
	"dotisle/settings/prefs"
	"dotisle/settings/ui"
	"github.com/egoist/mygo"
	mygoui "github.com/egoist/mygo/ui"
)

// centerHidden tells that the window was closed to the tray, where the process
// stays resident for the island pump and the desktop widgets.
var centerHidden atomic.Bool

// tuneGC keeps the resident process small: its live heap is a few megabytes, but
// the default GOGC=100 keeps a target of twice that plus the churn of a frame
// build every 90 ms while music plays. A lower percentage pulls the target close
// to live data, and the soft limit only bites on a pathological page; the runtime
// still exceeds it rather than fail an allocation.
func tuneGC() {
	debug.SetGCPercent(50)
	debug.SetMemoryLimit(192 << 20)
}

// trimWhenHidden returns heap pages to the OS once the window has stayed hidden,
// instead of waiting for the runtime's five-minute scavenger. Showing the window
// in the meantime cancels it, so a user reopening never pays for the trim.
func trimWhenHidden(win *mygo.Window) {
	win.OnShow(func() { centerHidden.Store(false) })
	win.OnHide(func() {
		centerHidden.Store(true)
		go func() {
			time.Sleep(3 * time.Second)
			if centerHidden.Load() {
				debug.FreeOSMemory()
			}
		}()
	})
}

func main() {
	tuneGC()
	dataFlag := flag.String("data-dir", "", "island data directory")
	warm := flag.Bool("warm", false, "start desktop widgets without showing the center")
	verify := flag.String("verify", "", "capture the packaged control center and desktop renderer")
	health := flag.String("module-health", "", "check embedded renderer and theme interfaces")
	flag.Parse()
	if *verify != "" || *health != "" {
		output := *health
		if *verify != "" {
			output = *verify
		}
		if err := verifyCenter(output, *verify != ""); err != nil {
			log.Print(err)
			os.Exit(1)
		}
		return
	}

	data := island.DataDir(*dataFlag)
	if !acquireSingleInstance(data) {
		// A center for this data directory is already resident: raise it instead of
		// leaving the user with a click that appears to do nothing.
		signalCenterShow(data)
		return
	}
	prefsPath, _, _ := prefs.Paths(data)
	p, err := prefs.Load(prefsPath)
	if err != nil {
		log.Printf("preferences: %v", err)
		p = prefs.Default()
	}

	app := &ui.App{Prefs: p, Data: data, SettingsTab: ui.TabRoot}
	app.LoadStartupSettings()
	host := desktop.New(data)
	defer host.Stop()

	mygo.App.WhenReady(func() {
		ui.RegisterFonts()
		win := mygo.NewWindow(mygo.WindowOptions{
			Title:             "点屿 · 控制中心",
			Hidden:            *warm,
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
		trimWhenHidden(win)
		if !*warm {
			app.StartSignature()
		}
		win.SetBackgroundColor("transparent")
		watchCenterShow(data, func() {
			win.Update(func() {
				if win.IsMinimized() {
					win.Restore()
				}
				win.Show()
			})
		})

		// The island pump: every snapshot lands on the main thread through Update.
		go func() {
			for st := range island.Run(data) {
				island.PumpTrace("update enter err=%q", st.Err)
				win.Update(func() {
					if st.Exited {
						host.Stop()
						mygo.App.Quit()
						return
					}
					if st.Err != "" {
						app.ApplyPacket(nil, false, st.Err)
					} else {
						app.ApplyPacket(st.Raw, true, "")
						if err := host.Apply(st.Raw); err != nil {
							log.Printf("desktop bridge: %v", err)
							app.ReportError(err.Error())
						}
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
