package ui

import (
	"fmt"
	"github.com/egoist/mygo/ui"
	"strconv"
	"strings"
)

func todoAccent(t goTodo, index int) uint32 {
	if color, err := strconv.ParseUint(strings.TrimPrefix(t.Color, "#"), 16, 24); err == nil && len(strings.TrimPrefix(t.Color, "#")) == 6 {
		return uint32(color)
	}
	return []uint32{0x57BFFF, 0x9A8BEB, 0xFF9EAD, 0xEAB36A}[index%4]
}
func todoTimeLabel(t goTodo, today string) string {
	date := t.Date
	if date == today {
		date = "今天"
	}
	if t.Daily {
		date = "每天"
	}
	return strings.TrimSpace(date + "  " + t.Time)
}

// The reference's agenda row: a quiet rounded surface, colored hairline, title
// and time. The trailing circle retains completion without an invented Join action.
func (a *App) todoAgendaRow(c *ui.Context, t goTodo, index int, x, y, w, h float32, featured, full bool) {
	p, s := &a.Palette, &a.Palette.Scheme
	today, _ := a.todoNow()
	done := todoDone(t, today)
	rowBg := AlphaCol(0x111827, 0.03)
	if p.Dark {
		rowBg = AlphaCol(0xFFFFFF, 0.055)
	}
	if featured && !done {
		if p.Dark {
			rowBg = AlphaCol(0xFFFFFF, 0.075)
		} else {
			rowBg = AlphaCol(0xFFFFFF, 0.92)
		}
	}
	if p.HighContrast {
		bg := mix(s.Container, s.OnSurface, .035)
		if featured && !done {
			bg = s.Container
		}
		rowBg = Col(bg)
	}
	if full {
		appleWidgetSurface(c, p, overviewRect{"", x, y, w, h})
	}
	row := ui.Box(c).Key("agenda-" + t.ID).Absolute().Left(x).Top(y).Width(w).Height(h).Radius(20)
	if !full {
		row.Background(rowBg)
		if featured && !done && !p.HighContrast {
			if p.Dark {
				row.Border(1, AlphaCol(0xFFFFFF, .09))
			} else {
				row.Border(1, AlphaCol(0x111827, .06)).Shadow(0, 2, 6, -1, AlphaCol(0x111827, .04))
			}
		} else if !p.HighContrast && p.Dark {
			row.Border(1, AlphaCol(0xFFFFFF, .05))
		}
	}
	row.Children(func() {
		accent := todoAccent(t, index)
		if done {
			accent = s.Outline
		}
		ui.Box(c).Absolute().Left(12).Top((h - 22) / 2).Width(4).Height(22).Radius(2).Background(Col(accent)).IgnoreInput(true)
		trailing := float32(42)
		if full {
			trailing = 98
		}
		edit := ui.ButtonBase(c).Absolute().Left(26).Top(0).Width(w - 26 - trailing).Height(h).Label("edit-todo-" + t.ID).Cursor(ui.CursorPointer)
		if edit.Clicked() {
			a.openTodoDialog(t.ID)
		}
		edit.Children(func() {
			fg := s.OnSurface
			if done {
				fg = s.OnSurfaceVariant
			}
			if h >= 64 {
				overviewText(c, t.Title, 6, (h-52)/2, w-58-trailing, overviewType{16, 24, 500}, fg, 1)
				overviewText(c, todoTimeLabel(t, today), 6, (h-52)/2+32, w-58-trailing, overviewCaption, s.OnSurfaceVariant, 1)
			} else {
				Label(c, t.Title, 6, (h-46)/2, 14, fg, !done, w-58-trailing)
				Label(c, todoTimeLabel(t, today), 6, (h-46)/2+25, 11, s.OnSurfaceVariant, false, w-58-trailing)
			}
		})
		label := "完成待办 " + t.ID
		if done {
			label = "恢复待办 " + t.ID
		}
		check := ui.ButtonBase(c).Absolute().Left(w - trailing).Top((h - 36) / 2).Width(36).Height(36).Radius(18).Label(label).Cursor(ui.CursorPointer)
		if check.Clicked() {
			a.command("todoComplete", map[string]any{"id": t.ID, "completed": !done})
		}
		check.Children(func() {
			mark := ui.Box(c).Absolute().Left(8).Top(8).Width(20).Height(20).Radius(10).Border(1.3, AlphaCol(s.OnSurface, .24))
			if done {
				doneBg, doneFg := uint32(0x16181D), uint32(0xFFFFFF)
				if p.Dark {
					doneBg, doneFg = 0xF3F4F6, 0x111316
				}
				if p.HighContrast {
					doneBg, doneFg = s.Primary, s.OnPrimary
				}
				mark.Background(Col(doneBg)).Border(0, ui.Transparent)
				Icon(c, "check", 11, 11, 14, doneFg)
			}
		})
		if full {
			IconButton(c, p, "移除待办 "+t.ID, w-50, (h-36)/2, 36, 36, "delete", false, true, func() { a.command("todoDelete", map[string]any{"id": t.ID}) })
		}
	})
}

func (a *App) todoAgendaEmpty(c *ui.Context, r overviewRect) {
	p, s := &a.Palette, &a.Palette.Scheme
	badgeBg := AlphaCol(s.Primary, 0.10)
	badgeRim := AlphaCol(s.Primary, 0.22)
	if p.Dark {
		badgeBg = AlphaCol(s.Primary, 0.16)
		badgeRim = AlphaCol(s.Primary, 0.30)
	}
	ui.Box(c).Absolute().Left(r.x+20).Top(r.y+56).Width(46).Height(46).Radius(14).
		Background(badgeBg).Border(1, badgeRim).IgnoreInput(true)
	Icon(c, "checklist", r.x+32, r.y+68, 22, s.Primary)
	Label(c, "0", r.x+78, r.y+52, 40, s.OnSurface, true, r.w-100)
	Label(c, "今日待办已清零", r.x+78, r.y+98, 11, s.OnSurfaceVariant, false, r.w-100)
	Label(c, "今天，轻松一点。", r.x+22, r.y+126, 16, s.OnSurface, true, r.w-44)
	Label(c, "右上角添加下一件要做的事", r.x+22, r.y+156, 11.5, s.OnSurfaceVariant, false, r.w-44)
}

func todoSummary(pending, total int) string {
	return fmt.Sprintf("%d 件未完成 · 共 %d 件", pending, total)
}
