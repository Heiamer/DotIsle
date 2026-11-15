// Compact page search, anchored below the header's search entry.
package ui

import "github.com/egoist/mygo/ui"

func (a *App) togglePalette() {
	a.paletteOpen = !a.paletteOpen
	a.paletteQuery = ""
	a.paletteSelected = 0
}

func (a *App) paletteMatches() []int {
	var matches []int
	for i, name := range Pages {
		if containsFold(name, a.paletteQuery) || containsFold(pageDescription(i), a.paletteQuery) {
			matches = append(matches, i)
		}
	}
	return matches
}

func (a *App) paletteView(c *ui.Context, w, h float32) {
	if !a.paletteOpen {
		if c.Shortcut(ui.Ctrl, ui.KeyK) {
			a.togglePalette()
		}
		if !a.paletteOpen {
			return
		}
	}
	ui.Overlay(c, func() {
		// The transparent input barrier leaves the page visually unchanged.
		back := ui.Box(c).Key("search-overlay").Absolute().Left(0).Top(0).Width(w).Height(h).
			Modal().Role(ui.RoleDialog).Label("搜索与跳转")
		if back.OverlayShortcut(0, ui.KeyEscape) || back.OverlayShortcut(ui.Ctrl, ui.KeyK) || back.Clicked() {
			a.paletteOpen = false
			return
		}
		back.Children(func() {
			s := &a.Palette.Scheme
			pw := float32(320)
			px := w - pw - 32
			if a.Page == PageTodos {
				px -= 178
			}
			py := float32(TodoActionY)
			matches := a.paletteMatches()
			ph := float32(68 + max(1, min(5, len(matches)))*40)
			panel := ui.Box(c).Key("search-card").Absolute().Left(px).Top(py).Width(pw).Height(ph).Label("搜索卡片")
			overlayStyle(&panel, &a.Palette)
			elevationStyle(&panel, &a.Palette, elevationPopover)
			panel.Radius(22)
			popoverMotion(c, &a.Palette, &panel)
			_ = panel.Clicked()
			panel.Children(func() {
				Icon(c, "search", 18, 17, 20, s.OnSurfaceVariant)
				previous := a.paletteQuery
				ui.TextInput(c, &a.paletteQuery).Absolute().Left(48).Top(10).
					Width(pw-64).Height(36).Font(UIFont).FontSize(14).TextColor(Col(s.OnSurface)).
					Placeholder("搜索页面或设置").Background(ui.Transparent).Border(0, ui.Transparent).FocusRing(false).AutoFocus().Label("搜索内容").
					OnTextKey(func(k ui.TextKey) bool {
						if k.Mods != 0 {
							return false
						}
						a.paletteQuery = k.Text()
						current := a.paletteMatches()
						if len(current) == 0 {
							return false
						}
						switch k.Key {
						case ui.KeyDown:
							a.paletteSelected = (a.paletteSelected + 1) % len(current)
						case ui.KeyUp:
							a.paletteSelected = (a.paletteSelected + len(current) - 1) % len(current)
						case ui.KeyEnter:
							a.paletteOpen = false
							a.navigate(current[min(a.paletteSelected, len(current)-1)])
						default:
							return false
						}
						return true
					})
				if previous != a.paletteQuery {
					a.paletteSelected = 0
					matches = a.paletteMatches()
					panel.Height(float32(68 + max(1, min(5, len(matches)))*40))
				}
				if len(matches) > 0 {
					a.paletteSelected = min(a.paletteSelected, len(matches)-1)
					if back.OverlayShortcut(0, ui.KeyEnter) {
						a.paletteOpen = false
						a.navigate(matches[a.paletteSelected])
						return
					}
				}
				ui.Box(c).Absolute().Left(16).Top(54).Width(pw - 32).Height(1).
					Background(AlphaCol(s.OnSurface, .07)).IgnoreInput(true)
				start := max(0, a.paletteSelected-4)
				end := min(len(matches), start+5)
				for index, page := range matches[start:end] {
					row := ui.ButtonBase(c).Key("search-" + Pages[page]).Absolute().Left(8).Top(60 + float32(index)*40).
						Width(pw - 16).Height(38).Radius(12).Label("搜索结果 " + Pages[page]).Cursor(ui.CursorPointer)
					base := ui.Transparent
					if index+start == a.paletteSelected {
						base = Col(s.High)
					}
					row.Background(base)
					hoverLayer(&row, base, s.OnSurface, true)
					if row.Clicked() {
						a.paletteOpen = false
						a.navigate(page)
					}
					row.Children(func() {
						Icon(c, navIcon(page), 12, 9, 20, s.OnSurfaceVariant)
						Label(c, Pages[page], 46, 9, 14, s.OnSurface, false, pw-80)
					})
				}
				if len(matches) == 0 {
					Label(c, "未找到相关页面", 18, 70, 13, s.OnSurfaceVariant, false, pw-36)
				}
			})
		})
	})
}

// containsFold matches CJK and ASCII search terms.
func containsFold(s, sub string) bool {
	return sub == "" || containsStr(s, sub) || containsStr(lower(s), lower(sub))
}

func lower(s string) string {
	out := []byte(s)
	for i, b := range out {
		if b >= 'A' && b <= 'Z' {
			out[i] = b + 32
		}
	}
	return string(out)
}

func containsStr(s, sub string) bool {
	for i := 0; i+len(sub) <= len(s); i++ {
		if s[i:i+len(sub)] == sub {
			return true
		}
	}
	return false
}
