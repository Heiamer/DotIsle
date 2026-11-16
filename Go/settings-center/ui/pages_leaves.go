// The settings leaf pages (tabs 0..9), ported 1:1 from the Vello renderers:
// views.rs fn settings (group rows), preferences_ui.rs (toggles/choices/range/
// action/sound_section/extra), appearance.rs, background.rs, software_update.rs
// and module_management.rs. Original theme only; layout constants, sizes, colors
// and Chinese strings are copied verbatim from the Rust sources.
package ui

import (
	"fmt"
	"math"
	"os"
	"strings"

	"dotisle/settings/prefs"
	"github.com/egoist/mygo/ui"
)

// leafSettingsBody dispatches one leaf tab (views.rs fn settings + extra) and
// returns the content height for the scroll bounds.
func (a *App) leafSettingsBody(c *ui.Context, p *Palette, x, y, w float32, tab uint8) float32 {
	switch tab {
	case 0:
		return a.appearanceBody(c, p, x, y, w)
	case 1:
		return a.lyricBody(c, p, x, y, w)
	case 2:
		return a.notificationPrefsBody(c, p, x, y, w)
	case 3:
		return a.aboutBody(c, p, x, y, w)
	case 4:
		return a.windowBody(c, p, x, y, w)
	case 5:
		return a.devicesBody(c, p, x, y, w)
	case 6:
		return a.soundsBody(c, p, x, y, w)
	case 7:
		return a.appRulesBody(c, p, x, y, w)
	case 8:
		return a.focusPrefsBody(c, p, x, y, w)
	case 9:
		return a.moduleManagerBody(c, p, x, y, w)
	}
	return y
}

// ---- shared row builders (preferences_ui.rs helpers) --------------------------------

type leafToggleRow struct {
	title, detail, key string
	def                bool
}

// leafHeading is the 21px section title, +44 advance.
func leafHeading(c *ui.Context, p *Palette, title string, x, y, w float32) float32 {
	Label(c, title, x, y+4, 16, p.Scheme.OnSurface, true, w)
	return y + 44
}

// leafToggles is the toggle panel: rows*76+12, each row 76 DIP with its switch.
func (a *App) leafToggles(c *ui.Context, p *Palette, x, y, w float32, rows []leafToggleRow) float32 {
	s := &p.Scheme
	Panel(c, p, x, y, w, float32(len(rows))*76+12, s.Container)
	for i, row := range rows {
		on := a.leafBool(row.key, row.def)
		ToggleRow(c, p, row.key, x+24, y+6+float32(i)*76, w-48, row.title, row.detail, on, func() {
			a.pushNative(row.key, !on)
		})
	}
	return y + float32(len(rows))*76 + 28
}

// leafAction is the 96px panel with a trailing tonal button, +112 advance.
func leafAction(c *ui.Context, p *Palette, x, y, w float32, title, detail, label string, onClick func()) float32 {
	s := &p.Scheme
	Panel(c, p, x, y, w, 96, s.Container)
	Label(c, title, x+24, y+16, 14, s.OnSurface, true, w-222)
	Label(c, detail, x+24, y+48, 12, s.OnSurfaceVariant, false, w-222)
	Button(c, p, title, x+w-176, y+26, 152, 44, label, "", true, true, onClick)
	return y + 112
}

// leafChoices is the 108px choice panel with full-width chips (38 high), +124.
func leafChoices(c *ui.Context, p *Palette, x, y, w float32, title string, options [][2]string, current string, onPick func(value string)) float32 {
	s := &p.Scheme
	Panel(c, p, x, y, w, 108, s.Container)
	Label(c, title, x+24, y+14, 14, s.OnSurface, true, w-48)
	labels := make([]string, len(options))
	selected := -1
	for i, opt := range options {
		labels[i] = opt[0]
		if current == opt[1] {
			selected = i
		}
	}
	Segmented(c, p, x+24, y+52, w-48, labels, selected, func(i int) { onPick(options[i][1]) })
	return y + 124
}

// leafRange is the 96px slider panel: 15px title, "{v:.1} {unit}" trailing at the
// right (13 bold primary), settings slider below; commits the rounded value.
func (a *App) leafRange(c *ui.Context, p *Palette, x, y, w float32, title, key string, def, min, max float64, unit string, onCommit func(float64)) float32 {
	s := &p.Scheme
	Panel(c, p, x, y, w, 96, s.Container)
	committed := a.leafFloat(key, def)
	value := min + (max-min)*leafSliderFrac(key, committed, min, max)
	Label(c, title, x+24, y+14, 15, s.OnSurface, false, w-160)
	leafTrailingLabel(c, fmt.Sprintf("%.1f %s", value, unit), x+w-24, y+14, 13, s.Primary, true, 108)
	leafSettingsSlider(c, p, title, key, x+24, y+52, w-48, min, max, committed, onCommit)
	return y + 112
}

// leafTrailingLabel right-aligns one line inside a max-width box (trailing_label).
func leafTrailingLabel(c *ui.Context, text string, right, y, size float32, rgb uint32, bold bool, maxw float32) {
	e := ui.Box(c).Absolute().Left(right - maxw).Top(y).Width(maxw).Height(LineHeight(size)).
		Justify(ui.End).AlignItems(ui.Start)
	e.Children(func() {
		t := ui.Text(c, text).Font(textFamily(text)).FontSize(size).FixedLineHeight(LineHeight(size)).
			TextColor(Col(rgb)).SingleLine()
		if bold {
			t.FontWeight(500)
		}
	})
}

// ---- settings slider (settings_slider.rs) --------------------------------------------

// The kit Slider tracks a 0..1 knob at its own geometry; the Rust settings pages
// draw the MIUIX pill slider (28px track, radius 14, thumb 28*0.36) with a
// commit-on-release contract: preview while dragging, one native edit on release.
type leafSliderState struct {
	frac      float64
	prev      float64
	hadPrev   bool
	commit    float64
	sawCommit bool
	held      bool // the pointer was down in some build since the last release
}

var leafSliders = map[string]*leafSliderState{}

func leafSliderStateFor(key string, committed, min, max float64) *leafSliderState {
	st, ok := leafSliders[key]
	if !ok {
		st = &leafSliderState{frac: leafFraction(committed, min, max), commit: committed}
		leafSliders[key] = st
	}
	return st
}

// leafSliderFrac is the live display fraction of one slider key.
func leafSliderFrac(key string, def, min, max float64) float64 {
	if st, ok := leafSliders[key]; ok {
		return st.frac
	}
	return leafFraction(def, min, max)
}

func leafFraction(v, min, max float64) float64 {
	if max <= min {
		return 0
	}
	f := (v - min) / (max - min)
	if f < 0 {
		return 0
	}
	if f > 1 {
		return 1
	}
	return f
}

// leafSettingsSlider mirrors settings_slider::render: outline_variant pill track,
// primary fill up to the thumb centre plus its radius, white thumb with the same
// shadow, engagement growth while pressed.
func leafSettingsSlider(c *ui.Context, p *Palette, label, key string, x, y, w float32, min, max, committed float64, onCommit func(float64)) {
	st := leafSliderStateFor(key, committed, min, max)
	knob := ui.SliderBase(c, &st.frac, 0, 1).Absolute().Left(x).Top(y).Width(w).Height(28).
		Label(label).FocusRing(false)
	frac := st.frac
	prev, hadPrev := st.prev, st.hadPrev
	// PressedButton follows the captured press wherever the pointer goes;
	// Pressed() drops it outside the 28px band and would commit mid-gesture.
	btn, down := knob.PressedButton()
	pressed := down && btn == 0
	knob.Children(func() { sliderVisual(c, p, w, 28, frac, true, pressed) })
	// Release commit mirrors kit.Slider: the held flag ends the gesture exactly
	// once even when the last move was already built before the up arrived (a
	// paused real release, or every PostMessage-driven drag); frac != prev alone
	// missed those. Keyboard commits stay behind the Focused() gate so stray
	// slot writes can never persist a value the user never touched.
	commitFrac := func(f float64, rewind bool) {
		v := math.Round(min + (max-min)*f)
		if v < min {
			v = min
		}
		if v > max {
			v = max
		}
		if v != st.commit {
			onCommit(v)
			st.commit = v
			st.sawCommit = true
		}
		// Only a finished gesture snaps the knob to the committed value. Rewinding
		// on keyboard commits too would fight mygo's 0.01 fraction grid: on small
		// ranges one arrow step is worth less than half a unit, so the rounded
		// rewind made every press re-commit the same value forever.
		if rewind {
			st.frac = leafFraction(v, min, max)
		}
	}
	if pressed {
		st.held = true
		st.commit = committed
		st.sawCommit = true
	} else if st.held {
		st.held = false
		if onCommit != nil {
			commitFrac(st.frac, true)
		}
	} else if hadPrev && frac != prev && knob.Focused() && onCommit != nil {
		commitFrac(frac, false)
	} else if !pressed {
		// Idle: follow the committed value when it changed elsewhere (snapshot sync).
		if !st.sawCommit || st.commit != committed {
			st.commit = committed
			st.sawCommit = true
			if hadPrev && frac == prev {
				st.frac = leafFraction(committed, min, max)
			}
		}
	}
	st.prev = st.frac
	st.hadPrev = true
}

// ---- inline edit rows (Action::EditSetting dialogs become inline fields) -------------

type leafEditField struct {
	text       string
	lastNative string
	seeded     bool
	focused    bool
}

var (
	leafEditFields = map[string]*leafEditField{}
	leafEditOpen   = map[string]bool{}
)

func leafIsListKey(key string) bool {
	switch key {
	case "IgnoredSources", "NotificationAllowedApps", "NotificationBlockedApps":
		return true
	}
	return false
}

// leafEditNativeText renders the native setting as editable text: arrays join with
// "; " exactly like Action::EditSetting.
func (a *App) leafEditNativeText(key string) string {
	raw, ok := a.settings()[key]
	if !ok {
		return ""
	}
	if leafIsListKey(key) {
		if arr, ok := raw.([]any); ok {
			parts := make([]string, 0, len(arr))
			for _, item := range arr {
				if s, ok := item.(string); ok {
					parts = append(parts, s)
				}
			}
			return strings.Join(parts, "; ")
		}
		return ""
	}
	s, _ := raw.(string)
	return s
}

// leafCommitEdit submits the dialog: list keys split on ';' / '；', trim and drop
// empties (SaveModal DialogKind::Setting); everything else sends the raw string.
func (a *App) leafCommitEdit(key, text string) {
	st := leafEditFields[key]
	if leafIsListKey(key) {
		parts := strings.FieldsFunc(text, func(r rune) bool { return r == ';' || r == '；' })
		list := make([]string, 0, len(parts))
		for _, part := range parts {
			part = strings.TrimSpace(part)
			if part != "" {
				list = append(list, part)
			}
		}
		a.pushNative(key, list)
		if st != nil {
			st.lastNative = strings.Join(list, "; ")
		}
		return
	}
	a.pushNative(key, text)
	if st != nil {
		st.lastNative = text
	}
}

// leafEditRow is the Action::EditSetting row; the button unfolds an inline field
// (the Rust dialog equivalent) and Enter or 保存 commits the native edit.
func (a *App) leafEditRow(c *ui.Context, p *Palette, x, y, w float32, key, title, detail, button, hint string) float32 {
	s := &p.Scheme
	y = leafAction(c, p, x, y, w, title, detail, button, func() {
		leafEditOpen[key] = !leafEditOpen[key]
	})
	if !leafEditOpen[key] {
		return y
	}
	st, ok := leafEditFields[key]
	if !ok {
		st = &leafEditField{}
		leafEditFields[key] = st
	}
	native := a.leafEditNativeText(key)
	if !st.seeded || (!st.focused && native != st.lastNative) {
		st.text = native
		st.lastNative = native
		st.seeded = true
	}
	Panel(c, p, x, y, w, 96, s.Container)
	input := ui.TextInput(c, &st.text).Absolute().Left(x + 24).Top(y + 16).Width(w - 184).Height(48).
		FontSize(14).TextColor(Col(s.OnSurface)).Placeholder(hint)
	fieldStyle(&input, p)
	st.focused = input.Focused()
	if input.Submitted() {
		a.leafCommitEdit(key, st.text)
		leafEditOpen[key] = false
		return y + 112
	}
	Button(c, p, "保存"+key, x+w-152, y+18, 128, 44, "保存", "check", true, true, func() {
		a.leafCommitEdit(key, st.text)
		leafEditOpen[key] = false
	})
	return y + 112
}

// ---- snapshot readers ----------------------------------------------------------------

func (a *App) leafBool(key string, def bool) bool {
	if v, ok := a.settings()[key].(bool); ok {
		return v
	}
	return def
}

func (a *App) leafString(key, def string) string {
	if v, ok := a.settings()[key].(string); ok {
		return v
	}
	return def
}

func (a *App) leafFloat(key string, def float64) float64 {
	if v, ok := a.settings()[key].(float64); ok {
		return v
	}
	return def
}

func leafStr(v any, def string) string {
	if s, ok := v.(string); ok {
		return s
	}
	return def
}

// ---- tab 0 · 控制中心外观 (theme_options + background.rs + appearance.rs) -------------

// backgroundSettingsBody is background.rs settings: the panel is 132 when empty and
// 488 once a picture is loaded. Original renders slot 0 (themes.rs background_slot).
func (a *App) backgroundSettingsBody(c *ui.Context, p *Palette, x, y, w float32) float32 {
	s := &p.Scheme
	const slot = 0
	if slot >= len(a.Prefs.Backgrounds) {
		return y
	}
	bg := &a.Prefs.Backgrounds[slot]
	loaded := bg.Path != ""
	height := float32(488)
	if !loaded {
		height = 132
	}
	Panel(c, p, x, y, w, height, s.Container)
	Label(c, "自定义图片背景", x+24, y+16, 14, s.OnSurface, true, w-380)
	Label(c, "仅用于「默认」 · 支持 PNG / JPEG / BMP", x+24, y+46, 12, s.OnSurfaceVariant, false, w-380)
	// Action::BackgroundPick: native file dialog, then the Rust import flow
	// (copy into Data/Backgrounds, set path+enabled, persist preferences.json).
	pickLabel := "选择图片"
	if loaded {
		pickLabel = "更换图片"
	}
	Button(c, p, "更换背景图片", x+w-300, y+20, 140, 44, pickLabel, "folder", false, true, func() {
		a.pickBackgroundImage()
	})
	Button(c, p, "恢复默认背景", x+w-148, y+20, 124, 44, "恢复默认", "", true, loaded, func() {
		a.Prefs.Backgrounds[slot] = prefs.Background{Opacity: 0.3, Brightness: 1, Mode: "Cover"}
		a.savePrefs()
	})
	// background.rs shows Cache::error in the status slot, in the error color, and
	// keeps the hint otherwise.
	status, statusColor := "选择自己的图片，预览后调整透明度、明暗与磨砂度", s.OnSurfaceVariant
	if _, errMsg := a.wallpaper(); errMsg != "" {
		status, statusColor = errMsg, s.Error
	} else if loaded {
		status = "图片已保存在本机，切换主题后仍会保留"
	}
	Label(c, status, x+24, y+90, 12, statusColor, false, w-48)
	if !loaded {
		return y + 148
	}
	ToggleRow(c, p, "使用图片背景", x+24, y+120, w-48, "使用图片背景", "关闭后保留图片与参数", bg.Enabled, func() {
		bg.Enabled = !bg.Enabled
		a.savePrefs()
	})
	// BackgroundMode: Cover / Contain chips.
	cw := (w - 56) / 2
	Chip(c, p, "背景填满", x+24, y+200, cw, 40, "填满 · 居中裁切", bg.Mode != "Contain", func() {
		bg.Mode = "Cover"
		a.savePrefs()
	})
	Chip(c, p, "背景适应", x+24+cw+8, y+200, cw, 40, "适应 · 完整显示", bg.Mode == "Contain", func() {
		bg.Mode = "Contain"
		a.savePrefs()
	})
	// Three preview sliders; commits write the local slot (background_value).
	type leafBgRow struct {
		title, key string
		v, max     float64
		unit       string
	}
	rows := [3]leafBgRow{
		{"图片透明度", "BackgroundOpacity", bg.Opacity * 100, 100, "%"},
		{"图片明暗", "BackgroundBrightness", bg.Brightness * 100, 200, "%"},
		{"图片磨砂度", "BackgroundBlur", bg.Blur, 40, ""},
	}
	for i, row := range rows {
		yy := y + 256 + float32(i)*72
		v := row.max * leafSliderFrac(row.key, row.v, 0, row.max)
		Label(c, row.title, x+24, yy, 14, s.OnSurface, false, w-200)
		leafTrailingLabel(c, fmt.Sprintf("%.0f%s", v, row.unit), x+w-24, yy, 12, s.OnSurfaceVariant, false, 88)
		leafSettingsSlider(c, p, row.title, row.key, x+24, yy+30, w-48, 0, row.max, row.v, func(out float64) {
			b := &a.Prefs.Backgrounds[slot]
			switch row.key {
			case "BackgroundOpacity":
				b.Opacity = clamp01(out / 100)
			case "BackgroundBrightness":
				b.Brightness = out / 100
				if b.Brightness > 2 {
					b.Brightness = 2
				}
			case "BackgroundBlur":
				b.Blur = out
				if b.Blur > 40 {
					b.Blur = 40
				}
			}
			a.savePrefs()
		})
	}
	return y + 504
}

// appearanceColorsBody is appearance.rs render (Original branch).
func (a *App) appearanceColorsBody(c *ui.Context, p *Palette, x, y, w float32) float32 {
	s := &p.Scheme
	y = leafChoices(c, p, x, y, w, "外观模式", [][2]string{{"跟随系统", "System"}, {"浅色", "Light"}, {"深色", "Dark"}},
		a.leafString("SettingsTheme", "System"), func(value string) {
			if value != "System" {
				a.Prefs.Dark = value == "Dark"
			}
			a.savePrefs()
			a.pushNative("SettingsTheme", value)
		})
	Panel(c, p, x, y, w, 164, s.Container)
	contrast := a.leafBool("DynamicColorHighContrast", false)
	ToggleRow(c, p, "增强颜色对比", x+24, y+8, w-48, "增强颜色对比", "提高文字和边界的辨识度", contrast, func() {
		a.pushNative("DynamicColorHighContrast", !contrast)
	})
	animationOn := a.leafBool("Animation", !a.Prefs.ReducedMotion)
	ToggleRow(c, p, "减少动态效果", x+24, y+88, w-48, "减少动态效果", "保留状态反馈，减少弹性与过渡", !animationOn, func() {
		a.Prefs.ReducedMotion = animationOn
		a.savePrefs()
		a.pushNative("Animation", !animationOn)
	})
	y += 180
	Label(c, "控制中心配色独立于专辑封面与系统强调色。", x+4, y, 12, s.OnSurfaceVariant, false, w-8)
	return y + 30
}

// artworkSeed reports whether a valid cover colour is available (m.artwork seed).
func (a *App) artworkSeed() float64 {
	if v, ok := stateFloat(a.State, "artworkSeed"); ok {
		return v
	}
	// The Go snapshot carries the cover preview path instead of the decoded seed.
	if preview, ok := a.State["musicPreview"].(map[string]any); ok {
		if path, _ := preview["coverPath"].(string); path != "" {
			return 1
		}
	}
	return 0
}

// ---- tab 1 · 歌词与本地文件 (views.rs group + extra tab 1) -----------------------------

func (a *App) lyricBody(c *ui.Context, p *Palette, x, y, w float32) float32 {
	s := &p.Scheme
	// views.rs group(): title 20 bold, panel rows*80+16, 0.5px separators.
	Label(c, "聆听偏好", x, y+5, 20, s.OnSurface, true, w)
	y += 48
	rows := [][2]string{
		{"显示翻译", "Translation"},
		{"逐字高亮", "WordHighlight"},
		{"在线歌曲补全", "OnlineSongInfo"},
	}
	details := [3]string{
		"有译文时在当前歌词下方显示",
		"歌词随歌曲进度渐次点亮",
		"使用现有音乐服务补全歌曲信息和歌词",
	}
	stored := [3]*bool{&a.Prefs.Translation, &a.Prefs.WordHighlight, &a.Prefs.Online}
	Panel(c, p, x, y, w, float32(len(rows)*80+16), s.Container)
	for i, row := range rows {
		key := row[1]
		on := a.leafBool(key, *stored[i])
		ToggleRow(c, p, key, x+24, y+8+float32(i)*80, w-48, row[0], details[i], on, func() {
			*stored[i] = !on
			a.savePrefs()
			a.pushNative(key, !on)
		})
		if i+1 < len(rows) {
			ui.Box(c).Absolute().Left(x + 24).Top(y + 88 + float32(i)*80).Width(w - 48).Height(1).
				Background(AlphaCol(s.Outline, 0.5))
		}
	}
	y += float32(len(rows)*80+16) + 28
	// 本地歌词 folder panel.
	Panel(c, p, x, y, w, 88, s.Container)
	Label(c, "本地歌词", x+24, y+16, 17, s.OnSurface, true, w-220)
	Label(c, fmt.Sprintf("%d 个文件夹 · 支持本地歌词优先匹配", len(a.Prefs.LocalFolders)),
		x+24, y+48, 12, s.OnSurfaceVariant, false, w-220)
	// Action::Paths: native directory picker, then append to LocalFolders.
	Button(c, p, "管理歌词文件夹", x+w-172, y+22, 148, 44, "管理文件夹", "folder", true, true, func() {
		a.pickLocalFolder()
	})
	y += 116
	Label(c, "同步偏移与播放器来源在「音乐与歌词」中统一管理。", x, y, 13, s.OnSurfaceVariant, false, w)
	y += 30
	// Folder list with working per-folder removal (NativeEdit LocalFolders array).
	if len(a.Prefs.LocalFolders) > 0 {
		h := float32(len(a.Prefs.LocalFolders))*56 + 16
		Panel(c, p, x, y, w, h, s.Container)
		for i, folder := range a.Prefs.LocalFolders {
			rowY := y + 8 + float32(i)*56
			inner := ui.Box(c).Absolute().Left(x + 8).Top(rowY).Width(w - 16).Height(52).
				Radius(16).Background(Col(s.Container))
			inner.Children(func() {
				Label(c, folder, 16, 15, 13, s.OnSurface, false, w-150)
			})
			index := i
			IconButton(c, p, "移除歌词文件夹", x+w-64, rowY+6, 44, 44, "close", false, true, func() {
				remaining := make([]string, 0, len(a.Prefs.LocalFolders)-1)
				remaining = append(remaining, a.Prefs.LocalFolders[:index]...)
				remaining = append(remaining, a.Prefs.LocalFolders[index+1:]...)
				a.Prefs.LocalFolders = remaining
				a.savePrefs()
				a.pushNative("LocalFolders", remaining)
			})
		}
		y += h + 16
	}
	// preferences_ui.rs extra(tab 1): Action::IslandSettings(4) → tab 132.
	y = leafAction(c, p, x, y, w, "播放器与歌词来源", "管理播放器排除列表、在线补全与同步偏移。", "播放器与同步", func() {
		a.openSettings(132)
	})
	return y
}

// ---- tab 2 · 应用通知与历史 (preferences_ui.rs extra tab 2) -----------------------------

func (a *App) notificationPrefsBody(c *ui.Context, p *Palette, x, y, w float32) float32 {
	y = leafHeading(c, p, "应用通知", x, y, w)
	y = a.leafToggles(c, p, x, y, w, []leafToggleRow{
		{"验证码灵动岛", "识别通知中的验证码，点击即可复制；遵循应用筛选和暂停设置。", "VerificationCodeEnabled", true},
		{"接收应用通知", "在灵动岛显示 Windows 通知。", "NotificationsEnabled", true},
		{"暂停通知", "暂时暂停应用通知与到期提醒。", "PauseNotifications", false},
		{"点击通知打开应用", "点击通知正文时打开来源。", "NotificationClickOpenApp", true},
		{"只接收允许列表", "开启后仅接收列表中的应用。", "NotificationWhitelistEnabled", false},
		{"屏蔽指定应用", "开启后忽略屏蔽列表中的应用。", "NotificationBlacklistEnabled", false},
	})
	y = a.leafEditRow(c, p, x, y, w, "NotificationAllowedApps",
		"通知允许列表", "应用标识用分号分隔。", "编辑列表", "通知允许列表 · 分号分隔")
	y = a.leafEditRow(c, p, x, y, w, "NotificationBlockedApps",
		"通知屏蔽列表", "应用标识用分号分隔。", "编辑列表", "通知屏蔽列表 · 分号分隔")
	y = leafAction(c, p, x, y, w, "通知预览", "用本地示例检查横幅外观。", "预览通知", func() {
		a.command("notificationPreview", nil)
	})
	y = leafHeading(c, p, "消息灵动岛", x, y, w)
	y = a.leafToggles(c, p, x, y, w, []leafToggleRow{
		{"微信", "仅提醒有新消息，不读取聊天内容。", "WeChatMessageHintsEnabled", true},
		{"飞书", "仅提醒有新消息，不读取聊天内容。", "FeishuMessageHintsEnabled", true},
		{"钉钉", "仅提醒有新消息，不读取聊天内容。", "DingTalkMessageHintsEnabled", true},
		{"企业微信", "仅提醒有新消息，不读取聊天内容。", "WeComMessageHintsEnabled", true},
	})
	y = leafHeading(c, p, "通知记录", x, y, w)
	y = a.leafToggles(c, p, x, y, w, []leafToggleRow{
		{"保留通知历史", "仅保留点屿实际接收的记录。", "NotificationHistoryEnabled", true},
	})
	y = a.leafRange(c, p, x, y, w, "历史保留时间", "NotificationHistoryDays", 7, 1, 30, "天", func(v float64) {
		a.pushNative("NotificationHistoryDays", int(v))
	})
	y = leafAction(c, p, x, y, w, "通知中心", "查看与清理已接收的记录。", "打开通知中心", func() {
		a.navigate(PageNotifications)
	})
	return y
}

// ---- tab 3 · 关于与更新 (software_update.rs render) ------------------------------------

func leafUpdateAction(phase string) (string, string) {
	switch phase {
	case "available":
		return "下载更新", "updateDownload"
	case "ready":
		return "重启并安装", "updateApply"
	case "downloading", "switching", "verifying":
		return "取消下载", "updateCancel"
	case "skipped":
		return "恢复此次更新", "updateUnskip"
	case "checking", "applying", "restarting":
		return "", ""
	}
	return "检查更新", "checkUpdate"
}

func leafUpdateStatus(phase string) string {
	switch phase {
	case "checking":
		return "正在检查更新…"
	case "available":
		return "发现新版本"
	case "downloading":
		return "正在下载更新"
	case "switching":
		return "正在切换更新源"
	case "verifying":
		return "正在校验更新包"
	case "ready":
		return "更新已就绪"
	case "applying":
		return "正在安装更新"
	case "restarting":
		return "正在重启点屿"
	case "completed":
		return "更新已完成"
	case "failed":
		return "更新未完成"
	case "cancelled":
		return "已取消更新"
	case "skipped":
		return "已跳过此次更新"
	}
	return "检查软件更新"
}

// ---- tab 4 · 启动、窗口与字体 (preferences_ui.rs extra tab 4) ---------------------------

func (a *App) windowBody(c *ui.Context, p *Palette, x, y, w float32) float32 {
	s := &p.Scheme
	y = leafHeading(c, p, "启动与窗口", x, y, w)
	y = a.leafToggles(c, p, x, y, w, []leafToggleRow{
		{"开机启动", "登录 Windows 后驻留托盘。", "AutoStart", true},
		{"设置开场动画", "Rossela 字标落墨入场，随后淡入控制中心；点击可跳过。", "StartupSignatureEnabled", true},
		{"开机问候", "启动时显示昨日用机时长和今日天气。", "StartupGreetingEnabled", true},
	})
	y = leafHeading(c, p, "天气与日常", x, y, w)
	y = a.leafEditRow(c, p, x, y, w, "WeatherCity", "所在城市", "用于今日天气和降水提醒，也可沿用天气组件的城市。", "设置城市", "城市 · 如 上海 / Beijing")
	y = a.leafToggles(c, p, x, y, w, []leafToggleRow{
		{"降水与降雪提醒", "提前约两小时提醒预计降水时段，精度随小时或三小时来源而定。", "WeatherAlertsEnabled", true},
	})
	y = leafAction(c, p, x, y, w, "用机时长", "本机记录；锁屏、休眠和连续 5 分钟无操作时不计时，从启用点屿时开始累计。", "查看概览", func() { a.navigate(PageHome) })
	y = leafChoices(c, p, x, y, w, "控制中心打开页面", [][2]string{
		{"概览", "overview"}, {"灵动岛", "island"}, {"计时", "timers"},
		{"待办", "todos"}, {"设置", "settings"},
	}, a.leafString("ControlCenterStartPage", "overview"), func(value string) {
		a.pushNative("ControlCenterStartPage", value)
	})
	y = leafChoices(c, p, x, y, w, "界面主题", [][2]string{
		{"跟随系统", "System"}, {"浅色", "Light"}, {"深色", "Dark"},
	}, a.leafString("SettingsTheme", "System"), func(value string) {
		if value == "Light" || value == "Dark" {
			a.Prefs.Dark = value == "Dark"
			a.savePrefs()
		}
		a.pushNative("SettingsTheme", value)
	})
	y = leafHeading(c, p, "快速提醒", x, y, w)
	// TODO: Action::QuickReminder opens the 一句话快速提醒 dialog (field, preview and
	// quickReminderCommit); the dialog flow lands before this button can work.
	Button(c, p, "快速提醒", x, y, 180, 44, "一句话创建提醒", "add", false, true, func() {
		a.showQuickReminder = true
	})
	y += 64
	hotkey := a.leafString("QuickReminderHotkey", "")
	hotkeyDetail := hotkey
	if hotkeyDetail == "" {
		hotkeyDetail = "尚未设置"
	}
	y = a.leafEditRow(c, p, x, y, w, "QuickReminderHotkey",
		"全局快捷键", hotkeyDetail, "修改快捷键", "例如 Ctrl+Alt+R · 留空关闭")
	y = leafAction(c, p, x, y, w, "紧凑尺寸", "恢复宽度、字号与固定位置。", "恢复尺寸", func() {
		a.command("resetAppearance", nil)
	})
	y = leafHeading(c, p, "字体", x, y, w)
	// The font scope panel (132): scope chips write the local FontScope preference.
	family := a.leafString("IslandFontFamily", "")
	if family == "" {
		family = "默认字体"
	}
	Panel(c, p, x, y, w, 132, s.Container)
	Label(c, "字体作用范围", x+24, y+14, 14, s.OnSurface, true, w-48)
	Label(c, fmt.Sprintf("当前字体：%s", family), x+24, y+40, 12, s.OnSurfaceVariant, false, w-48)
	scopeCw := (w - 48 - 16) / 3
	for i, opt := range [][2]string{{"仅灵动岛", "Island"}, {"仅控制中心", "Center"}, {"两者", "Both"}} {
		value := opt[1]
		Chip(c, p, "字体范围"+opt[0], x+24+float32(i)*(scopeCw+8), y+60, scopeCw, 40, opt[0],
			a.Prefs.FontScope == value, func() {
				a.Prefs.FontScope = value
				a.savePrefs()
			})
	}
	Label(c, "换字体后控制中心文字块仍锚定原基线，矩形不移动。", x+24, y+104, 11, s.OnSurfaceVariant, false, w-48)
	y += 148
	return y
}

// ---- tab 5 · 设备连接提醒 (preferences_ui.rs extra tab 5) -------------------------------

func (a *App) devicesBody(c *ui.Context, p *Palette, x, y, w float32) float32 {
	y = leafHeading(c, p, "设备连接", x, y, w)
	y = a.leafToggles(c, p, x, y, w, []leafToggleRow{
		{"设备连接提醒", "识别设备接入和移除。", "DeviceRemindersEnabled", true},
		{"耳机连接", "有线与蓝牙耳机接入时提醒。", "HeadphoneConnectionRemindersEnabled", true},
		{"音响连接", "音响或扬声器接入时提醒。", "SpeakerConnectionRemindersEnabled", true},
		{"手柄连接", "XInput 与 HID 手柄接入时提醒。", "GamepadConnectionRemindersEnabled", true},
		{"鼠标连接", "休眠唤醒可能重复连接，原版默认关闭。", "MouseConnectionRemindersEnabled", false},
		{"键盘连接", "休眠唤醒可能重复连接，原版默认关闭。", "KeyboardConnectionRemindersEnabled", false},
		{"设备断开", "移除设备时也显示提醒。", "DeviceDisconnectRemindersEnabled", true},
	})
	y = leafAction(c, p, x, y, w, "设备提醒预览", "显示本地耳机连接示例。", "预览设备", func() {
		a.command("devicePreview", map[string]any{"kind": "headphones", "connected": true})
	})
	y = leafAction(c, p, x, y, w, "系统状态提示", "大小写、小键盘、麦克风和触控板状态。", "前往设置", func() {
		a.openSettings(130)
	})
	return y
}

// ---- tab 6 · 提醒音效 (preferences_ui.rs extra tab 6 + sound_section) ------------------

// The 12 original cues (dotisle_domain::sound_catalog::CUES): id, name, detail.
var leafSoundCues = [12][3]string{
	{"Soft", "柔点", "轻柔圆润 · 推荐通知"},
	{"Glass", "澄光", "通透轻亮 · 不尖锐"},
	{"Bell", "轻铃", "温暖和声 · 推荐待办"},
	{"Connect", "相遇", "轻快上行 · 推荐设备"},
	{"Bubble", "气泡", "圆润弹起 · 灵巧反馈"},
	{"Bloom", "花开", "三音绽放 · 轻盈舒展"},
	{"Ripple", "涟漪", "细小回响 · 安静提醒"},
	{"Breeze", "微风", "缓入轻落 · 柔和低频"},
	{"Wood", "木语", "短促木质 · 克制清晰"},
	{"Rise", "上扬", "三阶轻跃 · 积极反馈"},
	{"Drop", "落点", "两音收束 · 自然落下"},
	{"Finish", "和鸣", "完整和弦 · 完成提醒"},
}

// leafDefaultCue is sound_catalog::default_for.
func leafDefaultCue(prefix string) string {
	switch prefix {
	case "Todo":
		return "Bell"
	case "Device":
		return "Connect"
	}
	return "Soft"
}

// soundOpen keeps the accordion state (m.details); Rust defaults to all closed.
var leafSoundOpen = [3]bool{}

func (a *App) soundsBody(c *ui.Context, p *Palette, x, y, w float32) float32 {
	s := &p.Scheme
	Label(c, "12 款原创短音 · 柔和起落、统一音量", x, y+4, 13, s.OnSurfaceVariant, false, w-210)
	Button(c, p, "恢复推荐音色", x+w-190, y-4, 190, 40, "恢复推荐音色", "", true, true, func() {
		a.command("resetBuiltinSounds", nil)
	})
	y += 56
	sections := [3]struct {
		index   int
		title   string
		prefix  string
		enabled string
		kind    string
		volume  string
		path    string
		level   float64
	}{
		{0, "应用通知", "Notification", "NotificationSoundEnabled", "NotificationSound",
			"NotificationVolume", "NotificationSoundPath", 35},
		{1, "待办与计时", "Todo", "TodoSoundEnabled", "TodoSound",
			"TodoVolume", "TodoSoundPath", 45},
		{2, "设备连接", "Device", "DeviceSoundEnabled", "DeviceSound",
			"DeviceVolume", "DeviceSoundPath", 55},
	}
	for _, sec := range sections {
		y = a.soundSection(c, p, x, y, w, sec.index, sec.title, sec.prefix,
			sec.enabled, sec.kind, sec.volume, sec.path, sec.level)
	}
	return y
}

// soundSection is preferences_ui.rs sound_section verbatim: 76px header, enabled
// toggle, the 12-cue radio list, the volume range and the custom WAV editor.
func (a *App) soundSection(c *ui.Context, p *Palette, x, y, w float32, index int,
	title, prefix, enabled, kind, volume, path string, level float64) float32 {
	s := &p.Scheme
	open := leafSoundOpen[index]
	current := a.leafString(kind, leafDefaultCue(prefix))
	customPath := a.leafString(path, "")
	custom := customPath != "" && a.leafString(prefix+"SoundSource", "") != "Builtin"
	cueName := leafSoundCues[0][1]
	for _, cue := range leafSoundCues {
		if cue[0] == current {
			cueName = cue[1]
			break
		}
	}
	previewCommand := "soundPreview"
	if prefix == "Device" {
		previewCommand = "deviceSoundPreview"
	}
	previewValue := func(kindID string) map[string]any {
		value := map[string]any{"todo": prefix == "Todo"}
		if kindID != "" {
			value["kind"] = kindID
		}
		return value
	}
	ink := s.OnSurface
	if open {
		ink = s.OnSecondaryContainer
	}
	// Header: panel + hit area + title + detail + expand hint + preview button.
	Panel(c, p, x, y, w, 76, func() uint32 {
		if open {
			return s.SecondaryContainer
		}
		return s.Container
	}())
	headerHit := ui.ButtonBase(c).Absolute().Left(x).Top(y).Width(w - 100).Height(76).
		Label("切换" + title).Cursor(ui.CursorPointer)
	hoverLayer(&headerHit, ui.Transparent, ink, true)
	if headerHit.Clicked() {
		leafSoundOpen[index] = !leafSoundOpen[index]
	}
	Label(c, title, x+24, y+11, 17, ink, true, w-250)
	volumeNow := math.Round(a.leafFloat(volume, level))
	Label(c, fmt.Sprintf("%s · %d%%", func() string {
		if custom {
			return "自定义 WAV"
		}
		return cueName
	}(), int(volumeNow)), x+24, y+40, 12, ink, false, w-250)
	expand := "展开 ∨"
	if open {
		expand = "收起 ∧"
	}
	Label(c, expand, x+w-190, y+26, 12, ink, false, 80)
	Button(c, p, "试听"+title, x+w-90, y+18, 70, 40, "试听", "", true, true, func() {
		a.command(previewCommand, previewValue(""))
	})
	y += 88
	if !open {
		return y + 12
	}
	// 启用提示音 toggle.
	y = a.leafToggles(c, p, x, y, w, []leafToggleRow{
		{"启用提示音", "温和提醒，不改变其他应用或系统音量。", enabled, true},
	})
	// The 12-cue list: 12*56+16 panel with 52px rows and the radio circles.
	listH := float32(len(leafSoundCues))*56 + 16
	Panel(c, p, x, y, w, listH, s.Container)
	for i, cue := range leafSoundCues {
		rowY := y + 8 + float32(i)*56
		selected := !custom && current == cue[0]
		rowInk := s.OnSurface
		if selected {
			rowInk = s.OnSecondaryContainer
			ui.Box(c).Absolute().Left(x + 8).Top(rowY).Width(w - 16).Height(52).
				Radius(16).Background(Col(s.SecondaryContainer))
		}
		cueID := cue[0]
		rowHit := ui.ButtonBase(c).Absolute().Left(x + 8).Top(rowY).Width(w - 16 - 96).Height(52).
			Label("选择音色" + cue[1]).Cursor(ui.CursorPointer)
		hoverLayer(&rowHit, ui.Transparent, rowInk, true)
		if rowHit.Clicked() {
			a.command("soundSelect", map[string]any{"category": prefix, "source": "Builtin", "kind": cueID})
		}
		// Radio: outer 9px ring, inner 4px dot when selected else 7px container.
		radioY := rowY + 26
		ui.Box(c).Absolute().Left(x + 32 - 9).Top(radioY - 9).Width(18).Height(18).Radius(9).
			Background(func() ui.Color {
				if selected {
					return Col(s.Primary)
				}
				return Col(s.Outline)
			}())
		innerR := float32(7)
		innerRGB := s.Container
		if selected {
			innerR = 4
			innerRGB = s.OnPrimary
		}
		ui.Box(c).Absolute().Left(x + 32 - innerR).Top(radioY - innerR).Width(2 * innerR).Height(2 * innerR).
			Radius(innerR).Background(Col(innerRGB))
		Label(c, cue[1], x+54, rowY+7, 14, rowInk, true, w-200)
		Label(c, cue[2], x+54, rowY+29, 11, rowInk, false, w-200)
		Button(c, p, "试听音色"+cue[1], x+w-92, rowY+7, 68, 38, "试听", "", true, true, func() {
			a.command(previewCommand, previewValue(cueID))
		})
	}
	y += listH + 16
	// 音量 range.
	y = a.leafRange(c, p, x, y, w, "音量", volume, level, 0, 100, "%", func(v float64) {
		a.pushNative(volume, int(v))
	})
	// 自定义 WAV editor (Action::EditSetting on the path key).
	wavDetail := customPath
	if wavDetail == "" {
		wavDetail = "本地 PCM WAV · 可选"
	}
	y = a.leafEditRow(c, p, x, y, w, path, "自定义 WAV", wavDetail, "设置文件", "PCM WAV 完整路径 · 留空使用内置音色")
	if customPath != "" {
		Button(c, p, "使用自定义音效"+title, x, y, leafMinF(w, 220), 40, "使用已保存的自定义文件", "", true, true, func() {
			a.command("soundSelect", map[string]any{"category": prefix, "source": "Custom"})
		})
		y += 56
	}
	return y + 12
}

func leafMinF(a, b float32) float32 {
	if a < b {
		return a
	}
	return b
}

// ---- tab 7 · 按应用提醒 (preferences_ui.rs extra tab 7) ---------------------------------

func (a *App) appRulesBody(c *ui.Context, p *Palette, x, y, w float32) float32 {
	s := &p.Scheme
	y = leafHeading(c, p, "按应用提醒", x, y, w)
	y = a.leafToggles(c, p, x, y, w, []leafToggleRow{
		{"进入应用时提醒", "进入指定应用时显示预设提醒。", "AppEntryRemindersEnabled", false},
		{"前台使用时长提醒", "按每天累计前台使用时间提醒。", "AppUsageRemindersEnabled", false},
	})
	// TODO: Action::EditRule(None) opens the 新建应用提醒 dialog (name, path, message,
	// cooldown and threshold fields, trigger chip, appRuleSave); the dialog flow
	// lands before this button can work.
	Button(c, p, "新建应用提醒", x, y, 176, 44, "新建应用提醒", "add", false, true, func() {
		a.ruleEditID = ""
		a.showRuleDialog = true
	})
	y += 64
	rules, _ := a.State["appRules"].([]any)
	for _, item := range rules {
		rule, _ := item.(map[string]any)
		if rule == nil {
			continue
		}
		id := leafStr(rule["id"], "")
		enabled := rule["enabled"] == true
		Panel(c, p, x, y, w, 164, s.Container)
		Label(c, leafStr(rule["name"], "应用提醒"), x+24, y+14, 18, s.OnSurface, true, w-260)
		Label(c, leafStr(rule["message"], ""), x+24, y+46, 13, s.OnSurfaceVariant, false, w-48)
		Label(c, leafStr(rule["executablePath"], ""), x+24, y+74, 11, s.OnSurfaceVariant, false, w-48)
		// Action::EditRule(Some(id)): the rule dialog pre-fills from this row.
		Button(c, p, "编辑规则"+id, x+24, y+110, 100, 36, "编辑", "", true, true, func() {
			a.ruleEditID = id
			a.showRuleDialog = true
		})
		action, label := "enable", "启用"
		if enabled {
			action, label = "disable", "停用"
		}
		revision := rule["revision"]
		Button(c, p, label+"规则"+id, x+136, y+110, 120, 36, label, "", true, true, func() {
			a.command("appRuleAction", map[string]any{"id": id, "action": action, "revision": revision})
		})
		y += 180
	}
	return y
}

// ---- tab 8 · 计时与待办偏好 (preferences_ui.rs extra tab 8) ------------------------------

func (a *App) focusPrefsBody(c *ui.Context, p *Palette, x, y, w float32) float32 {
	s := &p.Scheme
	y = leafHeading(c, p, "待办与专注", x, y, w)
	y = a.leafToggles(c, p, x, y, w, []leafToggleRow{
		{"待办到期提醒", "到期后显示提醒。", "TodoRemindersEnabled", true},
		{"待办灵动岛组件", "有未完成待办时可切换到清单。", "TodoIslandEnabled", true},
		{"专注正向计时", "为新建专注计时使用正向显示。", "FocusCountUp", false},
		{"锁屏暂停专注", "解锁后可逐项继续计时。", "FocusPauseOnLock", false},
	})
	// 新待办默认颜色 row with the current color swatch; the full palette picker
	// (Action::PickColor("TodoDefaultColor", …)) lands separately.
	Panel(c, p, x, y, w, 96, s.Container)
	Label(c, "新待办默认颜色", x+24, y+16, 14, s.OnSurface, true, w-222)
	Label(c, "单个待办仍保留各自颜色。", x+24, y+48, 12, s.OnSurfaceVariant, false, w-222)
	if rgb, ok := parseHexColor(a.leafString("TodoDefaultColor", "#F3D79B")); ok {
		ui.Box(c).Absolute().Left(x+w-260).Top(y+34).Width(28).Height(28).
			Radius(14).Border(1, Col(s.OutlineVariant)).Background(Col(rgb))
	}
	entry := Button(c, p, "选择待办颜色", x+w-176, y+26, 152, 44, "选择颜色", "", true, true, func() {
		current := a.leafString("TodoDefaultColor", "#F3D79B")
		a.openPicker("TodoDefaultColor", "新待办默认颜色", hexValue(current), 0xF3D79B)
	})
	a.anchorPicker(entry, "TodoDefaultColor")
	y += 112
	return y
}

// ---- tab 9 · 模块管理 (module_management.rs render) --------------------------------------

// The 15 internal modules (dotisle_module_api::MODULES): id, label.
var leafModules = [15][2]string{
	{"core", "CORE 核心"}, {"settings", "控制中心"}, {"focus", "计时与流程"},
	{"todos", "待办与快捷提醒"}, {"music", "音乐与播放器"}, {"lyrics", "歌词检索与同步"},
	{"notifications", "通知接收"}, {"history", "通知历史"}, {"automation", "应用提醒规则"},
	{"devices", "设备连接提醒"}, {"osd", "系统状态提示"}, {"sound", "音效"},
	{"widgets", "桌面组件"}, {"themes", "主题与配色"}, {"glow", "外围光效"},
}

// leafModuleVersion reads the module version from the moduleManagement snapshot,
// falling back to the moduleVersions map, then the Rust placeholder text.
func (a *App) leafModuleVersion(id string) string {
	if state, ok := a.State["moduleManagement"].(map[string]any); ok {
		if items, ok := state["items"].([]any); ok {
			for _, item := range items {
				if m, ok := item.(map[string]any); ok && m["id"] == id {
					if v, ok := m["version"].(string); ok && v != "" {
						return v
					}
				}
			}
		}
	}
	if versions, ok := a.State["moduleVersions"].(map[string]any); ok {
		if v, ok := versions[id].(string); ok && v != "" {
			return v
		}
	}
	return "尚未读取"
}

func (a *App) moduleManagerBody(c *ui.Context, p *Palette, x, y, w float32) float32 {
	s := &p.Scheme
	state, _ := a.State["moduleManagement"].(map[string]any)
	if state == nil {
		state = map[string]any{}
	}
	phase := leafStr(state["phase"], "idle")
	busy := phase == "checking" || phase == "applying"
	staged := phase == "staged"
	Label(c, "每个功能独立更新，CORE 也可以替换", x, y, 19, s.OnSurface, true, w)
	y += 34
	Label(c, "只选择可信的内部 DLL。校验通过后重启安装，启动失败会恢复原模块。", x, y, 12, s.OnSurfaceVariant, false, w)
	y += 32
	Panel(c, p, x, y, w, 132, s.Container)
	Label(c, leafStr(state["message"], "连接主程序后可以查看模块版本"), x+20, y+16, 13, s.OnSurface, true, w-40)
	Button(c, p, "刷新模块", x+20, y+66, 142, 42, "刷新模块", "", !busy, !busy, func() {
		a.command("moduleRefresh", nil)
	})
	Button(c, p, "安装并重启", x+178, y+66, 154, 42, "安装并重启", "", staged, staged, func() {
		a.command("moduleApply", map[string]any{"callerSettingsPid": os.Getpid()})
	})
	Button(c, p, "取消暂存", x+348, y+66, 142, 42, "取消暂存", "", staged, staged, func() {
		a.command("moduleCancel", nil)
	})
	y += 152
	for _, mod := range leafModules {
		id := mod[0]
		Panel(c, p, x, y, w, 112, s.Container)
		Label(c, mod[1], x+20, y+15, 17, s.OnSurface, true, w-365)
		Label(c, fmt.Sprintf("dotisle_%s.dll · %s · API 1", id, a.leafModuleVersion(id)),
			x+20, y+55, 12, s.OnSurfaceVariant, false, w-365)
		// platform::choose_module: native file dialog, then moduleStage {"id","path"}.
		Button(c, p, "选择替换"+id, x+w-324, y+34, 142, 42, "选择替换 DLL", "", !busy, !busy, func() {
			a.pickModuleDLL(id)
		})
		Button(c, p, "恢复上一版本"+id, x+w-166, y+34, 146, 42, "恢复上一版本", "", !busy, !busy, func() {
			a.command("moduleRollback", map[string]any{"id": id})
		})
		y += 128
	}
	return y
}
