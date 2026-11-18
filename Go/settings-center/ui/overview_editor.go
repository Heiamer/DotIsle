package ui

import (
	"strings"

	"dotisle/settings/island"
	"github.com/egoist/mygo/ui"
	"golang.org/x/sys/windows"
)

// overviewEditor edits one overview tile without touching desktop widgets. The
// island owns the content store; this dialog only drafts it.
type overviewEditorState struct {
	kind, title, value, field, label, err string
	config                                map[string]any
	limit                                 int
	saving                                bool
}

var weatherCities = []string{"北京", "上海", "广州", "深圳", "杭州", "成都"}
var worldCities = []string{"北京", "东京", "伦敦", "纽约", "悉尼", "UTC"}

// overviewTileConfig returns the saved tile config, or the legacy desktop widget
// config so an existing user starts from what the card already displays.
func (a *App) overviewTileConfig(kind string) map[string]any {
	if items, ok := a.State["overviewWidgets"].([]any); ok {
		for _, raw := range items {
			item, ok := raw.(map[string]any)
			if !ok || stateString(item, "", "kind") != kind {
				continue
			}
			config, _ := item["config"].(map[string]any)
			return config
		}
	}
	if items, ok := a.State["desktopWidgets"].([]any); ok {
		for _, raw := range items {
			item, ok := raw.(map[string]any)
			if !ok || stateString(item, "", "kind") != kind {
				continue
			}
			config, _ := item["config"].(map[string]any)
			return config
		}
	}
	return nil
}

func (a *App) openOverviewEditor(kind string) {
	if !overviewConfigurable(kind) || a.overviewEditor != nil {
		return
	}
	e := &overviewEditorState{kind: kind, config: map[string]any{}}
	if config := a.overviewTileConfig(kind); config != nil {
		for key, value := range config {
			e.config[key] = value
		}
	}
	e.title = stateString(e.config, "", "title")
	switch kind {
	case "note":
		e.field, e.label, e.limit = "text", "便签内容", 500
	case "anniversary":
		e.field, e.label, e.limit = "date", "日期 · YYYY-MM-DD", 10
	case "schedule":
		e.field, e.label, e.limit = "deadline", "日期与时间 · YYYY-MM-DD HH:mm", 16
	case "weather":
		e.field, e.label, e.limit = "city", "城市 · 可附省份或国家", 100
	case "worldClock":
		e.field, e.label, e.limit = "city", "城市 · 北京 / 东京 / 伦敦 / 纽约 / 悉尼 / UTC", 20
	case "storage":
		e.field, e.label, e.limit = "drive", "盘符 · 如 C: 或 D:", 3
	case "launcher":
		e.field, e.label, e.limit = "target", "程序、文件夹路径或 HTTPS 网页", 2048
	}
	e.value = stateString(e.config, "", e.field)
	a.overviewEditor = e
}

func (a *App) saveOverviewEditor(e *overviewEditorState) {
	if a.Win == nil || e.saving {
		return
	}
	e.saving = true
	config := map[string]any{}
	for key, value := range e.config {
		config[key] = value
	}
	config["title"] = e.title
	if e.field != "" {
		config[e.field] = e.value
	}
	a.startAsync(func() {
		reply, err := island.Request("overviewWidgetSet", map[string]any{"kind": e.kind, "config": config})
		a.applyAsync(func() {
			if a.overviewEditor != e {
				return
			}
			e.saving = false
			if err != nil {
				e.err = err.Error()
			} else if reply["success"] != true {
				e.err = stateString(reply, "保存失败，请重试", "message")
			} else {
				a.overviewEditor = nil
			}
		})
	})
}

// pickLauncherTarget fills the shortcut tile from the native file dialog.
func (a *App) pickLauncherTarget(e *overviewEditorState) {
	if a.Win == nil {
		return
	}
	path := openFileDialog(a.Win.NativeHandle(), "选择程序或快捷方式", "程序与快捷方式\x00*.exe;*.lnk\x00所有文件\x00*.*\x00\x00", "")
	if path != "" {
		e.value = path
	}
}

func availableDrives() []string {
	mask, err := windows.GetLogicalDrives()
	if err != nil {
		return nil
	}
	out := []string{}
	for i := 0; i < 26; i++ {
		if mask&(1<<uint(i)) != 0 {
			out = append(out, string(rune('A'+i))+":")
		}
	}
	return out
}

func (a *App) overviewEditorView(c *ui.Context) {
	e, p := a.overviewEditor, &a.Palette
	s := &p.Scheme
	chips := []string(nil)
	switch e.kind {
	case "weather":
		chips = weatherCities
	case "worldClock":
		chips = worldCities
	case "storage":
		chips = availableDrives()
	}
	height := float32(416)
	if len(chips) > 0 || e.kind == "launcher" {
		height = 460
	}
	open := true
	ui.DialogBase(c, &open, func(backdrop, panel ui.Element) {
		backdrop.Background(AlphaCol(0, .4))
		panel.Width(560).Height(height)
		overlayStyle(&panel, p)
		panel.Children(func() {
			Label(c, "设置"+kindLabel(e.kind)+"组件", 28, 24, 20, s.OnSurface, true, 480)
			a.dialogField(c, p, 28, 78, 504, "标题（可选）", &e.title, 60)
			if e.field == "text" {
				field := ui.Box(c).Absolute().Left(28).Top(158).Width(504).Height(122)
				surfaceStyle(&field, p, s.Container, FieldRadius)
				Label(c, e.label, 42, 166, 12, s.OnSurfaceVariant, true, 476)
				ui.TextAreaBase(c, &e.value).Absolute().Left(42).Top(190).Width(476).Height(78).Font(UIFont).FontSize(14).TextColor(Col(s.OnSurface)).MaxLength(e.limit)
			} else if e.field != "" {
				a.dialogField(c, p, 28, 158, 504, e.label, &e.value, e.limit)
			}
			if len(chips) > 0 {
				cw := (504 - float32(len(chips)-1)*8) / float32(len(chips))
				for i, chip := range chips {
					value := chip
					Chip(c, p, "选择"+chip, 28+float32(i)*(cw+8), 234, cw, 36, chip, strings.EqualFold(e.value, chip), func() { e.value = value })
				}
			}
			if e.kind == "launcher" {
				Button(c, p, "选择快捷启动目标", 28, 234, 160, 36, "选择程序", "folder_open", true, true, func() { a.pickLauncherTarget(e) })
			}
			if e.err != "" {
				Label(c, e.err, 28, height-82, 12, s.Error, false, 504)
			}
			Button(c, p, "取消设置组件", 316, height-56, 96, 40, "取消", "", true, !e.saving, func() { open = false })
			Button(c, p, "保存组件设置", 424, height-56, 108, 40, "保存", "", false, !e.saving && a.Connected, func() { a.saveOverviewEditor(e) })
		})
	})
	if !open && !e.saving {
		a.overviewEditor = nil
	}
}
