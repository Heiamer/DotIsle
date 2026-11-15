// The notification center page, ported 1:1 from notification_center.rs render()
// (Original theme). Filter state mirrors model.notice_filter (u8, UI-local).
package ui

import (
	"github.com/egoist/mygo/ui"
)

// noticeFilter mirrors Model::notice_filter: 0 全部, 1 应用通知, 2 待办, 3 计时, 4 设备.
// UI-local until it can move into App centrally.
var noticeFilter uint8

// goHistoryEvent mirrors lyric-native history::Event (camelCase wire format).
type goHistoryEvent struct {
	ID      string `json:"id"`
	Kind    string `json:"kind"`
	AppID   string `json:"appId"`
	AppName string `json:"appName"`
	Title   string `json:"title"`
	Body    string `json:"body"`
	Date    string `json:"date"`
	Clock   string `json:"clock"`
	TodoID  string `json:"todoId"`
}

func (a *App) historyEvents() []goHistoryEvent {
	if a.State == nil {
		return nil
	}
	raw, ok := a.State["notificationHistory"].([]any)
	if !ok {
		return nil
	}
	events := make([]goHistoryEvent, 0, len(raw))
	for _, v := range raw {
		value, ok := v.(map[string]any)
		if !ok {
			continue
		}
		events = append(events, goHistoryEvent{
			ID:      stateString(value, "", "id"),
			Kind:    stateString(value, "", "kind"),
			AppID:   stateString(value, "", "appId"),
			AppName: stateString(value, "", "appName"),
			Title:   stateString(value, "", "title"),
			Body:    stateString(value, "", "body"),
			Date:    stateString(value, "", "date"),
			Clock:   stateString(value, "", "clock"),
			TodoID:  stateString(value, "", "todoId"),
		})
	}
	return events
}

// notificationsPaused reads setting("PauseNotifications").
func (a *App) notificationsPaused() bool {
	if b, ok := a.pref("PauseNotifications").(bool); ok {
		return b
	}
	return false
}

// notificationsPage mirrors notification_center.rs render().
func (a *App) notificationsPage(c *ui.Context, x, y, w float32) float32 {
	p := &a.Palette
	s := &p.Scheme
	paused := a.notificationsPaused()
	events := a.historyEvents()
	errorText, _ := a.State["historyError"].(string)

	// Local geometry pass (Rust mutates y through the same steps).
	ly := float32(0)
	banner := false
	if access := stateString(a.State, "", "notificationState", "access"); access != "granted" && access != "allowed" {
		banner = true
	}
	kind := [5]string{"", "notice", "todo", "timer", "device"}[min8(noticeFilter, 4)]
	count := 0
	for _, e := range events {
		if kind == "" || e.Kind == kind {
			count++
		}
	}
	bottom := float32(56 + 56)
	if banner {
		bottom += 90
	}
	if count > 0 {
		bottom += float32(count) * 120
	} else {
		bottom += 232
	}
	if errorText != "" {
		bottom += 44
	}

	page := ui.Box(c).Margin(y, 0, 0, x).Width(w).Height(bottom)
	page.Children(func() {
		// pause(): 暂停/恢复通知 → update PauseNotifications (Action::NativeEdit).
		pauseLabel := "暂停通知"
		pauseIcon := "pause"
		if paused {
			pauseLabel = "恢复通知"
			pauseIcon = "play_arrow"
		}
		Button(c, p, pauseLabel, 0, 0, 132, 40, pauseLabel, pauseIcon, true, true, func() {
			a.pushNative("PauseNotifications", !paused)
		})
		// Action::SettingsTab(242): the 通知与提醒 group in settings_hub.
		Button(c, p, "通知设置", 144, 0, 132, 40, "通知设置", "settings", true, true, func() {
			a.openSettings(242)
		})
		Button(c, p, "清理记录", w-144, 0, 144, 40, "清理记录", "delete", true, len(events) > 0, func() {
			// Action::NativeCommand("historyClear", null).
			a.command("historyClear", nil)
		})
		ly = 56
		Segmented(c, p, 0, ly, w, []string{"全部", "应用通知", "待办", "计时", "设备"}, int(noticeFilter), func(index int) { noticeFilter = uint8(index) })
		ly += 56
		if banner {
			detail := stateString(a.State, "连接后显示 Windows 通知访问状态", "notificationState", "message")
			Panel(c, p, 0, ly, w, 72, s.High)
			Label(c, detail, 22, ly+23, 13, s.OnSurfaceVariant, false, w-44)
			ly += 90
		}
		shown := 0
		for _, e := range events {
			if kind != "" && e.Kind != kind {
				continue
			}
			// Retain only visible rows, with identities independent of the viewport.
			if y+ly+104 >= a.scrollY() && y+ly <= a.scrollY()+float32(clipH) {
				ui.Box(c).Key("history-" + e.ID).Absolute().Left(0).Top(ly).Width(w).Height(104).Children(func() {
					a.historyRow(c, p, e, 0, 0, w)
				})
			}
			ly += 120
			shown++
		}
		if shown == 0 {
			Panel(c, p, 0, ly, w, 212, s.Container)
			if svg := Symbol("volume_up"); svg != nil {
				ui.Icon(c, svg).FontSize(30).TextColor(Col(s.Primary)).Absolute().Left(26).Top(ly + 28)
			}
			Label(c, "暂时没有这类通知", 26, ly+88, 16, s.OnSurface, true, w-52)
			Label(c, "新收到的通知会自动出现在这里。", 26, ly+140, 13, s.OnSurfaceVariant, false, w-52)
			ly += 232
		}
		if errorText != "" {
			Label(c, errorText, 4, ly, 13, s.Error, false, w-8)
		}
	})
	return y + bottom
}

// historyRow is notification_center.rs row() with panel=true.
func (a *App) historyRow(c *ui.Context, p *Palette, e goHistoryEvent, x, y, w float32) {
	s := &p.Scheme
	Panel(c, p, x, y, w, 104, s.Container)
	title := e.Title
	if title == "" {
		title = "通知"
	}
	Label(c, title, x+24, y+9, 16, s.OnSurface, true, w-204)
	Label(c, e.Body, x+24, y+38, 12, s.OnSurfaceVariant, false, w-204)
	app := e.AppName
	if app == "" {
		app = "点屿"
	}
	Label(c, app+" · "+e.Date+" "+e.Clock, x+24, y+65, 11, s.OnSurfaceVariant, false, w-204)
	if e.Kind == "notice" {
		Button(c, p, "打开来源", x+w-156, y+25, 132, 40, "打开来源", "arrow_forward", true, true, func() {
			// Action::NativeCommand("historyOpen", event.id).
			a.command("historyOpen", e.ID)
		})
	} else if e.TodoID != "" {
		Button(c, p, "查看待办", x+w-156, y+25, 132, 40, "查看待办", "", true, true, func() {
			// Action::EditTodo(Some(id)): the shared Todo dialog, prefilled.
			a.openTodoDialog(e.TodoID)
		})
	}
}

func min8(v, cap uint8) uint8 {
	if v > cap {
		return cap
	}
	return v
}
