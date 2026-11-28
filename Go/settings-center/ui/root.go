// Native control center shell: adaptive sidebar, titlebar, header and scrolling pages.
package ui

import (
	"fmt"
	"os"
	"sync/atomic"
	"time"

	"dotisle/settings/island"
	"dotisle/settings/prefs"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

// Pages in Page::ALL order (model.rs), routed by the sidebar.
var Pages = []string{"概览", "组件", "计时", "待办", "通知", "设置", "插件"}

const (
	PageHome = iota
	PageWidgets
	PageFocus
	PageTodos
	PageNotifications
	PageSettings
	PagePlugins
)

// App is the whole control center state; the view reads it, click handlers and
// the island pump mutate it on the main thread through Window.Update.
type App struct {
	mailOtpEnabled atomic.Bool
	mail           mailState
	mailOpen       bool
	quotaFields    quotaFields
	quotaSources   quotaSourcePanel
	Prefs          prefs.Preferences
	Data           string
	State          map[string]any
	packetAt       time.Time
	Connected      bool
	ConnErr        string
	Page           int
	SettingsTab    uint8
	settingsPanel  settingsPanelState
	IslandTab      uint8
	Scroll         [7]ui.ScrollState
	contentH       [7]float32 // measured page content height, drives the scroll bounds
	Palette        Palette
	Win            *mygo.Window
	ShowCenter     func() // Restores the resident window or recreates it after idle release.
	asyncRequests  atomic.Int32
	overview       overviewState
	layout         prefs.CenterLayout
	layoutLoaded   bool
	navigationOpen bool

	todoDialog  bool
	editTodoID  string
	lastPending string

	// Focus page extras (workflow editor lives in the Go front-end draft).
	workflowDraftError string

	// Widgets page: per-kind size choices (model.widget_sizes), default medium.
	widgetSizes      map[string]string
	musicEditor      *musicEditor
	progressPreview  progressPreviewState
	gestureTutorial  *gestureTutorial
	tutorialRequired bool
	curveDraft       *motionCurveDraft
	widgetEditor     *widgetEditorState
	overviewEditor   *overviewEditorState
	widgetExamples   *App
	reminderFields   [4]leafEditField

	// Open color picker dialog and the toast message (fresh for a few seconds).
	picker         *pickerState
	toast          string
	toastAt        time.Time
	onCommand      func(name string, value any) // 仅用于无窗口单元测试拦截设置变更指令
	frameNow       time.Time
	contentMounted bool

	// Dialog switches owned by the page layer.
	showQuickReminder   bool
	showRuleDialog      bool
	ruleEditID          string
	fontPicker          *fontPickerState
	EnableCityPrompt    bool
	centerVisible       bool
	cityPromptAsked     bool
	cityPrompt          *cityPromptState
	wechatWarning       *wechatWarning
	updateNotes         *updateNotes
	notesCheckedVersion string
	updateReviewLoading bool
	updateDownloading   bool

	// Command palette (Ctrl+K).
	paletteOpen     bool
	paletteQuery    string
	paletteSelected int

	// Opening wordmark cover (brand_signature), started when the window shows.
	signatureStart          time.Time
	signaturePending        bool
	openingCanvas           *ui.Canvas
	openingLightValid       bool
	openingLightDark        bool
	openingLightBase        uint32
	openingLightAccent      uint32
	openingLightProg        float32
	startupSettingsLoaded   bool
	startupSignatureEnabled bool
	backdropWindow          uintptr
	backdropDark            bool
	backdropChecked         bool
	backdropMica            bool
	lightField              lightFieldState
}

// scrollY is the current page's scroll offset (gallery culling).
func (a *App) scrollY() float32 { return a.Scroll[a.centerPage()].Y }

// traceEvent appends one debug line when DOTISLE_TRACE_UI is set.
func traceEvent(format string, args ...any) {
	if os.Getenv("DOTISLE_TRACE_UI") == "" {
		return
	}
	path := os.Getenv("TEMP") + string(os.PathSeparator) + "ui-trace.txt"
	f, err := os.OpenFile(path, os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0o644)
	if err != nil {
		return
	}
	fmt.Fprintf(f, format+"\n", args...)
	_ = f.Close()
}

// traceFrame appends one line per view build when DOTISLE_TRACE_UI is set.
func traceFrame(w, h float32, page int) {
	if os.Getenv("DOTISLE_TRACE_UI") == "" {
		return
	}
	path := os.Getenv("TEMP") + string(os.PathSeparator) + "ui-trace.txt"
	f, err := os.OpenFile(path, os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0o644)
	if err != nil {
		return
	}
	fmt.Fprintf(f, "frame w=%v h=%v page=%d\n", w, h, page)
	_ = f.Close()
}

// logPanic writes a view-build panic to a file in the temp dir.
func logPanic(r any) {
	msg := fmt.Sprintf("view panic: %v\n", r)
	path := os.Getenv("TEMP") + string(os.PathSeparator) + "view-panic.txt"
	if f, err := os.OpenFile(path, os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0o644); err == nil {
		_, _ = f.WriteString(msg)
		_ = f.Close()
	}
}

// applyPendingPage routes the island's pendingPage request, exactly like
// native_sync.rs: timers→Focus, island→Settings tab128, and pageSeen ack.
func (a *App) applyPendingPage() {
	page, _ := a.State["pendingPage"].(string)
	if page == "" {
		a.lastPending = ""
		return
	}
	// Another portable runtime can share the default pipe. Never follow its page
	// requests; the pump stops a mismatched center, and this keeps one late packet
	// from flashing a window in the meantime.
	if dir, _ := a.State["dataDirectory"].(string); dir != "" && !island.SameDataDir(dir, a.Data) {
		return
	}
	requestKey := page
	if requestID, ok := a.State["pendingRequestId"]; ok {
		requestKey = fmt.Sprintf("%s/%v", page, requestID)
	}
	if requestKey == a.lastPending {
		return
	}
	a.lastPending = requestKey
	a.navigationOpen = false
	a.cancelOverviewGesture()
	switch page {
	case "timers":
		a.navigate(PageFocus)
	case "todos":
		a.navigate(PageTodos)
	case "overview":
		a.navigate(PageHome)
	case "gestureTutorial":
		a.forceGestureTutorial()
	case "plugins":
		a.navigate(PagePlugins)
	case "widgets":
		a.navigate(PageWidgets)
	case "notifications":
		a.navigate(PageNotifications)
	case "island":
		a.openSettings(TabIslandFirst)
	case "layout":
		a.openSettings(TabLayoutEditor)
	case "music", "lyrics":
		a.openSettings(241)
	case "about", "updates", "update":
		a.openSettings(3)
	default:
		a.openSettings(TabRoot)
	}
	if a.ShowCenter != nil {
		a.ShowCenter()
	}
	if request, ok := a.State["pendingCenterAction"].(map[string]any); ok {
		id := stateString(request, "", "id")
		switch stateString(request, "", "action") {
		case "todoEdit":
			a.openTodoDialog(id)
		case "widgetConfigure":
			a.openWidgetEditor(id)
		case "updateReview":
			a.openUpdateReview()
		}
	}
	a.command("pageSeen", map[string]any{"requestId": a.State["pendingRequestId"]})
}

// ApplyPacket installs one island snapshot; called on the main thread by the pump.
// It must invalidate: a packet alone gives mygo nothing to redraw, so without
// Refresh the UI keeps showing the pre-packet values until the next input event.
// A nil state (transient pipe failure) keeps the last good snapshot instead of
// flashing the whole UI back to defaults until the stream recovers.
func (a *App) ApplyPacket(state map[string]any, connected bool, connErr string) {
	if state != nil {
		a.State = state
		// native_sync.rs routes pendingPage on every snapshot; without this call the
		// island's "open this page" request (main.rs:1152, published as pendingPage and
		// cleared after the reply) never navigates, so the tray/CLI wake-up was a no-op.
		if enabled, ok := state["islandEnabled"].(bool); ok {
			a.Prefs.Island = enabled
		}
		a.applyPendingPage()
		a.syncGesturePractice(state)
	}
	a.Connected = connected
	a.mailOtpEnabled.Store(connected && a.settings()["VerificationCodeEnabled"] != false && a.settings()["PauseNotifications"] != true && a.State["sessionLocked"] != true)
	a.ConnErr = connErr
	a.packetAt = time.Now()
	a.Refresh()
	if connErr != lastTracedErr {
		lastTracedErr = connErr
		motion, _ := a.settings()["Motion"].(map[string]any)
		spring, _ := motion["Spring"].(map[string]any)
		traceEvent("packet conn=%v err=%q dur=%v mass=%v motionRaw=%v massRaw=%v",
			connected, connErr, a.islandFloat("MotionDuration", 480),
			a.islandFloat("SpringMass", 0.8), motion["Duration"], spring["Mass"])
	}
}

var lastTracedErr = "\x00first"

func (a *App) trackTitle() string {
	return stateString(a.State, "此刻，留一点空白", "title")
}

func (a *App) trackArtist() string {
	return stateString(a.State, "打开音乐播放器，点屿会自动连接", "artist")
}

func (a *App) playing() bool {
	return stateBool(a.State, "isPlaying")
}

// Refresh only queues a frame for a visible visit, never for the tray host.
func (a *App) Refresh() {
	if a.Win != nil && (a.centerVisible || a.ShowCenter == nil) {
		a.Win.Invalidate()
	}
}

// applyAsync completes a request on the UI thread even if the window was released.
func (a *App) applyAsync(fn func()) {
	mygo.RunOnMain(func() {
		fn()
		a.Refresh()
	})
}

func (a *App) startAsync(fn func()) {
	a.asyncRequests.Add(1)
	go func() {
		defer a.asyncRequests.Add(-1)
		fn()
	}()
}

func (a *App) settings() map[string]any {
	if a.State == nil {
		return map[string]any{}
	}
	if s, ok := a.State["settings"].(map[string]any); ok {
		return s
	}
	return map[string]any{}
}

func (a *App) pref(key string) any { return a.settings()[key] }

// navigate switches the top-level page (Action::Navigate).
func (a *App) navigate(page int) {
	if page == PageSettings {
		a.openSettings(TabRoot)
		return
	}
	traceEvent("navigate -> %d", page)
	a.navigationOpen = false
	a.cancelOverviewGesture()
	if a.Page == PageSettings {
		a.settingsPanel = settingsPanelState{}
	}
	a.Page = page
}

// The main window owns a single settings panel. Generic open requests bring
// the existing panel back without losing its route, search or scroll position.
func (a *App) openSettings(tab uint8) {
	a.navigationOpen, a.paletteOpen = false, false
	a.cancelOverviewGesture()
	if a.Page == PageSettings && tab == TabRoot {
		return
	}
	_, section := settingLocation(settingLeaf(tab))
	tab = section.tab
	if a.Page != PageSettings {
		a.settingsPanel = settingsPanelState{returnPage: a.Page}
	}
	if a.Page != PageSettings || a.SettingsTab != tab {
		a.settingsPanel.scroll, a.settingsPanel.bodyHeight = ui.ScrollState{}, 0
	}
	if a.settingsPanel.query != "" {
		a.settingsPanel.query, a.settingsPanel.navScroll = "", ui.ScrollState{}
	}
	a.Page, a.SettingsTab = PageSettings, tab
	if tab >= TabIslandFirst && tab <= TabIslandLast {
		a.IslandTab = tab - TabIslandFirst
	}
}

func (a *App) toggleDark() {
	a.Prefs.Dark = !resolveSettingsDark(&a.Prefs, a.settings(), a.State["settingsDark"])
	a.savePrefs()
	// config.rs validate_setting only accepts "System"/"Light"/"Dark" strings for
	// SettingsTheme; a structured value is rejected and the island keeps the old theme.
	theme := "Light"
	if a.Prefs.Dark {
		theme = "Dark"
	}
	a.pushNative("SettingsTheme", theme)
}

func (a *App) savePrefs() {
	path, _, _ := prefs.Paths(a.Data)
	if err := prefs.Save(path, &a.Prefs); err != nil {
		a.pushToast("偏好保存失败：" + err.Error())
	}
}

// pushNative sends one update command to the island.
func (a *App) pushNative(key string, value any) {
	a.command("update", map[string]any{"key": key, "value": value})
}

func boolInt(b bool) int {
	if b {
		return 1
	}
	return 0
}

// View builds one frame of the whole window. A build panic is logged instead of
// tearing the frame down, so integration issues surface in the log file.
func (a *App) View(c *ui.Context) {
	defer func() {
		if r := recover(); r != nil {
			logPanic(r)
		}
	}()
	if a.musicEditor != nil && (!a.onLayoutEditor() || !a.Connected) {
		a.musicEditor.Preview = nil
		a.musicEditor.PreviewKey = layoutPreviewKey{}
	}
	a.loadOverview()
	a.loadCenterLayout()
	if a.Page != PageHome || a.overview.config.View != "cards" || a.overview.manage {
		a.cancelOverviewGesture()
	} else if !a.overview.edit {
		a.finishOverviewResize(false)
	}
	a.Palette = BuildPalette(&a.Prefs, a.settings(), a.State)
	a.syncBackdropTheme()
	s := &a.Palette.Scheme
	w32, h32 := c.Size()
	w, h := w32, h32

	// The theme drives only MyGo's own defaults; every control below is styled
	// from the palette directly.
	t := *c.Theme()
	t.Dark = a.Palette.Dark
	t.Background = Col(s.Surface)
	c.Root().Background(Col(s.Surface))
	if a.materialActive(c) {
		t.Background = ui.Transparent
		c.Root().Background(ui.Transparent)
	}
	t.Font = UIFont
	t.Surface, t.SurfaceHover, t.SurfacePressed = Col(s.Container), Col(s.High), Col(s.SecondaryContainer)
	t.Text, t.TextMuted = Col(s.OnSurface), Col(s.OnSurfaceVariant)
	t.Accent, t.AccentText = Col(s.Primary), Col(s.OnPrimary)
	t.AccentHover, t.AccentPressed, t.InactiveAccent = Col(mix(s.Primary, s.OnPrimary, .1)), Col(mix(s.Primary, s.OnPrimary, .1)), Col(s.Primary)
	t.Border, t.Focus = ui.Transparent, Col(s.Primary)
	t.Radius, t.Spacing = FieldRadius, 4
	t.DisableAutoDimming = true
	t.FontSize = 14
	c.SetTheme(&t)

	// Frame trace: w/h per build, page markers on navigation (debug aid).
	traceFrame(w, h, a.Page)

	a.frameNow = c.Now()
	root := ui.Box(c).Fill()
	root.Children(func() {
		opaqueOpening := a.prepareOpening(c)
		if !opaqueOpening {
			page := ui.Box(c).Key("page-surface").Absolute().Left(0).Top(0).Width(w).Height(h)
			if a.navigationOpen || a.Page == PageSettings {
				page.IgnoreInput(true).AccessHidden(true).Disabled(true).DimDisabled(false)
			}
			page.Children(func() {
				if !a.materialActive(c) || opticalReduced(c, &a.Palette) {
					ui.Box(c).Fill().Background(Col(s.Surface)).IgnoreInput(true)
				}
				a.paintLightField(c, w, h)
				if a.materialActive(c) && a.leafBool("ControlCenterDynamicColorEnabled", false) {
					ui.Box(c).Absolute().Fill().Background(AlphaCol(s.Surface, .42)).IgnoreInput(true)
				}
				a.paintBackground(c, w, h)
				a.header(c, w, h)
				a.content(c, w, h)
				if a.Page != PageSettings {
					a.paletteView(c, w, h)
					a.overviewManager(c)
				}
			})
		}
		if a.Page != PageSettings {
			a.titlebar(c, w, h)
		}
		if !opaqueOpening {
			a.navigation(c, w, h)
			a.settingsPanelView(c, w, h)
			a.dialogViews(c, w, h)
			a.toastView(c, &a.Palette)
		}
		a.opening(c, w, h)
	})
}

func (a *App) titleGlassButton(c *ui.Context, label string, x, y, w, h float32, iconName string, onClick func()) {
	s := &a.Palette.Scheme
	e := ui.ButtonBase(c).Absolute().Left(x).Top(y).Width(w).Height(h).
		Justify(ui.Center).AlignItems(ui.Center).Label(label).Cursor(ui.CursorPointer)
	if !a.Palette.HighContrast {
		bg := AlphaCol(0xFFFFFF, 0.62)
		border := AlphaCol(0x1E293B, 0.11)
		rim := AlphaCol(0xFFFFFF, 0.96)
		if a.Palette.Dark {
			bg = AlphaCol(0xFFFFFF, 0.07)
			border = AlphaCol(0xFFFFFF, 0.11)
			rim = AlphaCol(0xFFFFFF, 0.14)
		}
		if e.Hovered() {
			if a.Palette.Dark {
				bg = AlphaCol(0xFFFFFF, 0.13)
			} else {
				bg = AlphaCol(0xFFFFFF, 0.84)
			}
		}
		e.Background(bg).Radius(12).Border(1, border).
			BorderConicGradient(opticalFresnelConic(a.Palette.Dark)).
			InsetShadow(0, 1, 0, 0, rim)
		elevationStyle(&e, &a.Palette, elevationControl)
	} else {
		e.Background(Col(s.Container)).Radius(12).Border(1, Col(s.Outline))
	}
	pressMotion(c, &a.Palette, &e, true)
	if onClick != nil && e.Clicked() {
		onClick()
	}
	e.Children(func() {
		ui.Icon(c, Symbol(iconName)).FontSize(18).TextColor(Col(s.OnSurface))
	})
}

// titlebar is the 52px drag strip plus optical glass navigation & caption controls.
func (a *App) titlebar(c *ui.Context, w, h float32) {
	s := &a.Palette.Scheme
	side := a.sidebarWidth(w)
	bar := ui.Box(c).Absolute().Left(side).Top(0).Width(w - side).Height(TitlebarH).DragWindow()
	if opticalReduced(c, &a.Palette) {
		bar.Background(Col(s.Surface))
	}
	a.titleGlassButton(c, "打开导航", 12, 8, 36, 36, "menu", func() { a.ToggleNavigation() })
	themeX := w - 142
	a.titleGlassButton(c, "切换明暗主题", themeX, 8, 34, 34, darkModeIcon(a.Palette.Dark), a.toggleDark)
	a.captionButton(c, w-92, "最小化窗口", "minimize", false, func() {
		if a.Win != nil {
			a.Win.Minimize()
		}
	})
	a.captionButton(c, w-46, "关闭窗口", "close", true, func() {
		if a.Win != nil {
			a.Win.Hide()
		}
	})
}

// captionButton draws one 46×52 window control with glass hover highlight.
func (a *App) captionButton(c *ui.Context, x float32, label, iconName string, close bool, onClick func()) {
	s := &a.Palette.Scheme
	e := ui.ButtonBase(c).Absolute().Left(x).Top(0).Width(46).Height(TitlebarH).
		Justify(ui.Center).AlignItems(ui.Center).Label(label)
	fg := Col(s.OnSurfaceVariant)
	if e.Hovered() {
		if close {
			e.Background(AlphaCol(0xE11D48, 0.88))
			fg = Col(0xFFFFFF)
		} else if a.Palette.Dark {
			e.Background(AlphaCol(0xFFFFFF, 0.09))
			fg = Col(s.OnSurface)
		} else {
			e.Background(AlphaCol(0x111827, 0.05))
			fg = Col(s.OnSurface)
		}
	}
	if onClick != nil && e.Clicked() {
		onClick()
	}
	e.Children(func() {
		ui.Icon(c, Symbol(iconName)).FontSize(18).TextColor(fg)
	})
}

// header is the minimalist architectural page title and floating search bar.
func (a *App) header(c *ui.Context, w, h float32) {
	s := &a.Palette.Scheme
	x := a.sidebarWidth(w) + 32
	page := a.centerPage()
	title, desc := Pages[page], pageDescription(page)
	if page == PageHome {
		title, desc = "概览", "音乐、计时与待办事项"
	}
	Label(c, title, x, TitleY-3, 24, s.OnSurface, true, w-x-220)
	Label(c, desc, x, SubtitleY+2, 13, s.OnSurfaceVariant, false, w-x-210)
	searchX := w - 252
	if page == PageTodos {
		searchX = w - 430
		Button(c, &a.Palette, "新建待办", w-178, TodoActionY, 146, 40, "新建待办", "add", false, true, func() { a.openTodoDialog("") })
	}
	search := ui.ButtonBase(c).Absolute().Left(searchX).Top(TodoActionY).Width(220).Height(40).
		Label("搜索与跳转").Cursor(ui.CursorPointer).Hidden(a.paletteOpen)
	if !a.Palette.HighContrast {
		bg := AlphaCol(0xFFFFFF, 0.62)
		rim := AlphaCol(0xFFFFFF, 0.96)
		if a.Palette.Dark {
			bg = AlphaCol(0xFFFFFF, 0.07)
			rim = AlphaCol(0xFFFFFF, 0.12)
		}
		if search.Hovered() {
			if a.Palette.Dark {
				bg = AlphaCol(0xFFFFFF, 0.11)
			} else {
				bg = AlphaCol(0xFFFFFF, 0.84)
			}
		}
		search.Background(bg).Radius(14).Border(1, AlphaCol(0xFFFFFF, 0.82)).
			BorderConicGradient(opticalFresnelConic(a.Palette.Dark)).
			InsetShadow(0, 1, 0, 0, rim)
		elevationStyle(&search, &a.Palette, elevationControl)
	} else {
		surfaceStyle(&search, &a.Palette, s.Container, FieldRadius)
	}
	if search.Clicked() {
		a.togglePalette()
	}
	search.Children(func() {
		Icon(c, "search", 14, 11, 17, s.OnSurfaceVariant)
		Label(c, "搜索...", 40, 11, 13, s.OnSurfaceVariant, false, 100)
		if !opticalReduced(c, &a.Palette) {
			kbdBg := AlphaCol(0x111827, 0.045)
			kbdBorder := AlphaCol(0x111827, 0.06)
			if a.Palette.Dark {
				kbdBg = AlphaCol(0xFFFFFF, 0.07)
				kbdBorder = AlphaCol(0xFFFFFF, 0.10)
			}
			ui.Box(c).Absolute().Left(154).Top(9).Width(52).Height(22).Radius(7).
				Background(kbdBg).Border(1, kbdBorder).IgnoreInput(true)
			CenteredText(c, "Ctrl K", 154, 9, 52, 22, 11, s.OnSurfaceVariant, false).IgnoreInput(true)
		}
	})
}

func pageDescription(page int) string {
	switch page {
	case PagePlugins:
		return "自由组合积木，扩展灵动岛、概览与桌面。"
	case PageWidgets:
		return "把时间、音乐与小事，轻轻放到桌面。"
	case PageFocus:
		return "选择一段时间，专心完成眼前的事。"
	case PageTodos:
		return "记录安排，按时提醒。"
	case PageNotifications:
		return "收拢每一条提醒，按自己的节奏查看。"
	case PageSettings:
		return "外观、灵动岛与日常偏好，一个地方就好。"
	}
	return ""
}

func darkModeIcon(dark bool) string {
	if dark {
		return "light_mode"
	}
	return "dark_mode"
}

func islandToggleText(p *prefs.Preferences) string {
	if p.Island {
		return "隐藏灵动岛"
	}
	return "显示灵动岛"
}

// content reserves the sidebar and gives every page a bounded scroll viewport.
func (a *App) content(c *ui.Context, w, h float32) {
	clipTop, clipBottom := float32(ContentTop), h-24
	clipH = float64(clipBottom - clipTop)
	side := a.sidebarWidth(w)
	bodyW := w - side - 56
	base := a.centerPage()
	route := fmt.Sprintf("content-%d", base)
	if base == PageHome {
		route += "-" + a.overview.config.View
	}
	body := ui.Box(c).Key(route).Absolute().Left(side + 28).Top(clipTop).Width(bodyW).Height(clipBottom - clipTop)
	initial := float32(1)
	if a.contentMounted {
		initial = 0
	}
	progress := appTweenValue(c, body, "entrance", initial, 1, motionPage, a.Palette.ReducedMotion)
	body.Translate(0, 12*(1-progress))
	a.contentMounted = true
	body.Clip()
	body.Children(func() {
		if base == PageHome && a.overview.config.View == "scroll" {
			a.overviewPage(c, 4, 9, bodyW-8)
			return
		}
		scroller := ui.Scroll(c).Scrollbars(false).Fill().TrackScroll(&a.Scroll[base])
		scroller.Children(func() {
			page := ui.Box(c).FillWidth().Height(a.contentH[base])
			page.Children(func() {
				if measured := a.pageBody(c, bodyW+64, base) + 20; measured != a.contentH[base] {
					a.contentH[base] = measured
					c.AnimationFrame()
				}
			})
		})
	})
}

// pageBody measures each page so every action remains reachable in small windows.
func (a *App) pageBody(c *ui.Context, w float32, page int) float32 {
	// Rust: x=36 in window coords (clip at 32), y = content_top + 9 - scroll.
	x, y := float32(4), float32(9)
	switch page {
	case PageHome:
		return a.overviewPage(c, x, y, w-72)
	case PagePlugins:
		return a.pluginsPage(c, x, y, w-72)
	case PageWidgets:
		return a.widgetsPage(c, x, y, w-72)
	case PageFocus:
		return a.focusPage(c, x, y, w-72)
	case PageTodos:
		return a.todosPage(c, x, y, w-72)
	case PageNotifications:
		return a.notificationsPage(c, x, y, w-72)
	}
	return 0
}

// fontWeight picks the semibold face for the selected nav label.
func fontWeight(selected bool) int {
	if selected {
		return 500
	}
	return 400
}

func navIcon(page int) string {
	switch page {
	case PageHome:
		return "home"
	case PageWidgets:
		return "folder"
	case PageFocus:
		return "timer"
	case PageTodos:
		return "checklist"
	case PageNotifications:
		return "volume_up"
	default:
		return "settings"
	}
}
