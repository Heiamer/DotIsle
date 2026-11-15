// UI state-logic tests: verify App state transitions and payload correctness
// without rendering (NewTester needs a GPU context which is unavailable in CI).
package ui

import (
	"strings"
	"testing"

	"dotisle/settings/prefs"
	"github.com/egoist/mygo/ui"
)

func newTestApp(t *testing.T) *App {
	return &App{
		Prefs:       prefs.Default(),
		Data:        t.TempDir(),
		SettingsTab: TabRoot,
		State: map[string]any{
			"settings": map[string]any{
				"SettingsTheme":                       "System",
				"IslandSizeMode":                      "Classic",
				"IslandFontFamily":                    "Noto Sans SC",
				"OffsetMilliseconds":                  float64(0),
				"HeadphoneConnectionRemindersEnabled": true,
				"IslandDynamicSeedColor":              "#A9E6CD",
				"TodoDefaultColor":                    "#F3D79B",
			},
			"todos":               []any{},
			"timers":              []any{},
			"desktopWidgets":      []any{},
			"notificationHistory": []any{},
			"appRules":            []any{},
			"version":             "5.0.0",
			"isPlaying":           false,
			"canSeek":             false,
			"pendingPage":         "island",
		},
		Connected: true,
	}
}

func TestNavRouting(t *testing.T) {
	app := newTestApp(t)
	app.openSettings(240)
	if app.SettingsTab != 240 {
		t.Errorf("openSettings(240) got tab %d", app.SettingsTab)
	}
	app.openSettings(128)
	if app.SettingsTab != 128 || app.IslandTab != 0 {
		t.Errorf("openSettings(128) got tab %d island %d", app.SettingsTab, app.IslandTab)
	}
	app.openSettings(243)
	if app.SettingsTab != 8 {
		t.Errorf("openSettings(243) got tab %d", app.SettingsTab)
	}
	app.openSettings(TabRoot)
	if app.SettingsTab != TabRoot {
		t.Errorf("openSettings(255) got tab %d", app.SettingsTab)
	}
}

func TestThemeDarkToggle(t *testing.T) {
	app := newTestApp(t)
	if app.Prefs.Dark {
		t.Fatal("initial dark should be false")
	}
	app.toggleDark()
	if !app.Prefs.Dark {
		t.Error("toggleDark should set Dark=true")
	}
	app.toggleDark()
	if app.Prefs.Dark {
		t.Error("second toggleDark should set Dark=false")
	}
}

func TestPaletteOverride(t *testing.T) {
	app := newTestApp(t)
	pal := BuildPalette(&app.Prefs, app.settings(), app.State)
	if pal.Scheme.Surface != 0xf5f5f5 {
		t.Errorf("light surface = %06X, want f5f5f5", pal.Scheme.Surface)
	}
	app.Prefs.Dark = true
	pal = BuildPalette(&app.Prefs, app.settings(), app.State)
	if pal.Scheme.Surface != 0x060606 {
		t.Errorf("dark surface = %06X, want 060606", pal.Scheme.Surface)
	}
}

func TestPendingPage(t *testing.T) {
	app := newTestApp(t)
	app.applyPendingPage()
	if app.Page != PageSettings || app.SettingsTab != 128 {
		t.Errorf("pendingPage island: page=%d tab=%d", app.Page, app.SettingsTab)
	}
	if app.lastPending != "island" {
		t.Errorf("lastPending = %q", app.lastPending)
	}
}

func TestOffsetShift(t *testing.T) {
	app := newTestApp(t)
	app.shiftOffset(-100)
	if app.Prefs.OffsetMs != -100 {
		t.Errorf("shiftOffset(-100) got %d", app.Prefs.OffsetMs)
	}
	app.shiftOffset(500)
	if app.Prefs.OffsetMs != 400 {
		t.Errorf("shiftOffset(500) got %d", app.Prefs.OffsetMs)
	}
	app.Prefs.OffsetMs = 9999
	app.shiftOffset(100)
	if app.Prefs.OffsetMs != 10000 {
		t.Errorf("clamp high got %d", app.Prefs.OffsetMs)
	}
}

func TestDeckRingMath(t *testing.T) {
	if deckPhase(0) != 0 {
		t.Errorf("deckPhase(0) = %f", deckPhase(0))
	}
	if deckPhase(deckStep) != 1 {
		t.Errorf("deckPhase(deckStep) should be 1.0")
	}
	if deckSnap(370) != deckStep {
		t.Errorf("deckSnap(370) = %f", deckSnap(370))
	}
}

func TestStateAccessors(t *testing.T) {
	app := newTestApp(t)
	if app.trackTitle() != "此刻，留一点空白" {
		t.Errorf("trackTitle = %q", app.trackTitle())
	}
	if app.trackArtist() != "打开音乐播放器，点屿会自动连接" {
		t.Errorf("trackArtist = %q", app.trackArtist())
	}
}

func TestSettingsHubName(t *testing.T) {
	for tab, name := range map[uint8]string{
		9: "模块管理", 128: "灵动岛尺寸与排版", 133: "播放进度条", 255: "设置",
	} {
		if settingsHubName(tab) != name {
			t.Errorf("settingsHubName(%d) = %q, want %q", tab, settingsHubName(tab), name)
		}
	}
}

func TestSettingsHubParent(t *testing.T) {
	for tab, parent := range map[uint8]uint8{
		0: 240, 128: 240, 1: 241, 132: 241, 2: 242, 8: 243, 4: 244, 3: 245,
	} {
		if settingsHubParent(tab) != parent {
			t.Errorf("settingsHubParent(%d) = %d, want %d", tab, settingsHubParent(tab), parent)
		}
	}
}

func TestWidgetKindLowerCase(t *testing.T) {
	for _, k := range widgetKindLabels {
		if strings.Contains(k.kind, " ") {
			t.Errorf("widget kind %q is not lowercase", k.kind)
		}
	}
}

func TestFormatOffset(t *testing.T) {
	if formatOffset(100) != "+100 ms" {
		t.Errorf("formatOffset(100) = %q", formatOffset(100))
	}
	if formatOffset(-100) != "-100 ms" {
		t.Errorf("formatOffset(-100) = %q", formatOffset(-100))
	}
}

func TestGroupEntries(t *testing.T) {
	if len(groupEntries(240)) != 6 {
		t.Errorf("group 240 entries = %d", len(groupEntries(240)))
	}
	if len(groupEntries(243)) != 1 {
		t.Errorf("group 243 entries = %d", len(groupEntries(243)))
	}
}

// TestFocusedFieldKeepsNextPress guards the dialog regression where a press on a
// button did nothing while a text field of the same panel had the focus, and only
// the second press worked. MyGo identifies an element by its parent and its index
// among the parent's children (ui/context.go: e.id = mix(p.id, p.nchild)), so a
// decoration built only while a field is focused shifts the identity of every
// element after it: the press bound to the button's old state is never read by the
// button the next frame builds. dialogField therefore builds its focus outline on
// every frame, transparent while the field is unfocused, and this test drives a
// real dialogField with Testers (software rasterizer, no window) to keep it so.
func TestFocusedFieldKeepsNextPress(t *testing.T) {
	app := newTestApp(t)
	app.Palette = BuildPalette(&app.Prefs, app.settings(), app.State)
	value := ""
	clicks := 0
	view := func(c *ui.Context) {
		root := ui.Box(c).Fill()
		root.Children(func() {
			app.dialogField(c, &app.Palette, 40, 40, 300, "名称", &value, 60)
			b := ui.Button(c, "save").Label("SAVE").Absolute().Left(40).Top(160).Size(120, 40)
			b.OnClick(func() { clicks++ })
		})
	}
	tt := ui.NewTester(view, 480, 320)
	tt.Frame()
	// Focus the field, as a press inside it does.
	tt.ClickAt(40+14+136, 40+26+18)
	tt.Frame()
	if active, _ := tt.TextInputActive(); !active {
		t.Fatal("the press inside the field did not give it the focus")
	}
	// One press on the button, with the field still focused: it must count once.
	tt.ClickAt(40+60, 160+20)
	tt.Frame()
	if clicks != 1 {
		t.Fatalf("press on the button after the field took the focus reached it %d times, want 1", clicks)
	}
	// A second press still counts: the first one must not have been left pending.
	tt.ClickAt(40+60, 160+20)
	tt.Frame()
	if clicks != 2 {
		t.Fatalf("second press reached the button %d times in total, want 2", clicks)
	}
}

// The hover state layer must blend over the button's own fill: mygo Background
// replaces, so a bare alpha tint erased the fill and left the white label
// invisible on light panels while the pointer rested on the button.
func TestHoverKeepsButtonFill(t *testing.T) {
	app := newTestApp(t)
	app.Palette = BuildPalette(&app.Prefs, app.settings(), app.State)
	view := func(c *ui.Context) {
		root := ui.Box(c).Fill().Background(Col(0xFFFFFF))
		root.Children(func() {
			Button(c, &app.Palette, "pick", 40, 40, 140, 44, "选择图片", "", false, true, nil)
		})
	}
	tt := ui.NewTester(view, 480, 320)
	tt.Frame()
	idle := tt.Image().RGBAAt(60, 62)
	tt.Move(60, 62)
	tt.Frame()
	hov := tt.Image().RGBAAt(60, 62)
	dist := func(r, g, b uint8) int {
		d := func(a, b uint8) int {
			if a > b {
				return int(a) - int(b)
			}
			return int(b) - int(a)
		}
		return d(hov.R, r) + d(hov.G, g) + d(hov.B, b)
	}
	// An 8% white layer shifts each channel by at most 20.4, so 61 total.
	if dist(idle.R, idle.G, idle.B) > 70 {
		t.Fatalf("hover rewrote the fill: idle %v hovered %v", idle, hov)
	}
	if dist(255, 255, 255) < 60 {
		t.Fatalf("hovered button faded into the panel: %v", hov)
	}
}

// Arrow keys on a small-range slider must advance the committed value: mygo
// snaps slider writes to a 0.01 fraction grid, so rewinding the slot to the
// rounded integer after each keyboard commit made every press re-commit the
// same value (one 0.01 step is worth less than half a unit below range ~50).
func TestLeafSliderKeyboardSmallRange(t *testing.T) {
	app := newTestApp(t)
	app.Palette = BuildPalette(&app.Prefs, app.settings(), app.State)
	committed := 5.0
	var got []float64
	view := func(c *ui.Context) {
		root := ui.Box(c).Fill().Background(Col(0xFFFFFF))
		root.Children(func() {
			leafSettingsSlider(c, &app.Palette, "History days", "TestHistoryDays", 40, 40, 300, 1, 30, committed, func(v float64) {
				got = append(got, v)
				committed = v
			})
		})
	}
	tt := ui.NewTester(view, 480, 320)
	tt.Frame()
	tt.ClickAt(40+150, 40+14)
	tt.Frame()
	for i := 0; i < 4; i++ {
		tt.Key(0, ui.KeyRight)
		tt.Frame()
	}
	if committed <= 5 {
		t.Fatalf("arrow keys never advanced the small-range slider: commits %v", got)
	}
}

// Leaving the 28px track mid-drag must not end the gesture: the press stays
// captured, so exactly one commit happens on the real release, none before.
func TestLeafSliderOutTrackDragCommitsOnce(t *testing.T) {
	app := newTestApp(t)
	app.Palette = BuildPalette(&app.Prefs, app.settings(), app.State)
	var got []float64
	view := func(c *ui.Context) {
		root := ui.Box(c).Fill().Background(Col(0xFFFFFF))
		root.Children(func() {
			leafSettingsSlider(c, &app.Palette, "Opacity", "TestOutTrack", 40, 40, 300, 0, 100, 30, func(v float64) {
				got = append(got, v)
			})
		})
	}
	tt := ui.NewTester(view, 480, 320)
	tt.Frame()
	tt.Press(40+100, 40+14)
	tt.Frame()
	tt.Move(40+200, 40+80)
	tt.Frame()
	tt.Move(40+250, 40+90)
	tt.Frame()
	if len(got) != 0 {
		t.Fatalf("committed mid-gesture outside the track: %v", got)
	}
	tt.Release(40+250, 40+90)
	tt.Frame()
	if len(got) != 1 {
		t.Fatalf("release committed %d times, want 1: %v", len(got), got)
	}
}
