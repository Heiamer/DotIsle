package ui

import (
	"fmt"

	"dotisle/settings/island"
)

// command returns pipe and application failures to the main thread. An accepted
// moduleApply must close this process so the updater can release its loaded DLLs.
func (a *App) command(name string, value any) {
	if a.Win == nil {
		return
	}
	go func() {
		reply, err := island.Request(name, value)
		if a.Win == nil {
			return
		}
		a.Win.Update(func() {
			if a.applyCommandReply(name, reply, err) {
				a.Win.Close()
			}
		})
	}()
}

// applyCommandReply reports a failure and returns whether this center must exit.
// A missing success field is not an acknowledgement of a mutation.
func (a *App) applyCommandReply(name string, reply map[string]any, err error) bool {
	if err != nil {
		a.pushToast("请求失败：" + err.Error())
		return false
	}
	if reply["success"] != true {
		message := stateString(reply, "操作未成功，请重试。", "message")
		a.pushToast(fmt.Sprintf("操作失败：%s", message))
		return false
	}
	if plugins, ok := reply["plugins"]; ok {
		if a.State == nil {
			a.State = map[string]any{}
		}
		a.State["plugins"] = plugins
	}
	return name == "moduleApply"
}
