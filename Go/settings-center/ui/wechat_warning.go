package ui

import (
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"time"

	"dotisle/settings/island"
	"github.com/egoist/mygo/ui"
)

const wechatHintsKey = "WeChatMessageHintsEnabled"
const wechatWarningDelay = 10 * time.Second

type wechatWarning struct {
	id       string
	deadline time.Time
	timer    *time.Timer
	armed    bool
	saving   bool
}

func (w *wechatWarning) remaining(now time.Time) int {
	if w.deadline.IsZero() {
		return int(wechatWarningDelay / time.Second)
	}
	return int((max(0, w.deadline.Sub(now)) + time.Second - 1) / time.Second)
}

func (a *App) openWechatWarning() {
	if a.wechatWarning != nil || !a.Connected || a.Win == nil {
		return
	}
	var nonce [16]byte
	if _, err := rand.Read(nonce[:]); err != nil {
		a.pushToast("无法打开微信提醒警告，请重试。")
		return
	}
	w := &wechatWarning{id: hex.EncodeToString(nonce[:])}
	a.wechatWarning = w
	a.Refresh()
	a.startAsync(func() {
		reply, err := island.Request("wechatWarningOpen", w.id)
		a.applyAsync(func() {
			if a.wechatWarning != w {
				a.revokeWechatWarning(w)
				return
			}
			if err != nil || reply["success"] != true {
				a.cancelWechatWarning()
				a.pushToast("无法启动确认倒计时，请重试。")
				return
			}
			w.armed = true
		})
	})
}

func (a *App) tickWechatWarning(w *wechatWarning) {
	if a.wechatWarning != w || w.remaining(time.Now()) == 0 {
		return
	}
	w.timer = time.AfterFunc(time.Second, func() {
		a.applyAsync(func() { a.tickWechatWarning(w) })
	})
}

func (a *App) revokeWechatWarning(w *wechatWarning) {
	a.startAsync(func() { _, _ = island.Request("wechatWarningCancel", w.id) })
}

func (a *App) cancelWechatWarning() {
	if w := a.wechatWarning; w != nil {
		a.wechatWarning = nil
		if w.timer != nil {
			w.timer.Stop()
		}
		if !w.saving {
			a.revokeWechatWarning(w)
		}
		a.Refresh()
	}
}

func (a *App) confirmWechatWarning(w *wechatWarning) {
	if a.wechatWarning != w || w.saving || !a.Connected || w.remaining(time.Now()) != 0 {
		return
	}
	w.saving = true
	a.Refresh()
	a.startAsync(func() {
		reply, err := island.Request("wechatWarningConfirm", w.id)
		a.applyAsync(func() {
			if a.wechatWarning == w {
				a.cancelWechatWarning()
			}
			if err != nil || reply["success"] != true {
				a.pushToast(stateString(reply, "未启用微信提醒，请重新打开警告后重试。", "message"))
				return
			}
			a.pushToast("微信任务栏提醒已启用。")
		})
	})
}

func (a *App) wechatWarningView(c *ui.Context, width, height float32) {
	w, p := a.wechatWarning, &a.Palette
	if w.armed && w.deadline.IsZero() {
		// Count from the first dialog frame after the native timer has been armed.
		w.deadline = time.Now().Add(wechatWarningDelay)
		a.tickWechatWarning(w)
	}
	s := &p.Scheme
	dw, dh := min(width-48, float32(536)), min(height-48, float32(280))
	open := true
	ui.DialogBase(c, &open, func(backdrop, panel ui.Element) {
		backdrop.Background(AlphaCol(0, .36))
		panel.Width(dw).Height(dh).Label("微信提醒风险提示")
		overlayStyle(&panel, p)
		panel.Children(func() {
			Label(c, "微信提醒风险提示", 28, 24, 22, s.OnSurface, true, dw-56)
			Label(c, "旧版微信正文读取可能触发账号风控、限制或封号。", 28, 76, 14, s.OnSurface, true, dw-56)
			Label(c, "此版本只检测 Windows 任务栏闪烁，不读取正文。", 28, 106, 14, s.OnSurfaceVariant, false, dw-56)
			Label(c, "默认关闭。请阅读提示，等待倒计时结束后再决定是否启用。", 28, 136, 14, s.OnSurfaceVariant, false, dw-56)
			left := w.remaining(time.Now())
			label := "确认并启用"
			if left != 0 {
				label = fmt.Sprintf("请阅读（%d 秒）", left)
			} else if w.saving {
				label = "正在启用…"
			}
			Button(c, p, "取消启用微信提醒", dw-265, dh-66, 96, 40, "取消", "", true, !w.saving, a.cancelWechatWarning)
			Button(c, p, "确认并启用微信提醒", dw-155, dh-66, 127, 40, label, "", false, left == 0 && !w.saving && a.Connected, func() { a.confirmWechatWarning(w) })
		})
	})
	if !open && !w.saving {
		a.cancelWechatWarning()
	}
}
