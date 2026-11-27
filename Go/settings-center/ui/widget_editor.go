package ui

import (
	"dotisle/settings/island"
	"github.com/egoist/mygo/ui"
)

type widgetEditorState struct {
	id, kind, title, value, field, label, err string
	config                                    map[string]any
	limit                                     int
	saving                                    bool
}

func (a *App) openWidgetEditor(id string) {
	items, _ := a.State["desktopWidgets"].([]any)
	for _, raw := range items {
		item, ok := raw.(map[string]any)
		if !ok || stateString(item, "", "id") != id {
			continue
		}
		e := &widgetEditorState{id: id, kind: stateString(item, "", "kind"), config: map[string]any{}}
		if config, ok := item["config"].(map[string]any); ok {
			for key, value := range config {
				e.config[key] = value
			}
		}
		e.title = stateString(e.config, "", "title")
		switch e.kind {
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
		case "photoFrame":
			e.field, e.label, e.limit = "folder", "照片文件夹路径", 512
		case "quote":
			e.field, e.label, e.limit = "text", "自定义日签语录（留空使用内置语录）", 200
		case "appFolder":
			e.field, e.label, e.limit = "caption", "网格模式 · 输入 4（2×2）或 9（3×3）", 2
		}
		e.value = stateString(e.config, "", e.field)
		a.widgetEditor = e
		return
	}
	a.pushToast("组件已移除")
}

func (a *App) saveWidgetEditor(e *widgetEditorState) {
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
		reply, err := island.Request("widgetUpdate", map[string]any{"id": e.id, "config": config})
		a.applyAsync(func() {
			if a.widgetEditor != e {
				return
			}
			e.saving = false
			if err != nil {
				e.err = err.Error()
			} else if reply["success"] != true {
				e.err = stateString(reply, "保存失败，请重试", "message")
			} else {
				a.widgetEditor = nil
			}
		})
	})
}

func (a *App) widgetEditorView(c *ui.Context) {
	e, p := a.widgetEditor, &a.Palette
	s := &p.Scheme
	open := true
	ui.DialogBase(c, &open, func(backdrop, panel ui.Element) {
		backdrop.Background(AlphaCol(0, .4))
		panel.Width(560).Height(416)
		overlayStyle(&panel, p)
		panel.Children(func() {
			Label(c, "编辑"+kindLabel(e.kind), 28, 24, 20, s.OnSurface, true, 480)
			a.dialogField(c, p, 28, 78, 504, "标题", &e.title, 60)
			if e.field == "text" {
				field := ui.Box(c).Absolute().Left(28).Top(158).Width(504).Height(122)
				surfaceStyle(&field, p, s.Container, FieldRadius)
				Label(c, e.label, 42, 166, 12, s.OnSurfaceVariant, true, 476)
				ui.TextAreaBase(c, &e.value).Absolute().Left(42).Top(190).Width(476).Height(78).Font(UIFont).FontSize(14).TextColor(Col(s.OnSurface)).MaxLength(e.limit)
			} else if e.field != "" {
				a.dialogField(c, p, 28, 158, 504, e.label, &e.value, e.limit)
			}
			if e.err != "" {
				Label(c, e.err, 28, 298, 12, s.Error, false, 504)
			}
			Button(c, p, "取消编辑组件", 316, 348, 96, 40, "取消", "", true, !e.saving, func() { open = false })
			Button(c, p, "保存桌面组件", 424, 348, 108, 40, "保存", "", false, !e.saving && a.Connected, func() { a.saveWidgetEditor(e) })
		})
	})
	if !open && !e.saving {
		a.widgetEditor = nil
	}
}
