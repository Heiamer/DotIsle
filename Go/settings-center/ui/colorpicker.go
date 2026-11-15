// Compact reference color card, with an HSV pad, pastel hue strip and HEX field.
package ui

import (
	"fmt"
	"math"
	"math/rand/v2"
	"strings"

	"github.com/egoist/mygo/ui"
)

var pickerPresets = [9]uint32{0xEE3F68, 0xCF4BE8, 0x8866ED, 0x32B68B, 0x26B4CD, 0x31B58A, 0x8ACB35, 0xF3A128, 0xFC722D}

type hsv struct{ h, s, v float64 }

func hsvFromRgb(rgb uint32) hsv {
	r := float64(rgb>>16&255) / 255
	g := float64(rgb>>8&255) / 255
	b := float64(rgb&255) / 255
	max, min := r, r
	if g > max {
		max = g
	}
	if b > max {
		max = b
	}
	if g < min {
		min = g
	}
	if b < min {
		min = b
	}
	delta := max - min
	v := max
	s := 0.0
	if max > 0 {
		s = delta / max
	}
	hh := 0.0
	if delta > 0 {
		switch max {
		case r:
			hh = (g - b) / delta
			if g < b {
				hh += 6
			}
		case g:
			hh = (b-r)/delta + 2
		default:
			hh = (r-g)/delta + 4
		}
		hh /= 6
	}
	return hsv{hh, s, v}
}

func (c hsv) rgb() uint32 {
	h := c.h - float64(int(c.h))
	i := int(h * 6)
	f := h*6 - float64(i)
	p := c.v * (1 - c.s)
	q := c.v * (1 - f*c.s)
	t := c.v * (1 - (1-f)*c.s)
	var r, g, b float64
	switch i % 6 {
	case 0:
		r, g, b = c.v, t, p
	case 1:
		r, g, b = q, c.v, p
	case 2:
		r, g, b = p, c.v, t
	case 3:
		r, g, b = p, q, c.v
	case 4:
		r, g, b = t, p, c.v
	default:
		r, g, b = c.v, p, q
	}
	return uint32(r*255+0.5)<<16 | uint32(g*255+0.5)<<8 | uint32(b*255+0.5)
}

// pickerState holds the draft until outside-click or Enter commits it.
type pickerState struct {
	key, title         string
	original, fallback uint32
	hsv                hsv
	hex, err           string
	anchor             ui.Rect
}

func (a *App) openPicker(key, title string, current, def uint32) {
	a.paletteOpen = false
	a.picker = &pickerState{
		key: key, title: title, original: current, fallback: def,
		hsv: hsvFromRgb(current), hex: fmt.Sprintf("#%06X", current),
	}
}

func (a *App) anchorPicker(e *ui.Element, key string) {
	if a.picker != nil && a.picker.key == key && a.picker.anchor.W == 0 {
		a.picker.anchor = e.Bounds()
	}
}

func (p *pickerState) setRGB(rgb uint32) {
	next := hsvFromRgb(rgb)
	if next.s == 0 {
		next.h = p.hsv.h
	}
	p.hsv, p.hex, p.err = next, fmt.Sprintf("#%06X", rgb), ""
}

func (p *pickerState) updateHex() {
	p.hex, p.err = fmt.Sprintf("#%06X", p.hsv.rgb()), ""
}

func (a *App) pickerSave() {
	p := a.picker
	if p == nil {
		return
	}
	rgb, ok := parseHexColor(strings.TrimSpace(p.hex))
	if !ok {
		p.err = "请输入 6 位 HEX 色值"
		return
	}
	if p.key == "IslandDynamicSeedColor" {
		a.pushNative("IslandColorSource", "Custom")
	}
	a.pushNative(p.key, fmt.Sprintf("#%06X", rgb))
	a.picker = nil
	a.toast, a.toastAt = "颜色已更新", a.frameNow
}

func (p *pickerState) bounds(w, h float32) ui.Rect {
	const pw, ph = float32(360), float32(540)
	x, y := (w-pw)/2, (h-ph)/2
	if p.anchor.W > 0 {
		x, y = p.anchor.X+p.anchor.W-pw, p.anchor.Y+p.anchor.H+10
		if y+ph > h-16 {
			y = p.anchor.Y - ph - 10
		}
		if y < TitlebarH+8 {
			y = h - ph - 16
		}
	}
	return ui.Rect{X: max(16, min(x, w-pw-16)), Y: max(TitlebarH+8, min(y, h-ph-16)), W: pw, H: ph}
}

func (a *App) colorPickerView(c *ui.Context, w, h float32) {
	p := a.picker
	if p == nil {
		return
	}
	previousHSV := p.hsv
	ui.Overlay(c, func() {
		back := ui.Box(c).Key("color-overlay").Absolute().Left(0).Top(0).Width(w).Height(h).
			Modal().Role(ui.RoleDialog).Label("调色板 · " + p.title)
		if back.OverlayShortcut(0, ui.KeyEscape) {
			a.picker = nil
			return
		}
		if back.Clicked() || back.OverlayShortcut(0, ui.KeyEnter) {
			a.pickerSave()
			if a.picker == nil {
				return
			}
		}
		back.Children(func() {
			r := p.bounds(w, h)
			card := ui.Box(c).Key("color-card").Absolute().Left(r.X).Top(r.Y).Width(r.W).Height(r.H).
				Radius(32).Background(Col(0xFFFFFF)).Label("颜色卡片")
			elevationStyle(&card, &a.Palette, elevationPopover)
			popoverMotion(c, &a.Palette, &card)
			_ = card.Clicked()
			card.Children(func() {
				for i, rgb := range pickerPresets {
					swatch := ui.ButtonBase(c).Key("preset-" + itoa(i)).Absolute().Left(17 + float32(i)*36).Top(16).
						Width(36).Height(40).Radius(20).Label(fmt.Sprintf("预设颜色 #%06X", rgb)).Cursor(ui.CursorPointer)
					if swatch.Clicked() {
						p.setRGB(rgb)
					}
					swatch.Children(func() {
						ui.Box(c).Absolute().Left(6).Top(8).Size(24, 24).Radius(12).Background(Col(rgb)).IgnoreInput(true)
					})
				}
				const padW, padH = float32(336), float32(336)
				pad := ui.Box(c).Key("color-sv").Absolute().Left(12).Top(60).Width(padW).Height(padH).
					Radius(24).Clip().Focusable().Label("饱和度与明度").Cursor(ui.CursorCrosshair)
				padClick(&pad, padW, padH, func(x, y float64) {
					p.hsv.s, p.hsv.v = x, 1-y
					p.updateHex()
				})
				if pad.Shortcut(0, ui.KeyLeft) {
					p.hsv.s = clamp01(p.hsv.s - .01)
					p.updateHex()
				}
				if pad.Shortcut(0, ui.KeyRight) {
					p.hsv.s = clamp01(p.hsv.s + .01)
					p.updateHex()
				}
				if pad.Shortcut(0, ui.KeyUp) {
					p.hsv.v = clamp01(p.hsv.v + .01)
					p.updateHex()
				}
				if pad.Shortcut(0, ui.KeyDown) {
					p.hsv.v = clamp01(p.hsv.v - .01)
					p.updateHex()
				}
				pad.Gradient(Col(0xFFFFFF), Col(hsv{p.hsv.h, 1, 1}.rgb()), 90)
				pad.Children(func() {
					ui.Box(c).Fill().Gradient(AlphaCol(0, 0), Col(0), 180).IgnoreInput(true)
					x := max(float32(12), min(float32(p.hsv.s)*padW, padW-12))
					y := max(float32(12), min(float32(1-p.hsv.v)*padH, padH-12))
					ui.Box(c).Absolute().Left(x-11).Top(y-11).Size(22, 22).Radius(11).
						Border(3.5, Col(0xFFFFFF)).Shadow(0, 1, 3, 0, AlphaCol(0, .24)).IgnoreInput(true)
				})

				const hueW, hueH = float32(264), float32(30)
				hue := ui.Box(c).Key("color-hue").Absolute().Left(18).Top(416).Width(hueW).Height(hueH).
					Radius(15).Clip().Focusable().Label("色相").Cursor(ui.CursorPointer)
				padClick(&hue, hueW, hueH, func(x, _ float64) {
					p.hsv.h = math.Mod(.94-x+1, 1)
					p.updateHex()
				})
				if hue.Shortcut(0, ui.KeyLeft) {
					p.hsv.h = clamp01(p.hsv.h - .005)
					p.updateHex()
				}
				if hue.Shortcut(0, ui.KeyRight) {
					p.hsv.h = clamp01(p.hsv.h + .005)
					p.updateHex()
				}
				stops := make([]ui.GradientStop, 7)
				for i := range stops {
					at := float64(i) / 6
					h := math.Mod(.94-at+1, 1)
					stops[i] = ui.GradientStop{Color: Col(hsv{h, .30, 1}.rgb()), At: float32(at)}
				}
				hue.LinearGradient(ui.LinearGradient{Angle: 90, Stops: stops})
				hue.Children(func() {
					x := 15 + float32(math.Mod(.94-p.hsv.h+1, 1))*(hueW-30)
					ui.Box(c).Absolute().Left(x-11).Top(4).Size(22, 22).Radius(11).
						Border(3.5, Col(0xFFFFFF)).Shadow(0, 1, 2, 0, AlphaCol(0, .16)).IgnoreInput(true)
				})
				shuffle := ui.ButtonBase(c).Key("color-shuffle").Absolute().Left(294).Top(407).Size(48, 48).
					Radius(24).Background(Col(0xEEEEEF)).Label("随机颜色").Tooltip("随机颜色").Cursor(ui.CursorPointer)
				hoverLayer(&shuffle, Col(0xEEEEEF), 0x1D1D1F, true)
				if shuffle.Clicked() {
					p.hsv = hsv{rand.Float64(), .5 + rand.Float64()*.4, .8 + rand.Float64()*.2}
					p.updateHex()
				}
				shuffle.Children(func() { Icon(c, "shuffle", 13, 13, 22, 0x29292B) })

				field := ui.Box(c).Key("color-field").Absolute().Left(18).Top(468).Width(324).Height(54).
					Radius(18).Background(Col(0xEEEEEF))
				if p.err != "" {
					field.Border(1, Col(0xE5484D))
				}
				field.Children(func() {
					reset := ui.ButtonBase(c).Key("color-reset").Absolute().Left(8).Top(7).Size(40, 40).
						Radius(20).Label("恢复默认颜色").Tooltip("恢复默认颜色").Cursor(ui.CursorPointer)
					if reset.Clicked() {
						p.setRGB(p.fallback)
					}
					reset.Children(func() {
						ui.Box(c).Absolute().Left(8).Top(8).Size(24, 24).Radius(12).Background(Col(p.hsv.rgb())).IgnoreInput(true)
					})
					previous := p.hex
					ui.TextInput(c, &p.hex).Absolute().Left(60).Top(8).Width(246).Height(38).
						Font(UIFont).FontSize(21).FontWeight(400).TextColor(Col(0x1D1D1F)).
						Background(ui.Transparent).Border(0, ui.Transparent).FocusRing(false).MaxLength(7).Label("HEX 色值").
						OnTextKey(func(k ui.TextKey) bool {
							if k.Key != ui.KeyEnter || k.Mods != 0 {
								return false
							}
							p.hex = k.Text()
							a.pickerSave()
							return true
						})
					if previous != p.hex {
						p.err = ""
						if rgb, ok := parseHexColor(p.hex); ok {
							next := hsvFromRgb(rgb)
							if next.s == 0 {
								next.h = p.hsv.h
							}
							p.hsv = next
						}
						c.AnimationFrame()
					}
				})
				if p.err != "" {
					Label(c, p.err, 26, 446, 11, 0xD33139, false, 258)
				}
			})
		})
	})
	if p.hsv != previousHSV {
		c.AnimationFrame()
	}
}

// PointerPosition is local to the element. Keep capture when dragging outside.
func padClick(e *ui.Element, w, h float32, onPoint func(fx, fy float64)) {
	button, pressed := e.PressedButton()
	x, y, _ := e.PointerPosition()
	if pressed && button == 0 {
		onPoint(clamp01(float64(x/w)), clamp01(float64(y/h)))
	}
}
