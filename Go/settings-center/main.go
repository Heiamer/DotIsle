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

	"dotisle/settings/consent"
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
func trimWhenHidden(win *mygo.Window, app *ui.App) {
	win.OnShow(func() { centerHidden.Store(false); app.BeginCenterVisit() })
	win.OnHide(func() {
		centerHidden.Store(true)
		app.EndCenterVisit()
		go func() {
			time.Sleep(3 * time.Second)
			if centerHidden.Load() {
				debug.FreeOSMemory()
			}
		}()
	})
}

func main() {
	startupMark("main")
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
	if !consent.Ensure(data) {
		return
	}
	prefsPath, _, _ := prefs.Paths(data)
	p, err := prefs.Load(prefsPath)
	if err != nil {
		log.Printf("preferences: %v", err)
		p = prefs.Default()
	}

	app := &ui.App{Prefs: p, Data: data, SettingsTab: ui.TabRoot, EnableCityPrompt: true}
	if !*warm {
		app.BeginCenterVisit()
	}
	app.LoadStartupSettings()
	host := desktop.New(data)
	defer host.Stop()
	// Connect while the native window and fonts initialize. The buffered pump
	// retains its first snapshot until the UI thread can apply it.
	snapshots := island.Run(data)
	startupMark("preferences")

	mygo.App.WhenReady(func() {
		startupMark("ready")
		ui.RegisterFonts()
		startupMark("fonts")
		if !*warm {
			app.StartSignature()
		}
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
			BackgroundColor:   "transparent",
			Content:           mygoui.View(app.View),
		})
		win.OnReadyToShow(func() {
			startupMark("first-frame")
			if err := win.SetIcon(ui.AppIcon); err != nil {
				log.Printf("window icon: %v", err)
			}
		})
		startupMark("window")
		app.Win = win
		// Window creation also overlaps the pipe read. Install any ready packet
		// before returning to the first frame, without waiting for a slow backend.
		var initial *island.State
		select {
		case initial = <-snapshots:
			if initial != nil {
				if initial.Exited {
					// No matching native process owns this center. Quit may arrive
					// before the native event loop starts, so end this stray host now.
					host.Stop()
					os.Exit(0)
				}
				if initial.Err == "" {
					app.ApplyPacket(initial.Raw, true, "")
					startupMark("first-state")
				} else {
					app.ApplyPacket(nil, false, initial.Err)
				}
			}
		default:
		}
		trimWhenHidden(win, app)
		watchCenterShow(data, func() {
			win.Update(func() {
				if win.IsMinimized() {
					win.Restore()
				}
				win.Show()
			})
		})
		startupMark("watch")

		// The island pump: every snapshot lands on the main thread through Update.
		go func() {
			firstState := initial == nil || initial.Err != ""
			if initial != nil && initial.Err == "" {
				win.Update(func() {
					if err := host.Apply(initial.Raw); err != nil {
						log.Printf("desktop bridge: %v", err)
						app.ReportError(err.Error())
					}
				})
			}
			for st := range snapshots {
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
						if firstState {
							startupMark("first-state")
							firstState = false
						}
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
