// Geometry and surfaces from HeroUI v3 Default. See docs/HEROUI-DEFAULT.md.
package ui

import "github.com/egoist/mygo/ui"

const (
	UIFont                = "HarmonyOS Sans, HarmonyOS Sans SC, Microsoft YaHei UI, sans-serif"
	CardRadius    float32 = 24
	FieldRadius   float32 = 12
	ButtonRadius  float32 = 24
	OverlayRadius float32 = 32
)

// Controls, cards and overlays have distinct contact and ambient shadows.
func surfaceStyle(e *ui.Element, p *Palette, rgb uint32, radius float32) {
	e.Background(Col(rgb)).Radius(radius).Border(0, ui.Transparent).NoShadow()
	level := elevationControl
	if radius >= 20 {
		level = elevationCard
	}
	elevationStyle(e, p, level)
}

func overlayStyle(e *ui.Element, p *Palette) {
	e.Background(Col(p.Scheme.Container)).Radius(OverlayRadius).Border(0, ui.Transparent).NoShadow()
	elevationStyle(e, p, elevationDialog)
	if p.Dark {
		e.InsetShadow(0, 0, 1, 0, AlphaCol(0xffffff, 0.30))
	}
}

func fieldStyle(e *ui.Element, p *Palette) {
	surfaceStyle(e, p, p.Scheme.Container, FieldRadius)
	e.Font(UIFont).FontSize(14).TextColor(Col(p.Scheme.OnSurface)).
		FocusRingColor(Col(p.Scheme.Primary)).FocusRingOffset(2)
}

// HeroUI Slider: 20px default track, 28px accent thumb, 24x16 light inset.
// Only rendering lives here; each caller retains its release/keyboard contract.
func sliderVisual(c *ui.Context, p *Palette, w, h float32, fraction float64, enabled, pressed bool) {
	s := &p.Scheme
	track, fill := Col(s.SecondaryContainer), Col(s.Primary)
	if !enabled {
		track, fill = AlphaCol(s.SecondaryContainer, .5), AlphaCol(s.Primary, .5)
	}
	cy := h / 2
	tx := float32(14) + (w-28)*float32(clamp01(fraction))
	ui.Box(c).Absolute().Left(0).Top(cy - 10).Width(w).Height(20).Radius(12).Background(track)
	ui.Box(c).Absolute().Left(0).Top(cy - 10).Width(tx).Height(20).Radius(12).Background(fill)
	thumb := ui.Box(c).Absolute().Left(tx - 14).Top(cy - 10).Width(28).Height(20).Radius(12).Background(fill)
	thumb.Children(func() {
		tw, th := float32(24), float32(16)
		if pressed {
			tw, th = tw*.9, th*.9
		}
		e := ui.Box(c).Absolute().Left((28 - tw) / 2).Top((20 - th) / 2).Width(tw).Height(th).
			Radius(8).Background(Col(0xffffff))
		if !p.Dark {
			e.Shadow(0, 1, 2, 0, AlphaCol(0, .06))
		}
	})
}

// Segmented follows tabs.css: default track, 4px inset, 32px segments.
func Segmented(c *ui.Context, p *Palette, x, y, w float32, labels []string, selected int, onPick func(int)) {
	if len(labels) == 0 {
		return
	}
	s := &p.Scheme
	ui.Box(c).Absolute().Left(x).Top(y).Width(w).Height(40).Radius(20).Background(Col(s.SecondaryContainer)).Children(func() {
		x, y := float32(0), float32(0)
		tw := (w - 8) / float32(len(labels))
		if selected >= 0 && selected < len(labels) {
			marker := ui.Box(c).Key("segment-marker").Absolute().Left(x + 4).Top(y + 4).Width(tw).Height(32).IgnoreInput(true)
			surfaceStyle(&marker, p, s.Container, 24)
			marker.Translate(appSpringValue(c, marker, "segment-x", float32(selected)*tw, motionSettle, p.ReducedMotion), 0)
		}
		for i, label := range labels {
			bg, fg := ui.Transparent, s.OnSurfaceVariant
			if i == selected {
				fg = s.OnSurface
			}
			tab := ui.ButtonBase(c).Absolute().Left(x + 4 + float32(i)*tw).Top(y + 4).Width(tw).Height(32).
				Radius(24).Background(bg).Justify(ui.Center).AlignItems(ui.Center).Label(label).Cursor(ui.CursorPointer)
			hoverLayer(&tab, bg, s.OnSurface, true)
			if tab.Clicked() && onPick != nil {
				onPick(i)
			}
			tab.Children(func() {
				ui.Text(c, label).Font(textFamily(label)).FontSize(14).FontWeight(500).TextColor(Col(fg)).SingleLine()
			})
		}
	})
}

func CheckControl(c *ui.Context, p *Palette, label string, x, y float32, selected bool, onClick func()) {
	s := &p.Scheme
	hit := ui.ButtonBase(c).Absolute().Left(x).Top(y).Width(36).Height(36).Radius(12).Label(label).Cursor(ui.CursorPointer)
	if hit.Clicked() && onClick != nil {
		onClick()
	}
	hit.Children(func() {
		box := ui.Box(c).Absolute().Left(10).Top(10).Width(16).Height(16).Radius(6)
		surfaceStyle(&box, p, s.Container, 6)
		if selected {
			box.Background(Col(s.Primary))
		} else {
			box.Border(1, Col(s.Outline))
		}
		box.Children(func() {
			if selected {
				ui.Icon(c, Symbol("check")).FontSize(11).TextColor(Col(s.OnPrimary)).Absolute().Left(2.5).Top(2.5)
			}
		})
	})
}
