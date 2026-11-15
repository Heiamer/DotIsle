package ui

import (
	"math"
	"testing"
	"time"

	"github.com/egoist/mygo/ui"
)

func TestCompactSearchKeyboardAndDismissal(t *testing.T) {
	a := newTestApp(t)
	tt := ui.NewTester(a.View, 1200, 800)
	before := tt.Image().RGBAAt(100, 400)
	if err := tt.Click("搜索与跳转"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	card, ok := tt.Find("搜索卡片")
	if !ok || card.W > 340 || card.H > 340 || card.X < 800 {
		t.Fatal("search is not a compact anchored card", card)
	}
	if tt.HasText("Ctrl K") {
		t.Fatal("shortcut badge is still visible")
	}
	if tt.HasText("搜索...") {
		t.Fatal("the collapsed search entry duplicated the open input")
	}
	if _, ok := tt.Access("自定义概览组件"); ok {
		t.Fatal("background controls are still accessible")
	}
	if tt.Image().RGBAAt(100, 400) != before {
		t.Fatal("search dimmed the page")
	}
	tt.Key(0, ui.KeyDown)
	tt.Frame()
	tt.Key(0, ui.KeyEnter)
	tt.Frame()
	if a.paletteOpen || a.Page != PageWidgets {
		t.Fatal("keyboard navigation failed", a.Page, a.paletteSelected)
	}
	tt.Key(ui.Ctrl, ui.KeyK)
	tt.Frame()
	tt.Type("计时")
	tt.Frame()
	if len(a.paletteMatches()) != 1 {
		t.Fatal("search did not filter", a.paletteMatches())
	}
	tt.Key(0, ui.KeyEnter)
	tt.Frame()
	if a.Page != PageFocus || a.paletteOpen {
		t.Fatal("filtered navigation failed")
	}
	tt.Key(ui.Ctrl, ui.KeyK)
	tt.Frame()
	tt.ClickAt(200, 400)
	tt.Frame()
	if a.paletteOpen {
		t.Fatal("outside click did not dismiss search")
	}
	tt.Key(ui.Ctrl, ui.KeyK)
	tt.Frame()
	for i := 0; i < len(Pages)-1; i++ {
		tt.Key(0, ui.KeyDown)
		tt.Frame()
	}
	if _, ok := tt.Find("搜索结果 " + Pages[len(Pages)-1]); !ok {
		t.Fatal("last result is outside the visible search window")
	}
	tt.Key(0, ui.KeyEnter)
	tt.Frame()
	if a.Page != len(Pages)-1 || a.paletteOpen {
		t.Fatal("keyboard navigation did not reach the last route")
	}
}

func TestCompactColorPickerDragAndHexStaySynchronized(t *testing.T) {
	a := newTestApp(t)
	tt := ui.NewTester(a.View, 1200, 800)
	a.openPicker("TodoDefaultColor", "待办颜色", 0x0485F7, 0xF3D79B)
	tt.Frame()
	tt.Advance(400 * time.Millisecond)
	tt.Frame()
	card, ok := tt.Find("颜色卡片")
	if !ok || card.W != 360 || card.H != 540 {
		t.Fatal("unexpected color card bounds", card)
	}
	pad, ok := tt.Find("饱和度与明度")
	if !ok {
		t.Fatal("HSV pad missing")
	}
	tt.Press(pad.X+pad.W*.25, pad.Y+pad.H*.25)
	tt.Frame()
	if math.Abs(a.picker.hsv.s-.25) > .01 || math.Abs(a.picker.hsv.v-.75) > .01 {
		t.Fatal("pointer was not normalized in local coordinates", a.picker.hsv)
	}
	tt.Move(pad.X+pad.W+40, pad.Y+pad.H+40)
	tt.Frame()
	if a.picker.hsv.s != 1 || a.picker.hsv.v != 0 {
		t.Fatal("drag lost capture at the edge", a.picker.hsv)
	}
	tt.Release(pad.X+pad.W+40, pad.Y+pad.H+40)
	tt.Frame()
	if err := tt.Click("HEX 色值"); err != nil {
		t.Fatal(err)
	}
	tt.Key(ui.Ctrl, ui.KeyA)
	tt.Type("#0485F7")
	tt.Frame()
	if a.picker.hsv.rgb() != 0x0485F7 {
		t.Fatal("HEX did not update HSV", a.picker.hex, a.picker.hsv)
	}
	if err := tt.Click("恢复默认颜色"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if a.picker.hex != "#F3D79B" {
		t.Fatal("reset did not restore the setting default")
	}
	tt.ClickAt(100, 400)
	tt.Frame()
	if a.picker != nil {
		t.Fatal("outside click did not save and dismiss the picker")
	}
}

func TestCompactColorPickerRejectsInvalidHexAndEscapeCancels(t *testing.T) {
	a := newTestApp(t)
	tt := ui.NewTester(a.View, 1200, 800)
	a.openPicker("TodoDefaultColor", "待办颜色", 0x0485F7, 0xF3D79B)
	tt.Frame()
	if err := tt.Click("HEX 色值"); err != nil {
		t.Fatal(err)
	}
	tt.Key(ui.Ctrl, ui.KeyA)
	tt.Type("#XYZ")
	tt.Frame()
	tt.ClickAt(100, 400)
	tt.Frame()
	if a.picker == nil || a.picker.err == "" {
		t.Fatal("invalid HEX was accepted")
	}
	tt.Key(0, ui.KeyEscape)
	tt.Frame()
	if a.picker != nil {
		t.Fatal("Escape did not cancel the draft")
	}
}
