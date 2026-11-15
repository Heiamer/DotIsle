// The font picker (font_picker.rs): 780×588 panel, search field, scrollable family
// list (9 rows), preview pane and the confirm/reset flow through IslandFontFamily.
// System families come from the Windows font registry (the Go side has no fontique).
package ui

import (
	"golang.org/x/sys/windows/registry"
	"sort"
	"strings"

	"github.com/egoist/mygo/ui"
)

const fontSample = "海边来信 · 让好音乐陪你"

var systemFamiliesCache []string

// systemFamilies enumerates installed font family names from the registry, stripping
// style suffixes like "(TrueType)" and collapsing the bold/italic duplicates.
func systemFamilies() []string {
	if systemFamiliesCache != nil {
		return systemFamiliesCache
	}
	seen := map[string]bool{}
	for _, root := range []registry.Key{registry.LOCAL_MACHINE, registry.CURRENT_USER} {
		key, err := registry.OpenKey(root, `SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts`,
			registry.QUERY_VALUE)
		if err != nil {
			continue
		}
		names, err := key.ReadValueNames(-1)
		_ = key.Close()
		if err != nil {
			continue
		}
		for _, name := range names {
			family := fontFamilyName(name)
			if family == "" || seen[family] {
				continue
			}
			seen[family] = true
			systemFamiliesCache = append(systemFamiliesCache, family)
		}
	}
	sort.Strings(systemFamiliesCache)
	return systemFamiliesCache
}

// fontFamilyName strips the file-style suffix and trailing style words.
func fontFamilyName(value string) string {
	for _, suffix := range []string{" (TrueType)", " (OpenType)", " (TrueType Signature)", " (PostScript)"} {
		if strings.HasSuffix(value, suffix) {
			value = strings.TrimSuffix(value, suffix)
			break
		}
	}
	// Registry names repeat the family for each style; cut at the style words.
	for _, style := range []string{" Bold", " Light", " Medium", " SemiBold", " Black", " Thin", " ExtraBold", " Italic", " Regular", " Oblique", " Bold Italic"} {
		if strings.HasSuffix(value, style) {
			value = strings.TrimSuffix(value, style)
		}
	}
	return strings.TrimSpace(value)
}

type fontPickerState struct {
	query    string
	selected string // "" means the default (Noto Sans SC)
	current  string
}

func (a *App) openFontPicker() {
	current := a.islandString("IslandFontFamily", "Noto Sans SC")
	selected := current
	if current == "Noto Sans SC" {
		selected = ""
	}
	a.fontPicker = &fontPickerState{current: current, selected: selected}
}

func (a *App) fontPickerPreviewFamily() string {
	if a.fontPicker.selected != "" && a.fontPicker.selected != "Noto Sans SC" {
		return a.fontPicker.selected
	}
	if a.fontPicker.current != "Noto Sans SC" {
		return a.fontPicker.current
	}
	return "Noto Sans SC"
}

// fontPickerView renders the dialog; call from dialogViews when a.fontPicker set.
func (a *App) fontPickerView(c *ui.Context, w, h float32) {
	p := a.fontPicker
	s := &a.Palette.Scheme
	mask := ui.Box(c).Fill().Background(AlphaCol(0, 0.5))
	if mask.Clicked() {
		a.fontPicker = nil
	}
	px, py := (w-780)/2, (h-588)/2
	overlay := Panel(c, &a.Palette, px, py, 780, 588, s.Surface)
	overlayStyle(overlay, &a.Palette)
	Label(c, "选择字体 · 灵动岛", px+28, py+20, 24, s.OnSurface, true, 480)
	IconButton(c, &a.Palette, "取消选择字体", px+780-64, py+16, 44, 44, "close", false, true, func() {
		a.fontPicker = nil
	})
	Label(c, "搜索本机已安装的字体；选中后先预览，确认才会应用。", px+28, py+58, 12, s.OnSurfaceVariant, false, 780-56)

	// Search field.
	ui.Box(c).Absolute().Left(px+28).Top(py+84).Width(420).Height(46).
		Radius(12).Background(Col(s.High)).Border(2, Col(s.Primary))
	ui.TextInput(c, &p.query).Absolute().Left(px + 42).Top(py + 90).Width(392).Height(34).
		FontSize(16).TextColor(Col(s.OnSurface)).Placeholder("搜索字体名称").
		Background(ui.Transparent).FocusRing(false)

	// Family list, nine visible rows with the chosen mark.
	listTop := py + 128
	listH := float32(588) - 128 - 84
	visible := 9
	rows := []string{}
	for _, family := range systemFamilies() {
		if p.query == "" || containsFold(family, p.query) {
			rows = append(rows, family)
		}
	}
	list := ui.Box(c).Absolute().Left(px + 28).Top(listTop).Width(420).Height(listH).Clip()
	list.Children(func() {
		start := 0
		for i, family := range rows {
			if i < start {
				continue
			}
			if i-start >= visible*4 { // soft cap: the scroll container clips the rest
				break
			}
			rowY := float32(i-start) * 34
			chosen := p.selected == family
			rowBase := ui.Transparent
			if chosen {
				rowBase = Col(s.SecondaryContainer)
			}
			row := ui.ButtonBase(c).Absolute().Left(0).Top(rowY).Width(420).Height(30).
				Radius(10).Background(rowBase).Label("字体 " + family)
			hoverLayer(&row, rowBase, s.OnSurface, true)
			if row.Clicked() {
				p.selected = family
			}
			ink := s.OnSurface
			if chosen {
				ink = s.OnSecondaryContainer
			}
			row.Children(func() {
				Label(c, family, 14, 7, 15, ink, chosen, 380)
				if chosen {
					if svg := Symbol("check"); svg != nil {
						ui.Icon(c, svg).FontSize(16).TextColor(Col(s.Primary)).
							Absolute().Left(392).Top(8)
					}
				}
			})
		}
	})
	if len(rows) == 0 {
		Label(c, "没有匹配的字体", px+42, listTop+10, 14, s.OnSurfaceVariant, false, 392)
	}

	// Preview pane.
	family := a.fontPickerPreviewFamily()
	ensurePreviewFont(family)
	rx := px + 472
	rw := float32(780) - 500
	Label(c, "预览", rx, py+84, 13, s.OnSurfaceVariant, true, rw)
	Panel(c, &a.Palette, rx, py+104, rw, 176, s.Container)
	ui.Box(c).Absolute().Left(rx + 14).Top(py + 122).Width(rw - 28).Height(44).
		Radius(10).Background(Col(s.High)).Children(func() {
		ui.Box(c).Absolute().Left(12).Top(0).Width(rw - 52).Height(44).Justify(ui.Center).AlignItems(ui.Center).Children(func() { ui.Text(c, fontSample).Font(family).FontSize(19).TextColor(Col(s.OnSurface)).SingleLine() })
	})
	Label(c, "示例文字", rx+14, py+180, 11, s.OnSurfaceVariant, false, rw-28)
	name := p.selected
	if name == "" {
		name = map[bool]string{true: p.current, false: "默认字体"}[p.current != "Noto Sans SC"]
	}
	ui.Box(c).Absolute().Left(rx + 14).Top(py + 204).Width(rw - 28).Height(40).Children(func() {
		CenteredText(c, name, 0, 0, rw-28, 40, 22, s.Primary, true)
	})
	Label(c, "字体名称", rx+14, py+246, 11, s.OnSurfaceVariant, false, rw-28)
	Label(c, "选择只会预览；点确认更换后才会应用到灵动岛。", rx, py+300, 11, s.OnSurfaceVariant, false, rw)
	if p.selected != "" {
		Label(c, "确认后会立即重新排版展开卡片与胶囊。", rx, py+320, 11, s.OnSurfaceVariant, false, rw)
	}

	// Actions: reset / cancel / confirm (font_picker.rs panel tail).
	Button(c, &a.Palette, "恢复默认", px+28, py+588-62, 150, 44, "恢复默认", "", true, true, func() {
		a.pushNative("IslandFontFamily", "Noto Sans SC")
		a.pushToast("已恢复默认字体")
		a.fontPicker = nil
	})
	Button(c, &a.Palette, "取消", px+780-256, py+588-62, 100, 44, "取消", "", true, true, func() {
		a.fontPicker = nil
	})
	Button(c, &a.Palette, "确认更换", px+780-142, py+588-62, 114, 44, "确认更换", "", false,
		p.selected != "", func() {
			a.pushNative("IslandFontFamily", p.selected)
			a.pushToast("灵动岛字体已切换为 " + p.selected)
			a.fontPicker = nil
		})
}
