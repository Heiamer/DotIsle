package ui

import (
	"strings"
	"time"
	"unicode/utf8"

	"dotisle/settings/island"
	"github.com/egoist/mygo/ui"
)

type cityPromptState struct {
	text, sent, error, location string
	saving, locating            bool
}

// Background widget startup must not ask for input until the center is shown.
func (a *App) BeginCenterVisit() {
	a.centerVisible = true
	if a.tutorialRequired {
		a.forceGestureTutorial()
	}
	a.cityPromptAsked = false
	if a.cityPrompt == nil || !a.cityPrompt.saving {
		a.cityPrompt = nil
	}
	a.Refresh()
}

func (a *App) EndCenterVisit() {
	a.centerVisible = false
	a.mailOpen = false
	a.mail.secret = ""
	a.mail.selected = 0
	a.mail.body = ""
	a.cancelWechatWarning()
	a.cancelGestureTutorial()
	a.signaturePending = false
	a.signatureStart = time.Time{}
	a.cancelOverviewGesture()
	if e := a.musicEditor; e != nil {
		e.Preview = nil
		e.PreviewKey = layoutPreviewKey{}
	}
}

func (a *App) configuredWeatherCity() string {
	if city := strings.TrimSpace(stateString(a.settings(), "", "WeatherCity")); city != "" {
		return city
	}
	for _, key := range []string{"overviewWidgets", "desktopWidgets"} {
		items, _ := a.State[key].([]any)
		for _, item := range items {
			widget, _ := item.(map[string]any)
			if stateString(widget, "", "kind") != "weather" {
				continue
			}
			config, _ := widget["config"].(map[string]any)
			if city := strings.TrimSpace(stateString(config, "", "city")); city != "" {
				return city
			}
		}
	}
	return ""
}

func (a *App) maybePromptCity() {
	if !a.EnableCityPrompt || !a.centerVisible || !a.Connected || a.cityPromptAsked || a.cityPrompt != nil || a.mailOpen {
		return
	}
	if _, ready := a.State["settings"].(map[string]any); !ready {
		return
	}
	if a.configuredWeatherCity() != "" || a.signaturePending || !a.signatureStart.IsZero() {
		return
	}
	if a.picker != nil || a.showRuleDialog || a.showQuickReminder || a.widgetEditor != nil || a.overviewEditor != nil || a.todoDialog || a.fontPicker != nil || a.gestureTutorial != nil || a.paletteOpen || a.navigationOpen || a.overview.manage {
		return
	}
	a.cityPromptAsked = true
	a.cityPrompt = &cityPromptState{text: a.configuredWeatherCity()}
}

func (a *App) applyCitySaveReply(draft *cityPromptState, reply map[string]any, err error) {
	if a.cityPrompt != draft {
		return
	}
	draft.saving = false
	if err != nil {
		draft.error = "保存失败：" + err.Error()
		a.Refresh()
		return
	}
	if reply["success"] != true {
		draft.error = stateString(reply, "城市未保存，请重试。", "message")
		a.Refresh()
		return
	}
	// Update the visible snapshot only after the backend confirms persistence.
	if a.State == nil {
		a.State = map[string]any{}
	}
	settings, _ := a.State["settings"].(map[string]any)
	if settings == nil {
		settings = map[string]any{}
		a.State["settings"] = settings
	}
	settings["WeatherCity"] = draft.sent
	a.cityPrompt = nil
	a.pushToast("城市已保存")
}

func (a *App) saveCityPrompt() {
	draft := a.cityPrompt
	if draft == nil || draft.saving {
		return
	}
	city := strings.TrimSpace(draft.text)
	if city == "" {
		draft.error = "请填写城市名称，例如上海或北京。"
		return
	}
	if utf8.RuneCountInString(city) > 64 {
		draft.error = "城市名称请控制在 64 个字以内。"
		return
	}
	if a.Win == nil || !a.Connected {
		draft.error = "暂未连接到点屿，请连接后重试。"
		return
	}
	draft.saving, draft.sent, draft.error = true, city, ""
	a.startAsync(func() {
		reply, err := island.Request("update", map[string]any{"key": "WeatherCity", "value": city})
		a.applyAsync(func() { a.applyCitySaveReply(draft, reply, err) })
	})
}

func (a *App) cityPromptView(c *ui.Context) {
	draft, p := a.cityPrompt, &a.Palette
	s := &p.Scheme
	open := true
	dw, dh := float32(560), float32(387)
	ui.DialogBase(c, &open, func(backdrop, panel ui.Element) {
		backdrop.Background(AlphaCol(0, .42))
		panel.Width(dw).Height(dh)
		setupDialogStyle(&panel, p)
		panel.Children(func() {
			Label(c, "设置所在城市", 28, 24, 20, s.OnSurface, true, dw-110)
			IconButton(c, p, "稍后填写城市", dw-68, 17, 44, 44, "close", false, !draft.saving, func() { open = false })
			Label(c, "填写城市后，查看当地天气和降水提醒。", 28, 69, 13, s.OnSurfaceVariant, false, dw-56)
			const inputTop, locationGap float32 = 104, 18
			const locationTop = inputTop + dialogFieldHeight + locationGap
			if draft.saving {
				Label(c, draft.sent, 42, 126, 16, s.OnSurface, true, dw-84)
			} else {
				a.dialogField(c, p, 28, inputTop, dw-56, "城市 · 例如 上海 / Beijing", &draft.text, 64)
			}
			locationLabel := "定位当前城市"
			if draft.locating {
				locationLabel = "正在定位…"
			}
			Button(c, p, "定位当前城市", 28, locationTop, 150, 38, locationLabel, "", true, !draft.locating && !draft.saving, func() { a.requestWeatherLocation(draft) })
			Label(c, "按网络 IP 估算（wttr.in），不上传 GPS。", 192, locationTop+10, 12, s.OnSurfaceVariant, false, dw-220)
			if draft.location != "" {
				Label(c, "定位结果："+draft.location+" · 请确认或直接修改", 28, locationTop+50, 12, s.OnSurfaceVariant, false, dw-56)
			}
			if draft.error != "" {
				Label(c, draft.error, 28, locationTop+77, 12, s.Error, false, dw-56)
			}
			Button(c, p, "稍后填写", 28, dh-65, 122, 40, "稍后填写", "", true, !draft.saving, func() { open = false })
			label := "保存城市"
			if draft.saving {
				label = "正在保存…"
			}
			Button(c, p, "保存城市", dw-160, dh-65, 132, 40, label, "", false, !draft.saving && a.Connected, a.saveCityPrompt)
		})
	})
	if !open && !draft.saving {
		a.cityPrompt = nil
	}
}
