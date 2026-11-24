package ui

import "github.com/egoist/mygo/ui"

// Kore ProgressIndicator: minimalist monochrome track & active fill.
func progressLine(c *ui.Context, p *Palette, x, y, w float32, fraction float64) {
	track := AlphaCol(0x111827, 0.09)
	fill := Col(0x16181D)
	if p != nil && p.Dark {
		track = AlphaCol(0xFFFFFF, 0.14)
		fill = Col(0xF3F4F6)
	}
	if p != nil && p.HighContrast {
		track = Col(mix(p.Scheme.Container, p.Scheme.OnSurface, .15))
		fill = Col(p.Scheme.Primary)
	}
	ui.Box(c).Absolute().Left(x).Top(y).Width(w).Height(5).Radius(2.5).
		Background(track).IgnoreInput(true)
	ui.Box(c).Absolute().Left(x).Top(y).Width(w * float32(clamp01(fraction))).Height(5).Radius(2.5).
		Background(fill).IgnoreInput(true)
}

func koreSeekVisual(c *ui.Context, p *Palette, w, h float32, fraction float64, enabled, pressed bool) {
	progressLine(c, p, 0, (h-5)/2, w, fraction)
}
