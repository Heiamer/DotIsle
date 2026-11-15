package ui

import (
	"dotisle/settings/prefs"
	"github.com/egoist/mygo/ui"
	"strings"
)

func (a *App) plugins() []map[string]any {
	out := []map[string]any{}
	raw, _ := a.State["plugins"].([]any)
	for _, value := range raw {
		if p, ok := value.(map[string]any); ok {
			out = append(out, p)
		}
	}
	return out
}
func pluginBlocks(p map[string]any, surface string) []any {
	views, _ := p["views"].(map[string]any)
	blocks, _ := views[surface].([]any)
	return blocks
}
func (a *App) pluginByID(id string) map[string]any {
	for _, p := range a.plugins() {
		if p["id"] == id {
			return p
		}
	}
	return nil
}
func (a *App) pluginCardIDs() []string {
	out := []string{}
	for _, p := range a.plugins() {
		key := "plugin:" + stateString(p, "", "id")
		if p["enabled"] == true && stateString(p, "", "error") == "" && len(pluginBlocks(p, "overview")) > 0 && prefs.ValidPluginKey(key) {
			out = append(out, key)
		}
	}
	return out
}
func (a *App) addPluginCard(id string) {
	a.loadOverview()
	key := "plugin:" + id
	if !prefs.ValidPluginKey(key) {
		return
	}
	for _, k := range a.overview.config.Cards {
		if k == key {
			a.pushToast("已经添加到概览")
			return
		}
	}
	a.overview.config.Cards = append(a.overview.config.Cards, key)
	a.saveOverview()
	a.pushToast("已添加到概览")
}
func (a *App) pluginsPage(c *ui.Context, x, y, w float32) float32 {
	p, s := &a.Palette, &a.Palette.Scheme
	Label(c, "插件积木", x, y, 24, s.OnSurface, true, w-170)
	Button(c, p, "安装插件", x+w-150, y, 150, 38, "添加本地插件", "add", true, a.Connected, func() {
		if a.Win != nil {
			folder := openFolderDialog(a.Win.NativeHandle(), "选择包含 manifest.json 的插件文件夹")
			if folder != "" {
				a.command("pluginInstall", map[string]any{"path": folder})
			}
		}
	})
	Label(c, "将自己的内容放到灵动岛、概览和桌面。", x, y+44, 12, s.OnSurfaceVariant, false, w)
	Label(c, "选择插件文件夹即可连接；修改清单与数据后自动刷新。", x, y+68, 11, s.OnSurfaceVariant, false, w)
	y += 112
	if err := stateString(a.State, "", "pluginError"); err != "" {
		Label(c, err, x, y, 12, s.Error, false, w)
		y += 40
	}
	entries := a.plugins()
	if len(entries) == 0 {
		appleWidgetSurface(c, p, overviewRect{"", x, y, w, 176})
		Label(c, "从第一块积木开始", x+24, y+30, 20, s.OnSurface, true, w-48)
		Label(c, "开发文档：docs/PLUGIN-DEVELOPMENT.md", x+24, y+76, 12, s.OnSurfaceVariant, false, w-48)
		Label(c, "内附示例：examples/plugins/hello", x+24, y+106, 12, s.OnSurfaceVariant, false, w-48)
		return y + 200
	}
	for _, entry := range entries {
		id := stateString(entry, "", "id")
		name := stateString(entry, id, "name")
		enabled := entry["enabled"] == true
		err := stateString(entry, "", "error")
		if y+194 < a.scrollY() || y > a.scrollY()+float32(clipH) {
			y += 214
			continue
		}
		appleWidgetSurface(c, p, overviewRect{"", x, y, w, 194})
		Label(c, name, x+22, y+20, 18, s.OnSurface, true, w-250)
		Label(c, id+" · "+stateString(entry, "", "version"), x+22, y+50, 11, s.OnSurfaceVariant, false, w-250)
		detail := stateString(entry, "", "description")
		color := s.OnSurfaceVariant
		if err != "" {
			detail = err
			color = s.Error
		}
		Label(c, detail, x+22, y+82, 12, color, false, w-44)
		Button(c, p, "启停插件 "+id, x+w-208, y+18, 86, 34, pluginChoice(enabled, "停用", "启用"), "", false, a.Connected, func() { a.command("pluginEnable", map[string]any{"id": id, "enabled": !enabled}) })
		Button(c, p, "移除插件 "+id, x+w-110, y+18, 86, 34, "移除", "delete", false, a.Connected, func() { a.command("pluginRemove", map[string]any{"id": id}) })
		bx := x + 22
		if len(pluginBlocks(entry, "island")) > 0 {
			Label(c, pluginChoice(enabled, "灵动岛 · 已接入", "灵动岛 · 已停用"), bx, y+136, 12, s.Primary, false, 170)
			bx += 174
		}
		if len(pluginBlocks(entry, "overview")) > 0 {
			Button(c, p, "添加插件到概览 "+id, bx, y+124, 136, 38, "添加到概览", "add", false, enabled && err == "", func() { a.addPluginCard(id) })
			bx += 148
		}
		if len(pluginBlocks(entry, "desktop")) > 0 {
			Button(c, p, "添加插件到桌面 "+id, bx, y+124, 136, 38, "添加到桌面", "add", false, a.Connected && enabled && err == "" && a.widgetCount() < 16, func() {
				a.command("widgetAdd", map[string]any{"kind": "plugin", "size": "large", "config": map[string]any{"pluginId": id, "title": name}})
			})
		}
		y += 214
	}
	return y + 12
}
func (a *App) pluginCard(c *ui.Context, r overviewRect) {
	id := strings.TrimPrefix(r.key, "plugin:")
	p := a.pluginByID(id)
	s := &a.Palette.Scheme
	title := stateString(p, id, "name")
	a.cardHeader(c, r, title)
	IconButton(c, &a.Palette, "管理插件 "+id, r.x+r.w-50, r.y+10, 32, 32, "more_horiz", false, true, func() { a.navigate(PagePlugins) })
	if p["enabled"] != true || stateString(p, "", "error") != "" {
		Label(c, "插件未启用或已移除", r.x+20, r.y+80, 13, s.OnSurfaceVariant, false, r.w-40)
		return
	}
	a.drawPluginBlocks(c, pluginBlocks(p, "overview"), r.x+20, r.y+58, r.w-40, r.h-74)
}
func (a *App) drawPluginBlocks(c *ui.Context, blocks []any, x, y, w, h float32) {
	s := &a.Palette.Scheme
	total := float32(0)
	blockHeight := func(kind string) float32 {
		switch kind {
		case "metric":
			return 56
		case "progress":
			return 40
		case "divider":
			return 12
		}
		return 28
	}
	for _, v := range blocks {
		b, _ := v.(map[string]any)
		total += blockHeight(stateString(b, "", "kind"))
	}
	scale := min(float32(1), h/max(float32(1), total))
	for _, v := range blocks {
		b, _ := v.(map[string]any)
		kind := stateString(b, "", "kind")
		hh := blockHeight(kind)
		switch kind {
		case "divider":
			ui.Box(c).Absolute().Left(x).Top(y + 5*scale).Width(w).Height(1).Background(Col(s.High)).IgnoreInput(true)
		case "progress":
			Label(c, stateString(b, "", "label"), x, y, max(float32(9), 11*scale), s.OnSurfaceVariant, false, w)
			value, _ := stateFloat(b, "value")
			progressLine(c, &a.Palette, x, y+25*scale, w, value)
		default:
			font := float32(13)
			if kind == "metric" {
				font = 26
			}
			Label(c, stateString(b, "—", "text"), x, y, max(float32(9), font*scale), s.OnSurface, kind == "metric", w)
			if kind == "metric" {
				Label(c, stateString(b, "", "label"), x, y+34*scale, max(float32(9), 11*scale), s.OnSurfaceVariant, false, w)
			}
		}
		y += hh * scale
	}
}

func pluginChoice(value bool, yes, no string) string {
	if value {
		return yes
	}
	return no
}
