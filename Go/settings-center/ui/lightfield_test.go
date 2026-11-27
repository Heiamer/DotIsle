package ui

import (
	"image/png"
	"os"
	"path/filepath"
	"sort"
	"testing"
	"time"

	"dotisle/settings/prefs"
	"github.com/egoist/mygo/ui"
)

func TestLightFieldRendersHorizonAurora(t *testing.T) {
	for _, dark := range []bool{false, true} {
		a := newTestApp(t)
		a.Prefs.Dark = dark
		if dark {
			a.settings()["SettingsTheme"] = "Dark"
		} else {
			a.settings()["SettingsTheme"] = "Light"
		}
		tt := ui.NewTester(func(c *ui.Context) {
			a.Palette = BuildPalette(&a.Prefs, a.settings(), a.State)
			a.paintLightField(c, 400, 260)
		}, 400, 260)
		tt.Frame()
		img := tt.Image()
		tl := img.RGBAAt(12, 12)
		mid := img.RGBAAt(200, 130)
		br := img.RGBAAt(388, 248)
		if tl == mid && mid == br {
			t.Fatalf("dark=%v light field rendered flat solid pixels: %v", dark, tl)
		}
		if !dark && (mid.R < 225 || mid.G < 225 || mid.B < 225) {
			t.Fatalf("light mode gallery daylight too dark/tinted at center: %v", mid)
		}
	}
}

func TestOpticalGlassAccessibilityFallback(t *testing.T) {
	a := newTestApp(t)
	a.State["pendingPage"] = ""
	tt := ui.NewTester(a.View, 640, 420)
	tt.SetPreferences(ui.Preferences{ReduceTransparency: true})
	tt.Frame()
	if tt.Image().RGBAAt(10, 10).A != 255 {
		t.Fatal("ReduceTransparency fallback left window non-opaque")
	}
	tt.SetPreferences(ui.Preferences{HighContrast: true})
	tt.Frame()
	if tt.Image().RGBAAt(10, 10).A != 255 {
		t.Fatal("HighContrast fallback left window non-opaque")
	}
}

func TestOpticalCardHoverSpecularDoesNotBreakClick(t *testing.T) {
	a := newTestApp(t)
	a.State["pendingPage"] = ""
	tt := ui.NewTester(a.View, 1200, 800)
	tt.Frame()
	r, ok := tt.Find("搜索与跳转")
	if !ok {
		t.Fatal("missing search glass pill")
	}
	tt.Move(r.X+r.W/2, r.Y+r.H/2)
	tt.Frame()
	tt.ClickAt(r.X+r.W/2, r.Y+r.H/2)
	tt.Frame()
	if !a.paletteOpen {
		t.Fatal("hover specular interfered with clicking glass pill")
	}
}

func TestOpticalExtraCaptures(t *testing.T) {
	output := os.Getenv("DOTISLE_DESIGN_CAPTURE")
	if output == "" {
		t.Skip("capture requested explicitly")
	}
	RegisterFonts()
	if err := os.MkdirAll(output, 0755); err != nil {
		t.Fatal(err)
	}
	save := func(name string, tt *ui.Tester) {
		t.Helper()
		f, err := os.Create(filepath.Join(output, name+".png"))
		if err != nil {
			t.Fatal(err)
		}
		defer f.Close()
		if err := png.Encode(f, tt.Image()); err != nil {
			t.Fatal(err)
		}
	}
	newFixture := func(dark bool) *App {
		theme := "Light"
		if dark {
			theme = "Dark"
		}
		return &App{
			Prefs:       prefs.Default(),
			Data:        t.TempDir(),
			SettingsTab: TabRoot,
			Connected:   true,
			State: map[string]any{
				"pendingPage": "",
				"version":     "5.2.0",
				"title":       "Quiet Light & Optical Glass",
				"artist":      "DotIsle Acoustic",
				"isPlaying":   true,
				"settings": map[string]any{
					"SettingsTheme": theme,
					"Animation":     true,
				},
				"musicPreview": map[string]any{
					"lyricFrame": map[string]any{
						"currentLine":        "晨光穿透超白浮法玻璃，留下一片沉静空间",
						"currentTranslation": "Architectural light field & minimalist glass",
					},
				},
				"todos": []any{
					map[string]any{"id": "1", "title": "复核极简白瓷玻璃与 Google Sans Flex 字体排版", "completed": true},
					map[string]any{"id": "2", "title": "试听 12 款物理建模光学提示音", "completed": false},
				},
			},
		}
	}

	for _, dark := range []bool{false, true} {
		suffix := "light"
		if dark {
			suffix = "dark"
		}
		// 1. 悬浮圆角玻璃侧栏（推进动画至完全展开到位）
		a := newFixture(dark)
		a.navigationOpen = true
		tt := ui.NewTester(a.View, WindowW, WindowH)
		tt.Frame()
		tt.Advance(350 * time.Millisecond)
		tt.Frame()
		save("sidebar-"+suffix, tt)

		// 2. Ctrl+K 命令面板
		a = newFixture(dark)
		a.paletteOpen = true
		tt = ui.NewTester(a.View, WindowW, WindowH)
		tt.Frame()
		save("palette-"+suffix, tt)

		// 3. 首页概览（悬停纯白丝绸微光）
		a = newFixture(dark)
		tt = ui.NewTester(a.View, WindowW, WindowH)
		tt.Move(460, 290)
		tt.Frame()
		save("home-hover-"+suffix, tt)

		// 4. 设置面板首页（瑞士分层白灰磨砂面板）
		a = newFixture(dark)
		a.openSettings(0)
		tt = ui.NewTester(a.View, WindowW, WindowH)
		tt.Frame()
		save("settings-appearance-"+suffix, tt)
	}
}

func TestMaterialSegmentedControlsAreIndependent(t *testing.T) {
	a := newTestApp(t)
	a.State["settings"] = map[string]any{
		"SettingsTheme":          "Light",
		"ClassicCapsuleMaterial": "LightField",
		"IslandMaterial":         "LightField",
		"MiniMaterial":           "LightField",
		"PopupMaterial":          "Acrylic",
	}
	a.onCommand = func(name string, value any) {
		if name == "update" {
			if m, ok := value.(map[string]any); ok {
				if k, ok := m["key"].(string); ok {
					a.settings()[k] = m["value"]
				}
			}
		}
	}
	a.Page, a.SettingsTab, a.IslandTab = PageSettings, TabIslandFirst+1, 1
	tt := ui.NewTester(a.View, WindowW, WindowH)
	tt.Frame()
	tt.Frame()
	// 滚动设置面板正文至「卡片与弹窗材质」区可见（设置页正文高度存储在 settingsPanel.bodyHeight）
	tt.Scroll(float32(WindowW)/2, 420, 0, float32(a.settingsPanel.bodyHeight)*0.28)
	tt.Frame()
	tt.Advance(400 * time.Millisecond)
	tt.Frame()

	collect := func(name string) []ui.Rect {
		var out []ui.Rect
		for _, info := range tt.AccessAll() {
			if info.Name == name && !info.Offscreen && info.Bounds.W > 0 && info.Bounds.H > 0 {
				out = append(out, info.Bounds)
			}
		}
		sort.Slice(out, func(i, j int) bool { return out[i].Y < out[j].Y })
		return out
	}

	acrylicTabs := collect("亚克力")
	lumenTabs := collect("曜光玻璃")
	liquidTabs := collect("液态玻璃 Beta")
	if len(acrylicTabs) != 4 || len(lumenTabs) != 4 || len(liquidTabs) != 4 {
		t.Fatalf("expected 4 visible material segmented groups, got acrylic=%d lumen=%d liquid=%d", len(acrylicTabs), len(lumenTabs), len(liquidTabs))
	}

	// 验证滑块渲染独立：前 3 组选中在「曜光玻璃」（纯白滑块较「亚克力」更亮），第 4 组选中在「亚克力」
	img := tt.Image()
	lumaAt := func(r ui.Rect) int {
		// 避开中心文字，采样选项胶囊右上边缘滑块底色
		px := img.RGBAAt(int(r.X+r.W-12), int(r.Y+6))
		return int(px.R) + int(px.G) + int(px.B)
	}
	for i := 0; i < 3; i++ {
		if lumaAt(lumenTabs[i]) <= lumaAt(acrylicTabs[i]) {
			t.Fatalf("group %d slider leaked away from LightField tab: lumenLuma=%d acrylicLuma=%d",
				i, lumaAt(lumenTabs[i]), lumaAt(acrylicTabs[i]))
		}
	}
	if lumaAt(acrylicTabs[3]) <= lumaAt(lumenTabs[3]) {
		t.Fatalf("popup group 3 slider leaked away from Acrylic tab: acrylicLuma=%d lumenLuma=%d",
			lumaAt(acrylicTabs[3]), lumaAt(lumenTabs[3]))
	}

	// 分别点击前三个控件的「亚克力」，确认独立切换至 Acrylic 且互不影响
	keys := []string{"ClassicCapsuleMaterial", "IslandMaterial", "MiniMaterial"}
	for i, key := range keys {
		r := acrylicTabs[i]
		tt.ClickAt(r.X+r.W/2, r.Y+r.H/2)
		tt.Frame()
		if got := a.settings()[key]; got != "Acrylic" {
			t.Fatalf("%s did not switch to Acrylic: got %v", key, got)
		}
	}

	// 分别点击四个控件的「液态玻璃 Beta」，确认独立切换至 LiquidGlass 且不影响原材质体系
	allKeys := []string{"ClassicCapsuleMaterial", "IslandMaterial", "MiniMaterial", "PopupMaterial"}
	for i, key := range allKeys {
		r := liquidTabs[i]
		tt.ClickAt(r.X+r.W/2, r.Y+r.H/2)
		tt.Frame()
		if got := a.settings()[key]; got != "LiquidGlass" {
			t.Fatalf("%s did not switch to LiquidGlass: got %v", key, got)
		}
	}

	// 分别点击四个控件的「曜光玻璃」，确认配置写入恢复材质
	for i, key := range allKeys {
		r := lumenTabs[i]
		tt.ClickAt(r.X+r.W/2, r.Y+r.H/2)
		tt.Frame()
		if got := a.settings()[key]; got != "LightField" {
			t.Fatalf("%s did not switch to LightField: got %v", key, got)
		}
	}
}
