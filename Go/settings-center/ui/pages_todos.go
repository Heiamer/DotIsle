// The todos page and its new/edit dialog, ported 1:1 from views.rs fn todos
// (959-1078) and the DialogKind::Todo branch of views.rs dialog (1207-1489),
// Original theme only. Layout constants, sizes and copy come from the Rust source.
package ui

import (
	"fmt"
	"sort"
	"time"

	"dotisle/settings/island"
	"github.com/egoist/mygo/ui"
)

// goTodo mirrors dotisle_domain::todos::Todo (camelCase wire format).
type goTodo struct {
	ID            string `json:"id"`
	Title         string `json:"title"`
	Content       string `json:"content"`
	Date          string `json:"date"`
	Time          string `json:"time"`
	Daily         bool   `json:"daily"`
	Color         string `json:"color"`
	Completed     bool   `json:"completed"`
	CompletedDate string `json:"completedDate"`
}

// todoItems parses the island snapshot's todos list.
func (a *App) todoItems() []goTodo {
	if a.State == nil {
		return nil
	}
	raw, ok := a.State["todos"].([]any)
	if !ok {
		return nil
	}
	items := make([]goTodo, 0, len(raw))
	for _, v := range raw {
		value, ok := v.(map[string]any)
		if !ok {
			continue
		}
		items = append(items, goTodo{
			ID:            stateString(value, "", "id"),
			Title:         stateString(value, "", "title"),
			Content:       stateString(value, "", "content"),
			Date:          stateString(value, "", "date"),
			Time:          stateString(value, "", "time"),
			Daily:         stateBool(value, "daily"),
			Color:         stateString(value, "", "color"),
			Completed:     stateBool(value, "completed"),
			CompletedDate: stateString(value, "", "completedDate"),
		})
	}
	return items
}

// todoNow reads the island clock: state keys today (date) and now (time),
// falling back to the local clock like todos::Now::local.
func (a *App) todoNow() (string, string) {
	date, _ := a.State["today"].(string)
	clock, _ := a.State["now"].(string)
	if date == "" || clock == "" {
		t := time.Now()
		if date == "" {
			date = t.Format("2006-01-02")
		}
		if clock == "" {
			clock = t.Format("15:04")
		}
	}
	return date, clock
}

// todoDone mirrors dotisle-todos engine: done = completed && (!daily || completed_date == today).
func todoDone(t goTodo, date string) bool {
	return t.Completed && (!t.Daily || t.CompletedDate == date)
}

// todoDialogDraft holds the Todo form until the fields move into App centrally.
var todoDialogDraft struct {
	id          string
	title       string
	date        string
	clock       string
	content     string
	daily       bool
	err         string
	initialized bool
}

// openTodoDialog prefills the draft (Action::EditTodo, model.rs 1310-1343).
func (a *App) openTodoDialog(id string) {
	date, clock := a.todoNow()
	todoDialogDraft.id = id
	todoDialogDraft.title = ""
	todoDialogDraft.date = date
	todoDialogDraft.clock = clock
	todoDialogDraft.content = ""
	todoDialogDraft.daily = false
	todoDialogDraft.err = ""
	todoDialogDraft.initialized = true
	if id != "" {
		for _, t := range a.todoItems() {
			if t.ID == id {
				todoDialogDraft.title = t.Title
				todoDialogDraft.date = t.Date
				todoDialogDraft.clock = t.Time
				todoDialogDraft.content = t.Content
				todoDialogDraft.daily = t.Daily
			}
		}
	}
	a.todoDialog = true
}

// todosPage mirrors views.rs fn todos at Original geometry. x/y/w arrive from
// root.go pageBody; everything below is positioned relative to them.
func (a *App) todosPage(c *ui.Context, x, y, w float32) float32 {
	p := &a.Palette
	s := &p.Scheme
	todos := a.todoItems()
	date, _ := a.todoNow()
	sort.SliceStable(todos, func(i, j int) bool {
		di, dj := todoDone(todos[i], date), todoDone(todos[j], date)
		if di != dj {
			return !di // not-done first, like the Rust key (done, date, time)
		}
		if todos[i].Date != todos[j].Date {
			return todos[i].Date < todos[j].Date
		}
		return todos[i].Time < todos[j].Time
	})
	pending := 0
	for _, t := range todos {
		if !todoDone(t, date) {
			pending++
		}
	}
	var bottom float32
	if len(todos) == 0 {
		bottom = 405
	} else {
		bottom = 42 + float32(len(todos))*88
	}
	page := ui.Box(c).Margin(y, 0, 0, x).Width(w).Height(bottom)
	page.Children(func() {
		Label(c, fmt.Sprintf("%d 件未完成 · 共 %d 件", pending, len(todos)), 0, 4, 13, s.OnSurfaceVariant, false, w)
		if len(todos) == 0 {
			Panel(c, p, 0, 45, w, 360, s.Container)
			// draw::circle(x+w/2, y+158, 42, secondary_container) + 44px leaf.
			ui.Box(c).Absolute().Left(w/2 - 42).Top(116).Width(84).Height(84).
				Radius(42).Background(Col(s.SecondaryContainer))
			if svg := Symbol("eco"); svg != nil {
				ui.Icon(c, svg).FontSize(44).TextColor(Col(s.Primary)).
					Absolute().Left(w/2 - 22).Top(136)
			}
			CenteredText(c, "心里，少一点牵挂", 0, 234, w, 44, 16, s.OnSurface, true)
			CenteredText(c, "点击右上角的新建待办，记下第一件事。", 0, 293, w, 28, 14, s.OnSurfaceVariant, false)
			return
		}
		top := float32(42)
		for index, item := range todos {
			if y+top+76 >= a.scrollY() && y+top <= a.scrollY()+float32(clipH) {
				a.todoAgendaRow(c, item, index, 0, top, w, 76, index == 0, true)
			}
			top += 88
		}
	})
	return y + bottom
}

// winSize returns the current window size for dialog centering.
func winSize(c *ui.Context) (float32, float32) {
	w, h := c.Size()
	return w, h
}

// todoDialogView is views.rs dialog for DialogKind::Todo: a 560×552 surface card
// centered over a 0.38 black backdrop (DialogBase, escape/outside click closes).
func (a *App) todoDialogView(c *ui.Context, w, h float32) {
	if !a.todoDialog {
		todoDialogDraft.initialized = false
		return
	}
	if !todoDialogDraft.initialized {
		a.openTodoDialog("") // header 新建待办 opens without prefilling
	}
	p := &a.Palette
	s := &p.Scheme
	const dw, dh = float32(560), float32(552)
	open := a.todoDialog
	ui.DialogBase(c, &open, func(backdrop, panel ui.Element) {
		backdrop.Background(AlphaCol(0, 0.5))
		panel.Width(dw).Height(dh)
		overlayStyle(&panel, p)
		panel.Children(func() {
			title := "记下一件事"
			if todoDialogDraft.id != "" {
				title = "编辑待办"
			}
			Label(c, title, 28, 24, 20, s.OnSurface, true, dw-110)
			IconButton(c, p, "取消", dw-68, 20, 44, 44, "close", false, true, func() {
				open = false
			})
			labels := [4]string{"要记住的事", "日期 · YYYY-MM-DD", "时间 · HH:MM", "备注"}
			values := [4]*string{&todoDialogDraft.title, &todoDialogDraft.date,
				&todoDialogDraft.clock, &todoDialogDraft.content}
			maxs := [4]int{120, 10, 5, 2048}
			var titleInput ui.Element
			for i := 0; i < 4; i++ {
				fy := 88 + float32(i)*84
				field := ui.Box(c).Absolute().Left(28).Top(fy).Width(dw - 56).Height(68).
					Radius(12).Background(Col(s.High))
				input := ui.TextInputBase(c, values[i]).Absolute().Left(42).Top(fy + 26).
					Width(dw - 56 - 28).Height(36).Font(UIFont).FontSize(14).TextColor(Col(s.OnSurface)).
					MaxLength(maxs[i])
				if i == 0 {
					titleInput = input
				}
				surfaceStyle(&field, p, s.Container, FieldRadius)
				// draw::outline(rect, 12, primary, 2) for the active field, built on
				// every frame (transparent while unfocused) so that the panel keeps
				// the same children: a box inserted on focus shifted the identity of
				// the 保存/取消 buttons and swallowed the press that hit them
				// (kit.go FieldOutline).
				FieldOutline(c, 28, fy, dw-56, 68, 12, input.Focused(), s.Primary)
				Label(c, labels[i], 42, fy+8, 12, s.OnSurfaceVariant, true, dw-88)
			}
			// Enter in the title offers the same save action as the button.
			if titleInput.Submitted() {
				a.sendTodoSave()
			}
			Chip(c, p, "每天重复", 28, 432, 128, 40, "每天重复", todoDialogDraft.daily, func() {
				todoDialogDraft.daily = !todoDialogDraft.daily
			})
			if todoDialogDraft.err != "" {
				Label(c, todoDialogDraft.err, 28, dh-104, 12, s.Error, true, dw-56)
			}
			Button(c, p, "取消待办", dw-248, dh-70, 96, 44, "取消", "", true, true, func() {
				open = false
			})
			Button(c, p, "保存待办", dw-140, dh-70, 112, 44, "保存", "", false, true, func() {
				a.sendTodoSave()
			})
		})
	})
	a.todoDialog = open
}

// sendTodoSave sends todoSave with the DialogKind::Todo payload of model.rs and
// mirrors apply_native_reply: success closes, failure shows the message.
func (a *App) sendTodoSave() {
	color := "#F3D79B"
	if todoDialogDraft.id != "" {
		for _, t := range a.todoItems() {
			if t.ID == todoDialogDraft.id {
				color = t.Color
			}
		}
	}
	payload := map[string]any{
		"id":      todoDialogDraft.id,
		"title":   todoDialogDraft.title,
		"date":    todoDialogDraft.date,
		"time":    todoDialogDraft.clock,
		"content": todoDialogDraft.content,
		"daily":   todoDialogDraft.daily,
		"color":   color,
	}
	go func() {
		reply, err := island.Request("todoSave", payload)
		if a.Win == nil {
			return
		}
		a.Win.Update(func() {
			switch {
			case err != nil:
				todoDialogDraft.err = err.Error()
			default:
				if ok, _ := reply["success"].(bool); ok || reply["success"] == nil {
					a.todoDialog = false // success: the dialog closes
				} else if msg, _ := reply["message"].(string); msg != "" {
					todoDialogDraft.err = msg
				} else {
					todoDialogDraft.err = "操作失败"
				}
			}
			a.Refresh()
		})
	}()
}
