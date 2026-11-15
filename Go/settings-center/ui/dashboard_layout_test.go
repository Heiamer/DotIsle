package ui

import (
	"dotisle/settings/prefs"
	"github.com/egoist/mygo/ui"
	"testing"
)

func TestOverviewSplitsMusicCalendarAndScreenTimeWithoutOverlap(t *testing.T) {
	r := overviewLayoutSized(prefs.DefaultOverview().Cards, nil, 0, 0, 1136)
	if len(r) != 6 || r[0].key != "music" || r[1].key != "widget:calendar" || r[2].key != "widget:screenTime" {
		t.Fatal("unexpected default cards", r)
	}
	if r[0].y != r[1].y || r[1].y != r[2].y || r[0].w != r[1].w || r[0].w*2+16 != r[2].w {
		t.Fatal("first row proportions changed", r)
	}
	for i, a := range r {
		for _, b := range r[i+1:] {
			if a.x < b.x+b.w && a.x+a.w > b.x && a.y < b.y+b.h && a.y+a.h > b.y {
				t.Fatal("cards overlap", a, b)
			}
		}
	}
	a := newTestApp(t)
	m := a.widgetSystemMetrics()
	if m.hasCPU || m.hasMemory || m.hasBattery {
		t.Fatal("missing telemetry became a fake reading", m)
	}
	a.overview.config.Cards = []string{"widget:system"}
	a.overview.loaded = true
	a.State["systemMetrics"] = map[string]any{"cpuPercent": 23.0, "memoryPercent": 47.0, "memoryUsedGiB": 7.5, "memoryTotalGiB": 16.0}
	tt := ui.NewTester(a.View, 1200, 800)
	if !tt.HasText("23%") || !tt.HasText("47%") || !tt.HasText("7.5 / 16.0 GiB") {
		t.Fatal("live metrics missing", tt.Texts())
	}
}
