package ui

import (
	"encoding/json"
	"testing"

	"github.com/egoist/mygo/ui"
)

// Geometry is a fixed IPC fixture here. Native geometry is covered by the DLL
// acceptance; these tests exercise editor input and draft/history ownership.
func editorFixture(t *testing.T) *App {
	t.Helper()
	saved := nativeMusicLayout
	t.Cleanup(func() { nativeMusicLayout = saved })
	nativeMusicLayout = func(value any) (map[string]any, error) {
		layout := musicLayout{Version: 1, Modules: []musicSlot{{"header", true}, {"controls", true}}, CoverSize: 60, ShowCover: true, Controls: []int{4}}
		if value != nil {
			if err := json.Unmarshal([]byte(layoutJSON(value)), &layout); err != nil {
				return nil, err
			}
		}
		title := musicElement{ID: 1, Label: "歌曲标题", Resizable: true, X: 24, Y: 60, W: 300, H: 24}
		for _, placement := range layout.Elements {
			if placement.Element == title.ID {
				title.X, title.Y = placement.X, placement.Y
				if placement.Width != nil {
					title.W = *placement.Width
				}
			}
		}
		return map[string]any{"layout": layout, "elements": []musicElement{title}, "height": float64(200)}, nil
	}
	a := newTestApp(t)
	a.State["pendingPage"] = ""
	a.Page, a.SettingsTab = PageSettings, 135
	return a
}

func TestLayoutSlidersStayBesidePreviewAndCreateOneUndoPerGesture(t *testing.T) {
	a := editorFixture(t)
	tt := ui.NewTester(a.View, 1200, 800)
	tt.Frame()
	preview, ok := tt.Find("布局预览面板")
	if !ok {
		t.Fatal("preview missing")
	}
	r, ok := tt.Find("布局横向")
	if !ok || r.Y < preview.Y || r.Y+r.H > preview.Y+preview.H || r.X < preview.X+preview.W {
		t.Fatal("position slider escaped inspector", preview, r)
	}
	before := layoutJSON(a.musicEditor.Draft)
	tt.Press(r.X+r.W*.2, r.Y+r.H/2)
	tt.Frame()
	tt.Move(r.X+r.W*.6, r.Y+r.H/2)
	tt.Frame()
	if layoutJSON(a.musicEditor.Draft) == before || len(a.musicEditor.Undo) != 0 {
		t.Fatal("slider did not stream its draft during drag")
	}
	tt.Release(r.X+r.W*.6, r.Y+r.H/2)
	tt.Frame()
	if len(a.musicEditor.Undo) != 1 {
		t.Fatal("slider created more than one undo entry", len(a.musicEditor.Undo))
	}
	if err := tt.Click("布局撤销"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if layoutJSON(a.musicEditor.Draft) != before {
		t.Fatal("undo did not restore layout")
	}
	if a.musicEditor.Sliders["横向"].Value != 24 {
		t.Fatal("slider ignored undo snapshot")
	}
	r, _ = tt.Find("布局宽度")
	tt.ClickAt(r.X+r.W*.4, r.Y+r.H/2)
	tt.Frame()
	if a.musicEditor.Geometry[0].W == 300 {
		t.Fatal("clicking width slider did not change geometry")
	}
}

func TestMusicLayoutClickDragUndoAndRedo(t *testing.T) {
	a := editorFixture(t)
	tt := ui.NewTester(a.View, 1200, 800)
	tt.Frame()
	before := layoutJSON(a.musicEditor.Draft)
	r, ok := tt.Find("布局元素歌曲标题")
	if !ok {
		t.Fatal("layout element missing", tt.Texts())
	}
	x, y := r.X+r.W/2, r.Y+r.H/2
	tt.ClickAt(x, y)
	tt.Frame()
	if layoutJSON(a.musicEditor.Draft) != before || len(a.musicEditor.Undo) != 0 || a.musicEditor.Selected != 1 {
		t.Fatal("selection changed layout or created undo history")
	}
	tt.Press(x, y)
	tt.Frame()
	tt.Move(x+36, y+18)
	tt.Frame()
	tt.Release(x+36, y+18)
	tt.Frame()
	after := layoutJSON(a.musicEditor.Draft)
	if after == before || len(a.musicEditor.Undo) != 1 || !a.musicEditor.Draft.Enabled {
		t.Fatal("drag did not create one reversible edit", a.musicEditor)
	}
	if err := tt.Click("布局撤销"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if layoutJSON(a.musicEditor.Draft) != before || len(a.musicEditor.Draft.Elements) != 0 {
		t.Fatal("undo retained explicit placement or enabled state")
	}
	if err := tt.Click("布局重做"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if layoutJSON(a.musicEditor.Draft) != after {
		t.Fatal("redo lost dragged placement")
	}
	a.musicEditor.restoreSaved()
	if layoutJSON(a.musicEditor.Draft) != before {
		t.Fatal("restore saved retained omitted fields")
	}
}

func TestMusicLayoutDragHistoryIsBoundedAndBlockedWhileSaving(t *testing.T) {
	a := editorFixture(t)
	tt := ui.NewTester(a.View, 1200, 800)
	tt.Frame()
	e := a.musicEditor
	for i := 0; i < 90; i++ {
		element := musicElement{ID: 1, X: float64(24 + i%2*16), Y: 60}
		e.drag(element, 4, 0, true)
		e.drag(element, 0, 0, false)
	}
	if len(e.Undo) != 80 {
		t.Fatal("unbounded drag history", len(e.Undo))
	}
	e.Pending = true
	before := layoutJSON(e.Draft)
	e.change(func(l *musicLayout) { l.Enabled = !l.Enabled })
	e.drag(musicElement{ID: 1, X: 24, Y: 60}, 20, 20, true)
	e.history(false)
	if layoutJSON(e.Draft) != before {
		t.Fatal("draft changed while applying")
	}
}
