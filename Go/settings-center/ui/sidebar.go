package ui

import (
	"dotisle/settings/prefs"
	"github.com/egoist/mygo/plugins/glass"
	"github.com/egoist/mygo/ui"
	"path/filepath"
)

const navigationPanelW float32 = 340

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
			outside := ui.ButtonBase(c).Absolute().Left(navigationPanelW).Top(0).
				Width(w - navigationPanelW).Height(h).Label("收起导航").Background(ui.Transparent)
			if outside.Clicked() {
				a.navigationOpen = false
			}
			panel := ui.Box(c).Key("navigation-panel").Absolute().Left(0).Top(0).
				Width(navigationPanelW).Height(h).Clip().Label("侧边导航栏")
			// Slide the material without fading its dark text or backdrop.
			progress := appTweenValue(c, panel, "entered", 0, 1, motionStandard, a.reducedMotion())
			panel.Translate(-24*(1-progress), 0)
			// Keep backdrop sampling out of an opacity group, which otherwise
			// samples an empty offscreen surface instead of the page beneath.
			// Keep foreground absolute too, so the tint paints before the text.
			alpha := .82
			if c.Preferences().ReduceTransparency || c.Preferences().HighContrast {
				alpha = 1
			} else {
				panel.Material(glass.Blur{Radius: 26})
			}
			panel.Children(func() {
				ui.Box(c).Absolute().Fill().Background(AlphaCol(0xFFFFFF, alpha)).IgnoreInput(true)
				ui.Box(c).Absolute().Left(navigationPanelW - 1).Top(0).Width(1).Height(h).
					Background(AlphaCol(0x000000, .06)).IgnoreInput(true)
				ui.Box(c).Absolute().Fill().Children(func() {
					p := a.Palette
					p.Scheme.OnSurface, p.Scheme.OnSurfaceVariant = 0x1D1D1F, 0x636366
					IconButton(c, &p, "关闭导航", navigationPanelW-60, 16, 40, 40, "close", false, true, func() { a.navigationOpen = false })
					x, y, rowH := float32(40), float32(82), float32(70)
					for row, page := range []int{PageHome, PageWidgets, PageFocus, PageTodos, PageNotifications, PagePlugins, PageSettings} {
						name := Pages[page]
						item := ui.ButtonBase(c).Key("nav-" + name).Absolute().Left(x).Top(y + float32(row)*rowH).
							Width(navigationPanelW - x - 32).Height(66).AlignItems(ui.Center).Justify(ui.Start).
							Label("切换到" + name).Cursor(ui.CursorPointer)
						if item.Clicked() {
							if page == PageSettings {
								a.openSettings(TabRoot)
							} else {
								a.navigate(page)
							}
						}
						item.Children(func() {
							ui.Text(c, name).Absolute().Left(0).Top(3).Font(textFamily(name)).FontSize(42).FontWeight(600).
								FixedLineHeight(60).TextColor(Col(0x1D1D1F)).SingleLine().IgnoreInput(true)
						})
					}
				})
			})
		})
	})
}
