// DotIsle control center, MyGo Native UI front-end (formal instruction: native UI
// only, no WebView). The Go layer renders and routes; every semantic goes through
// the island's settings pipe or the local JSON stores, exactly like the Rust UI.
package main

import (
	"flag"
	"log"
	"os"
	"runtime/debug"

	"dotisle/settings/consent"
	"dotisle/settings/desktop"
	"dotisle/settings/island"
	"dotisle/settings/lyrics"
	"dotisle/settings/prefs"
	"dotisle/settings/ui"
	"github.com/egoist/mygo"
)

// tuneGC keeps the resident process small: its live heap is a few megabytes, but
// the default GOGC=100 keeps a target of twice that plus the churn of a frame
// build every 90 ms while music plays. A lower percentage pulls the target close
// to live data, and the soft limit only bites on a pathological page; the runtime
// still exceeds it rather than fail an allocation.
func tuneGC() {
	debug.SetGCPercent(50)
	debug.SetMemoryLimit(96 << 20)
}

func main() {
	if len(os.Args) == 2 && os.Args[1] == "--lyrics-worker" {
		if lyrics.Serve(os.Stdin, os.Stdout) != nil {
			os.Exit(1)
		}
		return
	}
	startupMark("main")
	tuneGC()
	dataFlag := flag.String("data-dir", "", "island data directory")
	warm := flag.Bool("warm", false, "start desktop widgets without showing the center")
	verify := flag.String("verify", "", "capture the packaged control center and desktop renderer")
	health := flag.String("module-health", "", "check embedded renderer and theme interfaces")
	checkRenderer := flag.Bool("check-renderer-build", false, "require the production MyGo hardware renderer patch")
	flag.Parse()
	if *checkRenderer {
		if !ui.HasHardwareRenderer() {
			log.Print("missing production MyGo renderer patch")
			os.Exit(1)
		}
		os.Stdout.WriteString("DOTISLE_HARDWARE_RENDERER=1\n")
		return
	}
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
	app.LoadStartupSettings()
	defer app.StopMail()

	host := desktop.New(data)
	defer host.Stop()
	center := &centerWindow{app: app, allowIdleExit: os.Getenv("DOTISLE_CENTER_IDLE_EXIT") == "1", released: *warm}
	// Connect while the native window and fonts initialize. The buffered pump
	// retains its first snapshot until the UI thread can apply it.
	snapshots := island.Run(data)
	startupMark("preferences")

	mygo.App.WhenReady(func() {
		startupMark("ready")
		app.ShowCenter = center.show
		app.StartMail()
		// Closing the last MyGo window must not stop the embedded desktop host.
		mygo.App.OnWindowAllClosed(func() {})
		if !*warm {
			app.StartSignature()
			center.show()
		}
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
		watchCenterShow(data, func() { mygo.RunOnMain(center.show) })
		startupMark("watch")

		// The pump and desktop host keep running even when there is no window.
		go func() {
			firstState := initial == nil || initial.Err != ""
			if initial != nil && initial.Err == "" {
				mygo.RunOnMain(func() {
					if err := host.Apply(initial.Raw); err != nil {
						log.Printf("desktop bridge: %v", err)
						app.ReportError(err.Error())
					}
				})
			}
			for st := range snapshots {
				island.PumpTrace("update enter err=%q", st.Err)
				mygo.RunOnMain(func() {
					if center.idleExit {
						return
					}
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
					center.tryIdleExit()
				})
			}
		}()
	})
	if err := mygo.App.Run(); err != nil {
		log.Fatal(err)
	}
	if center.idleExit {
		host.Stop()
		os.Exit(35) // lyric-native replays an open request that raced this exit.
	}
}
