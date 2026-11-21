package ui

import (
	"dotisle/settings/prefs"
	"github.com/egoist/mygo/plugins/glass"
	"github.com/egoist/mygo/ui"
	"path/filepath"
)

const navigationPanelW float32 = 272

var navigationPages = [...]struct {
	page int
	icon string
}{
	{PageHome, "home"}, {PageWidgets, "filter_none"}, {PageFocus, "timer"},
	{PageTodos, "checklist"}, {PageNotifications, "bell"}, {PagePlugins, "puzzle"}, {PageSettings, "settings"},
}

func sidebarIcon(c *ui.Context, name string, x, y, size float32, rgb uint32) {
	if svg := Symbol(name); svg != nil {
		ui.Icon(c, svg).Absolute().Left(x).Top(y).Width(size).Height(size).FontSize(size).TextColor(Col(rgb))
	}
}

func (a *App) loadCenterLayout() {
	if a.layoutLoaded {
		return
	}
	a.layoutLoaded = true
	layout, err := prefs.LoadCenterLayout(filepath.Join(a.Data, "center-layout.json"))
	a.layout = layout
	if err != nil {
		a.pushToast("布局读取失败：" + err.Error())
	}
}

// Navigation is transient. Old saved expansion never takes space from the page.
func (a *App) ToggleNavigation() {
	a.navigationOpen = !a.navigationOpen
	if a.navigationOpen {
		a.cancelOverviewGesture()
		a.paletteOpen = false
	}
}

func (a *App) navigation(c *ui.Context, w, h float32) {
	if !a.navigationOpen {
		return
	}
	ui.Overlay(c, func() {
		menu := ui.Box(c).Key("navigation-overlay").Absolute().Left(0).Top(0).
			Width(w).Height(h).Modal().Role(ui.RoleDialog).Label("页面导航")
		if menu.OverlayShortcut(0, ui.KeyEscape) {
			a.navigationOpen = false
			c.AnimationFrame()
			return
		}
		menu.Children(func() {
			panelW := min(navigationPanelW, w)
			outside := ui.ButtonBase(c).Absolute().Left(panelW).Top(0).
				Width(w - panelW).Height(h).Label("收起导航").Background(ui.Transparent)
			if outside.Clicked() {
				a.navigationOpen = false
			}
			panel := ui.Box(c).Key("navigation-panel").Absolute().Left(0).Top(0).
				Width(panelW).Height(h).Clip().Label("侧边导航栏")
			progress := appTweenValue(c, panel, "entered", 0, 1, motionStandard, a.reducedMotion())
			panel.Translate(-24*(1-progress), 0)
			p := a.Palette
			s := &p.Scheme
			alpha := .94
			if c.Preferences().ReduceTransparency || c.Preferences().HighContrast {
				alpha = 1
			} else {
				panel.Material(glass.Blur{Radius: 26})
			}
			panel.Children(func() {
				ui.Box(c).Absolute().Fill().Background(AlphaCol(s.Surface, alpha)).IgnoreInput(true)
				ui.Box(c).Absolute().Left(panelW - 1).Top(0).Width(1).Height(h).
					Background(AlphaCol(s.OnSurface, .10)).IgnoreInput(true)
				Label(c, "点屿", 22, 18, 20, s.OnSurface, true, panelW-80).IgnoreInput(true)
				Label(c, "控制中心", 22, 46, 12, s.OnSurfaceVariant, false, panelW-80).IgnoreInput(true)
				IconButton(c, &p, "关闭导航", panelW-54, 18, 36, 36, "close", false, true, func() { a.navigationOpen = false })
				settingSeparator(c, &p, 20, 73, panelW-40)
				ui.Scroll(c).Key("navigation-pages").Absolute().Left(12).Top(84).
					Width(panelW - 24).Height(max(0, h-96)).Scrollbars(false).Children(func() {
					ui.Box(c).Width(panelW - 24).Height(float32(len(navigationPages)) * 43).Children(func() {
						for i, entry := range navigationPages {
							page := entry.page
							name := Pages[page]
							a.settingsNavItem(c, &p, "切换到"+name, name, entry.icon, panelW-24, float32(i)*43, a.Page == page, func() {
								if page == PageSettings {
									a.openSettings(TabRoot)
								} else {
									a.navigate(page)
								}
							})
						}
					})
				})
			})
		})
	})
}
