package ui

import (
	"image"
	"image/color"
	"testing"

	"github.com/egoist/mygo/ui"
)

func TestAlbumMaterialHasNoWhiteCornerCrescents(t *testing.T) {
	saved := coverCache
	defer func() { coverCache = saved }()
	album := image.NewNRGBA(image.Rect(0, 0, 32, 32))
	for y := 0; y < 32; y++ {
		for x := 0; x < 32; x++ {
			album.SetNRGBA(x, y, color.NRGBA{20, 40, 70, 255})
		}
	}
	coverCache.path = "corner-fixture"
	coverCache.bmp, coverCache.frost = ui.NewBitmap(album), ui.NewBitmap(album)
	a := newTestApp(t)
	a.State["musicPreview"] = map[string]any{"coverPath": "corner-fixture"}
	a.Palette = BuildPalette(&a.Prefs, a.settings(), a.State)
	tt := ui.NewTester(func(c *ui.Context) {
		ui.Box(c).Absolute().Fill().Background(Col(0xf5f5f5))
		a.overviewCardSurface(c, overviewRect{"music", 20, 20, 280, 264})
	}, 320, 304)
	tt.Frame()
	// These pixels are inside the continuous curve, outside the old circular clip.
	for _, pt := range []image.Point{{22, 32}, {297, 32}, {22, 271}, {297, 271}} {
		r, g, b, _ := tt.Image().At(pt.X, pt.Y).RGBA()
		if r > 0x9000 || g > 0x9000 || b > 0x9000 {
			t.Fatalf("bright corner crescent at %v: %d/%d/%d", pt, r, g, b)
		}
	}
}
