package ui

import (
	"fmt"
	"math"
	"testing"
	"time"

	"dotisle/settings/prefs"
	"github.com/egoist/mygo/ui"
)

func TestOverviewCatalogDrawer(t *testing.T) {
	for _, width := range []int{900, 1200} {
		a := newTestApp(t)
		tt := ui.NewTester(a.View, width, 680)
		r, _ := tt.Find("自定义概览组件")
		tt.ClickAt(r.X+r.W/2, r.Y+r.H/2)
		tt.Frame()
		panel, ok := tt.Find("概览组件预览栏")
		if !ok || panel.X < float32(width)/3 || panel.Y != 0 || panel.H != 680 {
			t.Fatalf("%d: not a right side sheet: %+v", width, panel)
		}
		r, ok = tt.Find("显示音乐")
		if !ok {
			t.Fatal("missing preview")
		}
		tt.ClickAt(r.X+r.W/2, r.Y+r.H/2)
		tt.Frame()
		for _, key := range a.overview.config.Cards {
			if key == "music" {
				t.Fatal("preview interior swallowed card selection")
			}
		}
		tt.Scroll(float32(width)-160, 400, 0, 10000)
		tt.Frame()
		last := ""
		for _, key := range a.catalogIDs() {
			if !overviewWide(key) {
				last = key
			}
		}
		if _, ok = tt.Find("显示" + overviewName(last)); !ok {
			t.Fatal("last preview cannot be reached")
		}
		tt.Key(0, ui.KeyEscape)
		tt.Frame()
		if a.overview.manage {
			t.Fatal("escape failed to close side sheet")
		}
	}
}

func TestLongListsRemainReachable(t *testing.T) {
	a := newTestApp(t)
	history, todos := []any{}, []any{}
	for i := 0; i < 500; i++ {
		history = append(history, map[string]any{"id": fmt.Sprint(i), "title": fmt.Sprint("通知", i), "kind": "notice"})
	}
	for i := 0; i < 100; i++ {
		todos = append(todos, map[string]any{"id": fmt.Sprint(i), "title": fmt.Sprint("待办", i), "date": "2026-11-15"})
	}
	a.State["notificationHistory"], a.State["todos"] = history, todos
	a.Page = PageNotifications
	tt := ui.NewTester(a.View, 1200, 800)
	tt.Frame()
	if tt.HasText("通知499") {
		t.Fatal("offscreen notification was built")
	}
	tt.Scroll(800, 500, 0, 100000)
	tt.Frame()
	if !tt.HasText("通知499") || tt.HasText("通知0") {
		t.Fatal("notification viewport did not reach its final row")
	}
	a.Page = PageTodos
	tt.Move(800, 490)
	tt.Frame()
	tt.Scroll(800, 500, 0, 100000)
	tt.Frame()
	r, ok := tt.Find("edit-todo-99")
	if !ok {
		t.Fatal("last todo cannot be reached")
	}
	tt.ClickAt(r.X+r.W/2, r.Y+r.H/2)
	tt.Frame()
	if !a.todoDialog || todoDialogDraft.id != "99" || todoDialogDraft.title != "待办99" {
		t.Fatal("recycled todo row lost its edit target")
	}
}

func TestDeckMotionSettlesAndHonorsReducedMotion(t *testing.T) {
	for _, reduced := range []bool{false, true} {
		deckPos, deckTarget, deckDragY = 0, 0, 0
		deckSpringIdle = true
		deckLastInput = time.Time{}
		a := newTestApp(t)
		a.Prefs.ReducedMotion = reduced
		a.overview.loaded = true
		a.overview.config = prefs.DefaultOverview()
		a.overview.config.View = "scroll"
		tt := ui.NewTester(a.View, 1200, 800)
		tt.Frame()
		r, _ := tt.Find("下一张概览卡片")
		tt.ClickAt(r.X+r.W/2, r.Y+r.H/2)
		if reduced {
			tt.Frame()
			if math.Abs(deckPos-deckStep) > .01 {
				t.Fatal("reduced motion still interpolates")
			}
			continue
		}
		tt.Advance(80 * time.Millisecond)
		if deckPos <= 0 || deckPos >= deckStep {
			t.Fatalf("card did not interpolate: %f", deckPos)
		}
		tt.Advance(time.Second)
		if math.Abs(deckPos-deckStep) > .01 {
			t.Fatalf("spring did not settle: %f", deckPos)
		}
	}
	deckPos, deckTarget, deckDragY = 0, 0, 0
	deckSpringIdle = true
}
