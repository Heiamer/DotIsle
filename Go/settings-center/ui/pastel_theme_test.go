package ui

import (
	"dotisle/settings/prefs"
	"github.com/egoist/mygo/ui"
	"image"
	"image/color"
	"image/png"
	"math"
	"os"
	"path/filepath"
	"testing"
)

func contrastRatio(a, b uint32) float64 {
	lum := func(c uint32) float64 {
		channel := func(v uint32) float64 {
			x := float64(v&255) / 255
			if x <= .04045 {
				return x / 12.92
			}
			return math.Pow((x+.055)/1.055, 2.4)
		}
		return .2126*channel(c>>16) + .7152*channel(c>>8) + .0722*channel(c)
	}
	x, y := lum(a), lum(b)
	return (max(x, y) + .05) / (min(x, y) + .05)
}

func TestPastelIsOptInAndKeepsReadablePairs(t *testing.T) {
	for _, dark := range []bool{false, true} {
		p := prefs.Default()
		p.Dark = dark
		settings := map[string]any{"ControlCenterMonetEnabled": true, "ControlCenterColorSource": "Content"}
		state := map[string]any{"artworkSeed": float64(0x00ff00)}
		if BuildPalette(&p, settings, state).Scheme != fallbackScheme(dark) {
			t.Fatal("old preference enabled beta without consent")
		}
		settings["ControlCenterDynamicColorEnabled"] = true
		for _, seed := range []uint32{0xff0000, 0x00ff00, 0x0000ff, 0xff66bb, 0, 0xffffff} {
			state["artworkSeed"] = float64(seed)
			s := BuildPalette(&p, settings, state).Scheme
			for _, pair := range [][2]uint32{{s.OnSurface, s.Container}, {s.Primary, s.OnPrimary}, {s.OnSecondaryContainer, s.SecondaryContainer}} {
				if ratio := contrastRatio(pair[0], pair[1]); ratio < 4.5 {
					t.Fatalf("low contrast %06x dark=%v pair=%06x/%06x ratio=%v", seed, dark, pair[0], pair[1], ratio)
				}
			}
			if !dark {
				hi, lo := max(s.Surface>>16&255, s.Surface>>8&255, s.Surface&255), min(s.Surface>>16&255, s.Surface>>8&255, s.Surface&255)
				if lo < 220 || hi-lo > 24 {
					t.Fatalf("surface lost pale restrained color: %06x", s.Surface)
				}
			}
		}
	}
}

func TestDeckEveryPageUsesTheSameBounds(t *testing.T) {
	a := newTestApp(t)
	a.overview.loaded = true
	a.overview.config = prefs.DefaultOverview()
	a.overview.config.View = "scroll"
	defer func() { deckPos, deckTarget, deckSpringIdle = 0, 0, true }()
	var first ui.Rect
	tt := ui.NewTester(a.View, 1200, 800)
	for i, key := range a.scrollPages() {
		a.deckTo(float64(i)*deckStep, true)
		tt.Frame()
		r, ok := tt.Find("浏览" + overviewName(key) + "卡片")
		if !ok {
			t.Fatal("page missing", key)
		}
		if i == 0 {
			first = r
		} else if r.W != first.W || r.H != first.H || r.X != first.X || r.Y != first.Y {
			t.Fatalf("page %s changed bounds: first=%v got=%v", key, first, r)
		}
	}
}

// An isolated synthetic album keeps screenshot verification independent of
// the user's current playback and makes color bleed/clipping visible.
func TestAlbumFrostAndPastelCaptures(t *testing.T) {
	out := os.Getenv("DOTISLE_DESIGN_CAPTURE")
	if out == "" {
		t.Skip("explicit visual audit")
	}
	RegisterFonts()
	if err := os.MkdirAll(out, 0755); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(t.TempDir(), "album.png")
	im := image.NewNRGBA(image.Rect(0, 0, 384, 384))
	for y := 0; y < 384; y++ {
		for x := 0; x < 384; x++ {
			ink := color.NRGBA{R: 28, G: 72, B: 133, A: 255}
			if x > 120 && y < 260 {
				ink = color.NRGBA{R: 213, G: 121, B: 169, A: 255}
			}
			if x < 170 && y > 210 {
				ink = color.NRGBA{R: 82, G: 156, B: 191, A: 255}
			}
			im.SetNRGBA(x, y, ink)
		}
	}
	f, _ := os.Create(path)
	png.Encode(f, im)
	f.Close()
	for _, mode := range []string{"cards", "scroll"} {
		a := newTestApp(t)
		a.State["pendingPage"] = ""
		a.State["title"], a.State["artist"] = "此刻，正在聆听", "专辑封面 · 隔离示例"
		a.State["musicPreview"] = map[string]any{"coverPath": path, "lyricFrame": map[string]any{"currentLine": "把这一刻留给自己", "currentTranslation": "A moment, just for you."}}
		a.overview.loaded = true
		a.overview.config = prefs.DefaultOverview()
		a.overview.config.View = mode
		deckPos, deckTarget, deckSpringIdle = 0, 0, true
		tt := ui.NewTester(a.View, 1200, 800)
		tt.Frame()
		if !tt.HasText("此刻，正在聆听") {
			t.Fatal("music body hidden by material")
		}
		f, err := os.Create(filepath.Join(out, "album-"+mode+".png"))
		if err != nil {
			t.Fatal(err)
		}
		png.Encode(f, tt.Image())
		f.Close()
	}
	for _, pair := range []struct{ name, color string }{{"blue", "#9ABFE8"}, {"green", "#A9D8C2"}, {"pink", "#E4B4CD"}} {
		a := newTestApp(t)
		a.State["pendingPage"] = ""
		a.settings()["ControlCenterDynamicColorEnabled"] = true
		a.settings()["ControlCenterColorSource"] = "Custom"
		a.settings()["ControlCenterCustomColor"] = pair.color
		tt := ui.NewTester(a.View, 1200, 800)
		tt.Frame()
		f, _ := os.Create(filepath.Join(out, "pastel-"+pair.name+".png"))
		png.Encode(f, tt.Image())
		f.Close()
	}
}
