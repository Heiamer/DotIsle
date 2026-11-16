// Equal-sized overview cards travel on a continuous ring. Only transforms
// and opacity change while the fixed content surfaces stay cached.
package ui

import (
	"math"
	"time"

	"github.com/egoist/mygo/ui"
)

// Ring geometry (card_ring.rs).
const (
	deckCount = 5
	deckStep  = 360.0
	deckGap   = 24.0
	deckCycle = deckStep * deckCount
)

func ringCount(counts []int) int {
	if len(counts) > 0 && counts[0] > 0 {
		return counts[0]
	}
	return deckCount
}

func deckPhase(position float64, counts ...int) float64 {
	cycle := deckStep * float64(ringCount(counts))
	p := math.Mod(position, cycle)
	if p < 0 {
		p += cycle
	}
	return p / deckStep
}

func deckSlotIndex(slot int, counts ...int) int {
	count := ringCount(counts)
	m := slot % count
	if m < 0 {
		m += count
	}
	return m
}

// deckSnap mirrors ring::snap; deckNearest mirrors ring::nearest.
func deckSnap(position float64) float64 { return math.Round(position/deckStep) * deckStep }

func deckNearest(position float64, card int, counts ...int) float64 {
	count := ringCount(counts)
	cycle := float64(count) * deckStep
	base := float64(card%count) * deckStep
	return base + math.Round((position-base)/cycle)*cycle
}

// deckCardHeight mirrors center_layout::deck_card_height.
func deckCardHeight(viewport float64) float64 {
	h := viewport - 369
	if h < 280 {
		h = 280
	}
	if h > 430 {
		h = 430
	}
	return h
}

// homePage gives every page the same shell and content bounds, including empty pages.
func (a *App) homePage(c *ui.Context, x, y, w, viewportH float32) {
	pages := a.scrollPages()
	count := len(pages)
	cardW := min(w, 980)
	x += (w - cardW) / 2
	h := float32(deckCardHeight(clipH + float64(ContentTop) + 24))

	motion := ui.Box(c).Key("deck-motion").Width(0).Height(0).IgnoreInput(true)
	spring := ui.Local(motion, "position", func() appSpring {
		return appSpring{value: deckPos, target: deckPos, at: c.Now()}
	})
	if deckSpringIdle {
		deckTarget = deckPos
		spring.jump(deckPos, c.Now())
	}
	if c.Now().Sub(deckLastInput) >= 160*time.Millisecond && deckTarget != deckSnap(deckTarget) {
		a.deckTo(deckSnap(deckTarget), false)
	}
	if deckTarget != deckSnap(deckTarget) {
		c.After(160 * time.Millisecond)
	}
	reduced := a.reducedMotion()
	if deckVel != 0 {
		spring.velocity, deckVel = deckVel, 0
	}
	deckPos = float64(spring.sample(c.Now(), deckTarget, 380, .82, reduced))
	if spring.settled() {
		deckPos = deckTarget
		deckSpringIdle = true
	} else {
		c.AnimationFrame()
	}
	// Four persistent slots recycle offscreen. Their layer state must survive
	// a ring seam instead of leaving new native textures for the next GC.
	progress := deckPos / deckStep
	base := int(math.Floor(progress))
	y = max(float32(24), (viewportH-h)/2)
	// Only these four slots can have nonzero visibility. No per-frame slice.
	order := [4]int{base - 1, base, base + 1, base + 2}
	for i := 1; i < len(order); i++ {
		for j := i; j > 0 && math.Abs(float64(order[j])-progress) > math.Abs(float64(order[j-1])-progress); j-- {
			order[j], order[j-1] = order[j-1], order[j]
		}
	}

	// One edge mask; no animated full-viewport blur or per-page reflow.
	black := ui.RGB(0, 0, 0)
	ui.Box(c).Key("deck-edges").Absolute().Left(0).Top(0).Width(w).Height(viewportH).
		MaskLinearGradient(ui.LinearGradient{Angle: 180, Stops: []ui.GradientStop{
			{Color: ui.Transparent, At: 0}, {Color: black, At: min(24/viewportH, .3)},
			{Color: black, At: max(1-24/viewportH, .7)}, {Color: ui.Transparent, At: 1},
		}}).Children(func() {
		layer := ui.Box(c).Key("deck-layer").Absolute().Fill()
		layer.Children(func() {
			for _, slot := range order {
				if count == 1 && slot != base {
					continue
				}
				i := deckSlotIndex(slot, count)
				d := float64(slot) - progress
				visibility := float32(clampF64(2-math.Abs(d), 0, 1))
				dy, visualH := deckGeometry(d, h)
				ty := y + float32(dy)
				hidden := visibility <= 0 || ty+h < 0 || ty > viewportH
				identity := deckSlotIndex(slot, 4)
				if count == 1 {
					identity = 0
				}
				card := ui.ButtonBase(c).Role(ui.RoleGroup).Key("deck-slot-"+itoa(identity)).Label("浏览"+overviewName(pages[i])+"卡片").Absolute().Left(x).Top(y).
					Width(cardW).Height(h).Translate(0, float32(dy)).Opacity(visibility).Hidden(hidden)
				dragged := a.deckGesture(c, card, count, h)
				if card.Clicked() && !dragged && math.Abs(d) >= .5 {
					a.deckTo(float64(slot)*deckStep, false)
				}
				card.Children(func() { a.deckCard(c, pages[i], cardW, h, visualH, d) })
			}
		})
	})
}

// Full-sized neighbours keep a constant gap at every intermediate phase.
func deckGeometry(d float64, h float32) (float64, float32) {
	return d * (float64(h) + deckGap), h
}

// The deck ancestor and card backgrounds accept drags; nested buttons retain
// their own presses. Release projects a short fling and never selects a card.
func (a *App) deckGesture(c *ui.Context, handle ui.Element, count int, h float32) bool {
	_, dy, held := handle.Dragged()
	if count < 2 {
		return false
	}
	factor := deckStep / (float64(h) + deckGap)
	if held {
		deckLastInput = c.Now()
		if dy != 0 {
			a.deckTo(deckPos-float64(dy)*factor, true)
			c.AnimationFrame()
		}
	}
	if _, vy, ended := handle.DragEnded(); ended {
		projection := clampF64(-float64(vy)*factor*.12, -deckStep, deckStep)
		a.deckTo(deckSnap(deckPos+projection), false)
		deckVel = clampF64(-float64(vy)*factor, -10000, 10000)
		c.AnimationFrame()
		return true
	}
	if handle.PressCanceled() {
		a.deckTo(deckSnap(deckPos), false)
		c.AnimationFrame()
		return true
	}
	return held
}

func (a *App) deckScroll(ev ui.InputEvent, h float32) bool {
	if ev.Kind != ui.InputScroll {
		return false
	}
	if len(a.scrollPages()) < 2 || ev.DY == 0 {
		return true
	}
	deckLastInput = ev.Time
	if ev.Precise {
		a.deckTo(deckPos+float64(ev.DY)*deckStep/(float64(h)+deckGap), true)
	} else {
		direction := math.Copysign(deckStep, float64(ev.DY))
		from := deckTarget
		if (deckTarget-deckPos)*direction < 0 {
			from = deckPos
		}
		center := deckSnap(deckPos)
		a.deckTo(clampF64(deckSnap(from)+direction, center-2*deckStep, center+2*deckStep), false)
	}
	return true
}

// StepOverview moves to an adjacent page, shared by controls and preview audits.
func (a *App) StepOverview(direction int) {
	if len(a.scrollPages()) > 1 {
		a.deckTo(deckSnap(deckTarget)+float64(direction)*deckStep, false)
	}
}

// PreviewOverviewProgress drives an isolated diagnostic through real, changing
// card geometry even when Windows has disabled automatic animations.
func (a *App) PreviewOverviewProgress(progress float64, now time.Time) {
	a.deckTo(progress*deckStep, true)
	deckLastInput = now
}

// clampF64 clamps v into [lo, hi].
func clampF64(v, lo, hi float64) float64 {
	if v < lo {
		return lo
	}
	if v > hi {
		return hi
	}
	return v
}

// deckTo moves the ring, snapping immediately when immediate is set.
func (a *App) deckTo(target float64, immediate bool) {
	deckTarget = target
	if immediate {
		deckPos = target
		deckVel = 0
		deckSpringIdle = true
	} else {
		deckSpringIdle = false
	}
}

// The same complete card surface moves through the viewport; only the closest
// page owns controls. Neighbour content remains visible without duplicate titles.
func (a *App) deckCard(c *ui.Context, key string, w, h, visualH float32, d float64) {
	a.overviewCardSurface(c, overviewRect{key, 0, 0, w, h})
	body := float32(.45 + .55*clampF64(1-math.Abs(d), 0, 1))
	ui.Box(c).Key("deck-body").Absolute().Left(0).Top(0).Width(w).Height(h).
		ClipInset(0, 12, 12, 12, 0).Opacity(body).Cache().
		IgnoreInput(math.Abs(d) >= .5).AccessHidden(math.Abs(d) >= .5).Children(func() {
		ui.Box(c).Key("deck-content").Width(w).Height(h).Children(func() {
			a.renderOverviewCardBody(c, overviewRect{key, 0, 0, w, h})
		})
	})
}
