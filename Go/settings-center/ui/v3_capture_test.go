package ui

import (
	"encoding/json"
	"fmt"
	"image/png"
	"os"
	"path/filepath"
	"testing"
	"time"

	"dotisle/settings/prefs"
	"github.com/egoist/mygo/ui"
)

// Explicit visual coverage of every route, both overview modes and overlays.
// These are production-view software renders of an isolated fixture, not desktop shots.
func TestV3SurfaceCaptures(t *testing.T) {
	output := os.Getenv("DOTISLE_DESIGN_CAPTURE")
	if output == "" {
		t.Skip("capture requested explicitly")
	}
	RegisterFonts()
	raw := []byte("{}")
	if path := os.Getenv("DOTISLE_DESIGN_SNAPSHOT"); path != "" {
		var err error
		raw, err = os.ReadFile(path)
		if err != nil {
			t.Fatal(err)
		}
	}
	type capture struct {
		name          string
		page          int
		tab           uint8
		dark          bool
		width, height int
		variant       string
		scroll        int
	}
	cases := []capture{}
	add := func(name string, page int, tab uint8, variant string) {
		for _, dark := range []bool{false, true} {
			suffix := "light"
			if dark {
				suffix = "dark"
			}
			cases = append(cases, capture{name + "-" + suffix, page, tab, dark, 1200, 800, variant, 0})
		}
	}
	for page := PageHome; page <= PagePlugins; page++ {
		add("page-"+itoa(page), page, TabRoot, "")
	}
	for _, tab := range []uint8{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 128, 129, 130, 131, 132, 133, 134, 135, 240, 241, 242} {
		add("settings-"+itoa(int(tab)), PageSettings, tab, "")
		if tab != 135 && tab < 240 {
			cases = append(cases, capture{"settings-" + itoa(int(tab)) + "-middle", PageSettings, tab, false, 1200, 800, "", 1}, capture{"settings-" + itoa(int(tab)) + "-bottom", PageSettings, tab, false, 1200, 800, "", 2})
		}
	}
	for i := 0; i < 5; i++ {
		add("overview-scroll-"+itoa(i), PageHome, TabRoot, "scroll-"+itoa(i))
	}
	add("overview-scroll-quick", PageHome, TabRoot, "scroll-quick")
	for _, phase := range []int{20, 50, 80} {
		add("overview-phase-"+itoa(phase), PageHome, TabRoot, "phase-"+itoa(phase))
	}
	for _, variant := range []string{"edit", "manager", "populated", "extras", "collapsed", "scroll-manager", "information-manager"} {
		add("overview-"+variant, PageHome, TabRoot, variant)
	}
	for _, variant := range []string{"update-beta", "update-downloading", "update-ready", "update-failed"} {
		add(variant, PageSettings, 3, variant)
	}
	for i := 1; i <= 3; i++ {
		add("timer-mode-"+itoa(i), PageFocus, TabRoot, "timer-"+itoa(i))
	}
	for _, variant := range []string{"todo", "quick", "rule", "font", "color", "palette", "prefs", "stage", "template", "templates", "modules", "module-edit"} {
		add("dialog-"+variant, PageFocus, TabRoot, variant)
	}
	for _, page := range []int{PageHome, PageWidgets, PageFocus, PageTodos, PageNotifications, PageSettings} {
		cases = append(cases, capture{"compact-" + itoa(page), page, TabRoot, false, 900, 680, "", 0})
	}
	cases = append(cases, capture{"overview-scroll-compact", PageHome, TabRoot, false, 900, 680, "scroll-0", 0}, capture{"overview-manager-compact", PageHome, TabRoot, false, 900, 680, "manager", 0}, capture{"overview-extras-bottom", PageHome, TabRoot, false, 1200, 800, "extras", 2}, capture{"sidebar-expanded-compact", PageHome, TabRoot, false, 900, 680, "expanded", 0}, capture{"information-compact", PageHome, TabRoot, false, 900, 680, "scroll-4", 0}, capture{"update-compact", PageSettings, 3, false, 900, 680, "update-beta", 2})
	if err := os.MkdirAll(output, 0755); err != nil {
		t.Fatal(err)
	}
	manifest := []capture{}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			savedFocus, savedDialog, savedWorkflow := focusUI, focusDialog, workflowUI
			defer func() { focusUI, focusDialog, workflowUI = savedFocus, savedDialog, savedWorkflow }()
			focusUI.mode, focusUI.selected, focusUI.minutes = 0, "", 25
			focusDialog.kind = fdNone
			deckPos, deckDragY, deckTarget = 0, 0, 0
			deckSpringIdle = true
			deckLastInput = time.Time{}
			noticeFilter = 0
			a := newTestApp(t)
			if err := json.Unmarshal(raw, &a.State); err != nil {
				t.Fatal(err)
			}
			if a.State == nil {
				a.State = map[string]any{}
			}
			a.State["pendingPage"] = ""
			settings := a.settings()
			if len(settings) == 0 {
				settings = map[string]any{}
				a.State["settings"] = settings
			}
			settings["SettingsTheme"] = "Light"
			if tc.dark {
				settings["SettingsTheme"] = "Dark"
			}
			a.Prefs.Dark = tc.dark
			a.Page, a.SettingsTab = tc.page, tc.tab
			if tc.tab >= TabIslandFirst && tc.tab <= TabIslandLast {
				a.IslandTab = tc.tab - TabIslandFirst
			}
			a.overview.loaded = true
			a.overview.config = prefs.DefaultOverview()
			switch tc.variant {
			case "scroll-quick":
				a.overview.config.View = "scroll"
				a.overview.config.ScrollPages = []string{"quick"}
			case "edit":
				a.overview.edit = true
			case "manager":
				a.overview.manage = true
			case "collapsed", "expanded":
				a.layoutLoaded = true
				a.layout.Sidebar = tc.variant
				a.navigationOpen = tc.variant == "expanded"
			case "scroll-manager":
				a.overview.config.View = "scroll"
				a.openOverviewCatalog("scroll")
			case "information-manager":
				a.overview.config.View = "scroll"
				deckPos = 4 * deckStep
				a.openOverviewCatalog("information")
			case "music-playing", "music-long", "music-pure":
				a.State["title"], a.State["artist"] = "此刻，正在聆听", "播放器 · 同步状态示例"
				a.State["isPlaying"], a.State["canSeek"] = true, true
				a.State["position"], a.State["duration"] = float64(66), float64(240)
				frame := map[string]any{"previousLine": "让时间慢下来", "currentLine": "把这一刻留给自己", "nextLine": "让每一份心绪都被听见", "currentTranslation": "A moment, just for you.", "currentLineIndex": float64(3), "wordScanProgress": float64(.6)}
				if tc.variant == "music-long" {
					frame["previousLine"] = "回望漫长旅途中那些细小而明亮的瞬间"
					frame["currentLine"] = "在每一个平凡而又值得记住的日子里听见自己的声音"
					frame["currentTranslation"] = "Hear your own voice in every ordinary day worth remembering."
				}
				if tc.variant == "music-pure" {
					frame = map[string]any{"isPureMusic": true}
				}
				a.State["musicPreview"] = map[string]any{"lyricFrame": frame}
			case "update-beta", "update-downloading", "update-ready", "update-failed":
				a.State["updateChannel"] = "beta"
				u := map[string]any{"phase": "available", "version": "v5.0.1-beta.1", "message": "发现新的测试版本，可下载后确认安装"}
				if tc.variant == "update-downloading" {
					u["phase"] = "downloading"
					u["receivedBytes"] = float64(42000000)
					u["totalBytes"] = float64(100000000)
				}
				if tc.variant == "update-ready" {
					u["phase"] = "ready"
					u["message"] = "下载与校验已完成，重启后安装"
				}
				if tc.variant == "update-failed" {
					u["phase"] = "failed"
					u["message"] = "网络连接中断，请检查网络后重试"
				}
				a.State["softwareUpdate"] = u
			case "extras":
				a.overview.config.Cards = append([]string{}, prefs.OverviewCardIDs...)
			case "populated":
				a.State["title"] = "播放中的音乐"
				a.State["artist"] = "本地播放器"
				a.State["duration"] = float64(240)
				a.State["position"] = float64(66)
				a.State["isPlaying"] = true
				a.State["canSeek"] = true
				a.State["todos"] = []any{map[string]any{"id": "capture-todo", "title": "整理本周安排", "date": "2026-11-14", "time": "18:00", "completed": false}}
				a.State["timers"] = []any{map[string]any{"id": "capture-timer", "mode": "pomodoro", "focus": float64(1500), "rest": float64(300), "phase": "focus", "elapsedMs": float64(60000), "running": true, "round": float64(1), "rounds": float64(4)}}
			case "todo":
				a.Page = PageTodos
				a.openTodoDialog("")
			case "quick":
				a.showQuickReminder = true
			case "rule":
				a.showRuleDialog = true
			case "font":
				a.openFontPicker()
			case "color":
				a.openPicker("IslandDynamicSeedColor", "灵动岛颜色", 0x008af3, 0x008af3)
			case "palette":
				a.paletteOpen = true
			case "prefs":
				a.openTimerPrefs()
			case "stage":
				a.openStageDialog(-1)
			case "template":
				a.openTemplateDialog()
			case "templates":
				focusDialog.kind = fdTemplates
			case "modules":
				focusDialog.kind = fdModules
			case "module-edit":
				a.openModuleDialog(-1)
			default:
				var index int
				if _, err := fmt.Sscanf(tc.variant, "phase-%d", &index); err == nil {
					a.overview.config.View = "scroll"
					deckPos = float64(index) / 100 * deckStep
				}
				if _, err := fmt.Sscanf(tc.variant, "scroll-%d", &index); err == nil {
					a.overview.config.View = "scroll"
					deckPos = float64(index) * deckStep
				}
				if _, err := fmt.Sscanf(tc.variant, "timer-%d", &index); err == nil {
					focusUI.mode = index
				}
			}
			tt := ui.NewTester(func(c *ui.Context) {
				var phase int
				if _, err := fmt.Sscanf(tc.variant, "phase-%d", &phase); err == nil {
					deckLastInput = c.Now()
				}
				a.View(c)
			}, tc.width, tc.height)
			tt.Frame()
			tt.Frame()
			if tc.scroll > 0 {
				amount := float32(a.contentH[a.Page]) / 2
				if tc.scroll == 2 {
					amount = 100000
				}
				tt.Scroll(float32(tc.width-100), 400, 0, amount)
				tt.Frame()
				tt.Frame()
			}
			if len(tt.Texts()) < 8 {
				t.Fatal("view did not produce page content")
			}
			f, err := os.Create(filepath.Join(output, tc.name+".png"))
			if err != nil {
				t.Fatal(err)
			}
			err = png.Encode(f, tt.Image())
			f.Close()
			if err != nil {
				t.Fatal(err)
			}
			texts, _ := json.MarshalIndent(tt.Texts(), "", "  ")
			if err = os.WriteFile(filepath.Join(output, tc.name+".texts.json"), texts, 0644); err != nil {
				t.Fatal(err)
			}
			manifest = append(manifest, tc)
		})
	}
	// Export readable metadata rather than unexported Go struct fields.
	rows := []map[string]any{}
	for _, tc := range manifest {
		rows = append(rows, map[string]any{"name": tc.name, "page": tc.page, "tab": tc.tab, "dark": tc.dark, "width": tc.width, "height": tc.height, "variant": tc.variant, "scroll": tc.scroll, "renderer": "MyGo software / production App.View"})
	}
	meta, _ := json.MarshalIndent(rows, "", "  ")
	if err := os.WriteFile(filepath.Join(output, "coverage.json"), meta, 0644); err != nil {
		t.Fatal(err)
	}
}

// Explicit contact sheets use the same example snapshot and preview renderer
// as the gallery. The captures stay opt-in and never connect to the core.
func TestWidgetExamplesCaptures(t *testing.T) {
	output := os.Getenv("DOTISLE_DESIGN_CAPTURE")
	if output == "" {
		t.Skip("capture requested explicitly")
	}
	RegisterFonts()
	if err := os.MkdirAll(output, 0755); err != nil {
		t.Fatal(err)
	}
	for _, size := range []string{"small", "medium", "large"} {
		t.Run(size, func(t *testing.T) {
			a := newTestApp(t)
			preview := a.widgetGalleryExamples(time.Now())
			tt := ui.NewTester(func(c *ui.Context) {
				a.Palette = BuildPalette(&a.Prefs, a.settings(), a.State)
				preview.Palette = a.Palette
				ui.Box(c).Absolute().Fill().Background(Col(a.Palette.Scheme.Surface))
				for i, entry := range widgetKindLabels {
					x, y := float32(i%3)*390+12, float32(i/3)*226+12
					dims := widgetDims(size)
					scale := float32(168 / dims[1])
					w, h := float32(dims[0])*scale, float32(dims[1])*scale
					ui.Box(c).Key(entry.kind).Absolute().Left(x).Top(y).Width(370).Height(210).Children(func() {
						preview.widgetPreview(c, &a.Palette, entry.kind, size, 0, (370-w)/2, 0, w, h)
						Label(c, entry.label+" · "+size, 12, 184, 12, a.Palette.Scheme.OnSurface, true, 346)
					})
				}
			}, 1180, ((len(widgetKindLabels)+2)/3)*226+12)
			tt.Frame()
			f, err := os.Create(filepath.Join(output, "widget-examples-"+size+".png"))
			if err != nil {
				t.Fatal(err)
			}
			err = png.Encode(f, tt.Image())
			f.Close()
			if err != nil {
				t.Fatal(err)
			}
		})
	}
}
