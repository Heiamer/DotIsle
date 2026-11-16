package ui

import (
	"time"

	"github.com/egoist/mygo/ui"
)

// LottieFiles motion-design: Premium personality, directional entrances,
// tactile feedback and a restrained secondary layer. No persistent idle work.
var motionEase = ui.CubicBezier(.2, 0, 0, 1)

const (
	motionQuick    = 120 * time.Millisecond
	motionSettle   = 220 * time.Millisecond
	motionStandard = 320 * time.Millisecond
	motionPage     = 420 * time.Millisecond
)

func entranceMotion(c *ui.Context, p *Palette, e *ui.Element, distance float32, duration time.Duration, initial float32) {
	progress := appTweenValue(c, *e, "entrance", initial, 1, duration, p.ReducedMotion)
	e.Translate(0, distance*(1-progress)).Scale(.995+.005*progress, .995+.005*progress).Opacity(progress)
}

// Popovers have a new overlay parent, so seed their pose explicitly rather
// than relying on a Transition whose parent has not appeared yet.
func popoverMotion(c *ui.Context, p *Palette, e *ui.Element) {
	progress := appTweenValue(c, *e, "popover-entered", 0, 1, motionStandard, p.ReducedMotion)
	e.Translate(0, 8*(1-progress)).Scale(.985+.015*progress, .985+.015*progress)
}

func pressMotion(c *ui.Context, p *Palette, e *ui.Element, enabled bool) {
	if !enabled {
		return
	}
	scale := float32(1)
	duration := motionSettle
	if e.Pressed() {
		scale = .97
		duration = motionQuick
	}
	scale = appTweenValue(c, *e, "press-scale", 1, scale, duration, p.ReducedMotion)
	e.Scale(scale, scale)
}

func micaActive(c *ui.Context) bool {
	p := c.Preferences()
	return c.Vibrancy() && !p.ReduceTransparency && !p.HighContrast
}

func (a *App) materialActive(c *ui.Context) bool {
	return micaActive(c) && (!a.backdropChecked || a.backdropMica)
}

// prepareOpening starts its clock on the first frame. The opaque phase skips
// all underlying page work, exactly as the 3.5.1 renderer did.
func (a *App) prepareOpening(c *ui.Context) bool {
	if !a.signatureEnabled() {
		a.signaturePending = false
		a.signatureStart = time.Time{}
		return false
	}
	if a.signaturePending {
		a.signaturePending = false
		a.signatureStart = c.Now()
	}
	if a.signatureStart.IsZero() {
		return false
	}
	reduced := a.reducedMotion()
	elapsed := c.Now().Sub(a.signatureStart).Seconds()
	if elapsed >= signatureDuration(reduced) || c.Shortcut(0, ui.KeyEscape) {
		a.signatureStart = time.Time{}
		c.AnimationFrame()
		return false
	}
	_, alpha := sigInk(elapsed, reduced)
	return alpha >= .999
}

func (a *App) opening(c *ui.Context, w, h float32) {
	if a.signatureStart.IsZero() {
		return
	}
	reduced := a.reducedMotion()
	elapsed := c.Now().Sub(a.signatureStart).Seconds()
	_, alpha := sigInk(elapsed, reduced)
	s := &a.Palette.Scheme
	cover := ui.ButtonBase(c).Key("opening-cover").Absolute().Left(0).Top(TitlebarH).
		Width(w).Height(h - TitlebarH).Background(Col(s.Surface)).Opacity(float32(alpha)).Label("跳过开屏动画")
	if cover.Clicked() {
		a.signatureStart = time.Time{}
		c.AnimationFrame()
		return
	}
	cover.Children(func() {
		wordW := min(float32(328), w-96)
		wordH := wordW * signatureHeight / signatureWidth
		wordY := (h - TitlebarH - wordH - 52) / 2
		a.drawSignature(c, (w-wordW)/2, wordY, wordW, wordH, elapsed, reduced)
		versionAlpha := float32(1)
		if !reduced {
			versionAlpha = motionEase(float32(clampF64((elapsed/sigSlow()-.62)/.22, 0, 1)))
		}
		CenteredText(c, stateString(a.State, "5.0.6", "version"), 0, wordY+wordH+28, w, 24, 12, s.OnSurfaceVariant, false).Opacity(versionAlpha)
	})
	c.AnimationFrame()
}
