package ui

import (
	"math"
	"path/filepath"
	"strings"
	"time"

	"dotisle/settings/prefs"
	"github.com/egoist/mygo/ui"
)

type overviewState struct {
	config                                                     prefs.Overview
	loaded, edit, manage                                       bool
	catalogScroll                                              ui.ScrollState
	catalogHeight                                              float32
	catalogMode                                                string
	informationScroll                                          ui.ScrollState
	pressed, dragging, target                                  string
	pressedAt                                                  time.Time
	startX, startY, pointerX, pointerY                         float32
	resizing                                                   string
	resizeStartX, resizeStartY, resizeInitialW, resizeInitialH float32
	resizeW, resizeH, resizeCanvas, resizeOrigin               float32
}

type overviewRect struct {
	key        string
	x, y, w, h float32
}

func (a *App) loadOverview() {
	if a.overview.loaded {
		return
	}
	a.overview.loaded = true
	p, err := prefs.LoadOverview(filepath.Join(a.Data, "center-overview.json"))
	a.overview.config = p
	if err != nil {
		a.pushToast("概览配置读取失败：" + err.Error())
	}
}
func (a *App) saveOverview() {
	if err := prefs.SaveOverview(filepath.Join(a.Data, "center-overview.json"), a.overview.config); err != nil {
		a.pushToast("概览配置保存失败：" + err.Error())
	}
}
func overviewName(key string) string {
	if strings.HasPrefix(key, "plugin:") {
		return "插件 · " + strings.TrimPrefix(key, "plugin:")
	}
	if key == "widget:system" {
		return "电脑状态"
	}
	if key == "information" {
		return "信息组件"
	}
	names := map[string]string{"music": "音乐", "clock": "时钟与日历", "todos": "待办事项", "focus": "专注计时", "quick": "快捷设置", "stats": "使用概况", "notifications": "通知记录", "widgets": "桌面组件", "wireless": "无线设备电量", "agenda": "接下来", "cpu": "处理器", "memory": "内存", "week": "本周进度"}
	if name := names[key]; name != "" {
		return name
	}
	kind := strings.TrimPrefix(key, "widget:")
	for _, item := range widgetKindLabels {
		if item.kind == kind {
			return item.label
		}
	}
	return key
}
func overviewLayout(keys []string, x, y, w float32) []overviewRect {
	columns := overviewColumns(w)
	unit := (w - float32(columns-1)*16) / float32(columns)
	col := 0
	rowY := y
	out := []overviewRect{}
	for _, key := range keys {
		span := 4
		if key == "music" || key == "widget:calendar" {
			span = 3
		}
		if key == "clock" || key == "stats" || key == "notifications" || key == "widgets" || key == "widget:system" || key == "widget:screenTime" {
			span = 6
		}
		if columns == 8 {
			span = 4
		}
		if col+span > columns {
			col = 0
			rowY += overviewCardHeight + 16
		}
		out = append(out, overviewRect{key, x + float32(col)*(unit+16), rowY, float32(span)*unit + float32(span-1)*16, overviewCardHeight})
		col += span
	}
	return out
}

func overviewColumns(w float32) int {
	if w < 840 {
		return 8
	}
	return 12
}
func (a *App) moveOverview(from, to string) {
	cards := a.overview.config.Cards
	fi, ti := -1, -1
	for i, key := range cards {
		if key == from {
			fi = i
		}
		if key == to {
			ti = i
		}
	}
	if fi < 0 || ti < 0 || fi == ti {
		return
	}
	card := cards[fi]
	cards = append(cards[:fi], cards[fi+1:]...)
	cards = append(cards, "")
	copy(cards[ti+1:], cards[ti:])
	cards[ti] = card
	a.overview.config.Cards = cards
	a.saveOverview()
}
func (a *App) overviewPage(c *ui.Context, x, y, w float32) float32 {
	p, s := &a.Palette, &a.Palette.Scheme
	if a.overview.edit && c.Shortcut(0, ui.KeyEscape) {
		a.cancelOverviewGesture()
		a.overview.edit = false
	}
	selected := 0
	if a.overview.config.View == "scroll" {
		selected = 1
	}
	if a.overview.config.ScrollPlugin {
		Segmented(c, p, x, y, 224, []string{"卡片视图", "滚动视图"}, selected, func(index int) {
			a.setOverviewView([]string{"cards", "scroll"}[index])
		})
	}
	if selected == 1 {
		pages := a.scrollPages()
		Label(c, "滚轮切换 · "+itoa(len(pages))+" 页", x+242, y+12, 12, s.OnSurfaceVariant, false, w-480)
		Button(c, p, "自定义滚动页", x+w-208, y+2, 120, 36, "自定义页面", "add", true, true, func() { a.openOverviewCatalog("scroll") })
		IconButton(c, p, "上一张概览卡片", x+w-80, y+2, 36, 36, "arrow_back", false, len(pages) > 1, func() { a.StepOverview(-1) })
		IconButton(c, p, "下一张概览卡片", x+w-36, y+2, 36, 36, "arrow_forward", false, len(pages) > 1, func() { a.StepOverview(1) })
		ring := ui.Box(c).Absolute().Left(x).Top(y + 56).Width(w).Height(float32(clipH) - y - 56).Clip()
		h := float32(deckCardHeight(clipH + float64(ContentTop) + 24))
		ring.HandleInput(func(ev ui.InputEvent) bool { return a.deckScroll(ev, h) })
		a.deckGesture(c, ring, len(pages), h)
		ring.Children(func() { a.homePage(c, 0, 64, w, float32(clipH)-y-56) })
		return y + float32(clipH) - 20
	}
	label := "调整排列"
	if a.overview.edit {
		label = "完成排列"
	}
	Button(c, p, "调整概览排列", x+w-224, y+2, 112, 36, label, "", true, true, func() { a.cancelOverviewGesture(); a.overview.edit = !a.overview.edit })
	Button(c, p, "自定义概览组件", x+w-104, y+2, 104, 36, "自定义", "add", true, true, func() { a.openOverviewCatalog("cards") })
	a.overview.resizeOrigin = x
	rects := overviewLayoutSized(a.overview.config.Cards, a.overview.config.Sizes, x, y+56, w)
	bottom := y + 56
	for _, rect := range rects {
		a.renderOverviewCard(c, rect)
		bottom = max(bottom, rect.y+rect.h)
	}
	for _, rect := range rects {
		a.overviewHandle(c, rect, rects)
		a.overviewResizeHandle(c, rect, w)
	}
	if a.overview.dragging != "" {
		for _, rect := range rects {
			if rect.key == a.overview.target {
				ui.Box(c).Absolute().Left(rect.x).Top(rect.y).Width(rect.w).Height(rect.h).IgnoreInput(true).
					Draw(func(pc *ui.Painter, r ui.Rect) {
						path := continuousCardPath(ui.Rect{X: r.X + 1, Y: r.Y + 1, W: r.W - 2, H: r.H - 2}, CardRadius-1)
						pc.FillPath(path, AlphaCol(s.Primary, .04))
						pc.StrokePath(path, 1, AlphaCol(s.Primary, .5))
					})
			}
		}
		ghost := ui.Box(c).Absolute().Left(a.overview.pointerX - 90).Top(a.overview.pointerY - 64).Width(180).Height(48).IgnoreInput(true)
		overlayStyle(&ghost, p)
		ghost.Children(func() {
			Icon(c, "open_with", 16, 15, 18, s.OnSurfaceVariant)
			Label(c, overviewName(a.overview.dragging), 46, 14, 14, s.OnSurface, true, 120)
		})
	}
	return bottom
}
func (a *App) overviewHandle(c *ui.Context, r overviewRect, rects []overviewRect) {
	st := &a.overview
	s := &a.Palette.Scheme
	height := float32(44)
	if st.edit {
		height = r.h
	}
	handle := ui.ButtonBase(c).Key("move-" + r.key).Absolute().Left(r.x).Top(r.y).Width(r.w - 60).Height(height).
		Radius(CardRadius).Label("长按排列" + overviewName(r.key)).Cursor(ui.CursorPointer)
	handle.HandleInput(func(ev ui.InputEvent) bool {
		if ev.Kind == ui.InputPointerUp && ev.Canceled && st.pressed == r.key {
			st.pressed = ""
			st.dragging = ""
			st.target = ""
		}
		return false
	})
	btn, held := handle.PressedButton()
	held = held && btn == 0
	px, py, _ := handle.PointerPosition()
	px += r.x
	py += r.y
	if held && st.resizing == "" {
		if st.pressed == "" {
			st.pressed = r.key
			st.pressedAt = c.Now()
			st.startX = px
			st.startY = py
		}
		if st.pressed == r.key {
			if st.dragging == "" && (st.edit || c.Now().Sub(st.pressedAt) >= 450*time.Millisecond) {
				if st.edit || math.Hypot(float64(px-st.startX), float64(py-st.startY)) < 8 {
					st.edit = true
					st.dragging = r.key
					st.target = r.key
				}
			}
			if st.dragging == r.key {
				st.pointerX, st.pointerY = px, py
				distance := math.MaxFloat64
				for _, target := range rects {
					dx := float64(px - (target.x + target.w/2))
					dy := float64(py - (target.y + target.h/2))
					if d := dx*dx + dy*dy; d < distance {
						distance = d
						st.target = target.key
					}
				}
			}
			c.AnimationFrame()
		}
	} else if st.pressed == r.key {
		if st.dragging == r.key {
			a.moveOverview(r.key, st.target)
		}
		st.pressed, st.dragging, st.target = "", "", ""
	}
	opacity := float32(0)
	if st.edit {
		opacity = 1
	}
	icon := Icon(c, "open_with", r.x+r.w-40, r.y+18, 16, s.OnSurfaceVariant)
	icon.Opacity(opacity).IgnoreInput(true)
}
func (a *App) overviewClock(c *ui.Context, r overviewRect) {
	s := &a.Palette.Scheme
	a.cardHeader(c, r, "时钟与日历")
	now := time.Now()
	c.After(time.Second)
	Label(c, "24 小时", r.x+r.w-76, r.y+20, 10, s.OnSurfaceVariant, false, 56)
	Label(c, now.Format("15:04"), r.x+20, r.y+51, 50, s.OnSurface, true, r.w-40)
	weekday := []string{"星期日", "星期一", "星期二", "星期三", "星期四", "星期五", "星期六"}[now.Weekday()]
	Label(c, now.Format("2006年1月2日")+" · "+weekday, r.x+22, r.y+116, 12, s.OnSurfaceVariant, false, r.w-44)
	start := now.AddDate(0, 0, -(int(now.Weekday())+6)%7)
	cell := (r.w - 32) / 7
	for i, name := range []string{"一", "二", "三", "四", "五", "六", "日"} {
		date := start.AddDate(0, 0, i)
		xx := r.x + 16 + float32(i)*cell
		fg := s.OnSurface
		if date.Day() == now.Day() {
			ui.Box(c).Absolute().Left(xx + 3).Top(r.y + r.h - 81).Width(cell - 6).Height(60).Radius(14).Background(Col(mix(s.Container, s.Primary, .1))).IgnoreInput(true)
			fg = s.Primary
		}
		CenteredText(c, name, xx, r.y+r.h-74, cell, 18, 10, s.OnSurfaceVariant, false)
		CenteredText(c, itoa(date.Day()), xx, r.y+r.h-50, cell, 24, 15, fg, true)
	}
}

func (a *App) overviewExtra(c *ui.Context, r overviewRect) {
	if r.key == "notifications" && r.w >= 620 && len(a.historyEvents()) == 0 {
		a.overviewEmpty(c, r, true)
		return
	}
	if r.key == "notifications" && expandedOverview(r) {
		a.expandedNotifications(c, r)
		return
	}
	p, s := &a.Palette, &a.Palette.Scheme
	x, y, w := r.x, r.y, r.w
	a.cardHeader(c, r, overviewName(r.key))
	switch r.key {
	case "stats":
		labels := []string{"待办事项", "进行中计时", "桌面组件", "通知记录"}
		history, _ := a.State["notificationHistory"].([]any)
		values := []int{a.pendingTodos(), a.runningTimers(), a.widgetCount(), len(history)}
		tileW := (w - 52) / 2
		for i, label := range labels {
			xx, yy := x+20+float32(i%2)*(tileW+12), y+54+float32(i/2)*82
			Label(c, itoa(values[i]), xx, yy+4, 32, s.OnSurface, true, tileW-12)
			Label(c, label, xx+2, yy+48, 11, s.OnSurfaceVariant, false, tileW-12)
		}
	case "notifications":
		events := a.historyEvents()
		cardBadge(c, p, itoa(len(events))+" 条", x+w-80, y+14, 60, false)
		if len(events) == 0 {
			Icon(c, "volume_up", x+20, y+64, 30, s.OnSurfaceVariant)
			Label(c, "暂时没有通知", x+66, y+65, 16, s.OnSurface, true, w-86)
			Label(c, "新收到的通知与到期提醒会显示在这里", x+20, y+115, 12, s.OnSurfaceVariant, false, w-40)
		}
		for i, event := range events {
			if i == 2 {
				break
			}
			top := y + 54 + float32(i)*63
			Label(c, event.Title, x+20, top, 14, s.OnSurface, true, w-104)
			Label(c, event.Clock, x+w-70, top+2, 10, s.OnSurfaceVariant, false, 50)
			Label(c, event.AppName+"  "+event.Body, x+20, top+28, 11, s.OnSurfaceVariant, false, w-40)
			if i == 0 {
				ui.Box(c).Absolute().Left(x + 20).Top(top + 53).Width(w - 40).Height(1).Background(Col(s.OutlineVariant)).IgnoreInput(true)
			}
		}
		widgetTextAction(c, p, "概览全部通知", "查看全部通知", x+20, y+r.h-44, w-40, func() { a.navigate(PageNotifications) })
	case "widgets":
		cardBadge(c, p, itoa(a.widgetCount())+" / 16", x+w-80, y+14, 60, false)
		Label(c, "添加常用组件到桌面", x+20, y+55, 12, s.OnSurfaceVariant, false, w-40)
		tw := (w - 56) / 3
		for i, item := range []struct{ kind, title, icon string }{{"clock", "时钟", "timer"}, {"music", "音乐", "music_note"}, {"todos", "待办", "checklist"}} {
			xx := x + 20 + float32(i)*(tw+8)
			tile := ui.ButtonBase(c).Key("quick-widget-" + item.kind).Absolute().Left(xx).Top(y + 84).Width(tw).Height(82).Radius(14).Background(Col(s.High)).Label("添加" + item.title + "桌面组件").Cursor(ui.CursorPointer)
			enabled := a.Connected && a.widgetCount() < 16
			tile.Disabled(!enabled)
			hoverLayer(&tile, Col(s.High), s.OnSurface, enabled)
			if enabled && tile.Clicked() {
				a.command("widgetAdd", map[string]any{"kind": item.kind, "size": "medium", "style": "original"})
			}
			tile.Children(func() {
				Icon(c, item.icon, (tw-24)/2, 13, 24, s.OnSurfaceVariant)
				CenteredText(c, item.title, 4, 47, tw-8, 22, 12, s.OnSurface, true)
			})
		}
		Button(c, p, "概览管理桌面组件", x+20, y+r.h-46, w-40, 32, "管理组件", "arrow_forward", true, true, func() { a.navigate(PageWidgets) })
	default:
		a.overviewWidget(c, r)
	}
}

// The catalog is a modal side sheet; it preserves the overview's scroll position.
func (a *App) overviewManager(c *ui.Context) {
	if !a.overview.manage {
		return
	}
	p, s := &a.Palette, &a.Palette.Scheme
	w, h := c.Size()
	dw := float32(520)
	if w < 1000 {
		dw = 480
	}
	open := a.overview.manage
	ui.DialogBase(c, &open, func(backdrop, panel ui.Element) {
		backdrop.Background(AlphaCol(0, .16))
		panel.Absolute().Left(w-dw).Top(0).Width(dw).Height(h).Radius(0).Background(Col(s.Surface)).
			Shadow(-4, 0, 18, 0, AlphaCol(0, .08)).Label("概览组件预览栏")
		panel.Children(func() {
			ui.Box(c).Absolute().Left(0).Top(0).Width(dw).Height(104).Background(Col(s.Container))
			title, detail := "自定义概览", "点选预览添加或移除，长按概览卡片标题排列"
			if a.overview.catalogMode == "scroll" {
				title, detail = "自定义滚动页", "点选添加或移除，下方箭头调整页面顺序"
			}
			if a.overview.catalogMode == "information" {
				title, detail = "自定义信息组件", "点选添加或移除，下方箭头调整组件顺序"
			}
			Label(c, title, 24, 23, 20, s.OnSurface, true, dw-90)
			Label(c, detail, 24, 61, 12, s.OnSurfaceVariant, false, dw-48)
			IconButton(c, p, "关闭组件预览栏", dw-58, 19, 34, 34, "close", false, true, func() { open = false })
			viewH := h - 186
			catalog := ui.Scroll(c).Scrollbars(false).Absolute().Left(0).Top(104).Width(dw).Height(viewH).
				TrackScroll(&a.overview.catalogScroll).Label("滚动组件预览")
			catalog.Children(func() {
				body := ui.Box(c).Width(dw).Height(a.overview.catalogHeight)
				body.Children(func() {
					y := float32(22)
					for _, wide := range []bool{true, false} {
						title := "方形卡片"
						if wide {
							title = "横向卡片"
						}
						Label(c, title, 24, y, 13, s.OnSurfaceVariant, true, dw-48)
						y += 30
						keys := []string{}
						for _, key := range a.catalogIDs() {
							if overviewWide(key) == wide {
								keys = append(keys, key)
							}
						}
						cw := (dw - 60) / 2
						ph := cw * overviewCardHeight / 432
						if !wide {
							ph = cw * overviewCardHeight / 282
						}
						ch := ph + 88
						for i, key := range keys {
							top := y + float32(i/2)*(ch+14)
							if top+ch < a.overview.catalogScroll.Y || top > a.overview.catalogScroll.Y+viewH {
								continue
							}
							a.overviewCatalogItem(c, key, 24+float32(i%2)*(cw+12), top, cw, ph)
						}
						y += float32((len(keys)+1)/2)*(ch+14) + 18
					}
					if y != a.overview.catalogHeight {
						a.overview.catalogHeight = y
						c.AnimationFrame()
					}
				})
			})
			ui.Box(c).Absolute().Left(0).Top(h - 82).Width(dw).Height(82).Background(Col(s.Container))
			Label(c, "已添加 "+itoa(len(a.catalogCards()))+" 个组件", 24, h-69, 12, s.OnSurfaceVariant, false, dw-48)
			Button(c, p, "恢复默认概览", 24, h-46, 118, 32, "恢复默认", "restart_alt", true, true, a.resetCatalog)
			Button(c, p, "完成自定义概览", dw-116, h-57, 92, 36, "完成", "", false, true, func() { open = false })
		})
	})
	a.overview.manage = open
}

func overviewWide(key string) bool {
	switch key {
	case "music", "clock", "stats", "notifications", "widgets", "information":
		return true
	}
	return false
}

func (a *App) toggleOverviewCard(key string) {
	keys := append([]string{}, a.catalogCards()...)
	for i, current := range keys {
		if current == key {
			if len(keys) == 1 {
				a.pushToast("至少保留一个项目")
				return
			}
			a.setCatalogCards(append(keys[:i], keys[i+1:]...))
			return
		}
	}
	a.setCatalogCards(append(keys, key))
}

func (a *App) overviewCatalogItem(c *ui.Context, key string, x, y, w, ph float32) {
	s := &a.Palette.Scheme
	selected := false
	for _, current := range a.catalogCards() {
		if current == key {
			selected = true
			break
		}
	}
	item := ui.ButtonBase(c).Key("catalog-" + key).Absolute().Left(x).Top(y).Width(w).Height(ph + 88).
		Background(Col(s.Container)).Radius(16).Label("显示" + overviewName(key)).Role(ui.RoleCheckBox).Checked(selected).Cursor(ui.CursorPointer)
	if selected {
		item.Border(1, AlphaCol(s.Primary, .55))
	} else {
		item.Border(1, AlphaCol(s.Outline, .55))
	}
	hoverLayer(&item, Col(s.Container), s.OnSurface, true)
	if item.Clicked() {
		a.toggleOverviewCard(key)
	}
	item.Children(func() {
		preview := ui.Box(c).Absolute().Left(8).Top(8).Width(w - 16).Height(ph - 8).Radius(12).Clip().IgnoreInput(true)
		preview.Children(func() {
			baseW := float32(282)
			if overviewWide(key) {
				baseW = 432
			}
			scale := (w - 16) / baseW
			ui.Box(c).Absolute().Left(0).Top(0).Width(baseW).Height(overviewCardHeight).TransformOrigin(0, 0).Scale(scale, scale).Children(func() {
				r := overviewRect{key, 0, 0, baseW, overviewCardHeight}
				a.renderOverviewCard(c, r)
			})
		})
		Label(c, overviewName(key), 14, ph+12, 13, s.OnSurface, true, w-52)
		icon, fg := "add", s.OnSurfaceVariant
		if selected {
			icon, fg = "check", s.Primary
		}
		Icon(c, icon, w-34, ph+13, 16, fg)
		if selected && a.overview.catalogMode != "" && a.overview.catalogMode != "cards" {
			index, total := 0, len(a.catalogCards())
			for i, current := range a.catalogCards() {
				if current == key {
					index = i
					break
				}
			}
			Label(c, itoa(index+1)+" / "+itoa(total), 14, ph+49, 10, s.OnSurfaceVariant, false, w-100)
			IconButton(c, &a.Palette, "前移"+overviewName(key), w-78, ph+42, 28, 28, "arrow_back", false, index > 0, func() { a.moveCatalogCard(key, -1) })
			IconButton(c, &a.Palette, "后移"+overviewName(key), w-44, ph+42, 28, 28, "arrow_forward", false, index < total-1, func() { a.moveCatalogCard(key, 1) })
		}
	})
}
