package ui

import (
	"fmt"
	"strconv"
	"strings"
	"time"

	"github.com/egoist/mygo/ui"
)

type quotaFields struct {
	five, week, savedFive, savedWeek string
	row, metric                      int
}

func quotaPercent(value string) (int, bool) {
	value = strings.TrimSpace(value)
	for _, c := range value {
		if c < '0' || c > '9' {
			return 0, false
		}
	}
	n, err := strconv.Atoi(value)
	return n, err == nil && n >= 0 && n <= 100
}

func quotaFormat(row map[string]any, key string) string {
	if v, ok := stateFloat(row, key); ok && 0 <= v && v <= 100 {
		return fmt.Sprintf("%.0f%%", v)
	}
	return "—"
}

func quotaRow(state map[string]any, index int) (map[string]any, int, int) {
	rows, _ := state["readings"].([]any)
	if len(rows) == 0 {
		return nil, 0, 0
	}
	index = min(max(index, 0), len(rows)-1)
	row, _ := rows[index].(map[string]any)
	return row, index, len(rows)
}

func quotaStatus(state, settings map[string]any) string {
	if state["nativeSources"] != true {
		return "额度监控未就绪"
	}
	if settings["enabled"] != true {
		return "监控未启用"
	}
	if state["pending"] == true {
		return "正在读取…"
	}
	status := stateString(state, "", "status")
	if status != "" && status != "就绪" {
		return status
	}
	return "暂无已启用的额度来源"
}

func (a *App) quotaPluginRow(c *ui.Context, x, y, w float32) float32 {
	const height = 452
	if y+height < a.scrollY() || y > a.scrollY()+float32(clipH) {
		return y + height + 12
	}
	p, s := &a.Palette, &a.Palette.Scheme
	state, supported := a.State["aiQuota"].(map[string]any)
	settings, _ := state["settings"].(map[string]any)
	five, week := 20, 10
	if n, ok := stateFloat(settings, "fiveHour"); ok {
		five = int(n)
	}
	if n, ok := stateFloat(settings, "weekly"); ok {
		week = int(n)
	}
	f := &a.quotaFields
	if value := strconv.Itoa(five); f.savedFive != value {
		f.five, f.savedFive = value, value
	}
	if value := strconv.Itoa(week); f.savedWeek != value {
		f.week, f.savedWeek = value, value
	}
	enabled := settings["enabled"] == true
	available := a.Connected && supported && state["nativeSources"] == true
	currency := "USD"
	if unit, ok := settings["relayCurrency"].(string); ok && (unit == "CNY" || unit == "POINTS") {
		currency = unit
	}
	send := func(on bool, five, week int, unit string) {
		a.command("aiQuotaConfigure", map[string]any{"enabled": on, "fiveHour": five, "weekly": week, "relayCurrency": unit})
	}
	appleWidgetSurface(c, p, overviewRect{"", x, y, w, height})
	Label(c, "AI 额度", x+22, y+17, 20, s.OnSurface, true, w-245)
	toggle := pluginChoice(enabled, "停用", "启用")
	Button(c, p, "配置额度来源", x+w-209, y+14, 104, 34, "配置来源", "", false, available, func() { a.quotaSources.open = true; a.Refresh() })
	Button(c, p, toggle+"AI 额度监控", x+w-94, y+14, 72, 34, toggle, "", enabled, available, func() { send(!enabled, five, week, currency) })
	Label(c, "按周期或余额提醒 · 未配置的来源不读取", x+22, y+53, 12, s.OnSurfaceVariant, false, w-44)
	layout := []struct {
		name   string
		value  *string
		offset float32
	}{{"5 小时低于（%）", &f.five, 0}, {"每周低于（%）", &f.week, 113}}
	for _, field := range layout {
		fx := x + 22 + field.offset
		Label(c, field.name, fx, y+84, 12, s.OnSurfaceVariant, false, 113)
		input := ui.TextInput(c, field.value).Absolute().Left(fx).Top(y + 104).Width(100).Height(36).Label(field.name)
		fieldStyle(&input, p)
		input.Disabled(!available)
	}
	fiveNext, fiveOK := quotaPercent(f.five)
	weekNext, weekOK := quotaPercent(f.week)
	Button(c, p, "保存额度阈值", x+248, y+104, 72, 36, "保存", "", true, available && fiveOK && weekOK && (five != fiveNext || week != weekNext), func() { send(enabled, fiveNext, weekNext, currency) })
	Button(c, p, "刷新额度数据", x+w-94, y+104, 72, 36, "刷新", "", false, available && enabled && state["pending"] != true, func() { a.command("aiQuotaRefresh", nil) })
	Label(c, "中转站低余额提醒", x+22, y+161, 13, s.OnSurface, true, 172)
	Button(c, p, "美元余额提醒", x+22, y+184, 98, 34, "美元 < US$1", "", currency != "USD", available, func() {
		if currency != "USD" {
			send(enabled, five, week, "USD")
		}
	})
	Button(c, p, "人民币余额提醒", x+129, y+184, 98, 34, "人民币 < ¥1", "", currency != "CNY", available, func() {
		if currency != "CNY" {
			send(enabled, five, week, "CNY")
		}
	})
	Button(c, p, "积分余额提醒", x+236, y+184, 98, 34, "积分 < 1", "", currency != "POINTS", available, func() {
		if currency != "POINTS" {
			send(enabled, five, week, "POINTS")
		}
	})
	Label(c, "只提醒所选单位，不换算 TOKEN。", x+22, y+222, 12, s.OnSurfaceVariant, false, w-44)
	row, index, count := quotaRow(state, f.row)
	if row == nil {
		Label(c, quotaStatus(state, settings), x+22, y+247, 15, s.OnSurface, true, w-44)
		Label(c, "可添加 DeepSeek Harness、New API、Sub2API 等已转译来源。", x+22, y+278, 12, s.OnSurfaceVariant, false, w-44)
	} else {
		Label(c, stateString(row, "额度来源", "name"), x+22, y+242, 17, s.OnSurface, true, w-174)
		Label(c, strings.Trim(strings.Join([]string{stateString(row, "", "status"), stateString(row, "", "plan")}, " · "), " ·"), x+22, y+273, 12, s.OnSurfaceVariant, false, w-44)
		Label(c, fmt.Sprintf("%d / %d", index+1, count), x+w-139, y+248, 12, s.OnSurfaceVariant, false, 57)
		IconButton(c, p, "上一个额度来源", x+w-83, y+237, 28, 28, "arrow_back", false, index > 0, func() { f.row = index - 1; f.metric = 0 })
		IconButton(c, p, "下一个额度来源", x+w-50, y+237, 28, 28, "arrow_forward", false, index+1 < count, func() { f.row = index + 1; f.metric = 0 })
		metrics := quotaDetailMetrics(row)
		at := min(max(f.metric, 0), max(len(metrics)-1, 0)/3*3)
		for i := at; i < min(at+3, len(metrics)); i++ {
			metric, _ := metrics[i].(map[string]any)
			label := stateString(metric, "额度", "label")
			if epoch, ok := stateFloat(metric, "resetAt"); ok {
				label += " · " + time.Unix(int64(epoch), 0).Format("01/02 15:04") + " 重置"
			}
			line := y + 308 + float32(i-at)*31
			Label(c, label, x+22, line, 12, s.OnSurfaceVariant, false, w*.55-28)
			Label(c, stateString(metric, "—", "value"), x+w*.56, line, 13, s.OnSurface, true, w*.44-22)
		}
		if len(metrics) == 0 {
			Label(c, "该渠道还没有可显示的额度。", x+22, y+311, 12, s.OnSurfaceVariant, false, w-44)
		} else if len(metrics) > 3 {
			IconButton(c, p, "上一组额度", x+w-83, y+396, 28, 28, "arrow_back", false, at > 0, func() { f.metric = at - 3 })
			IconButton(c, p, "下一组额度", x+w-50, y+396, 28, 28, "arrow_forward", false, at+3 < len(metrics), func() { f.metric = at + 3 })
		}
	}
	Button(c, p, "将 AI 额度卡添加到概览", x+22, y+406, 132, 32, "添加到概览", "add", false, true, a.addQuotaOverviewCard)
	return y + height + 12
}

func (a *App) addQuotaOverviewCard() {
	a.loadOverview()
	for _, key := range a.overview.config.Cards {
		if key == "aiQuota" {
			a.pushToast("AI 额度已在概览中")
			return
		}
	}
	a.overview.config.Cards = append(a.overview.config.Cards, "aiQuota")
	a.saveOverview()
	a.pushToast("AI 额度已添加到概览")
}

func quotaHasWindows(row map[string]any) bool {
	return quotaFormat(row, "fiveHour") != "—" || quotaFormat(row, "weekly") != "—"
}

func quotaTokenMetric(raw any) bool {
	metric, _ := raw.(map[string]any)
	_, ok := metric["tokenQuota"].(map[string]any)
	return ok
}

func quotaDetailMetrics(row map[string]any) []any {
	metrics, _ := row["metrics"].([]any)
	if row["provider"] != "zcode" || !quotaHasWindows(row) {
		return metrics
	}
	visible := make([]any, 0, len(metrics))
	for _, raw := range metrics {
		if !quotaTokenMetric(raw) {
			visible = append(visible, raw)
		}
	}
	return visible
}

func quotaOverviewToken(row map[string]any) string {
	if row["provider"] != "zcode" || quotaHasWindows(row) {
		return ""
	}
	metrics, _ := row["metrics"].([]any)
	for _, raw := range metrics {
		if quotaTokenMetric(raw) {
			metric, _ := raw.(map[string]any)
			return stateString(metric, "", "value")
		}
	}
	return ""
}

func (a *App) dashboardQuota(c *ui.Context, r overviewRect) {
	p, s := &a.Palette, &a.Palette.Scheme
	a.cardHeader(c, r, "AI 额度")
	state, _ := a.State["aiQuota"].(map[string]any)
	settings, _ := state["settings"].(map[string]any)
	row, index, count := quotaRow(state, a.quotaFields.row)
	x, y, w := r.x+20, r.y, r.w-40
	if row == nil {
		Label(c, quotaStatus(state, settings), x, y+62, 15, s.OnSurface, true, w)
		Label(c, "设置额度来源，查看实时余额和用量。", x, y+92, 12, s.OnSurfaceVariant, false, w)
	} else {
		Label(c, stateString(row, "额度来源", "name"), x, y+58, 15, s.OnSurface, true, w-75)
		if count > 1 {
			IconButton(c, p, "概览：上一个额度来源", x+w-64, y+51, 27, 27, "arrow_back", false, index > 0, func() { a.quotaFields.row = index - 1; a.quotaFields.metric = 0 })
			IconButton(c, p, "概览：下一个额度来源", x+w-30, y+51, 27, 27, "arrow_forward", false, index+1 < count, func() { a.quotaFields.row = index + 1; a.quotaFields.metric = 0 })
		}
		Label(c, stateString(row, "等待更新", "status"), x, y+86, 12, s.OnSurfaceVariant, false, w)
		metrics, _ := row["metrics"].([]any)
		if quotaHasWindows(row) {
			for i, m := range []struct{ title, key string }{{"5 小时剩余", "fiveHour"}, {"本周剩余", "weekly"}} {
				left := x + float32(i)*(w/2+4)
				Label(c, m.title, left, y+123, 12, s.OnSurfaceVariant, false, w/2-4)
				Label(c, quotaFormat(row, m.key), left, y+149, 24, s.OnSurface, true, w/2-4)
			}
		} else if token := quotaOverviewToken(row); token != "" {
			Label(c, "剩余 TOKEN / 总量", x, y+123, 12, s.OnSurfaceVariant, false, w)
			Label(c, token, x, y+149, 16, s.OnSurface, true, w)
		} else if row["provider"] == "zcode" {
			Label(c, "当前额度", x, y+123, 12, s.OnSurfaceVariant, false, w)
			Label(c, "—", x, y+149, 21, s.OnSurface, true, w)
		} else if len(metrics) > 0 {
			metric, _ := metrics[0].(map[string]any)
			Label(c, stateString(metric, "当前额度", "label"), x, y+123, 12, s.OnSurfaceVariant, false, w)
			Label(c, stateString(metric, "—", "value"), x, y+149, 21, s.OnSurface, true, w)
		}
	}
	Button(c, p, "打开 AI 额度插件设置", x, y+r.h-51, min(138, w), 32, "额度设置", "", false, true, func() { a.navigate(PagePlugins) })
}
