package ui

import (
	"image/png"
	"math"
	"os"
	"path/filepath"
	"reflect"
	"testing"
	"time"

	"dotisle/settings/prefs"
	"github.com/egoist/mygo/ui"
)

func TestCardResizePersistsAndCancellationKeepsSavedSize(t *testing.T) {
	for _, cancel := range []string{"", "escape", "capture"} {
		t.Run(cancel, func(t *testing.T) {
			a := newTestApp(t)
			a.loadOverview()
			a.overview.edit = true
			tt := ui.NewTester(a.View, 1200, 800)
			r, ok := tt.Find("调整大小音乐")
			if !ok {
				t.Fatal("missing resize grip")
			}
			tt.Press(r.X+17, r.Y+17)
			tt.Move(r.X-100, r.Y+89)
			tt.Frame()
			if a.overview.resizing != "music" || a.overview.resizeH <= overviewCardHeight {
				t.Fatal("resize did not track pointer")
			}
			if cancel == "escape" {
				tt.Key(0, ui.KeyEscape)
			}
			if cancel == "capture" {
				tt.CancelPress()
			}
			tt.Release(r.X-100, r.Y+89)
			tt.Frame()
			saved, err := prefs.LoadOverview(filepath.Join(a.Data, "center-overview.json"))
			if err != nil {
				t.Fatal(err)
			}
			if cancel != "" {
				if len(saved.Sizes) != 0 || a.overview.resizing != "" {
					t.Fatal("canceled resize was saved or remained active")
				}
				return
			}
			size := saved.Sizes["music"]
			if size.Height != 336 || math.Abs(float64(size.Width-.25)) > .001 {
				t.Fatalf("wrong persisted size: %+v", size)
			}
			for _, width := range []float32{600, 896, 1136} {
				rects := overviewLayoutSized(saved.Cards, saved.Sizes, 4, 56, width)
				for i, r := range rects {
					if r.x < 4 || r.x+r.w > width+4.01 {
						t.Fatal("card outside canvas", r)
					}
					for _, other := range rects[i+1:] {
						if r.x < other.x+other.w && r.x+r.w > other.x && r.y < other.y+other.h && r.y+r.h > other.y {
							t.Fatal("resized cards overlap", r, other)
						}
					}
				}
			}
		})
	}
}

func TestResetCardLayoutAlsoResetsSizes(t *testing.T) {
	a := newTestApp(t)
	a.loadOverview()
	a.overview.config.Sizes = map[string]prefs.CardSize{"music": {Width: 1, Height: 560}}
	a.overview.catalogMode = "cards"
	a.resetCatalog()
	saved, err := prefs.LoadOverview(filepath.Join(a.Data, "center-overview.json"))
	if err != nil || !reflect.DeepEqual(saved, prefs.DefaultOverview()) {
		t.Fatal("default layout retained custom sizes", saved, err)
	}
}

func TestSignaturePaintAndOpeningLifecycle(t *testing.T) {
	RegisterFonts()
	a := newTestApp(t)
	a.Palette = BuildPalette(&a.Prefs, a.settings(), a.State)
	// A partial first letter must paint at the centered destination, not its
	// untransformed source coordinates. This catches the original clipped ink.
	tt := ui.NewTester(func(c *ui.Context) {
		ui.Box(c).Fill().Background(ui.RGB(255, 255, 255))
		a.drawSignature(c, 136, 60, 328, 166, .28, false)
	}, 600, 220)
	dark := 0
	img := tt.Image()
	for y := 45; y < 215; y++ {
		for x := 130; x < 470; x++ {
			r, g, b, _ := img.At(x, y).RGBA()
			if r < 32000 && g < 32000 && b < 32000 {
				dark++
			}
		}
	}
	if dark < 50 {
		t.Fatalf("partial signature missing: %d ink pixels", dark)
	}
	for _, reduced := range []bool{false, true} {
		a = newTestApp(t)
		a.Prefs.ReducedMotion = reduced
		a.StartSignature()
		tt = ui.NewTester(a.View, 1200, 800)
		if _, ok := tt.Find("跳过开屏动画"); !ok {
			t.Fatal("opening was never connected to the view")
		}
		tt.Advance(time.Duration((signatureDuration(reduced) + .02) * float64(time.Second)))
		tt.Frame()
		if !a.signatureStart.IsZero() {
			t.Fatal("opening did not settle")
		}
		a.StartSignature()
		tt.Frame()
		tt.Key(0, ui.KeyEscape)
		tt.Frame()
		if !a.signatureStart.IsZero() {
			t.Fatal("escape did not skip opening")
		}
	}
}

func TestSeekbarTracksFirstEnabledPacket(t *testing.T) {
	p := BuildPalette(&prefs.Preferences{}, nil, nil)
	value, enabled := 0.0, false
	tt := ui.NewTester(func(c *ui.Context) { Seekbar(c, &p, "packet-sync-regression", 10, 10, 200, 20, value, enabled, nil) }, 230, 40)
	value, enabled = .6, true
	tt.Frame()
	tt.Frame()
	r, g, b, _ := tt.Image().At(60, 20).RGBA()
	if b <= r || b <= g {
		t.Fatal("newly enabled seekbar did not paint its current blue fraction")
	}
}

// Explicit visual evidence for new cards and the signature, using the production
// view. Hardware Mica and the Rust desktop renderer are validated separately.
func TestResumeCaptures(t *testing.T) {
	out := os.Getenv("DOTISLE_RESUME_CAPTURE")
	if out == "" {
		t.Skip("explicit captures only")
	}
	RegisterFonts()
	if err := os.MkdirAll(out, 0755); err != nil {
		t.Fatal(err)
	}
	capture := func(name string, tt *ui.Tester) {
		t.Helper()
		f, err := os.Create(filepath.Join(out, name+".png"))
		if err != nil {
			t.Fatal(err)
		}
		defer f.Close()
		if err = png.Encode(f, tt.Image()); err != nil {
			t.Fatal(err)
		}
	}
	for _, dark := range []bool{false, true} {
		a := newTestApp(t)
		a.settings()["SettingsTheme"] = "Light"
		suffix := "light"
		if dark {
			a.settings()["SettingsTheme"] = "Dark"
			suffix = "dark"
		}
		a.loadOverview()
		a.overview.config.Cards = []string{"wireless", "agenda", "cpu", "memory", "week"}
		a.State["systemMetrics"] = map[string]any{"cpuPercent": float64(38), "memoryPercent": float64(62), "wirelessDevices": []any{map[string]any{"id": "fixture-mouse", "name": "无线鼠标", "kind": "mouse", "connected": true, "batteryPercent": float64(76)}, map[string]any{"id": "fixture-headset", "name": "蓝牙耳机", "kind": "headphones", "connected": true}}}
		a.State["todos"] = []any{map[string]any{"id": "fixture-todo", "title": "整理本周安排", "date": "2026-11-15", "time": "10:00"}}
		tt := ui.NewTester(a.View, 1200, 800)
		capture("new-cards-"+suffix, tt)
		a.StartSignature()
		tt.Frame()
		for i := 0; i < 5; i++ {
			tt.Advance(180 * time.Millisecond)
			capture("opening-"+suffix+"-"+itoa(i), tt)
		}
	}
}
