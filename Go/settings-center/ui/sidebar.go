package ui

import (
	"dotisle/settings/prefs"
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
			// 全高贴边建筑玻璃抽屉：几何严格契合 (0, 0, panelW, h) 且绝不扰动右侧页面像素；
			// 直接绘制高密度冰白/曜石霜膜，避免动画中抓取下层画面。
			panel := ui.Box(c).Key("navigation-panel").Absolute().Left(0).Top(0).
				Width(panelW).Height(h).Radius(0).Clip().Label("侧边导航栏")
			progress := appTweenValue(c, panel, "entered", 0, 1, motionStandard, a.reducedMotion())
			panel.Translate(-24*(1-progress), 0)
			p := a.Palette
			s := &p.Scheme
			reduced := opticalReduced(c, &p)
			if reduced {
				panel.Background(Col(s.Surface)).Border(1, Col(s.Outline))
			} else {
				panel.Background(opticalOverlayFill(&p))
				if p.Dark {
					panel.Shadow(8, 0, 28, -4, AlphaCol(0x000000, 0.36))
				} else {
					panel.Shadow(8, 0, 28, -4, AlphaCol(0x1E293B, 0.10))
				}
			}
			panel.Children(func() {
				if !reduced {
					ui.Box(c).Absolute().Fill().IgnoreInput(true).Draw(func(pc *ui.Painter, r ui.Rect) {
						sheen := AlphaCol(0xFFFFFF, 0.22)
						edge := AlphaCol(0x1E293B, 0.10)
						if p.Dark {
							sheen = AlphaCol(0xFFFFFF, 0.04)
							edge = AlphaCol(0xFFFFFF, 0.12)
						}
						pc.FillGradient(r, ui.LinearGradient{
							From:  sheen,
							To:    AlphaCol(0xFFFFFF, 0),
							Angle: 145,
						}, 0)
						pc.Line(r.X+r.W-0.5, r.Y, r.X+r.W-0.5, r.Y+r.H, 1, edge)
					})
				}
				Label(c, "点屿", 20, 18, 20, s.OnSurface, true, panelW-76).IgnoreInput(true)
				Label(c, "控制中心", 20, 46, 12, s.OnSurfaceVariant, false, panelW-76).IgnoreInput(true)
				IconButton(c, &p, "关闭导航", panelW-50, 18, 34, 34, "close", false, true, func() { a.navigationOpen = false })
				settingSeparator(c, &p, 18, 74, panelW-36)
				ui.Scroll(c).Key("navigation-pages").Absolute().Left(10).Top(86).
					Width(panelW - 20).Height(max(0, h-98)).Scrollbars(false).Children(func() {
					ui.Box(c).Width(panelW - 20).Height(float32(len(navigationPages)) * 43).Children(func() {
						for i, entry := range navigationPages {
							page := entry.page
							name := Pages[page]
							a.settingsNavItem(c, &p, "切换到"+name, name, entry.icon, panelW-20, float32(i)*43, a.Page == page, func() {
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
