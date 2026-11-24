// Native control primitives use HeroUI v3 Default metrics and surfaces.
// Source rules and the screenshot overrides are documented in docs/HEROUI-DEFAULT.md.
// Layout values are float32 DIPs, matching the MyGo APIs.
package ui

import (
	"fmt"
	"math"
	"unicode"

	"github.com/egoist/mygo/ui"
)

func textFamily(value string) string {
	for _, r := range value {
		if unicode.Is(unicode.Han, r) {
			return "Noto Sans SC, Microsoft YaHei UI, sans-serif"
		}
	}
	return UIFont
}

// Col wraps an opaque RGB color.
func Col(rgb uint32) ui.Color {
	return ui.RGBA(uint8(rgb>>16&255), uint8(rgb>>8&255), uint8(rgb&255), 1)
}

// AlphaCol wraps an RGB color with a separate alpha, like draw::alpha.
func AlphaCol(rgb uint32, a float64) ui.Color {
	return ui.RGBA(uint8(rgb>>16&255), uint8(rgb>>8&255), uint8(rgb&255), float32(a))
}

// MixCol blends two opaque colors, identical to widgets::Ui::mix.
func MixCol(a, b uint32, t float64) ui.Color {
	return Col(mix(a, b, t))
}

// The fixed line box keeps the original page measurements stable.
func LineHeight(size float32) float32 { return size * 1.35 }

// textLine uses the embedded Noto Sans SC family for Latin and Chinese content.
func textLine(c *ui.Context, text string, size float32, rgb uint32, bold bool) ui.Element {
	t := ui.Text(c, text).Font(textFamily(text)).FontSize(size).FixedLineHeight(LineHeight(size)).TextColor(Col(rgb))
	if bold {
		t = t.FontWeight(500)
	}
	return t
}

// Label places one text line at (x, y) with the Vello label semantics; maxw cuts the
// line with an ellipsis like the Vello clip.
func Label(c *ui.Context, text string, x, y, size float32, rgb uint32, bold bool, maxw float32) *ui.Element {
	e := ui.Box(c).Absolute().Left(x).Top(y).Width(maxw)
	e.Children(func() {
		line := ui.Text(c, text).Font(textFamily(text)).FontSize(size).FixedLineHeight(LineHeight(size)).
			TextColor(Col(rgb)).SingleLine()
		if bold {
			line.FontWeight(500)
		}
	})
	return &e
}

// CenteredText centers one line in a rect (widgets::Ui::center).
func CenteredText(c *ui.Context, text string, x, y, w, h, size float32, rgb uint32, bold bool) *ui.Element {
	e := ui.Box(c).Absolute().Left(x).Top(y).Width(w).Height(h).
		Justify(ui.Center).AlignItems(ui.Center)
	e.Children(func() {
		line := ui.Text(c, text).Font(textFamily(text)).FontSize(size).FixedLineHeight(LineHeight(size)).TextColor(Col(rgb))
		if bold {
			line.FontWeight(500)
		}
	})
	return &e
}

// Icon draws a Material Symbol at (x, y) with a size and color.
func Icon(c *ui.Context, name string, x, y, size float32, rgb uint32) *ui.Element {
	e := ui.Box(c).Absolute().Left(x).Top(y).Width(size).Height(size).
		Justify(ui.Center).AlignItems(ui.Center)
	if svg := Symbol(name); svg != nil {
		e.Children(func() {
			ui.Icon(c, svg).FontSize(size).TextColor(Col(rgb))
		})
	}
	return &e
}

// Panel applies the shared Default card surface.
func Panel(c *ui.Context, p *Palette, x, y, w, h float32, rgb uint32) *ui.Element {
	e := ui.Box(c).Absolute().Left(x).Top(y).Width(w).Height(h).
		Background(Col(rgb))
	continuousCardStyle(&e, p, rgb, CardRadius)
	return &e
}

// hoverLayer composites feedback over the real fill, preserving opaque buttons.
func hoverLayer(e *ui.Element, base ui.Color, fg uint32, enabled bool) {
	if !enabled {
		return
	}
	a := 0.0
	if e.Hovered() {
		a = 0.08
	}
	if e.Pressed() {
		a = 0.12
	}
	if a == 0 {
		return
	}
	// mygo Background replaces the fill (element.go e.bg = c), so the state
	// layer must be composited over the base here: a bare alpha tint would
	// erase solid fills and leave white-on-white controls invisible while the
	// pointer rests on them (widgets.rs blends the state layer over the fill).
	e.Background(overCol(base, AlphaCol(fg, a)))
}

// overCol composites tint over base with source-over alpha, like draw::over.
func overCol(base, tint ui.Color) ui.Color {
	if base.A == 0 {
		return tint
	}
	ta, ba := float64(tint.A)/255, float64(base.A)/255
	oa := ta + ba*(1-ta)
	if oa <= 0 {
		return ui.Transparent
	}
	ch := func(tc, bc uint8) uint8 {
		return uint8(math.Round((float64(tc)*ta + float64(bc)*ba*(1-ta)) / oa))
	}
	return ui.RGBA(ch(tint.R, base.R), ch(tint.G, base.G), ch(tint.B, base.B), float32(oa))
}

// FieldOutline strokes the active field of a dialog with the 2px primary line of
// views.rs draw::outline. The box is built on every frame, transparent while the
// field is unfocused, rather than left out of the tree while it is unfocused:
// MyGo identifies an element by its parent and its index among that parent's
// children (mygo ui/context.go: e.id = mix(p.id, uint64(p.nchild)+...)), so a
// decoration inserted on focus shifts the identity of every later sibling, the
// press already bound to a button below the field (mygo ui/input.go pointerDown ->
// rt.pressed) is never read by the button the next frame builds, and the click is
// swallowed until the second press. Keep the panel's child count constant, and
// IgnoreInput so the stroke over the field does not take the field's own press.
func FieldOutline(c *ui.Context, x, y, w, h, radius float32, focused bool, rgb uint32) {
	border := ui.Color{}
	if focused {
		border = Col(rgb)
	}
	ui.Box(c).Absolute().Left(x).Top(y).Width(w).Height(h).
		Radius(radius).Border(2, border).IgnoreInput(true)
}

// Button is the minimalist monochrome pill button: obsidian black in light mode,
// pure ivory white in dark mode; tonal secondary uses frosted glass.
func Button(c *ui.Context, p *Palette, label string, x, y, w, h float32, text, iconName string, tonal, enabled bool, onClick func()) *ui.Element {
	s := &p.Scheme
	var bgRGB, fgRGB uint32 = s.SecondaryContainer, s.OnSurface
	if enabled {
		if tonal {
			bgRGB, fgRGB = s.SecondaryContainer, s.OnSecondaryContainer
		} else {
			bgRGB, fgRGB = s.Primary, s.OnPrimary
		}
	}
	e := ui.ButtonBase(c).Absolute().Left(x).Top(y).Width(w).Height(h).
		Justify(ui.Center).AlignItems(ui.Center).Gap(8).
		Radius(ButtonRadius).Label(label).Cursor(ui.CursorPointer)
	if !enabled {
		e.Background(Col(bgRGB)).Disabled(true).Opacity(.5)
	} else if !tonal && !p.HighContrast {
		bg := uint32(0x16181D)
		fgRGB = 0xFFFFFF
		rim := AlphaCol(0xFFFFFF, 0.22)
		if p.Dark {
			bg, fgRGB = 0xF3F4F6, 0x111316
			rim = AlphaCol(0xFFFFFF, 0.90)
		}
		if e.Hovered() {
			if p.Dark {
				bg = 0xFFFFFF
			} else {
				bg = 0x262932
			}
		}
		if e.Pressed() {
			if p.Dark {
				bg = 0xD9DCE3
			} else {
				bg = 0x0B0C0F
			}
		}
		e.Background(Col(bg)).
			Border(1, AlphaCol(0x111827, 0.14)).
			InsetShadow(0, 1, 0, 0, rim)
		elevationStyle(&e, p, elevationControl)
	} else if tonal && !p.HighContrast {
		base := AlphaCol(0xFFFFFF, 0.64)
		border := AlphaCol(0x1E293B, 0.11)
		rim := AlphaCol(0xFFFFFF, 0.96)
		if p.Dark {
			base = AlphaCol(0xFFFFFF, 0.07)
			border = AlphaCol(0xFFFFFF, 0.10)
			rim = AlphaCol(0xFFFFFF, 0.14)
		}
		e.Background(base).
			Border(1, border).
			InsetShadow(0, 1, 0, 0, rim)
		elevationStyle(&e, p, elevationControl)
		hoverLayer(&e, base, fgRGB, true)
	} else {
		e.Background(Col(bgRGB))
		hoverLayer(&e, Col(bgRGB), fgRGB, true)
	}
	pressMotion(c, p, &e, enabled)
	if enabled && onClick != nil && e.Clicked() {
		onClick()
	}
	e.Children(func() {
		if iconName != "" {
			ui.Icon(c, Symbol(iconName)).FontSize(16).TextColor(Col(fgRGB))
		}
		ui.Text(c, text).Font(textFamily(text)).FontSize(14).FixedLineHeight(LineHeight(14)).TextColor(Col(fgRGB)).FontWeight(500).SingleLine()
	})
	return &e
}

// IconButton is the minimalist glass icon control.
func IconButton(c *ui.Context, p *Palette, label string, x, y, w, h float32, iconName string, filled, enabled bool, onClick func()) *ui.Element {
	s := &p.Scheme
	var fgRGB uint32 = s.OnSurfaceVariant
	var bg ui.Color = ui.Transparent
	if filled && enabled {
		bg, fgRGB = Col(s.Primary), s.OnPrimary
	} else if !enabled {
		bg = Col(s.High)
	}
	e := ui.ButtonBase(c).Absolute().Left(x).Top(y).Width(w).Height(h).
		Justify(ui.Center).AlignItems(ui.Center).
		Radius(ButtonRadius).Background(bg).
		Label(label).Cursor(ui.CursorPointer)
	if !enabled {
		e.Disabled(true).Opacity(.5)
	} else if filled && !p.HighContrast {
		fill := uint32(0x16181D)
		fgRGB = 0xFFFFFF
		rim := AlphaCol(0xFFFFFF, 0.20)
		if p.Dark {
			fill, fgRGB = 0xF3F4F6, 0x111316
			rim = AlphaCol(0xFFFFFF, 0.90)
		}
		if e.Hovered() {
			if p.Dark {
				fill = 0xFFFFFF
			} else {
				fill = 0x262932
			}
		}
		e.Background(Col(fill)).InsetShadow(0, 1, 0, 0, rim)
		elevationStyle(&e, p, elevationControl)
	} else {
		if !p.HighContrast && e.Hovered() {
			glassHover := AlphaCol(0x111827, 0.055)
			if p.Dark {
				glassHover = AlphaCol(0xFFFFFF, 0.10)
			}
			e.Background(glassHover)
		} else {
			hoverLayer(&e, bg, fgRGB, enabled)
		}
	}
	pressMotion(c, p, &e, enabled)
	if enabled && onClick != nil && e.Clicked() {
		onClick()
	}
	e.Children(func() {
		ui.Icon(c, Symbol(iconName)).FontSize(18).TextColor(Col(fgRGB))
	})
	return &e
}

// Chip is the selection pill: minimalist muted unselected, pure white floating when selected.
func Chip(c *ui.Context, p *Palette, label string, x, y, w, h float32, text string, selected bool, onClick func()) *ui.Element {
	s := &p.Scheme
	radius, bg, fg := float32(24), Col(s.SecondaryContainer), Col(s.OnSurfaceVariant)
	if !p.HighContrast {
		if p.Dark {
			bg = AlphaCol(0xFFFFFF, 0.06)
		} else {
			bg = AlphaCol(0x111827, 0.05)
		}
	}
	if selected {
		bg, fg = Col(s.Container), Col(s.OnSurface)
	}
	e := ui.ButtonBase(c).Absolute().Left(x).Top(y).Width(w).Height(h).
		Justify(ui.Center).AlignItems(ui.Center).
		Radius(radius).Background(bg).
		Label(label).Cursor(ui.CursorPointer)
	if selected {
		surfaceStyle(&e, p, s.Container, radius)
	} else if !p.HighContrast {
		e.Border(1, AlphaCol(0x111827, 0.05))
		if p.Dark {
			e.Border(1, AlphaCol(0xFFFFFF, 0.08))
		}
	}
	hoverLayer(&e, bg, s.OnSurface, true)
	if onClick != nil && e.Clicked() {
		onClick()
	}
	e.Children(func() {
		t := ui.Text(c, text).Font(textFamily(text)).FontSize(14).FixedLineHeight(LineHeight(14)).TextColor(fg).SingleLine()
		if selected {
			t.FontWeight(500)
		}
	})
	return &e
}

// Switch uses an optical 42x22 track with an independent key per toggle and centered 16x16 circular puck.
func Switch(c *ui.Context, p *Palette, label string, x, y float32, on bool, onClick func()) *ui.Element {
	s := &p.Scheme
	track := s.SecondaryContainer
	if on {
		track = s.Primary
	}
	keyBase := fmt.Sprintf("%.0f-%.0f-%s", x, y, label)
	e := ui.ButtonBase(c).Key("switch-" + keyBase).Absolute().Left(x).Top(y + 2).Width(42).Height(22).
		Radius(11).Label(label).Cursor(ui.CursorPointer)
	thumbCol := uint32(0xFFFFFF)
	var borderCol ui.Color
	hasBorder := false
	if !p.HighContrast && on {
		onBg := uint32(0x181A1F)
		borderCol = AlphaCol(0x111827, 0.10)
		if p.Dark {
			onBg = 0xF3F4F6
			thumbCol = 0x111316
			borderCol = AlphaCol(0xFFFFFF, 0.18)
		}
		if e.Hovered() {
			if p.Dark {
				onBg = 0xFFFFFF
			} else {
				onBg = 0x262932
			}
		}
		e.Background(Col(onBg)).InsetShadow(0, 1, 0, 0, AlphaCol(0xFFFFFF, 0.25))
		hasBorder = true
	} else if !p.HighContrast && !on {
		offBg := AlphaCol(0x0F172A, 0.16)
		borderCol = AlphaCol(0x0F172A, 0.09)
		if p.Dark {
			offBg = AlphaCol(0xFFFFFF, 0.14)
			borderCol = AlphaCol(0xFFFFFF, 0.12)
			thumbCol = 0xE5E7EB
		}
		e.Background(offBg)
		hoverLayer(&e, offBg, s.OnSurface, true)
		hasBorder = true
	} else {
		e.Background(Col(track))
		hoverLayer(&e, Col(track), s.OnSurface, true)
	}
	if onClick != nil && e.Clicked() {
		onClick()
	}
	thumbOffset := float32(0)
	if on {
		thumbOffset = 20
	}
	e.Children(func() {
		if hasBorder {
			ui.Box(c).Absolute().Fill().Radius(11).Border(1, borderCol).IgnoreInput(true)
		}
		thumb := ui.Box(c).Key("switch-thumb-" + keyBase).Absolute().Left(3).Top(3).Width(16).Height(16).
			Radius(8).Background(Col(thumbCol)).IgnoreInput(true)
		thumb.Translate(appSpringValue(c, thumb, "thumb-x", thumbOffset, motionSettle, p.ReducedMotion), 0)
		if !p.Dark {
			thumb.Shadow(0, 1, 3, 0, AlphaCol(0x111827, .18))
		} else {
			thumb.Shadow(0, 1, 3, 0, AlphaCol(0x000000, .30))
		}
	})
	return &e
}

// ToggleRow is a settings row: title 16 at y+9, detail 12 at y+36, switch right.
func ToggleRow(c *ui.Context, p *Palette, label string, x, y, w float32, title, detail string, on bool, onClick func()) *ui.Element {
	Label(c, title, x, y+9, 14, p.Scheme.OnSurface, false, w-100)
	Label(c, detail, x, y+32, 12, p.Scheme.OnSurfaceVariant, false, w-100)
	return Switch(c, p, label, x+w-52, y+8, on, onClick)
}

// Slider is the 5px track with a 22px thumb (widgets::Ui::slider look). Dragging
// previews locally; releasing commits once — the shell.rs pointer-up path — so a
// slow pipe round trip never snaps the knob back mid-drag. Idle frames follow the
// committed value when it changed elsewhere (snapshot sync).
type sliderDrag struct {
	frac    float64
	prev    float64
	hadPrev bool
	commit  float64
	saw     bool
	held    bool // the pointer was down in some build since the last release
}

var sliderDrags = map[string]*sliderDrag{}

func Slider(c *ui.Context, p *Palette, label string, x, y, w, h float32, committed float64, enabled bool, onCommit func(fraction float64)) *ui.Element {
	return sliderControl(c, p, label, x, y, w, h, committed, enabled, onCommit, sliderVisual)
}

// Seekbar keeps the native captured drag and keyboard contract with Kore's thin track.
func Seekbar(c *ui.Context, p *Palette, label string, x, y, w, h float32, committed float64, enabled bool, onCommit func(float64)) *ui.Element {
	return sliderControl(c, p, label, x, y, w, h, committed, enabled, onCommit, koreSeekVisual)
}

func sliderControl(c *ui.Context, p *Palette, label string, x, y, w, h float32, committed float64, enabled bool, onCommit func(float64), visual func(*ui.Context, *Palette, float32, float32, float64, bool, bool)) *ui.Element {
	st := sliderDrags[label]
	if st == nil {
		st = &sliderDrag{frac: clamp01(committed), commit: committed, saw: true}
		sliderDrags[label] = st
	}
	var e ui.Element
	if enabled {
		knob := ui.SliderBase(c, &st.frac, 0, 1).Absolute().Left(x).Top(y).Width(w).Height(h).
			Label(label).FocusRing(false)
		// Capture-aware: Pressed() drops the press outside the track rect and
		// would end the gesture (and commit) while the pointer is still down.
		btn, down := knob.PressedButton()
		pressed := down && btn == 0
		frac := st.frac
		prev, hadPrev := st.prev, st.hadPrev
		if pressed {
			// While the pointer is down the slot is the live preview; the snapshot
			// only matters for a rejected-commit snap-back after release.
			st.held = true
			st.commit = clamp01(committed)
			st.saw = true
		} else if st.held {
			// Pointer-up ends the gesture: commit the final slot exactly once,
			// even when the last move was already built before the up arrived
			// (a paused real release, or every PostMessage-driven drag). Requiring
			// frac != prev here missed those releases and snapped the knob back.
			st.held = false
			if onCommit != nil {
				onCommit(clamp01(st.frac))
			}
			st.commit = clamp01(st.frac)
			st.saw = true
		} else if hadPrev && frac != prev && knob.Focused() && onCommit != nil {
			// Keyboard moved the slot between builds (arrows need focus). Without
			// the focus gate, any spurious slot write - e.g. an automation nudge -
			// would commit a value the user never touched.
			onCommit(clamp01(frac))
			st.commit = clamp01(frac)
			st.saw = true
		} else {
			// Idle: follow the committed value when it changed elsewhere.
			if !st.saw || st.commit != clamp01(committed) {
				st.commit = clamp01(committed)
				st.saw = true
				if !hadPrev || frac == prev {
					st.frac = clamp01(committed)
				}
			}
		}
		knob.Children(func() { visual(c, p, w, h, st.frac, true, pressed) })
		st.prev = st.frac
		st.hadPrev = true
		e = knob
	} else {
		box := ui.Box(c).Absolute().Left(x).Top(y).Width(w).Height(h)
		box.Children(func() { visual(c, p, w, h, committed, false, false) })
		e = box
	}
	return &e
}

func clamp01(v float64) float64 {
	if v < 0 {
		return 0
	}
	if v > 1 {
		return 1
	}
	return v
}
