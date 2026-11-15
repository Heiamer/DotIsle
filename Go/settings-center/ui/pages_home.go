// The overview deck (overview.rs): five cards on an endless ring, the focused card
// expanded and its neighbours compressed above and below. Ring math is ported from
// dotisle-domain card_ring.rs so motion never meets an endpoint.
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

// homePage keeps body layout fixed while shells compress on a continuous ring.
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
	blur := float32(0)
	if !reduced && count > 1 && !deckSpringIdle {
		// Retained velocity is stable across repeated builds at the same time.
		blur = float32(math.Round(clampF64(math.Abs(spring.velocity)/1600, 0, 1.5)*4) / 4)
	}
	// Keep slot identities across the wrap seam, including a captured press.
	progress := deckPos / deckStep
	base := int(math.Floor(progress))
	pageHeight := func(slot int) float32 {
		key := pages[deckSlotIndex(slot, count)]
		return a.deckPageHeight(key, h)
	}
	currentH, nextH := pageHeight(base), pageHeight(base+1)
	currentH = appSpringValue(c, motion, "height-"+pages[deckSlotIndex(base, count)], currentH, motionStandard, reduced)
	nextH = appSpringValue(c, motion, "height-"+pages[deckSlotIndex(base+1, count)], nextH, motionStandard, reduced)
	phase := progress - float64(base)
	ease := float32(phase * phase * (3 - 2*phase))
	y += (h - (currentH + (nextH-currentH)*ease)) / 2
	// Only these four slots can have nonzero visibility. No per-frame slice.
	order := [4]int{base - 1, base, base + 1, base + 2}
	for i := 1; i < len(order); i++ {
		for j := i; j > 0 && math.Abs(float64(order[j])-progress) > math.Abs(float64(order[j-1])-progress); j-- {
			order[j], order[j-1] = order[j-1], order[j]
		}
	}

	// A single transparent edge mask and foreground blur for the entire deck.
	black := ui.RGB(0, 0, 0)
	ui.Box(c).Key("deck-edges").Absolute().Left(0).Top(0).Width(w).Height(viewportH).
		MaskLinearGradient(ui.LinearGradient{Angle: 180, Stops: []ui.GradientStop{
			{Color: ui.Transparent, At: 0}, {Color: black, At: min(58/viewportH, .3)},
			{Color: black, At: max(1-72/viewportH, .7)}, {Color: ui.Transparent, At: 1},
		}}).Children(func() {
		layer := ui.Box(c).Key("deck-layer").Absolute().Fill()
		if blur > .05 {
			layer.Blur(blur)
		}
		layer.Children(func() {
			for _, slot := range order {
				if count == 1 && slot != base {
					continue
				}
				i := deckSlotIndex(slot, count)
				d := float64(slot) - progress
				visibility := float32(clampF64(2-math.Abs(d), 0, 1))
				if visibility <= 0 {
					continue
				}
				dy, visualH := deckPairGeometry(progress-float64(base), currentH, nextH, slot-base)
				bodyH := pageHeight(slot)
				scale := float32(1 - .035*math.Min(math.Abs(d), 3))
				ty := y + float32(dy)
				if ty+visualH*scale < 0 || ty > viewportH {
					continue
				}
				dx := cardW * (1 - scale) / 2
				card := ui.ButtonBase(c).Role(ui.RoleGroup).Key("deck-slot-"+itoa(slot)).Label("浏览"+overviewName(pages[i])+"卡片").Absolute().Left(x).Top(y).
					Width(cardW).Height(h).TransformOrigin(0, 0).Scale(scale, scale).Translate(dx, float32(dy)).Opacity(visibility)
				// Hit bounds match the compressed shell, not the expanded body.
				card.Height(visualH)
				dragged := a.deckGesture(c, card, count, h)
				if card.Clicked() && !dragged && math.Abs(d) >= .5 {
					a.deckTo(float64(slot)*deckStep, false)
				}
				card.Children(func() { a.deckCard(c, pages[i], cardW, bodyH, visualH, d) })
			}
		})
	})
}

func (a *App) deckPageHeight(key string, full float32) float32 {
	if (key == "todos" && a.pendingTodos() == 0) || (key == "notifications" && len(a.historyEvents()) == 0) {
		return min(full, float32(280))
	}
	return full
}

// Adjacent pages can have different resting heights. Position the next shell
// from the current shell's bottom, preserving the 16-DIP overlap at every phase.
func deckPairGeometry(phase float64, currentH, nextH float32, relative int) (float64, float32) {
	t := float32(clampF64(phase, 0, 1))
	ease := t * t * (3 - 2*t)
	top := -t * 56
	current := currentH + (72-currentH)*ease
	next := 72 + (nextH-72)*ease
	switch relative {
	case -1:
		return float64(top - 56), 72
	case 0:
		return float64(top), current
	case 1:
		return float64(top + current - 16), next
	default:
		return float64(top + current + next - 32), 72
	}
}

// Adjacent shells overlap by only 16 DIPs throughout the transition. Their
// heights add to h+72, rather than both staying nearly full size at mid-phase.
func deckGeometry(d float64, h float32) (float64, float32) {
	t := math.Min(math.Abs(d), 1)
	ease := t * t * (3 - 2*t)
	height := h + (72-h)*float32(ease)
	dy := d * 56
	if d >= 0 && d <= 1 {
		dy += float64(h-72) * ease
	} else if d > 1 {
		dy = float64(h-16) + (d-1)*56
	}
	return dy, height
}

// The deck ancestor and card backgrounds accept drags; nested buttons retain
// their own presses. Release projects a short fling and never selects a card.
func (a *App) deckGesture(c *ui.Context, handle ui.Element, count int, h float32) bool {
	_, dy, held := handle.Dragged()
	if count < 2 {
		return false
	}
	factor := deckStep / float64(h-16)
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
		a.deckTo(deckPos+float64(ev.DY)*deckStep/float64(h-16), true)
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

// deckCard fades the fixed-size body continuously, without the former 180-DIP
// layout switch. The shell keeps its continuous corners throughout compression.
func (a *App) deckCard(c *ui.Context, key string, w, h, visualH float32, d float64) {
	a.overviewCardSurface(c, overviewRect{key, 0, 0, w, visualH})
	body := float32(clampF64(1-math.Abs(d)*1.2, 0, 1))
	if body < .999 {
		ink := a.Palette.Scheme.OnSurface
		if key == "music" {
			ink = a.musicCardPalette().Scheme.OnSurface
		}
		Label(c, overviewName(key), 24, 20, 16, ink, true, w-80).Opacity(1 - body).IgnoreInput(true)
	}
	if body <= .001 {
		return
	}
	ui.Box(c).Key("deck-body").Absolute().Left(0).Top(0).Width(w).Height(h).
		ClipInset(0, 12, h-visualH+12, 12, 0).Opacity(body).
		IgnoreInput(math.Abs(d) >= .5).AccessHidden(math.Abs(d) >= .5).Children(func() {
		// Keep the fixed content bitmap while only its shell and transform move.
		// A live value, focus or pressed state automatically invalidates it.
		ui.Box(c).Key("deck-content").Width(w).Height(h).Cache().Children(func() {
			a.renderOverviewCardBody(c, overviewRect{key, 0, 0, w, h})
		})
	})
}
