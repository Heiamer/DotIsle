package ui

import "github.com/egoist/mygo/ui"

const overviewInset float32 = 24

type overviewType struct {
	size, line float32
	weight     int
}

var (
	overviewTitle   = overviewType{16, 24, 600}
	overviewLead    = overviewType{24, 32, 600}
	overviewBody    = overviewType{16, 24, 400}
	overviewCaption = overviewType{13, 20, 400}
	overviewNumber  = overviewType{60, 72, 500}
)

// Fixed line boxes preserve the same baseline rhythm across all deck pages.
func overviewText(c *ui.Context, text string, x, y, w float32, style overviewType, color uint32, lines int, align ...ui.Align) *ui.Element {
	e := ui.Box(c).Absolute().Left(x).Top(y).Width(w).Height(style.line * float32(lines))
	e.Children(func() {
		t := ui.Text(c, text).Font(textFamily(text)).FontSize(style.size).FixedLineHeight(style.line).
			FontWeight(style.weight).TextColor(Col(color)).FontFeatures("tnum")
		if len(align) > 0 {
			t.FillWidth().TextAlign(align[0])
		}
		if lines == 1 {
			t.SingleLine()
		} else {
			t.MaxLines(lines)
		}
	})
	return &e
}

func (a *App) overviewEmpty(c *ui.Context, r overviewRect, notification bool) {
	s := &a.Palette.Scheme
	title, message, detail := "待办事项", "暂无待办事项", "新建待办，安排今天的计划"
	if notification {
		title, message, detail = "通知记录", "还没有通知", "收到的通知与到期提醒会显示在这里"
	}
	a.cardHeader(c, r, title)
	y := r.y + 64 + (r.h-88-132)/2
	overviewText(c, message, r.x+24, y, r.w-48, overviewType{22, 32, 600}, s.OnSurface, 1, ui.Center)
	overviewText(c, detail, r.x+24, y+44, r.w-48, overviewCaption, s.OnSurfaceVariant, 1, ui.Center)
	if notification {
		widgetTextAction(c, &a.Palette, "概览全部通知", "查看通知记录", r.x+(r.w-100)/2, y+88, 100, func() { a.navigate(PageNotifications) })
	} else {
		Button(c, &a.Palette, "概览空态添加待办", r.x+(r.w-144)/2, y+88, 144, 40, "新建待办", "add", false, true, func() { a.openTodoDialog("") })
	}
}

type overviewGuides struct{ left, top, bottom, visualW, bodyX, bodyW float32 }

// A shared inset, leading visual column and body column replace per-card offsets.
func expandedGuides(r overviewRect) overviewGuides {
	visual := min(float32(224), (r.w-80)*.3)
	left := r.x + overviewInset
	body := left + visual + 32
	return overviewGuides{left, r.y + 64, r.y + r.h - overviewInset, visual, body, r.x + r.w - overviewInset - body}
}

func (a *App) overviewCount(c *ui.Context, g overviewGuides, count int, unit string) {
	s := &a.Palette.Scheme
	y := (g.top + g.bottom - 104) / 2
	overviewText(c, itoa(count), g.left, y, g.visualW, overviewNumber, s.OnSurface, 1)
	overviewText(c, unit, g.left, y+80, g.visualW, overviewBody, s.OnSurfaceVariant, 1)
}
