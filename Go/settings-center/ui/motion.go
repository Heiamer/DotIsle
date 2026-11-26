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
		a.resetOpeningLight()
		return false
	}
	if a.signaturePending {
		a.signaturePending = false
		a.signatureStart = c.Now()
	}
	if a.signatureStart.IsZero() {
		a.resetOpeningLight()
		return false
	}
	reduced := a.reducedMotion()
	elapsed := c.Now().Sub(a.signatureStart).Seconds()
	if elapsed >= signatureDuration(reduced) || c.Shortcut(0, ui.KeyEscape) {
		a.signatureStart = time.Time{}
		a.resetOpeningLight()
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
	cover := ui.ButtonBase(c).Key("opening-cover").Absolute().Left(0).Top(0).
		Width(w).Height(h).Background(Col(s.Surface)).Opacity(float32(alpha)).Label("跳过开屏动画")
	if cover.Clicked() {
		a.signatureStart = time.Time{}
		a.resetOpeningLight()
		c.AnimationFrame()
		return
	}
	cover.Children(func() {
		slow := sigSlow()
		t := elapsed / slow
		a.paintOpeningLight(c, w, h, t, reduced)

		wordW := min(float32(328), w-96)
		wordH := wordW * signatureHeight / signatureWidth
		wordY := (h - wordH - 36) / 2

		a.drawSignature(c, (w-wordW)/2, wordY, wordW, wordH, elapsed, reduced)

		versionAlpha := float32(0.78)
		versionOffset := float32(0)
		if !reduced {
			vt := motionEase(float32(clampF64((t-0.66)/0.28, 0, 1)))
			versionAlpha = vt * 0.78
			versionOffset = 7 * (1 - vt)
			if t > sigWrite+sigHold {
				exitP := motionEase(float32(clampF64((t-sigWrite-sigHold)/sigExit, 0, 1)))
				versionOffset -= 4 * exitP
			}
		}
		CenteredText(c, stateString(a.State, "5.2.4", "version"), 0, wordY+wordH+22+versionOffset, w, 24, 12, s.OnSurfaceVariant, false).Opacity(versionAlpha)
	})
	c.AnimationFrame()
}
