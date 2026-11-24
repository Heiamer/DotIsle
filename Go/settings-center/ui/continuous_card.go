package ui

import (
	"github.com/egoist/mygo/ui"
)

// Two cubic segments per corner meet straight edges with zero curvature.
// This matches dotisle-render::draw::continuous_rect, including radius clamping.
func continuousCardPath(r ui.Rect, radius float32) *ui.Path {
	a := max(float32(0), min(radius, min(r.W, r.H)/2))
	var p ui.Path
	corner := func(cx, cy, xx, xy, yx, yy float32) {
		pt := func(x, y float32) (float32, float32) { return cx + (xx*x+yx*y)*a, cy + (xy*x+yy*y)*a }
		x1, y1 := pt(.62, 0)
		x2, y2 := pt(.40, 0)
		x3, y3 := pt(.20, .20)
		p.CubeTo(x1, y1, x2, y2, x3, y3)
		x1, y1 = pt(0, .40)
		x2, y2 = pt(0, .62)
		x3, y3 = pt(0, 1)
		p.CubeTo(x1, y1, x2, y2, x3, y3)
	}
	right, bottom := r.X+r.W, r.Y+r.H
	p.MoveTo(right-a, r.Y)
	corner(right, r.Y, -1, 0, 0, 1)
	p.LineTo(right, bottom-a)
	corner(right, bottom, 0, -1, -1, 0)
	p.LineTo(r.X+a, bottom)
	corner(r.X, bottom, 1, 0, 0, -1)
	p.LineTo(r.X, r.Y+a)
	corner(r.X, r.Y, 0, 1, 1, 0)
	p.Close()
	return &p
}

// A tinted surface over the shared light field: one continuous outline, no
// per-card backdrop capture or blurred intermediate surface.
func continuousCardStyle(e *ui.Element, p *Palette, rgb uint32, radius float32) {
	e.Background(ui.Transparent).Radius(radius).Border(0, ui.Transparent).NoShadow()
	elevationStyle(e, p, elevationCard)
	highContrast := p != nil && p.HighContrast
	dark := p != nil && p.Dark

	fill := AlphaCol(0xFFFFFF, 0.68)
	border := AlphaCol(0x1E293B, 0.13)
	sheenFrom := AlphaCol(0xFFFFFF, 0.34)
	spot := AlphaCol(0xFFFFFF, 0.38)
	if dark {
		fill = AlphaCol(0x161921, 0.80)
		border = AlphaCol(0xFFFFFF, 0.13)
		sheenFrom = AlphaCol(0xFFFFFF, 0.045)
		spot = AlphaCol(0xFFFFFF, 0.07)
	}
	if highContrast {
		hcFill := Col(rgb)
		hcBorder := Col(p.Scheme.Outline)
		e.Draw(func(pc *ui.Painter, r ui.Rect) {
			paintContinuousCard(pc, r, radius, hcFill, hcBorder)
		})
		return
	}

	px, py, over := e.PointerPosition()
	e.Draw(func(pc *ui.Painter, r ui.Rect) {
		if r.W <= 4 || r.H <= 4 {
			return
		}
		if pc.Preferences().ReduceTransparency {
			paintContinuousCard(pc, r, radius, Col(rgb), AlphaCol(p.Scheme.Outline, 0.65))
			return
		}
		paintContinuousCard(pc, r, radius, fill, border)
		// A rounded, inset sheen stays inside the continuous outline without a
		// full-card path clip. These fills use the D3D rounded rectangle shader.
		inner := ui.Rect{X: r.X + 1, Y: r.Y + 1, W: r.W - 2, H: r.H - 2}
		ir := max(float32(0), radius-1)
		pc.FillGradient(inner, ui.LinearGradient{From: sheenFrom, To: ui.Transparent, Angle: 150}, ir)
		if over {
			pc.FillRadialGradient(inner, ui.RadialGradient{
				Center: ui.Point{X: clamp01F(px / r.W), Y: clamp01F(py / r.H)},
				Radius: min(max(r.W, r.H)*0.44, float32(185)),
				Stops:  []ui.GradientStop{{Color: spot, At: 0}, {Color: ui.Transparent, At: 1}},
			}, ir)
		}
	})
}

// Limit curve evaluation to the four corner tiles. A full-card path makes
// every interior pixel run the general path shader while the deck changes size.
func paintContinuousCard(pc *ui.Painter, r ui.Rect, radius float32, fill, border ui.Color) {
	a := max(float32(0), min(radius, min(r.W, r.H)/2))
	if a == 0 {
		pc.Fill(r, fill, 0)
		return
	}
	pc.Fill(ui.Rect{X: r.X + a, Y: r.Y, W: r.W - 2*a, H: r.H}, fill, 0)
	pc.Fill(ui.Rect{X: r.X, Y: r.Y + a, W: a, H: r.H - 2*a}, fill, 0)
	pc.Fill(ui.Rect{X: r.X + r.W - a, Y: r.Y + a, W: a, H: r.H - 2*a}, fill, 0)
	corner := func(cx, cy, xx, xy, yx, yy float32) {
		point := func(x, y float32) (float32, float32) { return cx + (xx*x+yx*y)*a, cy + (xy*x+yy*y)*a }
		var arc ui.Path
		x, y := point(1, 0)
		arc.MoveTo(x, y)
		x1, y1 := point(.62, 0)
		x2, y2 := point(.40, 0)
		x3, y3 := point(.20, .20)
		arc.CubeTo(x1, y1, x2, y2, x3, y3)
		x1, y1 = point(0, .40)
		x2, y2 = point(0, .62)
		x3, y3 = point(0, 1)
		arc.CubeTo(x1, y1, x2, y2, x3, y3)
		tile := arc
		x, y = point(1, 1)
		tile.LineTo(x, y)
		tile.Close()
		pc.FillPath(&tile, fill)
		pc.StrokePath(&arc, .75, border)
	}
	right, bottom := r.X+r.W, r.Y+r.H
	corner(right, r.Y, -1, 0, 0, 1)
	corner(right, bottom, 0, -1, -1, 0)
	corner(r.X, bottom, 1, 0, 0, -1)
	corner(r.X, r.Y, 0, 1, 1, 0)
	pc.Line(r.X+a, r.Y, right-a, r.Y, .75, border)
	pc.Line(right, r.Y+a, right, bottom-a, .75, border)
	pc.Line(r.X+a, bottom, right-a, bottom, .75, border)
	pc.Line(r.X, r.Y+a, r.X, bottom-a, .75, border)
}
