package ui

import (
	"github.com/egoist/mygo/ui"
	"image"
)

// Keep the 3.5.1 album material: frosted artwork, diagonal charcoal veil,
// and a second lower veil behind transport. Frost is prepared once per cover,
// from the already decoded and capped pixels the cover bitmap uses.
func frostPixels(src *image.NRGBA) (*ui.Bitmap, uint32) {
	w, h := fitWithin(src.Rect.Dx(), src.Rect.Dy(), 512, 512)
	pixels := scaleNRGBA(src, w, h)
	seed := albumSeed(pixels)
	return ui.NewBitmap(frosted(pixels, 10, 1)), seed
}

// Use the strongest hue cluster rather than averaging complementary colors
// into gray. The thumbnail is already bounded; this runs only on a new cover.
func albumSeed(pixels *image.NRGBA) uint32 {
	var weights [36]float64
	var colors [36][3]float64
	for y := 0; y < pixels.Rect.Dy(); y += 4 {
		for x := 0; x < pixels.Rect.Dx(); x += 4 {
			p := pixels.NRGBAAt(x, y)
			hi, lo := max(p.R, p.G, p.B), min(p.R, p.G, p.B)
			if p.A < 128 || hi-lo < 20 || hi < 35 {
				continue
			}
			rgb := uint32(p.R)<<16 | uint32(p.G)<<8 | uint32(p.B)
			bucket := min(int(colorHue(rgb)/10), 35)
			weight := float64(hi-lo) / 255
			weights[bucket] += weight
			colors[bucket][0] += float64(p.R) * weight
			colors[bucket][1] += float64(p.G) * weight
			colors[bucket][2] += float64(p.B) * weight
		}
	}
	best := 0
	for i := range weights {
		if weights[i] > weights[best] {
			best = i
		}
	}
	if weights[best] == 0 {
		return 0x9abfe8
	}
	return uint32(colors[best][0]/weights[best])<<16 | uint32(colors[best][1]/weights[best])<<8 | uint32(colors[best][2]/weights[best])
}

func (a *App) overviewCardSurface(c *ui.Context, r overviewRect) {
	if r.key != "music" || a.coverBitmap() == nil || coverCache.frost == nil {
		appleWidgetSurface(c, &a.Palette, r)
		return
	}
	// The material owns the surface. A white card beneath a circular clip
	// exposes four white crescents because continuous corners have a different path.
	bmp := coverCache.frost
	card := ui.Box(c).Absolute().Left(r.x).Top(r.y).Width(r.w).Height(r.h).
		Radius(CardRadius).Background(ui.Transparent).IgnoreInput(true)
	elevationStyle(&card, &a.Palette, elevationCard)
	card.Draw(func(pc *ui.Painter, box ui.Rect) {
		pc.ClipPath(continuousCardPath(box, CardRadius), func() {
			pc.Image(bmp, ui.Rect{X: box.X - 28, Y: box.Y - 28, W: box.W + 56, H: box.H + 56}, ui.Cover)
			pc.FillGradient(box, ui.LinearGradient{From: AlphaCol(0x080b14, .75), To: AlphaCol(0x080b14, .32), Angle: 135}, 0)
			lower := ui.Rect{X: box.X, Y: box.Y + max(float32(0), box.H-150), W: box.W, H: min(box.H, float32(150))}
			pc.FillGradient(lower, ui.LinearGradient{From: AlphaCol(0x080b14, 0), To: AlphaCol(0x080b14, .56), Angle: 180}, 0)
		})
	})
}

func (a *App) musicCardPalette() Palette {
	p := a.Palette
	if a.coverBitmap() != nil && coverCache.frost != nil {
		p.Dark = true
		p.Scheme = fallbackScheme(true)
		p.Scheme.OnSurface, p.Scheme.OnSurfaceVariant = 0xf5f5f7, 0xd4d6dd
		p.Scheme.Primary, p.Scheme.OnPrimary = 0xf5f5f7, 0x111319
		p.Scheme.Container, p.Scheme.High = 0x272a34, 0x343844
	}
	return p
}
