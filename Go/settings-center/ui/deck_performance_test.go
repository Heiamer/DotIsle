package ui

import (
	"math"
	"os"
	"testing"
	"time"

	"dotisle/settings/prefs"
	"github.com/egoist/mygo/ui"
)

func TestDeckKeepsContentLayoutAndHidesOutgoingControls(t *testing.T) {
	a := newTestApp(t)
	a.overview.loaded = true
	a.overview.config = prefs.DefaultOverview()
	a.overview.config.View = "scroll"
	deckPos, deckTarget, deckDragY, deckSpringIdle = 0, 0, 0, true
	defer func() { deckPos, deckTarget, deckDragY, deckSpringIdle = 0, 0, 0, true }()
	// Hold the simulated gesture so the idle snap cannot replace its phase.
	tt := ui.NewTester(func(c *ui.Context) { deckLastInput = c.Now(); a.View(c) }, 1200, 800)
	before, ok := tt.Find("还没有播放音乐")
	if !ok {
		t.Fatal("focused music body missing")
	}
	for _, phase := range []float64{.1, .2} {
		a.deckTo(deckStep*phase, true)
		tt.Frame()
		after, ok := tt.Find("还没有播放音乐")
		if !ok || math.Abs(float64(after.W)-float64(before.W)) > 1 {
			t.Fatalf("music content reflowed during movement: before=%v after=%v phase=%v", before, after, phase)
		}
	}
	a.deckTo(deckStep*.6, true)
	tt.Frame()
	if _, ok := tt.Access("上一首"); ok {
		t.Fatal("outgoing music controls can still receive input")
	}
	a.deckTo(deckStep, true)
	tt.Frame()
	if _, ok := tt.Access("概览专注 15 分钟"); !ok {
		t.Fatal("incoming focused page controls are unavailable")
	}
	a.StepOverview(1)
	tt.Frame()
	tt.Advance(time.Second)
	tt.Frame()
	if math.Abs(deckPos-2*deckStep) > .01 {
		t.Fatal("adjacent-page navigation lost the ring target")
	}
}

func TestUnavailableNativeMicaUsesOpaqueFallback(t *testing.T) {
	a := newTestApp(t)
	a.backdropChecked = true
	a.backdropMica = false
	active := true
	tt := ui.NewTester(func(c *ui.Context) { active = a.materialActive(c) }, 200, 120)
	tt.SetVibrancy(true)
	tt.Frame()
	if active {
		t.Fatal("unsupported DWM backdrop left the page transparent")
	}
}

func TestDeckDragWheelReversalAndWrap(t *testing.T) {
	a := newTestApp(t)
	a.loadOverview()
	a.overview.config.View = "scroll"
	deckPos, deckTarget, deckSpringIdle = 0, 0, true
	defer func() { deckPos, deckTarget, deckSpringIdle = 0, 0, true }()
	tt := ui.NewTester(a.View, 1200, 800)
	r, ok := tt.Find("浏览音乐卡片")
	if !ok {
		t.Fatal("missing card drag target")
	}
	x, y := r.X+r.W/2, r.Y+28
	tt.Press(x, y)
	tt.Advance(16 * time.Millisecond)
	tt.Move(x, y-100)
	tt.Frame()
	if deckPos < 50 {
		t.Fatalf("upward drag did not move to next page: %v", deckPos)
	}
	tt.Advance(16 * time.Millisecond)
	tt.Move(x, y-240)
	tt.Release(x, y-240)
	tt.Advance(time.Second)
	if deckPos < deckStep || deckPos != deckSnap(deckPos) {
		t.Fatalf("release did not snap forward: %v", deckPos)
	}
	// A wheel notch must turn a page, even when its DIP delta is small.
	a.deckTo(0, true)
	tt.Frame()
	tt.Wheel(700, 450, 0, 30)
	tt.Advance(60 * time.Millisecond)
	if deckTarget != deckStep {
		t.Fatalf("wheel notch did not target one page: %v", deckTarget)
	}
	tt.Wheel(700, 450, 0, -30)
	tt.Advance(time.Second)
	if deckPos > 0 || deckPos != deckSnap(deckPos) {
		t.Fatalf("wheel reversal continued toward the old target: %v", deckPos)
	}
	// Keep the same captured card when the ring crosses its modulo seam.
	a.deckTo(float64(len(a.scrollPages()))*deckStep-30, true)
	tt.Frame()
	r, ok = tt.Find("浏览音乐卡片")
	if !ok {
		t.Fatal("wrapped music page missing")
	}
	x, y = r.X+r.W/2, r.Y+28
	tt.Press(x, y)
	tt.Advance(16 * time.Millisecond)
	tt.Move(x, y-100)
	tt.Advance(16 * time.Millisecond)
	tt.Move(x, y-200)
	tt.Frame()
	if deckPos <= float64(len(a.scrollPages()))*deckStep+80 {
		t.Fatalf("drag lost capture at wrap seam: %v", deckPos)
	}
	tt.CancelPress()
	tt.Advance(time.Second)
	if deckPos != deckSnap(deckPos) {
		t.Fatal("canceled drag left a partial page")
	}
}

func TestInformationScrollConsumesWheelBeforeDeck(t *testing.T) {
	a := newTestApp(t)
	a.loadOverview()
	a.overview.config.View = "scroll"
	a.overview.config.Information = prefs.InformationCardIDs()
	deckPos, deckTarget, deckSpringIdle = 4*deckStep, 4*deckStep, true
	defer func() { deckPos, deckTarget, deckSpringIdle = 0, 0, true }()
	tt := ui.NewTester(a.View, 1200, 800)
	r, ok := tt.Find("滚动信息组件")
	if !ok {
		t.Fatal("missing information scroll area")
	}
	x, y := r.X+r.W/2, r.Y+100
	tt.Scroll(x, y, 0, 140)
	tt.Frame()
	if a.overview.informationScroll.Y <= 0 || deckTarget != 4*deckStep {
		t.Fatalf("inner scrolling changed the deck: inner=%v deck=%v", a.overview.informationScroll, deckTarget)
	}
	tt.Scroll(x, y, 0, 100000)
	tt.Frame()
	tt.Wheel(x, y, 0, 30)
	tt.Advance(time.Second)
	if deckPos == 4*deckStep {
		t.Fatal("information scroll boundary trapped the deck")
	}
}

func TestDeckShellsRemainSeparatedAtIntermediatePhases(t *testing.T) {
	for i := 0; i <= 100; i++ {
		phase := float64(i) / 100
		y, h := deckGeometry(-phase, 430)
		nextY, _ := deckGeometry(1-phase, 430)
		if math.Abs(nextY-y-float64(h)-deckGap) > .001 {
			t.Fatalf("shells intersect at phase %.2f: bottom=%v next=%v", phase, y+float64(h), nextY)
		}
	}
}

// This measures view construction separately from the software renderer.
// GPU presentation and display cadence must be measured in the native preview.
func TestDeckInteractionProfile(t *testing.T) {
	if os.Getenv("DOTISLE_PROFILE_UI") == "" {
		t.Skip("explicit profiling only")
	}
	RegisterFonts()
	a := newTestApp(t)
	a.overview.loaded = true
	a.overview.config = prefs.DefaultOverview()
	a.overview.config.View = "scroll"
	deckPos, deckTarget, deckDragY, deckSpringIdle = 0, 0, 0, true
	defer func() { deckPos, deckTarget, deckDragY, deckSpringIdle = 0, 0, 0, true }()
	builds, buildTime := 0, time.Duration(0)
	tt := ui.NewTester(func(c *ui.Context) {
		start := time.Now()
		a.View(c)
		buildTime += time.Since(start)
		builds++
	}, 1200, 800)
	builds, buildTime = 0, 0
	start := time.Now()
	for i := 0; i < 120; i++ {
		if i%30 == 0 {
			a.deckTo(deckTarget+deckStep, false)
		}
		tt.Advance(time.Second / 120)
		tt.Frame()
	}
	t.Logf("DECK_PROFILE builds=%d build_us=%.1f software_frame_ms=%.2f", builds,
		float64(buildTime.Microseconds())/float64(builds), float64(time.Since(start).Microseconds())/120000)
}
