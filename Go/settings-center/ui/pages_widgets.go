// The widgets page (desktop_cards.rs gallery): per-kind size chips and style cards
// that add widgets through the island, plus the list of desktop widgets. Preview
// interiors render each kind's real content at its 1:1 desktop size (desktop_cards.rs
// card + widget_extras.rs render), scaled down by the card container exactly like
// the Rust gallery append transform. Original uses the live palette; the four other
// styles use fixed schemes approximating the Rust theme modules.
package ui

import (
	"fmt"
	"math"
	"sort"
	"time"

	"dotisle/settings/themes"
	"github.com/egoist/mygo/ui"
)

// Kind 值为 serde camelCase（desktop_widgets.rs rename_all = "camelCase"）。
var widgetKindLabels = []struct {
	kind, label string
}{
	{"clock", "时钟"}, {"calendar", "日历"}, {"todos", "待办"}, {"focus", "专注计时"},
	{"music", "音乐"}, {"system", "系统状态"}, {"note", "便签"}, {"anniversary", "纪念日倒数"},
	{"weather", "天气"}, {"today", "今日概览"}, {"battery", "电量"}, {"storage", "存储空间"},
	{"worldClock", "世界时钟"}, {"launcher", "快捷启动"}, {"focusStats", "专注统计"}, {"wireless", "无线设备电量"}, {"schedule", "日程倒计时"}, {"screenTime", "用机时长"},
}

var widgetStyleLabels = [5]string{"默认", "Miuix", "终端", "像素", "画廊"}

var widgetSizes = []struct {
	key      string
	label    string
	wPx, hPx float64
}{
	{"small", "小", 176, 176},
	{"medium", "中", 352, 176},
	{"large", "大", 352, 352},
}

// widgetSizeOf reads the per-kind size choice (model.widget_sizes); default medium.
func (a *App) widgetSizeOf(kind string) string {
	if size := a.widgetSizes[kind]; size != "" {
		return size
	}
	if v, ok := a.State["desktopWidgetSizes"].(map[string]any); ok {
		if s, ok := v[kind].(string); ok {
			return s
		}
	}
	return "medium"
}

// widgetsPage mirrors desktop_cards.rs gallery at the same section rhythm.
func (a *App) widgetsPage(c *ui.Context, x, y, w float32) float32 {
	return a.appleWidgetsGallery(c, x, y, w)
}

// setWidgetSize persists the per-kind size choice through the island (WidgetSize).
func (a *App) setWidgetSize(kind, size string) {
	if a.widgetSizes == nil {
		a.widgetSizes = map[string]string{}
	}
	a.widgetSizes[kind] = size
}

// widgetStyleCard draws one style card: live widget preview, style label, add button.
func (a *App) widgetStyleCard(c *ui.Context, p *Palette, x, y, cw float32, kind, size string, style int) {
	s := &p.Scheme
	Panel(c, p, x, y, cw, 250, s.Container)
	dims := widgetDims(size)
	pw, ph := dims[0], dims[1]
	scale := float64(cw-32) / pw
	if 150/ph < scale {
		scale = 150 / ph
	}
	a.widgetPreview(c, p, kind, size, style,
		x+(cw-float32(pw*scale))/2, y+12, float32(pw*scale), float32(ph*scale))
	Label(c, widgetStyleLabels[style], x+18, y+172, 15, s.OnSurface, true, cw-36)
	styleKey := widgetStyleKeys[style]
	Button(c, p, "add-"+kind+styleKey, x+16, y+206, cw-32, 32, "添加到桌面", "add", false,
		a.widgetCount() < 16, func() {
			a.command("widgetAdd", map[string]any{
				"kind": kind, "size": size, "style": styleKey})
		})
}

var widgetStyleKeys = [5]string{"original", "miuix", "terminal", "pixel", "gallery"}

func widgetDims(size string) [2]float64 {
	switch size {
	case "small":
		return [2]float64{176, 176}
	case "large":
		return [2]float64{352, 352}
	}
	return [2]float64{352, 176}
}

// widgetScheme is the fixed presentation of one preview style: color scheme plus
// the corner and type language of the matching Rust theme module.
type widgetScheme struct {
	colors   themes.Scheme
	face     string  // preferred font stack, empty for the app default
	serif    string  // serif face for large display lines (Gallery), empty to skip
	radius   float32 // panel corner radius (widgets.rs panel)
	terminal bool    // square panels without the hairline stroke
	pixel    bool    // fixed-size label ramp (widgets.rs label pixel branch)
	sharp    bool    // square record art and flat fills
}

// widgetStyleScheme resolves one style's scheme: Original uses the live palette,
// the others use the fixed schemes of miuix.rs / terminal.rs / pixel.rs / swiss.rs.
func widgetStyleScheme(style int, p *Palette) *widgetScheme {
	return &widgetScheme{colors: p.Scheme, face: UIFont, radius: 24}
}

// widgetPreview draws one widget's real content into a scaled container at (x, y):
// the interior renders at its 1:1 desktop size (pw×ph from Size::dimensions) and
// the container applies the gallery scale, like the Rust append transform.
func (a *App) widgetPreview(c *ui.Context, p *Palette, kind, size string, styleIndex int, x, y, w, h float32) {
	a.appleDesktopPreview(c, p, kind, size, x, y, w, h)
}

// widgetCardBody ports desktop_cards.rs fn card for the six native cards.
func (a *App) widgetCardBody(c *ui.Context, ws *widgetScheme, kind, size string, ix, iy, iw, ih float32) {
	s := &ws.colors
	bg := s.Container
	if kind == "focus" {
		bg = s.TertiaryContainer
	} else if kind == "music" {
		bg = s.PrimaryContainer
	}
	wPanel(c, ws, ix, iy, iw, ih, bg)
	x, y := ix+16, iy+16
	w := iw - 32
	big := size == "large"
	small := size == "small"
	wLabel(c, ws, kindLabel(kind), x, y, 12, s.OnSurfaceVariant, true, w)
	switch kind {
	case "clock":
		a.widgetClock(c, ws, x, y, w, ix, iy, iw, big, small)
	case "calendar":
		a.widgetCalendar(c, ws, x, y+22, w, ih-54, big)
	case "todos":
		a.widgetTodos(c, ws, x, y, w, ix, iy, ih, big, small)
	case "focus":
		a.widgetFocus(c, ws, x, y, w, ix, iy, iw, ih, big, small)
	case "music":
		a.widgetMusic(c, ws, x, y, w, ix, iy, iw, ih, big, small)
	case "system":
		a.widgetSystem(c, ws, x, y, w, ix, iy, ih, big, small)
	}
}

// widgetClock renders Kind::Clock: the large time, the date line and, on the
// large size, the analog face at 239 below the panel top (desktop_cards.rs).
func (a *App) widgetClock(c *ui.Context, ws *widgetScheme, x, y, w, ix, iy, iw float32, big, small bool) {
	s := &ws.colors
	now := time.Now()
	hour, minute := now.Hour(), now.Minute()
	wLabel(c, ws, twoDigits(hour)+":"+twoDigits(minute), x, y+31,
		boolSize(small, 34, 48), s.OnSurface, true, w)
	month, day := int(now.Month()), now.Day()
	wLabel(c, ws, itoa(month)+"月"+itoa(day)+"日 · "+widgetWeekdayLabel(now.Year(), month, day),
		x, y+boolSize(small, 88, 102), 11, s.OnSurfaceVariant, false, w)
	if !big {
		return
	}
	cx := ix + iw/2
	cy := iy + 239
	ui.Box(c).Absolute().Left(cx - 66).Top(cy - 66).Width(132).Height(132).
		Radius(66).Background(Col(s.High))
	for i := 0; i < 12; i++ {
		deg := float64(i) * 30
		rad := deg * math.Pi / 180
		ui.Box(c).Absolute().
			Left(cx+float32(math.Sin(rad)*59)-1).Top(cy-float32(math.Cos(rad)*59)-3).
			Width(2).Height(6).Radius(1).Background(Col(s.OnSurfaceVariant)).
			TransformOrigin(0.5, 0.5).Rotate(float32(deg))
	}
	// Hour and minute hands, rotated bars pinned at the face center.
	wHand(c, cx, cy, 34, 4, float64(hour%12)*30+float64(minute)*0.5, s.OnSurface)
	wHand(c, cx, cy, 49, 3, float64(minute)*6, s.Primary)
	ui.Box(c).Absolute().Left(cx - 4).Top(cy - 4).Width(8).Height(8).
		Radius(4).Background(Col(s.Primary))
}

// wHand draws one clock hand: a bar standing on the face center, rotated clockwise.
func wHand(c *ui.Context, cx, cy, length, width float32, degrees float64, rgb uint32) {
	ui.Box(c).Absolute().Left(cx-width/2).Top(cy-length).Width(width).Height(length).
		Radius(width/2).Background(Col(rgb)).
		TransformOrigin(0.5, 1).Rotate(float32(degrees))
}

// widgetCalendar renders Kind::Calendar: month header and the day grid with a
// filled pill on today (desktop_cards.rs).
func (a *App) widgetCalendar(c *ui.Context, ws *widgetScheme, x, y, w, h float32, big bool) {
	drawWidgetCalendar(c, ws, x, y, w, h, time.Now(), big)
}

func drawWidgetCalendar(c *ui.Context, ws *widgetScheme, x, y, w, h float32, now time.Time, big bool) {
	s := &ws.colors
	year, month, day := now.Year(), int(now.Month()), now.Day()
	wLabel(c, ws, itoa(year)+" 年 "+itoa(month)+" 月", x, y,
		boolSize(big, 22, 18), s.Primary, true, w)
	cw := w / 7
	top := y + boolSize(big, 45, 32)
	headerH := boolSize(big, 24, 14)
	first, days := widgetFirstWeekday(year, month), widgetDaysInMonth(year, month)
	rows := (first + days + 6) / 7
	ch := (y + h - top - headerH) / float32(rows)
	for i, head := range [7]string{"一", "二", "三", "四", "五", "六", "日"} {
		wCenter(c, ws, head, x+float32(i)*cw, top, cw, headerH, boolSize(big, 12, 10), s.OnSurfaceVariant, false)
	}
	for d := 1; d <= days; d++ {
		n := first + d - 1
		cellX := x + float32(n%7)*cw
		cellY := top + headerH + float32(n/7)*ch
		today := d == day
		if today {
			ui.Box(c).Absolute().Left(cellX + 1).Top(cellY).Width(cw - 2).Height(ch).
				Radius(ch / 2).Background(Col(s.Primary))
		}
		ink, bold := s.OnSurface, false
		if today {
			ink, bold = s.OnPrimary, true
		}
		wCenter(c, ws, itoa(d), cellX, cellY, cw, ch, boolSize(big, 15, 10), ink, bold)
	}
}

// widgetTodos renders Kind::Todos: the pending count, the next rows and the add
// button (desktop_cards.rs; live todo items, "清爽的一天" when empty).
func (a *App) widgetTodos(c *ui.Context, ws *widgetScheme, x, y, w, ix, iy, ih float32, big, small bool) {
	s := &ws.colors
	now, _ := a.todoNow()
	items := []goTodo{}
	for _, t := range a.todoItems() {
		if !todoDone(t, now) {
			items = append(items, t)
		}
	}
	sort.Slice(items, func(i, j int) bool {
		if items[i].Date != items[j].Date {
			return items[i].Date < items[j].Date
		}
		return items[i].Time < items[j].Time
	})
	wLabel(c, ws, itoa(len(items))+" 件小事", x, y+24,
		boolSize(small, 22, 27), s.OnSurface, true, w)
	if len(items) == 0 {
		wLabel(c, ws, "清爽的一天", x, y+71, 14, s.OnSurfaceVariant, false, w)
	}
	rows := 1
	if big {
		rows = 4
	}
	for i := 0; i < len(items) && i < rows; i++ {
		ty := y + 67 + float32(i)*50
		Icon(c, "check", x-2, ty+6, 24, s.OnSurfaceVariant)
		wLabel(c, ws, items[i].Title, x+33, ty+7, 13, s.OnSurface, true, w-37)
	}
	wButton(c, ws, x, iy+ih-44, w, 30, "添加待办", "add", true)
}

// widgetFocus renders Kind::Focus: the timer clock, optional subtitle, primary
// action button and the reset control (desktop_cards.rs).
func (a *App) widgetFocus(c *ui.Context, ws *widgetScheme, x, y, w, ix, iy, iw, ih float32, big, small bool) {
	s := &ws.colors
	timer := a.currentTimer()
	clock := "25:00"
	if timer != nil {
		clock = timerClock(timer)
	}
	wLabel(c, ws, clock, x, y+27, boolSize(small, 32, 43), s.OnSurface, true, w)
	if big {
		// Big cards carry the subtitle line between clock and button.
		subtitle := "25 分钟专注 · 5 分钟休息"
		if timer != nil {
			subtitle = timerSubtitle(timer)
		}
		wLabel(c, ws, subtitle, x, y+111, 13, s.OnSurfaceVariant, false, w)
	}
	label, icon := "开始专注", "play_arrow"
	if timer != nil {
		switch timerPrimaryAction(timer) {
		case "pause":
			label, icon = "暂停", "pause"
		case "resume":
			label, icon = "继续", "play_arrow"
		case "next":
			label, icon = "下一阶段", "skip_next"
		default:
			label = "重新开始"
		}
	}
	bw := w
	if !small {
		bw = w - 44
	}
	wButton(c, ws, x, iy+ih-53, bw, 38, label, icon, false)
	if !small {
		Icon(c, "restart_alt", ix+iw-52+7, iy+ih-55+7, 24, s.OnSurfaceVariant)
	}
}

// widgetMusic renders Kind::Music: cover disc, track line, transport and — on the
// large size — the current lyric line and the progress bar (desktop_cards.rs).
func (a *App) widgetMusic(c *ui.Context, ws *widgetScheme, x, y, w, ix, iy, iw, ih float32, big, small bool) {
	s := &ws.colors
	cover := float32(64)
	if small {
		cover = 42
	}
	// views::record without artwork: flat disc with a music glyph.
	radius := float32(15)
	recordBg := s.SecondaryContainer
	if ws.sharp {
		radius, recordBg = 0, s.High
	} else if ws.face == "MiSans" {
		radius = 16
	}
	ui.Box(c).Absolute().Left(x).Top(y + 27).Width(cover).Height(cover).
		Radius(radius).Background(Col(recordBg))
	glyph := cover * 0.32
	Icon(c, "music_note", x+(cover-glyph)/2, y+27+(cover-glyph)/2, glyph, s.OnSurfaceVariant)
	tx, ty, tw := x, y+73, w
	if !small {
		tx, ty, tw = x+cover+12, y+30, w-cover-12
	}
	wLabel(c, ws, a.trackTitle(), tx, ty, boolSize(small, 13, 17), s.OnPrimaryContainer, true, tw)
	if !small {
		wLabel(c, ws, a.trackArtist(), tx, ty+30, 12, s.OnPrimaryContainer, false, tw)
	}
	if big {
		line := stateString(a.lyricFrame(), "", "currentLine")
		if line == "" {
			line = "把耳朵交给音乐，把此刻留给自己。"
		}
		wLabel(c, ws, line, x, y+125, 16, s.OnPrimaryContainer, false, w)
		fraction, duration := a.mediaFraction()
		a.widgetProgress(c, ws, x, y+185, w, 24, fraction, fraction*duration, duration)
	}
	// Transport: prev / play-pause (filled accent) / next, centered on the card.
	by := iy + ih - 49
	bw, gap := float32(42), float32(18)
	if small {
		bw, gap = 36, 8
	}
	total := 3*bw + 2*gap
	bx := ix + iw/2 - total/2
	Icon(c, "skip_previous", bx+(bw-22)/2, by+(bw-22)/2, 22, s.OnSurface)
	ui.Box(c).Absolute().Left(bx + bw + gap).Top(by).Width(bw).Height(bw).
		Radius(bw / 2).Background(Col(s.Primary))
	playIcon := "play_arrow"
	if a.playing() {
		playIcon = "pause"
	}
	Icon(c, playIcon, bx+bw+gap+(bw-24)/2, by+(bw-24)/2, 24, s.OnPrimary)
	Icon(c, "skip_next", bx+2*(bw+gap)+(bw-22)/2, by+(bw-22)/2, 22, s.OnSurface)
}

// widgetProgress draws the static music progress line with its time labels
// (views.rs progress on the disabled slider look).
func (a *App) widgetProgress(c *ui.Context, ws *widgetScheme, x, y, w, h float32, fraction, pos, duration float64) {
	s := &ws.colors
	mid := y + h/2 - 2.5
	ui.Box(c).Absolute().Left(x).Top(mid).Width(float32(w)).Height(5).Radius(2.5).
		Background(AlphaCol(s.Outline, 0.4))
	ui.Box(c).Absolute().Left(x).Top(mid).Width(w * float32(clamp01(fraction))).Height(5).
		Radius(2.5).Background(AlphaCol(s.Primary, 0.4))
	wLabel(c, ws, widgetFormatTime(pos), x, y+h+2, 11, s.OnSurfaceVariant, false, 80)
	e := ui.Box(c).Absolute().Left(x + w - 90).Top(y + h + 2).Width(90).Justify(ui.End)
	e.Children(func() {
		line := ui.Text(c, widgetFormatTime(duration)).FontSize(11).
			FixedLineHeight(LineHeight(11)).TextColor(Col(s.OnSurfaceVariant)).SingleLine()
		if ws.face != "" {
			line.Font(ws.face)
		}
	})
}

// widgetSystem renders Kind::System: CPU / memory / battery rows with meter bars
// on the large size and the memory footnote (desktop_cards.rs).
func (a *App) widgetSystem(c *ui.Context, ws *widgetScheme, x, y, w, ix, iy, ih float32, big, small bool) {
	s := &ws.colors
	m := a.widgetSystemMetrics()
	entries := []struct {
		title string
		value float64
		has   bool
	}{
		{"CPU", m.cpu, m.hasCPU},
		{"内存", m.memory, m.hasMemory},
		{"电池", m.battery, m.hasBattery},
	}
	count := 3
	if small {
		count = 2
	}
	for i := 0; i < count; i++ {
		entry := entries[i]
		ty := y + 31 + float32(i)*boolSize(big, 78, 34)
		wLabel(c, ws, entry.title, x, ty, 12, s.OnSurfaceVariant, false, 65)
		text := "暂无读数"
		if entry.has {
			text = itoa(int(math.Round(entry.value))) + "%"
		}
		wLabel(c, ws, text, x+boolSize(small, 54, 82), ty,
			boolSize(big, 22, 15), s.OnSurface, true, w-60)
		if big {
			ui.Box(c).Absolute().Left(x).Top(ty + 36).Width(w).Height(7).
				Radius(3).Background(Col(s.High))
			if entry.has {
				fw := float32(math.Max(math.Min(entry.value, 100), 0)) / 100 * w
				ui.Box(c).Absolute().Left(x).Top(ty + 36).Width(fw).Height(7).
					Radius(3).Background(Col(s.Primary))
			}
		}
	}
	if big {
		footer := "等待系统读数"
		if m.hasSizes {
			footer = fmt.Sprintf("内存 %.1f / %.1f GiB", m.usedGiB, m.totalGiB)
		}
		wLabel(c, ws, footer, x, iy+ih-43, 12, s.OnSurfaceVariant, false, w)
	}
}

// Missing telemetry stays unknown; preview defaults must never become live data.
type widgetMetricsSnapshot struct {
	cpu, memory, battery float64
	usedGiB, totalGiB    float64
	hasCPU, hasMemory    bool
	hasBattery, hasSizes bool
	power                bool
}

func (a *App) widgetSystemMetrics() widgetMetricsSnapshot {
	m := widgetMetricsSnapshot{}
	if a.State == nil {
		return m
	}
	raw, ok := a.State["systemMetrics"].(map[string]any)
	if !ok {
		return m
	}
	if v, ok := stateFloat(raw, "cpuPercent"); ok {
		m.cpu, m.hasCPU = v, true
	}
	if v, ok := stateFloat(raw, "memoryPercent"); ok {
		m.memory, m.hasMemory = v, true
	}
	if v, ok := stateFloat(raw, "batteryPercent"); ok {
		m.battery, m.hasBattery = v, true
	}
	used, uok := stateFloat(raw, "memoryUsedGiB")
	total, tok := stateFloat(raw, "memoryTotalGiB")
	m.usedGiB, m.totalGiB, m.hasSizes = used, total, uok && tok
	if v, ok := raw["externalPower"].(bool); ok {
		m.power = v
	}
	return m
}

// widgetExtrasBody ports widget_extras.rs render for the nine information
// widgets: tinted rounded surface with a soft shadow, accent title and the
// Apple-style hierarchy of values.
func (a *App) widgetExtrasBody(c *ui.Context, ws *widgetScheme, kind, size string, ix, iy, iw, ih float32) {
	dark := ws.colors.Surface < 0x808080
	bg, fg, accent := extraColors(kind, dark)
	ui.Box(c).Absolute().Left(ix).Top(iy).Width(iw).Height(ih).Radius(24).
		Background(Col(bg)).
		Shadow(0, 3, 7, 0, ui.RGBA(0, 0, 0, 0.09))
	x, y := ix+16, iy+16
	w := iw - 32
	baseline := y + 24
	bottom := iy + ih
	small := size == "small"
	large := size == "large"
	data := a.widgetPreviewData(kind)
	title := kindLabel(kind)
	switch kind {
	case "weather":
		title = wdataString(data, "city", "北京")
	case "worldClock":
		title = "UTC"
	}
	wLabel(c, ws, title, x, y, 14, accent, true, w)
	switch kind {
	case "note":
		text := []string{"记下一点想法", "点击编辑便签"}
		size := float32(16)
		if small {
			size = 14
		}
		rows := 4
		if large {
			rows = 10
		}
		for i, line := range text {
			if i >= rows {
				break
			}
			wLabel(c, ws, line, x, baseline+float32(i)*size*1.5, size, fg, false, w)
		}
	case "anniversary":
		wLabel(c, ws, "值得期待", x, baseline+8, boolSize(small, 22, 28), fg, true, w)
		wLabel(c, ws, "点击设置一个重要日期", x, baseline+66, 12, accent, false, w)
	case "weather":
		temperature := wdataFloat(data, "temperature", 18)
		code := int(wdataFloat(data, "code", 2))
		wLabel(c, ws, itoa(int(math.Round(temperature)))+"°", x, baseline+6,
			boolSize(small, 44, 52), fg, false, w)
		label := "天气更新中"
		switch {
		case code == 0:
			label = "晴"
		case code >= 1 && code <= 3:
			label = "多云"
		case code == 45 || code == 48:
			label = "有雾"
		case code >= 51 && code <= 67:
			label = "有雨"
		case code >= 71 && code <= 77:
			label = "有雪"
		case code >= 80 && code <= 86:
			label = "阵雨"
		case code >= 95 && code <= 99:
			label = "雷雨"
		}
		wLabel(c, ws, label, x, bottom-38, 12, fg, false, w)
		if large {
			high, hok := stateFloat(data, "high")
			if !hok {
				high = temperature
			}
			low, lok := stateFloat(data, "low")
			if !lok {
				low = temperature
			}
			wLabel(c, ws, "最高 "+itoa(int(math.Round(high)))+"° / 最低 "+
				itoa(int(math.Round(low)))+"°", x, baseline+112, 14, fg, false, w)
			wLabel(c, ws, "Open-Meteo", x, bottom-62, 10, accent, false, w)
		}
		if !small {
			name := "light_mode"
			if int(wdataFloat(data, "day", 1)) == 0 {
				name = "dark_mode"
			}
			Icon(c, name, ix+iw-77, baseline+7, 44, fg)
		}
	case "today":
		now := time.Now()
		wLabel(c, ws, itoa(int(now.Month()))+"月"+itoa(now.Day())+"日", x, baseline,
			boolSize(small, 24, 30), fg, true, w)
		wLabel(c, ws, itoa(a.pendingTodos())+" 件待办", x, baseline+52, 17, fg, true, w)
		wLabel(c, ws, itoa(a.runningTimers())+" 个计时进行中", x, baseline+82, 12, accent, false, w)
		if large {
			row := 0
			for _, t := range a.todoItems() {
				if t.Completed {
					continue
				}
				if row >= 4 {
					break
				}
				wLabel(c, ws, t.Title, x, baseline+132+float32(row)*30, 13, fg, false, w)
				row++
			}
		}
	case "battery":
		m := a.widgetSystemMetrics()
		text, status := "—", "未检测到电池"
		if m.hasBattery {
			text = itoa(int(m.battery)) + "%"
			if m.power {
				status = "已连接电源"
			} else {
				status = "正在使用电池"
			}
		}
		wLabel(c, ws, text, x, baseline, boolSize(small, 44, 52), fg, true, w)
		wMeter(c, x, bottom-52, w-4, 13, float32(m.battery/100), accent, mix(bg, accent, 0.18))
		wLabel(c, ws, status, x, bottom-31, 11, accent, false, w)
	case "storage":
		free := wdataFloat(data, "free", 128000000000)
		total := wdataFloat(data, "total", 512000000000)
		wLabel(c, ws, itoa(int(math.Round(free/1e9)))+" GB", x, baseline,
			boolSize(small, 30, 40), fg, true, w)
		wLabel(c, ws, "C: 可用空间", x, baseline+53, 12, accent, false, w)
		wMeter(c, x, bottom-39, w, 8, float32(1-free/math.Max(total, 1)), accent, mix(bg, accent, 0.16))
		if large {
			wLabel(c, ws, "总容量 "+itoa(int(math.Round(total/1e9)))+" GB",
				x, baseline+110, 15, fg, false, w)
		}
	case "worldClock":
		wLabel(c, ws, wdataString(data, "clock", "09:41"), x, baseline+4,
			boolSize(small, 36, 56), fg, true, w)
		wLabel(c, ws, wdataString(data, "date", "11月12日"), x, bottom-35, 12, accent, false, w)
		if large {
			name := "dark_mode"
			if hour := wdataFloat(data, "hour", 9); hour >= 6 && hour < 18 {
				name = "light_mode"
			}
			Icon(c, name, x, baseline+108, 52, accent)
		}
	case "launcher":
		wLabel(c, ws, "添加入口", x, baseline+8, boolSize(small, 20, 28), fg, true, w)
		wLabel(c, ws, "点击选择常用程序或网页", x, bottom-32, 11, accent, false, w)
	case "focusStats":
		todayMs, week := a.widgetFocusStats()
		wLabel(c, ws, itoa(int(todayMs/60000)), x, baseline, boolSize(small, 39, 48), fg, true, w)
		wLabel(c, ws, "分钟 · 今日专注", x, baseline+58, 11, accent, false, w)
		if large {
			max := 1.0
			for _, v := range week {
				if v > max {
					max = v
				}
			}
			cw := w / 7
			for i := 0; i < 7; i++ {
				amount := 0.0
				if i < len(week) {
					amount = week[i] / max
				}
				bh := float32(math.Max(amount*98, 3))
				ui.Box(c).Absolute().Left(x + float32(i)*cw).Top(bottom - 51 - bh).
					Width(cw - 8).Height(bh).Radius(5).Background(Col(accent))
			}
			wLabel(c, ws, "近七天 · 番茄专注与流程计时", x, bottom-32, 11, accent, false, w)
		}
	}
}

// extraColors ports widget_extras.rs colors(kind, dark): the per-kind background,
// foreground and accent of the information widgets.
func extraColors(kind string, dark bool) (uint32, uint32, uint32) {
	if dark {
		accent := uint32(0xA7BFFF)
		switch kind {
		case "note":
			accent = 0xF4D475
		case "battery":
			accent = 0x77DDA8
		case "weather":
			accent = 0x94C9FF
		}
		return 0x20232B, 0xF4F5F8, accent
	}
	switch kind {
	case "note":
		return 0xFFF1BC, 0x40331E, 0x9D7119
	case "anniversary":
		return 0xF1E9FC, 0x352C49, 0x8861BD
	case "weather":
		return 0x336CB3, 0xFFFFFF, 0xDAEBFF
	case "battery":
		return 0xEDF8F1, 0x193A2A, 0x268E58
	case "focusStats":
		return 0xF0EDFC, 0x362C50, 0x8964D3
	}
	return 0xFAFBFD, 0x262A33, 0x5786C9
}

// widgetPreviewData returns the island's live widgetData for the first desktop
// widget of this kind, so previews show real readings when one exists; each
// field falls back to the Rust demo fixture (widget_extras.rs verify).
func (a *App) widgetPreviewData(kind string) map[string]any {
	if a.State == nil {
		return nil
	}
	items, _ := a.State["desktopWidgets"].([]any)
	root, _ := a.State["widgetData"].(map[string]any)
	for _, item := range items {
		widget, ok := item.(map[string]any)
		if !ok || stateString(widget, "", "kind") != kind {
			continue
		}
		id, _ := widget["id"].(string)
		if d, ok := root[id].(map[string]any); ok {
			return d
		}
	}
	return nil
}

// widgetFocusStats reads a.State["widgetData"]["focusStats"] with the Rust demo
// fixture (75 minutes, one populated week) as fallback.
func (a *App) widgetFocusStats() (float64, []float64) {
	todayMs := 4500000.0
	week := []float64{1200000, 2400000, 1800000, 3600000, 2700000, 4200000, 4500000}
	if a.State == nil {
		return todayMs, week
	}
	root, _ := a.State["widgetData"].(map[string]any)
	stats, _ := root["focusStats"].(map[string]any)
	if stats == nil {
		return todayMs, week
	}
	if v, ok := stateFloat(stats, "todayMs"); ok {
		todayMs = v
	}
	if arr, ok := stats["week"].([]any); ok {
		parsed := []float64{}
		for _, v := range arr {
			if f, ok := v.(float64); ok {
				parsed = append(parsed, f)
			}
		}
		if len(parsed) > 0 {
			week = parsed
		}
	}
	return todayMs, week
}

// runningTimers counts timers currently running (widget_extras.rs Today).
func (a *App) runningTimers() int {
	if a.State == nil {
		return 0
	}
	n := 0
	raw, _ := a.State["timers"].([]any)
	for _, t := range raw {
		if timer, ok := t.(map[string]any); ok && stateBool(timer, "running") {
			n++
		}
	}
	return n
}

// wdataFloat reads one numeric widget data field, falling back to the demo value.
func wdataFloat(data map[string]any, key string, demo float64) float64 {
	if v, ok := stateFloat(data, key); ok {
		return v
	}
	return demo
}

// wdataString reads one string widget data field, falling back to the demo value.
func wdataString(data map[string]any, key, demo string) string {
	if s := stateString(data, "", key); s != "" {
		return s
	}
	return demo
}

// widgetDaysInMonth ports dotisle_domain Date::days.
func widgetDaysInMonth(year, month int) int {
	switch month {
	case 4, 6, 9, 11:
		return 30
	case 2:
		if year%4 == 0 && (year%100 != 0 || year%400 == 0) {
			return 29
		}
		return 28
	}
	return 31
}

// widgetFirstWeekday ports Date::first_weekday: Gregorian calendar, Monday first.
func widgetFirstWeekday(year, month int) int {
	y := year - 1
	days := 365*y + y/4 - y/100 + y/400
	for m := 1; m < month; m++ {
		days += widgetDaysInMonth(year, m)
	}
	return days % 7
}

// widgetWeekdayLabel ports Date::weekday_label.
func widgetWeekdayLabel(year, month, day int) string {
	return [7]string{"星期一", "星期二", "星期三", "星期四", "星期五", "星期六", "星期日"}[(widgetFirstWeekday(year, month)+day-1)%7]
}

// widgetFormatTime formats seconds as m:ss (h:mm:ss past an hour), like the
// timers engine format_time.
func widgetFormatTime(seconds float64) string {
	s := int(math.Floor(seconds))
	if s < 0 {
		s = 0
	}
	h, m, sec := s/3600, (s/60)%60, s%60
	if h > 0 {
		return itoa(h) + ":" + twoDigits(m) + ":" + twoDigits(sec)
	}
	return itoa(m) + ":" + twoDigits(sec)
}

// boolSize picks one of two sizes (the Rust small/big branches).
func boolSize(flag bool, when, otherwise float32) float32 {
	if flag {
		return when
	}
	return otherwise
}

// wPanel draws the widget surface with the style's corner language and the
// on_surface hairline stroke (widgets.rs panel; Terminal skips the stroke).
func wPanel(c *ui.Context, ws *widgetScheme, x, y, w, h float32, rgb uint32) {
	e := ui.Box(c).Absolute().Left(x).Top(y).Width(w).Height(h).
		Radius(ws.radius).Background(Col(rgb))
	if !ws.terminal {
		alpha := 0.045
		if ws.colors.Surface < 0x808080 {
			alpha = 0.075
		}
		e.Border(1, AlphaCol(ws.colors.OnSurface, alpha))
	}
}

// wLabel draws one single-line preview label with the style's type treatment
// (widgets.rs label: the pixel size ramp and the Gallery serif display face).
func wLabel(c *ui.Context, ws *widgetScheme, text string, x, y, size float32, rgb uint32, bold bool, maxw float32) {
	size = rampSize(ws, size)
	face := ws.face
	if ws.serif != "" && size >= 20 {
		face = ws.serif
	}
	e := ui.Box(c).Absolute().Left(x).Top(y).Width(maxw)
	e.Children(func() {
		line := ui.Text(c, text).FontSize(size).FixedLineHeight(LineHeight(size)).
			TextColor(Col(rgb)).SingleLine()
		if face != "" {
			line.Font(face)
		}
		if bold {
			line.FontWeight(600)
		}
	})
}

// wCenter centers one line in a rect, like the calendar cells (text line_fixed).
func wCenter(c *ui.Context, ws *widgetScheme, text string, x, y, w, h, size float32, rgb uint32, bold bool) {
	size = rampSize(ws, size)
	face := ws.face
	if ws.serif != "" && size >= 20 {
		face = ws.serif
	}
	e := ui.Box(c).Absolute().Left(x).Top(y).Width(w).Height(h).
		Justify(ui.Center).AlignItems(ui.Center)
	e.Children(func() {
		line := ui.Text(c, text).FontSize(size).FixedLineHeight(LineHeight(size)).
			TextColor(Col(rgb)).SingleLine()
		if face != "" {
			line.Font(face)
		}
		if bold {
			line.FontWeight(600)
		}
	})
}

// rampSize applies the pixel style's fixed label ramp (widgets.rs label pixel branch).
func rampSize(ws *widgetScheme, size float32) float32 {
	if !ws.pixel {
		return size
	}
	if size >= 28 {
		return 36
	}
	if size >= 20 {
		return 24
	}
	return 12
}

// wButton draws a static preview pill button (widgets.rs button look: filled
// primary or tonal container, 14px semibold label with an optional leading icon).
func wButton(c *ui.Context, ws *widgetScheme, x, y, w, h float32, text, icon string, tonal bool) {
	bg, fg := ws.colors.Primary, ws.colors.OnPrimary
	if tonal {
		bg, fg = ws.colors.SecondaryContainer, ws.colors.OnSecondaryContainer
	}
	e := ui.Row(c).Absolute().Left(x).Top(y).Width(w).Height(h).
		Justify(ui.Center).AlignItems(ui.Center).Gap(8).
		Radius(h / 2).Background(Col(bg))
	e.Children(func() {
		if icon != "" {
			if svg := Symbol(icon); svg != nil {
				ui.Icon(c, svg).FontSize(20).TextColor(Col(fg))
			}
		}
		line := ui.Text(c, text).Font(textFamily(text)).FontSize(14).FixedLineHeight(LineHeight(14)).
			TextColor(Col(fg)).FontWeight(600).SingleLine()
		if ws.face != "" {
			line.Font(ws.face)
		}
	})
}

// wMeter draws the rounded progress meter (widget_extras.rs meter).
func wMeter(c *ui.Context, x, y, w, h, progress float32, accent, track uint32) {
	ui.Box(c).Absolute().Left(x).Top(y).Width(w).Height(h).
		Radius(h / 2).Background(Col(track))
	if progress > 0 {
		ui.Box(c).Absolute().Left(x).Top(y).Width(w * float32(clamp01(float64(progress)))).Height(h).
			Radius(h / 2).Background(Col(accent))
	}
}

// widgetRows lists the desktop widgets with size chips and remove (gallery tail).
func (a *App) widgetRows(c *ui.Context, p *Palette, x, y, w float32) float32 {
	s := &p.Scheme
	raw, _ := a.State["desktopWidgets"].([]any)
	for _, item := range raw {
		widget, ok := item.(map[string]any)
		if !ok {
			continue
		}
		id, _ := widget["id"].(string)
		kind := stateString(widget, "", "kind")
		Panel(c, p, x, y, w, 76, s.Container)
		Label(c, kindLabel(kind), x+22, y+16, 17, s.OnSurface, true, w-430)
		Label(c, "拖动调整位置 · 右键编辑、调整尺寸或移除", x+22, y+46, 11, s.OnSurfaceVariant, false, w-430)
		IconButton(c, p, "编辑桌面组件 "+id, x+w-392, y+16, 40, 44, "edit", false, true, func() { a.openWidgetEditor(id) })
		for i, sz := range widgetSizes {
			current := stateString(widget, "medium", "size")
			Chip(c, p, "wr-"+id+sz.key, x+w-340+float32(i)*76, y+18, 68, 38, sz.label,
				current == sz.key, func() {
					a.command("widgetUpdate", map[string]any{"id": id, "size": sz.key})
				})
		}
		IconButton(c, p, "移除桌面组件 "+id, x+w-62, y+16, 44, 44, "delete", false, true, func() {
			a.command("widgetRemove", map[string]any{"id": id})
		})
		y += 90
	}
	return y
}

func kindLabel(kind string) string {
	for _, k := range widgetKindLabels {
		if k.kind == kind {
			return k.label
		}
	}
	return kind
}

func styleLabel(style string) string {
	for i, key := range widgetStyleKeys {
		if key == style {
			return widgetStyleLabels[i]
		}
	}
	return style
}
