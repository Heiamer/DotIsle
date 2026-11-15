package ui

import (
	"github.com/egoist/mygo/ui"
	"os"
	"path/filepath"
	"testing"
	"time"
	_ "time/tzdata"
)

func TestOpeningHonorsSavedSwitchAndLatestSnapshot(t *testing.T) {
	a := newTestApp(t)
	if err := os.WriteFile(filepath.Join(a.Data, "settings.json"), []byte(`{"StartupSignatureEnabled":false}`), 0600); err != nil {
		t.Fatal(err)
	}
	a.StartSignature()
	tt := ui.NewTester(a.View, 1200, 800)
	if _, ok := tt.Find("跳过开屏动画"); ok {
		t.Fatal("disabled opening shown before the first snapshot")
	}
	a.settings()["StartupSignatureEnabled"] = true
	a.StartSignature()
	tt.Frame()
	if _, ok := tt.Find("跳过开屏动画"); !ok {
		t.Fatal("snapshot did not override saved switch")
	}
	a.settings()["StartupSignatureEnabled"] = false
	tt.Frame()
	if !a.signatureStart.IsZero() {
		t.Fatal("disabled opening remained active")
	}
}

func TestCalendarDateDistanceIgnoresTimeAndDST(t *testing.T) {
	for _, zone := range []string{"Asia/Shanghai", "America/New_York"} {
		loc, err := time.LoadLocation(zone)
		if err != nil {
			t.Fatal(err)
		}
		for _, hour := range []int{0, 12, 23} {
			_, days, err := calendarDateDistance("2026-03-09", time.Date(2026, 3, 8, hour, 45, 0, 0, loc))
			if err != nil || days != 1 {
				t.Fatalf("%s %d: %d %v", zone, hour, days, err)
			}
		}
	}
}

func TestLeavingOverviewCancelsResize(t *testing.T) {
	for _, leave := range []func(*App){func(a *App) { a.setOverviewView("scroll") }, func(a *App) { a.openOverviewCatalog("cards") }, func(a *App) { a.navigate(PageTodos) }} {
		a := newTestApp(t)
		a.loadOverview()
		a.overview.edit = true
		a.overview.resizing = "music"
		a.overview.dragging = "clock"
		leave(a)
		if a.overview.resizing != "" || a.overview.dragging != "" || len(a.overview.config.Sizes) != 0 {
			t.Fatal("leaving retained or committed transient resize")
		}
	}
}

func TestNegativeWorkflowIndicesAreRejected(t *testing.T) {
	old := workflowUI
	defer func() { workflowUI = old }()
	workflowUI.stages = []wfStageDraft{{"A", 5}, {"B", 5}, {"C", 5}}
	if wfDeleteStage(-1) == nil || wfMoveStage(-1, 1) == nil || wfNudgeStage(-1, 1) == nil {
		t.Fatal("invalid index accepted")
	}
	timer := goTimer{Workflow: &goWorkflow{StageIndex: -1}}
	if timer.stage() != nil {
		t.Fatal("negative stage returned")
	}
}

func TestCenterRouteOpensExactEditorAndDoesNotReopenSnapshot(t *testing.T) {
	a := newTestApp(t)
	a.State["desktopWidgets"] = []any{map[string]any{"id": "note-1", "kind": "note", "config": map[string]any{"title": "标题", "text": "内容", "city": "保留"}}}
	a.State["pendingPage"] = "widgets"
	a.State["pendingRequestId"] = float64(1)
	a.State["pendingCenterAction"] = map[string]any{"action": "widgetConfigure", "id": "note-1"}
	a.applyPendingPage()
	if a.widgetEditor == nil || a.widgetEditor.value != "内容" {
		t.Fatal("exact widget editor not opened")
	}
	a.widgetEditor = nil
	a.applyPendingPage()
	if a.widgetEditor != nil {
		t.Fatal("same packet reopened dismissed editor")
	}
	a.State["pendingRequestId"] = float64(2)
	a.applyPendingPage()
	if a.widgetEditor == nil {
		t.Fatal("repeated request on same page ignored")
	}
}

func TestMicaFallbackFollowsAccessibility(t *testing.T) {
	active := false
	tt := ui.NewTester(func(c *ui.Context) { active = micaActive(c) }, 200, 120)
	tt.SetVibrancy(true)
	tt.Frame()
	if !active {
		t.Fatal("material unexpectedly disabled")
	}
	for _, p := range []ui.Preferences{{ReduceTransparency: true}, {HighContrast: true}} {
		tt.SetPreferences(p)
		tt.Frame()
		if active {
			t.Fatal("material ignored accessibility preference")
		}
	}
}
