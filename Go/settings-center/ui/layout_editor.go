package ui

import (
	"dotisle/settings/desktop"
	"dotisle/settings/island"
	"encoding/json"
	"fmt"
	"github.com/egoist/mygo/ui"
	"math"
	"time"
)

type musicSlot struct {
	Kind    string `json:"kind"`
	Visible bool   `json:"visible"`
}
type musicPlacement struct {
	Element int      `json:"element"`
	X       float64  `json:"x"`
	Y       float64  `json:"y"`
	Width   *float64 `json:"width,omitempty"`
}
type musicLayout struct {
	Version        int              `json:"version"`
	Enabled        bool             `json:"enabled"`
	Modules        []musicSlot      `json:"modules"`
	Alignment      int              `json:"alignment"`
	Gap            float64          `json:"gap"`
	CoverSize      float64          `json:"coverSize"`
	ShowCover      bool             `json:"showCover"`
	Controls       []int            `json:"controls"`
	TransportTheme string           `json:"transportTheme"`
	BarStyle       string           `json:"barStyle"`
	ThumbStyle     string           `json:"thumbStyle"`
	ThumbEnabled   bool             `json:"thumbEnabled"`
	ThumbSpin      bool             `json:"thumbSpin"`
	Elements       []musicPlacement `json:"elements,omitempty"`
}
type musicElement struct {
	ID         int    `json:"id"`
	Label      string `json:"label"`
	Resizable  bool   `json:"resizable"`
	X, Y, W, H float64
}
type musicEditor struct {
	BaseLayout      string
	FontSize        float64
	Draft           musicLayout
	Baseline        string
	Undo, Redo      []string
	Selected        int
	Geometry        []musicElement
	Height          float64
	Cache           string
	Error           string
	Pending         bool
	DragID          int
	DragBefore      string
	DragX, DragY    float64
	Tab             int
	Scroll          ui.ScrollState
	InspectorHeight float32
	Sliders         map[string]*editorSliderState
	Preview         *layoutPreviewFrame
	PreviewLoad     bool
	PreviewKey      layoutPreviewKey
	PreviewAt       time.Time
	PreviewError    string
	Applied         bool
}

var nativeMusicLayout = desktop.Layout

func layoutJSON(value any) string { raw, _ := json.Marshal(value); return string(raw) }
func decodeLayout(value any, base string, font float64) (musicLayout, []musicElement, float64, error) {
	result, err := nativeMusicLayout(map[string]any{"draft": value, "baseLayout": base, "fontSize": font})
	if err != nil {
		return musicLayout{}, nil, 0, err
	}
	var layout musicLayout
	var elements []musicElement
	if err = json.Unmarshal([]byte(layoutJSON(result["layout"])), &layout); err != nil {
		return layout, nil, 0, err
	}
	if err = json.Unmarshal([]byte(layoutJSON(result["elements"])), &elements); err != nil {
		return layout, nil, 0, err
	}
	height, _ := result["height"].(float64)
	return layout, elements, height, nil
}
func (e *musicEditor) refresh() {
	key := layoutJSON([]any{e.Draft, e.BaseLayout, e.FontSize})
	if key == e.Cache {
		return
	}
	_, elements, height, err := decodeLayout(e.Draft, e.BaseLayout, e.FontSize)
	if err != nil {
		e.Error = err.Error()
		return
	}
	e.Geometry, e.Height, e.Cache = elements, height, key
	e.Error = ""
}
func (e *musicEditor) change(change func(*musicLayout)) {
	e.edit(change, true)
}
func (e *musicEditor) edit(change func(*musicLayout), enable bool) {
	if e.Pending {
		return
	}
	before := layoutJSON(e.Draft)
	change(&e.Draft)
	if layoutJSON(e.Draft) != before {
		if enable {
			e.Draft.Enabled = true
		}
		e.Applied = false
		e.record(before)
		e.refresh()
	}
}
func (e *musicEditor) record(before string) {
	e.Undo = append(e.Undo, before)
	if len(e.Undo) > 80 {
		e.Undo = e.Undo[len(e.Undo)-80:]
	}
	e.Redo = nil
}
func (e *musicEditor) restore(raw string) {
	var restored musicLayout
	if err := json.Unmarshal([]byte(raw), &restored); err != nil {
		e.Error = err.Error()
		return
	}
	e.Draft = restored
	e.refresh()
}
func (e *musicEditor) restoreSaved() {
	var restored musicLayout
	if err := json.Unmarshal([]byte(e.Baseline), &restored); err != nil {
		e.Error = err.Error()
		return
	}
	e.edit(func(l *musicLayout) { *l = restored }, false)
}
func (e *musicEditor) drag(element musicElement, dx, dy float32, active bool) {
	if e.Pending {
		return
	}
	if active {
		// A press without movement only selects; it must not enable a custom layout.
		if dx == 0 && dy == 0 {
			return
		}
		if e.DragID != element.ID {
			e.DragID = element.ID
			e.DragBefore = layoutJSON(e.Draft)
			e.DragX, e.DragY = element.X, element.Y
		}
		e.DragX += float64(dx)
		e.DragY += float64(dy)
		e.place(element.ID, e.DragX, e.DragY, nil)
		e.Cache = ""
	} else if e.DragID == element.ID {
		if layoutJSON(e.Draft) != e.DragBefore {
			e.record(e.DragBefore)
		}
		e.DragID = -1
	}
}
func (e *musicEditor) history(redo bool) {
	if e.Pending {
		return
	}
	from, to := &e.Undo, &e.Redo
	if redo {
		from, to = &e.Redo, &e.Undo
	}
	if len(*from) == 0 {
		return
	}
	*to = append(*to, layoutJSON(e.Draft))
	raw := (*from)[len(*from)-1]
	*from = (*from)[:len(*from)-1]
	e.restore(raw)
}
func (e *musicEditor) maxElementY(id int) float64 {
	for _, element := range e.Geometry {
		if element.ID == id {
			h := element.H
			if id == 6 {
				h = h * 43 / 22
			} // Leave room for the native seek activation chip.
			return math.Max(12, e.Height-h-10)
		}
	}
	return math.Max(12, e.Height-34)
}

func (e *musicEditor) place(id int, x, y float64, width *float64) {
	e.Applied = false
	x = math.Max(12, math.Min(388, math.Round(x/4)*4))
	y = math.Max(12, math.Min(e.maxElementY(id), math.Round(y/4)*4))
	if width != nil {
		v := math.Max(24, math.Min(376, math.Round(*width/4)*4))
		width = &v
	}
	for i := range e.Draft.Elements {
		if e.Draft.Elements[i].Element == id {
			p := &e.Draft.Elements[i]
			p.X, p.Y = x, y
			if width != nil {
				p.Width = width
			}
			e.Draft.Enabled = true
			return
		}
	}
	e.Draft.Elements = append(e.Draft.Elements, musicPlacement{id, x, y, width})
	e.Draft.Enabled = true
}
func (a *App) applyMusicLayout() {
	e := a.musicEditor
	if e == nil || e.Pending || !a.Connected || a.Win == nil {
		return
	}
	_, _, _, err := decodeLayout(e.Draft, e.BaseLayout, e.FontSize)
	if err != nil {
		e.Error = err.Error()
		return
	}
	raw := layoutJSON(e.Draft)
	var value any
	_ = json.Unmarshal([]byte(raw), &value)
	e.Pending = true
	a.startAsync(func() {
		reply, err := island.Request("update", map[string]any{"key": "MusicCustomLayout", "value": value})
		a.applyAsync(func() {
			e.Pending = false
			if err != nil {
				e.Error = err.Error()
				return
			}
			if reply["success"] != true {
				e.Error = stateString(reply, "布局保存失败", "message")
				return
			}
			e.Baseline = raw
			e.Error = ""
			e.Applied = true
		})
	})
}

// The editor is a bounded workbench: the preview stays on screen while only
// the inspector scrolls. No settings are appended below the preview.
func (a *App) musicLayoutEditor(c *ui.Context, p *Palette, x, y, w float32) float32 {
	if a.musicEditor == nil {
		draft, elements, height, err := decodeLayout(a.settings()["MusicCustomLayout"], a.leafString("MusicExpandedLayout", "SideBySide"), a.leafFloat("FontSize", 14))
		if err != nil {
			Label(c, "布局编辑器加载失败："+err.Error(), x, y, 13, p.Scheme.Error, false, w)
			return y + 60
		}
		selected := -1
		if len(elements) > 0 {
			selected = elements[0].ID
			for _, element := range elements {
				if element.ID == 1 {
					selected = 1
					break
				}
			}
		}
		a.musicEditor = &musicEditor{Draft: draft, Baseline: layoutJSON(draft), Geometry: elements, Height: height, Selected: selected, DragID: -1}
	}
	e, s := a.musicEditor, &p.Scheme
	e.BaseLayout, e.FontSize = a.leafString("MusicExpandedLayout", "SideBySide"), a.leafFloat("FontSize", 14)
	e.refresh()
	a.requestLayoutPreview(e)
	Label(c, "自定义布局", x, y+9, 13, s.OnSurface, true, 90)
	Switch(c, p, "启用自定义布局", x+92, y+6, e.Draft.Enabled, func() { e.edit(func(l *musicLayout) { l.Enabled = !l.Enabled }, false) })
	Button(c, p, "布局撤销", x+161, y, 62, 36, "撤销", "", true, len(e.Undo) > 0 && !e.Pending, func() { e.history(false) })
	Button(c, p, "布局重做", x+231, y, 62, 36, "重做", "", true, len(e.Redo) > 0 && !e.Pending, func() { e.history(true) })
	applyLabel := "应用到灵动岛"
	if e.Applied && layoutJSON(e.Draft) == e.Baseline {
		applyLabel = "已应用"
	}
	Button(c, p, "应用灵动岛布局", x+w-136, y, 136, 36, applyLabel, "check", false, a.Connected && !e.Pending && layoutJSON(e.Draft) != e.Baseline, a.applyMusicLayout)
	y += 52
	panelH := max(float32(400), float32(clipH)-y-24)
	previewW := min(float32(480), (w-16)*.46)
	rightX, rightW := x+previewW+16, w-previewW-16
	Panel(c, p, x, y, previewW, panelH, s.Container).Label("布局预览面板")
	Label(c, "实时预览", x+20, y+18, 15, s.OnSurface, true, previewW-130)
	if e.Preview != nil {
		Label(c, fmt.Sprintf("%.0f × %.0f", e.Preview.Width, e.Preview.Height), x+previewW-108, y+20, 12, s.OnSurfaceVariant, false, 92)
	}
	stageY, stageH := y+56, panelH-140
	ui.Box(c).Absolute().Left(x + 16).Top(stageY).Width(previewW - 32).Height(stageH).Radius(16).Background(Col(s.High)).IgnoreInput(true).
		Draw(func(pc *ui.Painter, r ui.Rect) {
			var grid ui.Path
			for dx := float32(24); dx < r.W; dx += 24 {
				grid.MoveTo(r.X+dx, r.Y)
				grid.LineTo(r.X+dx, r.Y+r.H)
			}
			for dy := float32(24); dy < r.H; dy += 24 {
				grid.MoveTo(r.X, r.Y+dy)
				grid.LineTo(r.X+r.W, r.Y+dy)
			}
			pc.StrokePath(&grid, 1, AlphaCol(s.OnSurface, .035))
		})
	a.drawLayoutPreview(c, p, e, x, stageY, previewW, stageH)
	if e.Preview != nil && e.Preview.Glass {
		Label(c, "玻璃预览不含桌面折射", x+20, y+panelH-72, 12, s.OnSurfaceVariant, false, previewW-40)
	} else if e.Preview != nil && !e.Preview.HasTrack {
		Label(c, "暂无播放", x+20, y+panelH-72, 12, s.OnSurfaceVariant, false, previewW-40)
	}
	Button(c, p, "布局还原已保存", x+16, y+panelH-48, (previewW-40)/2, 32, "还原已保存", "", true, !e.Pending && layoutJSON(e.Draft) != e.Baseline, e.restoreSaved)
	Button(c, p, "布局恢复推荐", x+24+(previewW-40)/2, y+panelH-48, (previewW-40)/2, 32, "恢复推荐", "", true, !e.Pending, func() {
		draft, _, _, err := decodeLayout(nil, e.BaseLayout, e.FontSize)
		if err == nil {
			draft.Enabled = true
			e.edit(func(l *musicLayout) { *l = draft }, false)
		}
	})
	Panel(c, p, rightX, y, rightW, panelH, s.Container).Label("布局调整面板")
	Segmented(c, p, rightX+16, y+14, rightW-32, []string{"元素", "布局", "模块", "按钮", "样式"}, e.Tab, func(tab int) { e.Tab = tab; e.Scroll = ui.ScrollState{} })
	if e.Error != "" {
		Label(c, e.Error, rightX+20, y+58, 12, s.Error, false, rightW-40)
	}
	inset := float32(64)
	if e.Error != "" {
		inset = 88
	}
	ui.Scroll(c).Key("layout-inspector-" + itoa(e.Tab)).Absolute().Left(rightX + 16).Top(y + inset).Width(rightW - 32).Height(panelH - inset - 16).
		Scrollbars(false).TrackScroll(&e.Scroll).Label("布局选项").Children(func() {
		ui.Box(c).Width(rightW - 32).Height(max(e.InspectorHeight, panelH-inset-16)).Children(func() {
			nextH := a.layoutInspector(c, p, 0, 0, rightW-32) + 8
			if nextH != e.InspectorHeight {
				e.InspectorHeight = nextH
				c.AnimationFrame()
			}
		})
	})
	return y + panelH
}

type editorSliderState struct {
	Value       float64
	Before      string
	Held        bool
	Last        float64
	Committed   float64
	Initialized bool
}

// Stream the draft during the captured gesture but record one undo entry at
// release. The shared settings slider intentionally commits only on release.
func (e *musicEditor) slider(c *ui.Context, p *Palette, label string, x, y, w float32, value, lo, hi float64, set func(float64)) {
	if e.Sliders == nil {
		e.Sliders = map[string]*editorSliderState{}
	}
	st := e.Sliders[label]
	if st == nil {
		st = &editorSliderState{}
		e.Sliders[label] = st
	}
	if !st.Initialized || (!st.Held && value != st.Committed) {
		st.Value = value
	}
	st.Committed = value
	knob := ui.SliderBase(c, &st.Value, lo, hi).Absolute().Left(x).Top(y + 24).Width(w).Height(28).Label("布局" + label).
		Disabled(e.Pending).DimDisabled(false).FocusRing(false)
	button, down := knob.PressedButton()
	held := down && button == 0 && !e.Pending
	if held && !st.Held {
		st.Before = layoutJSON(e.Draft)
	}
	if held && st.Value != value {
		set(st.Value)
		e.Draft.Enabled = true
		e.Applied = false
		e.refresh()
	}
	if !held && st.Held {
		if st.Value != value {
			set(st.Value)
			e.Draft.Enabled = true
			e.Applied = false
		}
		e.refresh()
		if layoutJSON(e.Draft) != st.Before {
			e.record(st.Before)
		}
	} else if !held && st.Initialized && st.Value != value && knob.Focused() && !e.Pending {
		e.change(func(l *musicLayout) { set(st.Value) })
	}
	st.Held, st.Last, st.Initialized = held, st.Value, true
	Label(c, label, x, y, 12, p.Scheme.OnSurface, true, w-90)
	Label(c, fmt.Sprintf("%.0f px", st.Value), x+w-78, y, 12, p.Scheme.OnSurfaceVariant, false, 78)
	knob.Children(func() { sliderVisual(c, p, w, 28, (st.Value-lo)/(hi-lo), !e.Pending, held) })
}

func (a *App) layoutInspector(c *ui.Context, p *Palette, x, y, w float32) float32 {
	e, s := a.musicEditor, &p.Scheme
	names := map[string]string{"header": "封面与歌曲信息", "lyrics": "当前歌词", "translation": "翻译与下一句", "status": "播放状态", "progress": "播放进度", "controls": "播放控制"}
	controlNames := map[int]string{2: "播放模式", 9: "喜欢", 16: "音量", 3: "上一首", 4: "播放 / 暂停", 5: "下一首", 6: "翻译", 7: "设置"}
	switch e.Tab {
	case 0:
		selected := musicElement{ID: -1}
		elements := e.editorElements()
		for _, v := range elements {
			if v.ID == e.Selected {
				selected = v
				break
			}
		}
		if selected.ID < 0 {
			Label(c, "点击左侧预览选择元素", x+4, y, 12, s.OnSurfaceVariant, false, w-8)
			return y + 36
		}
		Label(c, selected.Label, x+4, y, 15, s.OnSurface, true, w-8)
		y += 36
		e.slider(c, p, "横向", x+4, y, w-8, selected.X, 12, 388, func(v float64) { e.place(selected.ID, v, selected.Y, nil) })
		y += 68
		e.slider(c, p, "纵向", x+4, y, w-8, selected.Y, 12, e.maxElementY(selected.ID), func(v float64) { e.place(selected.ID, selected.X, v, nil) })
		y += 68
		if selected.Resizable {
			e.slider(c, p, "宽度", x+4, y, w-8, selected.W, 24, 376, func(v float64) { e.place(selected.ID, selected.X, selected.Y, &v) })
			y += 68
		}
		y += 16
		Label(c, "选择元素", x+4, y+6, 13, s.OnSurface, true, w-8)
		y += 34
		cw := (w - 8) / 2
		for i, element := range elements {
			id := element.ID
			Chip(c, p, "选择布局元素"+element.Label, x+float32(i%2)*(cw+8), y+float32(i/2)*36, cw, 30, element.Label, e.Selected == id, func() { e.Selected = id })
		}
		y += float32((len(elements)+1)/2)*36 + 18

	case 1:
		y = leafChoices(c, p, x, y, w, "内容对齐", [][2]string{{"靠左", "0"}, {"居中", "1"}, {"靠右", "2"}}, itoa(e.Draft.Alignment), func(v string) { e.change(func(l *musicLayout) { l.Alignment = intValue(v) }) })
		for _, row := range []struct {
			name          string
			value, lo, hi float64
			set           func(float64)
		}{
			{"模块间距", e.Draft.Gap, 4, 16, func(v float64) { e.Draft.Gap = math.Round(v) }},
			{"封面尺寸", e.Draft.CoverSize, 40, 80, func(v float64) { e.Draft.CoverSize = math.Round(v) }},
		} {
			r := row
			e.slider(c, p, r.name, x+4, y, w-8, r.value, r.lo, r.hi, r.set)
			y += 72
		}
		ToggleRow(c, p, "布局显示封面", x, y, w, "显示专辑封面", "", e.Draft.ShowCover, func() { e.change(func(l *musicLayout) { l.ShowCover = !l.ShowCover }) })
		y += 76
	case 2:
		Label(c, "拖动预览调整位置；箭头调整模块顺序。", x+4, y+4, 12, s.OnSurfaceVariant, false, w-8)
		y += 36
		for i, slot := range e.Draft.Modules {
			index := i
			required := slot.Kind == "header" || slot.Kind == "controls"
			Label(c, names[slot.Kind], x+4, y+14, 13, s.OnSurface, true, w-148)
			if !required {
				Switch(c, p, "布局模块"+slot.Kind, x+w-138, y+12, slot.Visible, func() { e.change(func(l *musicLayout) { l.Modules[index].Visible = !l.Modules[index].Visible }) })
			}
			for j, d := range []int{-1, 1} {
				delta := d
				Button(c, p, "布局移动"+slot.Kind+itoa(d), x+w-88+float32(j)*46, y+5, 40, 34, []string{"↑", "↓"}[j], "", true, !e.Pending && i+d >= 0 && i+d < len(e.Draft.Modules), func() {
					e.change(func(l *musicLayout) {
						l.Modules[index], l.Modules[index+delta] = l.Modules[index+delta], l.Modules[index]
					})
				})
			}
			y += 52
		}
	case 3:
		Label(c, "播放按钮排列", x+4, y+4, 14, s.OnSurface, true, w-8)
		y += 38
		for i, id := range e.Draft.Controls {
			index, control := i, id
			Label(c, controlNames[id], x+4, y+10, 13, s.OnSurface, true, w-156)
			Button(c, p, "移除布局按钮"+itoa(id), x+w-148, y, 52, 34, "移除", "", true, !e.Pending && id != 4, func() {
				e.change(func(l *musicLayout) { l.Controls = append(l.Controls[:index], l.Controls[index+1:]...) })
			})
			for j, d := range []int{-1, 1} {
				delta := d
				Button(c, p, "移动布局按钮"+itoa(control)+itoa(d), x+w-88+float32(j)*46, y, 40, 34, []string{"↑", "↓"}[j], "", true, !e.Pending && i+d >= 0 && i+d < len(e.Draft.Controls), func() {
					e.change(func(l *musicLayout) {
						l.Controls[index], l.Controls[index+delta] = l.Controls[index+delta], l.Controls[index]
					})
				})
			}
			y += 44
		}
		y += 16
		for _, id := range []int{2, 9, 16, 3, 4, 5, 6, 7} {
			present := false
			for _, current := range e.Draft.Controls {
				if id == current {
					present = true
				}
			}
			if !present {
				control := id
				Button(c, p, "添加布局按钮"+itoa(id), x, y, w, 34, "添加"+controlNames[id], "add", true, !e.Pending, func() { e.change(func(l *musicLayout) { l.Controls = append(l.Controls, control) }) })
				y += 42
			}
		}
	case 4:
		for _, row := range []struct {
			name, value string
			options     [][2]string
			set         func(*musicLayout, string)
		}{
			{"按钮风格", e.Draft.TransportTheme, [][2]string{{"礁石", "Reef"}, {"涟漪", "Ripple"}, {"沙", "Sand"}, {"雾", "Mist"}, {"釉", "Glaze"}, {"墨岛", "InkIsle"}, {"潮", "Tide"}, {"留白", "Whitespace"}, {"光环", "Halo"}, {"叠层", "Layered"}}, func(l *musicLayout, v string) { l.TransportTheme = v }},
			{"进度条风格", e.Draft.BarStyle, [][2]string{{"标准", "Normal"}, {"波浪", "Wavy"}, {"圆角", "Rounded"}, {"细线", "Thin"}, {"粗线", "Thick"}, {"渐变", "Gradient"}, {"分段", "Segmented"}, {"圆点", "Dots"}}, func(l *musicLayout, v string) { l.BarStyle = v }},
			{"进度游标", e.Draft.ThumbStyle, [][2]string{{"默认", "Default"}, {"圆", "Circle"}, {"方", "Square"}, {"胶囊", "Pill"}, {"菱形", "Diamond"}, {"花", "Flower"}, {"爱心", "Heart"}, {"饼干", "Cookie"}, {"蓬松", "Puffy"}, {"四叶", "Clover"}, {"八叶", "Clover8"}, {"宝石", "Gem"}, {"三角", "Triangle"}, {"五角", "Pentagon"}, {"十二叶", "Cookie12"}, {"贝壳", "Clam"}}, func(l *musicLayout, v string) { l.ThumbStyle = v }},
		} {
			r := row
			Label(c, r.name, x+4, y+4, 14, s.OnSurface, true, w-8)
			y += 36
			cw := (w - 16) / 3
			for i, opt := range r.options {
				v := opt[1]
				Chip(c, p, "布局样式"+r.name+v, x+float32(i%3)*(cw+8), y+float32(i/3)*38, cw, 32, opt[0], r.value == v, func() { e.change(func(l *musicLayout) { r.set(l, v) }) })
			}
			y += float32((len(r.options)+2)/3)*38 + 20
		}
		ToggleRow(c, p, "布局显示进度游标", x, y, w, "显示进度游标", "", e.Draft.ThumbEnabled, func() { e.change(func(l *musicLayout) { l.ThumbEnabled = !l.ThumbEnabled }) })
		y += 76
		ToggleRow(c, p, "布局游标缓慢旋转", x, y, w, "游标缓慢旋转", "", e.Draft.ThumbSpin, func() { e.change(func(l *musicLayout) { l.ThumbSpin = !l.ThumbSpin }) })
		y += 76
	}
	return y
}

func intValue(s string) int {
	switch s {
	case "1":
		return 1
	case "2":
		return 2
	}
	return 0
}
