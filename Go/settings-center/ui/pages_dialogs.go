// The QuickReminder (一句话快速提醒) and app Rule (新建/编辑应用提醒) dialogs,
// ported from the DialogKind::QuickReminder and DialogKind::Rule branches of
// views.rs fn dialog. The payloads follow model.rs (quickReminderPreview /
// quickReminderCommit / appRuleSave / appRuleAction) and the reply handling
// follows native_sync.rs apply_native_reply: success closes the dialog, the
// host message lands in the dialog error line.
//
// Visibility contract (dialogs_hook.go dialogViews): the caller sets
// a.showQuickReminder / a.showRuleDialog + a.ruleEditID and calls these
// builders only while the flag is up; this file clears the flag when the
// dialog closes (Esc, backdrop click or 取消).
package ui

import (
	"fmt"
	"strconv"
	"strings"

	"dotisle/settings/island"
	"github.com/egoist/mygo/ui"
)

// dialogUI holds the two dialog drafts (the same pattern as focusDialog /
// todoDialogDraft). The shown shadows detect the open transition so every
// open prefills fresh, like the Rust Action::QuickReminder / Action::EditRule.
var dialogUI struct {
	quickShown bool   // the dialog was rendered since the flag went up
	quickText  string // the one field, 300 runes (model.rs Field::new)
	quickSent  string // text the last quickReminderPreview was sent for
	quickSeq   uint64 // bumped per request; stale replies are dropped
	quickToken string // previewToken of the latest successful parse
	quickDate  string
	quickTime  string
	quickTitle string
	quickErr   string

	ruleShown    bool   // the dialog was rendered since the flag went up
	ruleInitID   string // editID the draft was last prefilled for
	ruleName     string
	rulePath     string
	ruleMsg      string
	ruleCool     string
	ruleThresh   string
	ruleDaily    bool // trigger chip: foregroundDaily vs enter
	ruleErr      string
	ruleRevision any // the edited rule's revision for appRuleAction
}

// openQuickReminder is Action::QuickReminder (model.rs): raise the flag; the
// field resets on the first rendered frame.
func (a *App) openQuickReminder() {
	a.showQuickReminder = true
}

// openRuleDialog is Action::EditRule(id) (model.rs): "" creates a rule, an id
// edits the matching a.State["appRules"] item.
func (a *App) openRuleDialog(id string) {
	a.ruleEditID = id
	a.showRuleDialog = true
}

// prefillRuleDraft is the Action::EditRule field setup (model.rs): the five
// fields and the trigger chip read from the appRules item, falling back to the
// Rust defaults (cooldown 30, threshold 60, trigger enter) when missing.
func (a *App) prefillRuleDraft(id string) {
	var rule map[string]any
	if items, ok := a.State["appRules"].([]any); ok {
		for _, v := range items {
			item, _ := v.(map[string]any)
			if item != nil && leafStr(item["id"], "") == id {
				rule = item
				break
			}
		}
	}
	dialogUI.ruleName = ""
	dialogUI.rulePath = ""
	dialogUI.ruleMsg = ""
	dialogUI.ruleCool = "30"
	dialogUI.ruleThresh = "60"
	dialogUI.ruleDaily = false
	dialogUI.ruleRevision = nil
	if rule == nil {
		return
	}
	dialogUI.ruleName = leafStr(rule["name"], "")
	dialogUI.rulePath = leafStr(rule["executablePath"], "")
	dialogUI.ruleMsg = leafStr(rule["message"], "")
	if v, ok := rule["cooldownMinutes"].(float64); ok {
		dialogUI.ruleCool = strconv.Itoa(int(v))
	}
	if v, ok := rule["thresholdMinutes"].(float64); ok {
		dialogUI.ruleThresh = strconv.Itoa(int(v))
	}
	dialogUI.ruleDaily = leafStr(rule["trigger"], "") == "foregroundDaily"
	dialogUI.ruleRevision = rule["revision"]
}

// parseRuleMinutes mirrors Rust d.fields[i].value.parse::<u32>().unwrap_or(0).
func parseRuleMinutes(s string) uint32 {
	if v, err := strconv.ParseUint(strings.TrimPrefix(s, "+"), 10, 32); err == nil {
		return uint32(v)
	}
	return 0
}

// quickReminderDialog renders views.rs dialog for DialogKind::QuickReminder:
// the 560x380 card with one field at y+88, the format hint at y+166, the parse
// line at y+206 and the 确认创建提醒 button at y+250, error at h-104, cancel
// and 解析时间 at h-70. The parse (quickReminderPreview) re-sends whenever the
// text changes so the preview follows typing; 解析时间 sends it again like
// Action::SaveModal does.
func (a *App) quickReminderDialog(c *ui.Context, w, h float32) {
	_ = w // DialogBase centers the 560x380 panel over the window
	_ = h
	// First frame after the flag went up: Action::QuickReminder opens with a
	// fresh field and no parse result.
	if !dialogUI.quickShown {
		dialogUI.quickShown = true
		dialogUI.quickText = ""
		dialogUI.quickSent = ""
		dialogUI.quickToken = ""
		dialogUI.quickDate = ""
		dialogUI.quickTime = ""
		dialogUI.quickTitle = ""
		dialogUI.quickErr = ""
		dialogUI.quickSeq++
	}
	// A text change clears the parse and the error like Model::input, then
	// requests a fresh quickReminderPreview for the new text.
	if dialogUI.quickText != dialogUI.quickSent {
		dialogUI.quickSent = dialogUI.quickText
		dialogUI.quickToken = ""
		dialogUI.quickErr = ""
		a.sendQuickPreview()
	}
	p := &a.Palette
	s := &p.Scheme
	const dw, dh = float32(560), float32(380)
	open := true
	ui.DialogBase(c, &open, func(backdrop, panel ui.Element) {
		backdrop.Background(AlphaCol(0, 0.5))
		panel.Width(dw).Height(dh)
		overlayStyle(&panel, p)
		panel.Children(func() {
			Label(c, "一句话快速提醒", 28, 24, 20, s.OnSurface, true, dw-110)
			IconButton(c, p, "取消", dw-68, 20, 44, 44, "close", false, true, func() {
				open = false
			})
			a.dialogField(c, p, 28, 88, dw-56, "例如：20分钟后取快递", &dialogUI.quickText, 300)
			Label(c, "支持几分钟后、今天 / 明天 HH:MM，以及指定日期。", 28, 166, 12,
				s.OnSurfaceVariant, false, dw-56)
			if dialogUI.quickToken != "" {
				Label(c, fmt.Sprintf("%s %s · %s", dialogUI.quickDate, dialogUI.quickTime,
					dialogUI.quickTitle), 28, 206, 17, s.Primary, true, dw-56)
				Button(c, p, "确认创建提醒", 28, 250, 196, 44, "确认创建提醒", "check", true, true,
					a.sendQuickCommit)
			}
			if dialogUI.quickErr != "" {
				Label(c, dialogUI.quickErr, 28, dh-104, 12, s.Error, true, dw-56)
			}
			Button(c, p, "取消快速提醒", dw-248, dh-70, 96, 44, "取消", "", true, true, func() {
				open = false
			})
			Button(c, p, "解析时间", dw-140, dh-70, 112, 44, "解析时间", "", false, true,
				a.sendQuickPreview)
		})
	})
	if !open {
		a.showQuickReminder = false
		dialogUI.quickShown = false
	}
}

// sendQuickPreview is Action::SaveModal DialogKind::QuickReminder (model.rs):
// quickReminderPreview {"text": ...}. The latest request wins; success shows
// the date/time/title line, failure the host message (native_sync.rs).
func (a *App) sendQuickPreview() {
	text := dialogUI.quickText
	dialogUI.quickSeq++
	seq := dialogUI.quickSeq
	a.startAsync(func() {
		reply, err := island.Request("quickReminderPreview", map[string]any{"text": text})
		a.applyAsync(func() {
			if !a.showQuickReminder || seq != dialogUI.quickSeq {
				a.Refresh()
				return
			}
			dialogUI.quickToken = ""
			switch {
			case err != nil:
				dialogUI.quickErr = err.Error()
			case reply["success"] == true:
				dialogUI.quickToken, _ = reply["previewToken"].(string)
				dialogUI.quickDate, _ = reply["date"].(string)
				dialogUI.quickTime, _ = reply["time"].(string)
				dialogUI.quickTitle, _ = reply["title"].(string)
				dialogUI.quickErr = ""
			default:
				dialogUI.quickErr = leafStr(reply["message"], "操作失败")
			}
			a.Refresh()
		})
	})
}

// sendQuickCommit is Action::QuickCommit (model.rs): quickReminderCommit with
// the previewToken and the "vello-" clientRequestId; success closes the dialog
// and raises the 提醒已创建 toast (native_sync.rs apply_native_reply).
func (a *App) sendQuickCommit() {
	token := dialogUI.quickToken
	if token == "" {
		return
	}
	a.startAsync(func() {
		reply, err := island.Request("quickReminderCommit", map[string]any{
			"previewToken":    token,
			"clientRequestId": "vello-" + token,
		})
		a.applyAsync(func() {
			switch {
			case err != nil:
				dialogUI.quickErr = err.Error()
			case reply["success"] == false:
				dialogUI.quickErr = leafStr(reply["message"], "操作失败")
			default:
				a.showQuickReminder = false
				dialogUI.quickShown = false
				dialogUI.quickToken = ""
				dialogUI.quickErr = ""
				a.pushToast("提醒已创建")
			}
			a.Refresh()
		})
	})
}

// ruleDialog renders views.rs dialog for DialogKind::Rule: the 560x620 card
// with five fields on the 74px pitch, the trigger chip at y+470, the current
// mode line at y+480, error at h-104 and the cancel/save pair at h-70. editID
// is the rule being edited ("" = new rule); the draft prefills on every open
// and whenever the edit target changes.
func (a *App) ruleDialog(c *ui.Context, w, h float32, editID string) {
	_ = w // DialogBase centers the 560x620 panel over the window
	_ = h
	// First frame after the flag went up (Action::EditRule) or a new edit
	// target: prefill the five fields from the appRules item.
	if !dialogUI.ruleShown || dialogUI.ruleInitID != editID {
		dialogUI.ruleShown = true
		dialogUI.ruleInitID = editID
		dialogUI.ruleErr = ""
		a.prefillRuleDraft(editID)
	}
	p := &a.Palette
	s := &p.Scheme
	const dw, dh = float32(560), float32(620)
	open := true
	ui.DialogBase(c, &open, func(backdrop, panel ui.Element) {
		backdrop.Background(AlphaCol(0, 0.5))
		panel.Width(dw).Height(dh)
		overlayStyle(&panel, p)
		panel.Children(func() {
			title := "新建应用提醒"
			if editID != "" {
				title = "编辑应用提醒"
			}
			Label(c, title, 28, 24, 20, s.OnSurface, true, dw-110)
			IconButton(c, p, "取消", dw-68, 20, 44, 44, "close", false, true, func() {
				open = false
			})
			labels := [5]string{
				"规则名称",
				"程序完整路径 · .exe",
				"提醒内容",
				"进入提醒冷却 · 5 / 15 / 30 / 60 分钟",
				"前台累计阈值 · 10–240 分钟",
			}
			values := [5]*string{&dialogUI.ruleName, &dialogUI.rulePath,
				&dialogUI.ruleMsg, &dialogUI.ruleCool, &dialogUI.ruleThresh}
			maxs := [5]int{60, 260, 200, 3, 3}
			for i := 0; i < 5; i++ {
				a.dialogField(c, p, 28, 88+float32(i)*74, dw-56, labels[i], values[i], maxs[i])
			}
			Chip(c, p, "按前台累计时长触发", 28, 470, 232, 38, "按前台累计时长触发",
				dialogUI.ruleDaily, func() {
					dialogUI.ruleDaily = !dialogUI.ruleDaily
				})
			way := "当前方式：进入应用时提醒"
			if dialogUI.ruleDaily {
				way = "当前方式：每天累计前台使用时长"
			}
			Label(c, way, 280, 480, 12, s.OnSurfaceVariant, false, dw-308)
			if dialogUI.ruleErr != "" {
				Label(c, dialogUI.ruleErr, 28, dh-104, 12, s.Error, true, dw-56)
			}
			if editID != "" {
				// The delete entry: appRuleAction "delete" (lyric-native main.rs
				// handles disable/enable/delete with the revision guard).
				Button(c, p, "删除应用提醒", 28, dh-70, 96, 44, "删除", "", true, true,
					a.sendRuleDelete)
			}
			Button(c, p, "取消应用提醒", dw-248, dh-70, 96, 44, "取消", "", true, true, func() {
				open = false
			})
			Button(c, p, "保存应用提醒", dw-140, dh-70, 112, 44, "保存", "", false, true,
				a.sendRuleSave)
		})
	})
	if !open {
		a.showRuleDialog = false
		dialogUI.ruleShown = false
	}
}

// sendRuleSave is Action::SaveModal DialogKind::Rule (model.rs): the appRuleSave
// payload with the u32 parses (unwrap_or 0) and the chip-driven trigger.
// Success closes the dialog; failure shows the host message in the dialog.
func (a *App) sendRuleSave() {
	trigger := "enter"
	if dialogUI.ruleDaily {
		trigger = "foregroundDaily"
	}
	payload := map[string]any{
		"id":               a.ruleEditID,
		"name":             dialogUI.ruleName,
		"executablePath":   dialogUI.rulePath,
		"message":          dialogUI.ruleMsg,
		"cooldownMinutes":  parseRuleMinutes(dialogUI.ruleCool),
		"thresholdMinutes": parseRuleMinutes(dialogUI.ruleThresh),
		"trigger":          trigger,
		"enabled":          true,
	}
	a.startAsync(func() {
		reply, err := island.Request("appRuleSave", payload)
		a.applyAsync(func() {
			switch {
			case err != nil:
				dialogUI.ruleErr = err.Error()
			case reply["success"] == false:
				dialogUI.ruleErr = leafStr(reply["message"], "操作失败")
			default:
				// native_sync.rs: appRuleSave success closes the dialog without
				// a toast; the refreshed rules list comes with the snapshot.
				a.showRuleDialog = false
				dialogUI.ruleShown = false
			}
			a.Refresh()
		})
	})
}

// sendRuleDelete is the delete entry: appRuleAction {"id", "action": "delete",
// "revision"} as in preferences_ui.rs, with the revision guard of the host.
// Success closes the dialog because the rule is gone.
func (a *App) sendRuleDelete() {
	if a.ruleEditID == "" {
		return
	}
	payload := map[string]any{
		"id":       a.ruleEditID,
		"action":   "delete",
		"revision": dialogUI.ruleRevision,
	}
	a.startAsync(func() {
		reply, err := island.Request("appRuleAction", payload)
		a.applyAsync(func() {
			switch {
			case err != nil:
				dialogUI.ruleErr = err.Error()
			case reply["success"] == false:
				dialogUI.ruleErr = leafStr(reply["message"], "操作失败")
			default:
				a.showRuleDialog = false
				dialogUI.ruleShown = false
			}
			a.Refresh()
		})
	})
}
