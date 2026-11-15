package ui

import (
	"fmt"
	"os"
	"testing"
	"time"

	"github.com/egoist/mygo/ui"
)

// Runs only on request. The fixture measures history/todo load, not empty cards.
func TestInteractionProfile(t *testing.T) {
	if os.Getenv("DOTISLE_PROFILE_UI") == "" {
		t.Skip("explicit profiling only")
	}
	RegisterFonts()
	for _, page := range []int{PageHome, PageTodos, PageNotifications, PageSettings} {
		a := newTestApp(t)
		a.Page = page
		todos, history := []any{}, []any{}
		for i := 0; i < 100; i++ {
			todos = append(todos, map[string]any{"id": fmt.Sprint(i), "title": "测试待办", "date": "2026-11-15", "time": "09:00", "completed": i%2 == 0})
		}
		for i := 0; i < 500; i++ {
			history = append(history, map[string]any{"id": fmt.Sprint(i), "title": "测试通知", "body": "用于重现列表滚动负载", "appName": "点屿", "kind": "notice", "date": "2026-11-15", "clock": "09:00"})
		}
		a.State["todos"], a.State["notificationHistory"] = todos, history
		builds := 0
		buildTime := time.Duration(0)
		tt := ui.NewTester(func(c *ui.Context) { start := time.Now(); a.View(c); buildTime += time.Since(start); builds++ }, 1200, 800)
		builds, buildTime = 0, 0
		start := time.Now()
		for i := 0; i < 20; i++ {
			tt.Scroll(800, 500, 0, 24)
			tt.Frame()
		}
		if builds == 0 {
			t.Fatal("no builds measured")
		}
		t.Logf("PROFILE page=%d build_us=%.1f interaction_ms=%.2f builds=%d", page, float64(buildTime.Microseconds())/float64(builds), float64(time.Since(start).Microseconds())/20000, builds)
	}
}
