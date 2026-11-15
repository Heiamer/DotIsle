package ui

import "github.com/egoist/mygo/ui"

// Kore ProgressIndicator: a round-capped solid line over a muted track.
// Reference: https://kore-3j8.pages.dev/#docs/ProgressIndicator
func progressLine(c *ui.Context, p *Palette, x, y, w float32, fraction float64) {
	s := &p.Scheme
	ui.Box(c).Absolute().Left(x).Top(y).Width(w).Height(6).Radius(3).
		Background(Col(mix(s.Container, s.OnSurface, .15))).IgnoreInput(true)
	ui.Box(c).Absolute().Left(x).Top(y).Width(w * float32(clamp01(fraction))).Height(6).Radius(3).
		Background(Col(s.Primary)).IgnoreInput(true)
}

func koreSeekVisual(c *ui.Context, p *Palette, w, h float32, fraction float64, enabled, pressed bool) {
	progressLine(c, p, 0, (h-6)/2, w, fraction)
}
