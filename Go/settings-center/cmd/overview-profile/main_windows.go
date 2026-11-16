// Native D3D soak test of the production overview, isolated from user data.
package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"runtime"
	"runtime/debug"
	"runtime/pprof"
	"sort"
	"time"
	"unsafe"

	"dotisle/settings/prefs"
	center "dotisle/settings/ui"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
	"golang.org/x/sys/windows"
)

type memoryCounters struct {
	Size, Faults                                                                                    uint32
	PeakWorking, Working, PeakPaged, Paged, PeakNonpaged, Nonpaged, Pagefile, PeakPagefile, Private uintptr
}

type sample struct {
	Frame                                                            int     `json:"frame"`
	Phase                                                            string  `json:"phase"`
	Seconds                                                          float64 `json:"seconds"`
	Heap, HeapInuse, HeapSys, Released, TotalAlloc, Private, Working uint64
	Collections                                                      uint32
	PauseNs                                                          uint64
}

func main() {
	output := flag.String("output", "", "report JSON and adjacent heap profile")
	frames := flag.Int("frames", 3600, "unique native view frames to render")
	pages := flag.Int("pages", 5, "number of overview pages, 1 to 5")
	flag.Parse()
	if *output == "" || *frames < 120 || *pages < 1 || *pages > 5 {
		log.Fatal("invalid profile arguments")
	}
	debug.SetGCPercent(50)
	debug.SetMemoryLimit(192 << 20)
	data, err := os.MkdirTemp("", "dotisle-overview-profile-")
	if err != nil {
		log.Fatal(err)
	}
	defer os.RemoveAll(data)
	os.Setenv("LYRICISLAND_TEST_PIPE", fmt.Sprintf("LyricIsland.Settings.test.profile.%d", os.Getpid()))
	overview := prefs.DefaultOverview()
	overview.View = "scroll"
	overview.ScrollPages = overview.ScrollPages[:*pages]
	if err := prefs.SaveOverview(filepath.Join(data, "center-overview.json"), overview); err != nil {
		log.Fatal(err)
	}
	state := map[string]any{"pendingPage": "", "settings": map[string]any{"SettingsTheme": "Light", "StartupSignatureEnabled": false, "Animation": true, "ControlCenterDynamicColorEnabled": false}}
	var todos, notices []any
	for i := 0; i < 40; i++ {
		todos = append(todos, map[string]any{"id": fmt.Sprint(i), "title": "持续滚动测试待办", "date": "2026-11-16", "completed": i%2 == 0})
	}
	for i := 0; i < 100; i++ {
		notices = append(notices, map[string]any{"id": fmt.Sprint(i), "title": "持续滚动测试通知", "body": "固定合成内容，不读取用户通知", "appName": "点屿", "kind": "notice", "date": "2026-11-16", "clock": "09:00"})
	}
	state["todos"], state["notificationHistory"] = todos, notices
	a := &center.App{Prefs: prefs.Default(), Data: data, State: state, SettingsTab: center.TabRoot}
	getMemory := windows.NewLazySystemDLL("psapi.dll").NewProc("GetProcessMemoryInfo")
	start := time.Now()
	var samples []sample
	var buildUs []float64
	var frameMs []float64
	var previous time.Time
	frame := 0
	phase := 0
	phaseAt := time.Time{}
	capture := func(label string) {
		var stats runtime.MemStats
		runtime.ReadMemStats(&stats)
		counter := memoryCounters{}
		counter.Size = uint32(unsafe.Sizeof(counter))
		ok, _, callErr := getMemory.Call(uintptr(windows.CurrentProcess()), uintptr(unsafe.Pointer(&counter)), uintptr(counter.Size))
		if ok == 0 {
			log.Fatal(callErr)
		}
		s := sample{Frame: frame, Phase: label, Seconds: time.Since(start).Seconds(), Heap: stats.HeapAlloc, HeapInuse: stats.HeapInuse, HeapSys: stats.HeapSys, Released: stats.HeapReleased, TotalAlloc: stats.TotalAlloc, Private: uint64(counter.Private), Working: uint64(counter.Working), Collections: stats.NumGC, PauseNs: stats.PauseTotalNs}
		samples = append(samples, s)
		log.Printf("frame=%d phase=%s private=%.1fMiB working=%.1fMiB heap=%.1fMiB gc=%d", frame, label, float64(s.Private)/(1<<20), float64(s.Working)/(1<<20), float64(s.Heap)/(1<<20), s.Collections)
	}
	mygo.App.WhenReady(func() {
		capture("ready")
		center.RegisterFonts()
		capture("fonts")
		win := mygo.NewWindow(mygo.WindowOptions{Title: "点屿 · 概览内存压力测试", Width: center.WindowW, Height: center.WindowH, MinWidth: center.WindowW, MinHeight: center.WindowH, MaxWidth: center.WindowW, MaxHeight: center.WindowH, DisableResize: true, DisableMaximize: true, Frameless: true, Vibrancy: mygo.VibrancyMica, Content: ui.View(func(c *ui.Context) {
			unique := !c.Now().Equal(previous)
			if unique && phase == 0 {
				if !previous.IsZero() {
					frameMs = append(frameMs, float64(c.Now().Sub(previous).Microseconds())/1000)
				}
				previous = c.Now()
				frame++
				// Exercise hundreds of ring seams in both directions.
				progress := float64(frame) * .2
				if frame > *frames/2 {
					progress = float64(*frames-frame) * .2
				}
				a.PreviewOverviewProgress(progress, c.Now())
			}
			at := time.Now()
			a.View(c)
			if unique && phase == 0 {
				buildUs = append(buildUs, float64(time.Since(at).Microseconds()))
			}
			if phase == 0 {
				if unique && frame%120 == 0 {
					capture("scroll")
				}
				if frame >= *frames {
					capture("scroll-end")
					phase = 1
					phaseAt = c.Now()
				}
				c.AnimationFrame()
			} else if c.Now().Sub(phaseAt) < 3*time.Second {
				c.After(100 * time.Millisecond)
			} else if phase == 1 {
				capture("idle-natural")
				// Diagnostic only: distinguish live objects from delayed heap release.
				debug.FreeOSMemory()
				capture("idle-forced-gc")
				phase = 2
				phaseAt = c.Now()
				c.AnimationFrame()
			} else {
				capture("final")
				mygo.App.Quit()
			}
		})})
		a.Win = win
		win.SetBackgroundColor("transparent")
	})
	if err := mygo.App.Run(); err != nil {
		log.Fatal(err)
	}
	sort.Float64s(buildUs)
	sort.Float64s(frameMs)
	percentile := func(v []float64, p float64) float64 {
		if len(v) == 0 {
			return 0
		}
		return v[min(int(float64(len(v)-1)*p), len(v)-1)]
	}
	report := map[string]any{"success": frame >= *frames, "frames": frame, "pages": *pages, "seconds": time.Since(start).Seconds(), "buildMedianUs": percentile(buildUs, .5), "buildP95Us": percentile(buildUs, .95), "frameMedianMs": percentile(frameMs, .5), "frameP95Ms": percentile(frameMs, .95), "samples": samples, "synthetic": true, "renderer": "native MyGo D3D11"}
	if err := os.MkdirAll(filepath.Dir(*output), 0755); err != nil {
		log.Fatal(err)
	}
	raw, _ := json.MarshalIndent(report, "", "  ")
	if err := os.WriteFile(*output, raw, 0644); err != nil {
		log.Fatal(err)
	}
	if f, err := os.Create(*output + ".heap"); err == nil {
		_ = pprof.WriteHeapProfile(f)
		_ = f.Close()
	}
}
