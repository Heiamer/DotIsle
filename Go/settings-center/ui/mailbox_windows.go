package ui

import (
	"context"
	"os"
	"strings"
	"time"

	"dotisle/settings/island"
	"dotisle/settings/mailbox"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

type mailState struct {
	address, server, secret, status, body string
	connected, configured, working        bool
	messages                              []mailbox.Message
	selected                              uint32
	validity                              uint32
	cancel                                context.CancelFunc
	refresh                               chan struct{}
	generation                            uint64
	period                                int
}

func (a *App) StartMail() {
	account, err := mailbox.Load(a.Data)
	if err != nil {
		if !os.IsNotExist(err) {
			a.mail.configured = true
			a.mail.status = "本机账户凭据不可读取，请移除后重新连接。"
		}
		return
	}
	a.watchMail(account)
}
func (a *App) StopMail() {
	a.mail.generation++
	if a.mail.cancel != nil {
		a.mail.cancel()
		a.mail.cancel = nil
	}
}
func (a *App) watchMail(account mailbox.Account) {
	a.StopMail()
	m := &a.mail
	m.connected, m.configured, m.address, m.server, m.status = true, true, account.Address, account.Server, "正在检查收件箱…"
	ctx, cancel := context.WithCancel(context.Background())
	m.cancel = cancel
	wake := make(chan struct{}, 1)
	m.refresh = wake
	generation := m.generation
	go func() {
		var seen uint32
		var validity uint32
		var baseline bool
		for ctx.Err() == nil {
			request, stop := context.WithTimeout(ctx, 14*time.Second)
			inbox, err := mailbox.Recent(request, account)
			stop()
			status := "已连接 · 只读收件箱"
			if err != nil {
				status = err.Error()
			}
			fallingBack := 90 * time.Second
			if err == nil {
				fallingBack = 45 * time.Second
				if baseline && inbox.Validity != 0 && inbox.Validity == validity && a.mailOtpEnabled.Load() {
					for i, message := range inbox.Messages {
						if i >= 3 || message.UID <= seen {
							break
						}
						// Check only newly arriving mail, not the saved inbox at startup.
						// The local island applies the same verification parser as toasts.
						req, cancelBody := context.WithTimeout(ctx, 12*time.Second)
						text, _ := mailbox.Body(req, account, inbox.Validity, message.UID)
						cancelBody()
						if ctx.Err() == nil {
							_, _ = island.Request("mailOtp", map[string]any{"subject": message.Subject, "body": text})
						}
					}
				}
				seen = 0
				if len(inbox.Messages) > 0 {
					seen = inbox.Messages[0].UID
				}
				baseline, validity = true, inbox.Validity
			}
			mygo.RunOnMain(func() {
				if a.mail.generation != generation {
					return
				}
				a.mail.status = status
				if err == nil {
					if a.mail.validity != inbox.Validity {
						a.mail.selected = 0
						a.mail.body = ""
					}
					a.mail.messages, a.mail.validity = inbox.Messages, inbox.Validity
					a.mail.period = min(a.mail.period, max(0, (len(inbox.Messages)-1)/5))
				}
				if a.mailOpen {
					a.Refresh()
				}
			})
			timer := time.NewTimer(fallingBack)
			select {
			case <-ctx.Done():
			case <-wake:
			case <-timer.C:
			}
			timer.Stop()
		}
	}()
}

func (a *App) connectMail() {
	m := &a.mail
	if m.working {
		return
	}
	account, err := mailbox.Validate(mailbox.Account{Address: m.address, Server: m.server, Secret: m.secret})
	if err != nil {
		m.status = err.Error()
		a.Refresh()
		return
	}
	m.working, m.status = true, "正在测试连接…"
	a.startAsync(func() {
		ctx, cancel := context.WithTimeout(context.Background(), 14*time.Second)
		defer cancel()
		_, err := mailbox.Recent(ctx, account)
		if err == nil {
			if saveErr := mailbox.Save(a.Data, account); saveErr != nil {
				err = mailStoreError{}
			}
		}
		a.applyAsync(func() {
			m.working = false
			if err != nil {
				m.status = err.Error()
			} else {
				m.secret = ""
				a.watchMail(account)
			}
			a.Refresh()
		})
	})
}

type mailStoreError struct{}

func (mailStoreError) Error() string { return "无法在当前 Windows 用户下保存加密凭据" }

func (a *App) readMail(message mailbox.Message) {
	m := &a.mail
	m.selected, m.body = message.UID, "正在加载邮件…"
	generation, validity := m.generation, m.validity
	a.startAsync(func() {
		ctx, cancel := context.WithTimeout(context.Background(), 14*time.Second)
		defer cancel()
		account, err := mailbox.Load(a.Data)
		var text string
		if err == nil {
			text, err = mailbox.Body(ctx, account, validity, message.UID)
		}
		a.applyAsync(func() {
			if m.generation != generation || m.selected != message.UID || m.validity != validity || !a.mailOpen {
				return
			}
			switch {
			case err != nil:
				m.body = "暂时无法读取正文，请刷新重试。"
			case text == "":
				m.body = "这封邮件没有可显示的纯文本。"
			default:
				m.body = text
			}
			a.Refresh()
		})
	})
}

func (a *App) mailPluginRow(c *ui.Context, x, y, w float32) float32 {
	const height = 102
	if y+height < a.scrollY() || y > a.scrollY()+float32(clipH) {
		return y + height + 12
	}
	p, s := &a.Palette, &a.Palette.Scheme
	appleWidgetSurface(c, p, overviewRect{"", x, y, w, height})
	Label(c, "邮箱", x+22, y+16, 18, s.OnSurface, true, w-160)
	Label(c, "查看新邮件 · 验证码可在灵动岛复制", x+22, y+52, 12, s.OnSurfaceVariant, false, w-168)
	Button(c, p, "打开邮箱", x+w-131, y+29, 108, 38, "打开邮箱", "", true, true, func() { a.mailOpen = true; a.Refresh() })
	return y + height + 12
}

func (a *App) mailDialog(c *ui.Context) {
	m, p := &a.mail, &a.Palette
	s := &p.Scheme
	open := true
	const w, h float32 = 620, 489
	ui.DialogBase(c, &open, func(backdrop, panel ui.Element) {
		backdrop.Background(AlphaCol(0, .42))
		panel.Width(w).Height(h)
		setupDialogStyle(&panel, p)
		panel.Children(func() {
			Label(c, "邮箱", 24, 22, 21, s.OnSurface, true, w-110)
			IconButton(c, p, "关闭邮箱", w-67, 14, 44, 44, "close", false, !m.working, func() { open = false })
			status := m.status
			if status == "" {
				status = "请在邮箱网站开启 IMAP，并生成专用授权码。"
			}
			Label(c, status, 24, 60, 12, s.OnSurfaceVariant, false, w-48)
			if !m.connected {
				const formTop, gap = 96, 12
				a.dialogFieldInput(c, p, 24, formTop, w-48, "邮箱地址", &m.address, 254, false, m.working)
				a.dialogFieldInput(c, p, 24, formTop+dialogFieldHeight+gap, w-48, "IMAP 域名 · QQ / 163 / 126 可留空", &m.server, 253, false, m.working)
				a.dialogFieldInput(c, p, 24, formTop+2*(dialogFieldHeight+gap), w-48, "IMAP 授权码", &m.secret, 256, true, m.working)
				Label(c, "只读收件箱，授权码用 Windows 用户级加密保存。", 24, 344, 12, s.OnSurfaceVariant, false, w-48)
				Label(c, "仅检查新到邮件的验证码；不保存邮件正文，不标记已读。", 24, 370, 12, s.OnSurfaceVariant, false, w-48)
				Button(c, p, "连接邮箱", w-159, h-62, 135, 38, "连接邮箱", "", false, !m.working && !m.configured, a.connectMail)
			} else if m.selected != 0 {
				Button(c, p, "返回收件箱", 24, 86, 119, 36, "返回收件箱", "", true, true, func() { m.selected = 0; m.body = ""; a.Refresh() })
				for _, msg := range m.messages {
					if msg.UID == m.selected {
						Label(c, msg.Subject, 24, 140, 17, s.OnSurface, true, w-48)
						Label(c, msg.From, 24, 168, 12, s.OnSurfaceVariant, false, w-48)
					}
				}
				// The preview is plain text; HTML never runs and remote images are never loaded.
				runes := []rune(m.body)
				for i, y := 0, float32(205); i < len(runes) && y < 414; y += 22 {
					n := min(i+47, len(runes))
					Label(c, strings.TrimSpace(string(runes[i:n])), 24, y, 13, s.OnSurface, false, w-48)
					i = n
				}
			} else {
				Label(c, m.address, 24, 92, 13, s.OnSurface, true, w-241)
				Button(c, p, "刷新收件箱", w-146, 83, 122, 36, "刷新收件箱", "", true, true, func() {
					select {
					case m.refresh <- struct{}{}:
					default:
					}
				})
				if len(m.messages) == 0 {
					Label(c, "暂时没有最近邮件。", 24, 148, 14, s.OnSurfaceVariant, false, w-48)
				}
				for i, start := 0, m.period*5; start+i < len(m.messages) && i < 5; i++ {
					msg := m.messages[start+i]
					y := float32(132 + i*49)
					subject := msg.Subject
					if subject == "" {
						subject = "（无主题）"
					}
					Button(c, p, "查看邮件 "+subject, 24, y, w-164, 38, subject, "", true, true, func() { a.readMail(msg); a.Refresh() })
					Label(c, msg.Date.Local().Format("01-02 15:04"), w-126, y+11, 12, s.OnSurfaceVariant, false, 103)
				}
				if m.period > 0 {
					Button(c, p, "上一页邮件", 24, 393, 86, 32, "上一页", "", true, true, func() { m.period--; a.Refresh() })
				}
				if (m.period+1)*5 < len(m.messages) {
					Button(c, p, "下一页邮件", 119, 393, 86, 32, "下一页", "", true, true, func() { m.period++; a.Refresh() })
				}
			}
			if m.configured {
				Button(c, p, "断开并删除本机凭据", 24, h-49, 185, 32, "断开并删除本机凭据", "", true, !m.working, func() {
					if err := mailbox.Forget(a.Data); err != nil {
						m.status = "无法删除本机凭据，请重试。"
					} else {
						a.StopMail()
						g := m.generation
						*m = mailState{generation: g, status: "已断开并删除本机凭据。"}
					}
					a.Refresh()
				})
			}
		})
	})
	if !open && !m.working {
		a.mailOpen = false
		m.secret = ""
		m.selected = 0
		m.body = ""
	}
}
