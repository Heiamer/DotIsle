package ui

import (
	"fmt"

	"dotisle/settings/island"
	"github.com/egoist/mygo"
)

// ReportError displays an error from an embedded native module on the UI thread.
func (a *App) ReportError(message string) { a.pushToast(message) }

// command returns pipe and application failures to the main thread. An accepted
// moduleApply must close this process so the updater can release its loaded DLLs.
func (a *App) command(name string, value any) {
	if a.onCommand != nil {
		a.onCommand(name, value)
		return
	}
	if a.Win == nil {
		return
	}
	a.startAsync(func() {
		reply, err := island.Request(name, value)
		a.applyAsync(func() {
			if a.applyCommandReply(name, reply, err) {
				mygo.App.Quit()
			}
		})
	})
}

// applyCommandReply reports a failure and returns whether this center must exit.
// A missing success field is not an acknowledgement of a mutation.
func (a *App) applyCommandReply(name string, reply map[string]any, err error) bool {
	if name == "updateReview" {
		a.updateReviewLoading = false
	}
	if name == "updateDownload" {
		a.updateDownloading = false
	}
	if err != nil {
		a.pushToast("请求失败：" + err.Error())
		return false
	}
	if reply["success"] != true {
		message := stateString(reply, "操作未成功，请重试。", "message")
		a.pushToast(fmt.Sprintf("操作失败：%s", message))
		return false
	}
	switch name {
	case "aiQuotaSourceTrigger":
		a.pushToast(stateString(reply, "余额检测方式已保存", "message"))
	case "updateReview":
		version, channel := stateString(reply, "", "version"), stateString(reply, "", "channel")
		if version == "" || (channel != "stable" && channel != "beta") {
			a.pushToast("更新信息不完整，请重新检查。")
			return false
		}
		a.updateNotes = &updateNotes{version: version, channel: channel, source: stateString(reply, "", "source"),
			paragraphs: notesParagraphs(stateString(reply, "", "releaseNotes"))}
		a.Refresh()
	case "updateDownload":
		a.updateNotes = nil
		a.Refresh()
	}
	if plugins, ok := reply["plugins"]; ok {
		if a.State == nil {
			a.State = map[string]any{}
		}
		a.State["plugins"] = plugins
	}
	if reminders, ok := reply["reminderPlugins"]; ok {
		if a.State == nil {
			a.State = map[string]any{}
		}
		a.State["reminderPlugins"] = reminders
		a.pushToast("已保存")
	}
	if today, ok := reply["hydrationToday"]; ok {
		if a.State == nil {
			a.State = map[string]any{}
		}
		a.State["hydrationToday"] = today
		a.pushToast(stateString(reply, "已记录喝水", "message"))
	}
	return name == "moduleApply"
}
