package ui

import (
	"github.com/egoist/mygo/ui"
	"math"
)

func appleWidgetSurface(c *ui.Context, p *Palette, r overviewRect) {
	card := ui.Box(c).Absolute().Left(r.x).Top(r.y).Width(r.w).Height(r.h)
	continuousCardStyle(&card, p, p.Scheme.Container, 24)
}

func widgetCircle(c *ui.Context, x, y, size float32, fraction float64, ink, track uint32) {
	ui.Box(c).Absolute().Left(x).Top(y).Width(size).Height(size).IgnoreInput(true).Draw(func(pc *ui.Painter, r ui.Rect) {
		stroke := float32(5)
		radius := (r.W - stroke) / 2
		for i, amount := range []float64{1, clamp01(fraction)} {
			if amount <= 0 {
				continue
			}
			color := track
			if i == 1 {
				color = ink
			}
			var path ui.Path
			steps := int(math.Ceil(80 * amount))
			for step := 0; step <= steps; step++ {
				a := -math.Pi/2 + 2*math.Pi*amount*float64(step)/float64(steps)
				xx := r.X + r.W/2 + float32(math.Cos(a))*radius
				yy := r.Y + r.H/2 + float32(math.Sin(a))*radius
				if step == 0 {
					path.MoveTo(xx, yy)
				} else {
					path.LineTo(xx, yy)
				}
			}
			pc.StrokePath(&path, stroke, Col(color))
		}
	})
}

func widgetTextAction(c *ui.Context, p *Palette, label, text string, x, y, w float32, action func()) {
	button := ui.ButtonBase(c).Absolute().Left(x).Top(y).Width(w).Height(32).Radius(10).Label(label).Cursor(ui.CursorPointer)
	ink := p.Scheme.OnSurfaceVariant
	if button.Hovered() {
		ink = p.Scheme.OnSurface
	}
	if p.HighContrast {
		ink = p.Scheme.Primary
	}
	hoverLayer(&button, ui.Transparent, p.Scheme.OnSurface, true)
	if button.Clicked() {
		action()
	}
	button.Children(func() { Label(c, text, 0, 6, 12, ink, true, w-4) })
}

func widgetCheck(c *ui.Context, p *Palette, label string, x, y float32, action func()) {
	button := ui.ButtonBase(c).Absolute().Left(x).Top(y).Width(32).Height(36).Radius(16).Label(label).Cursor(ui.CursorPointer)
	hoverLayer(&button, ui.Transparent, p.Scheme.Primary, true)
	if button.Clicked() {
		action()
	}
	button.Children(func() {
		ui.Box(c).Absolute().Left(6).Top(8).Width(20).Height(20).Radius(10).Border(1.5, Col(p.Scheme.Outline))
	})
}
