package ui

import (
	"dotisle/settings/prefs"
	"github.com/egoist/mygo/ui"
	"math"
)

func (a *App) scrollPages() []string {
	if len(a.overview.config.ScrollPages) == 0 {
		return prefs.DefaultOverview().ScrollPages
	}
	return a.overview.config.ScrollPages
}
func (a *App) openOverviewCatalog(mode string) {
	a.cancelOverviewGesture()
	a.overview.manage = true
	a.overview.catalogMode = mode
	a.overview.catalogScroll = ui.ScrollState{}
}
func (a *App) catalogCards() []string {
	switch a.overview.catalogMode {
	case "scroll":
		return a.scrollPages()
	case "information":
		return a.overview.config.Information
	default:
		return a.overview.config.Cards
	}
}
func (a *App) catalogIDs() []string {
	switch a.overview.catalogMode {
	case "scroll":
		return append(prefs.ScrollPageIDs(), a.pluginCardIDs()...)
	case "information":
		return append(prefs.InformationCardIDs(), a.pluginCardIDs()...)
	default:
		return append(append([]string{}, prefs.OverviewCardIDs...), a.pluginCardIDs()...)
	}
}
func (a *App) setCatalogCards(keys []string) {
	switch a.overview.catalogMode {
	case "scroll":
		old := a.scrollPages()
		current := old[deckSlotIndex(int(math.Round(deckPos/deckStep)), len(old))]
		a.overview.config.ScrollPages = keys
		index := 0
		for i, key := range keys {
			if key == current {
				index = i
				break
			}
		}
		deckPos, deckTarget = float64(index)*deckStep, float64(index)*deckStep
		deckSpringIdle = true
	case "information":
		a.overview.config.Information = keys
	default:
		a.overview.config.Cards = keys
	}
	a.saveOverview()
}
func (a *App) resetCatalog() {
	d := prefs.DefaultOverview()
	keys := d.Cards
	if a.overview.catalogMode != "scroll" && a.overview.catalogMode != "information" {
		a.overview.config.Sizes = nil
	}
	if a.overview.catalogMode == "scroll" {
		keys = d.ScrollPages
	}
	if a.overview.catalogMode == "information" {
		keys = d.Information
	}
	a.setCatalogCards(keys)
}
func (a *App) moveCatalogCard(key string, delta int) {
	keys := append([]string{}, a.catalogCards()...)
	for i, current := range keys {
		if current == key && i+delta >= 0 && i+delta < len(keys) {
			keys[i], keys[i+delta] = keys[i+delta], keys[i]
			a.setCatalogCards(keys)
			return
		}
	}
}

// The final default scroll page shows information absent from the four activity
// pages. Each tile uses the exact same card renderer as the grid overview.
func (a *App) overviewInformation(c *ui.Context, r overviewRect) {
	p := &a.Palette
	a.cardHeader(c, r, "信息组件")
	Button(c, p, "自定义滚动信息组件", r.x+r.w-136, r.y+10, 116, 32, "自定义组件", "add", true, true, func() { a.openOverviewCatalog("information") })
	keys := a.overview.config.Information
	if len(keys) == 0 {
		keys = prefs.DefaultOverview().Information
	}
	columns := 3
	if r.w < 700 {
		columns = 2
	}
	tileW := (r.w - 40 - float32(columns-1)*12) / float32(columns)
	tileH := r.h - 78
	if tileH < 180 {
		tileH = 180
	}
	rows := (len(keys) + columns - 1) / columns
	ui.Scroll(c).Scrollbars(false).Key("information-mosaic").Absolute().Left(r.x + 20).Top(r.y + 54).Width(r.w - 40).Height(r.h - 70).
		Scrollbars(false).TrackScroll(&a.overview.informationScroll).Label("滚动信息组件").HandleInput(func(ev ui.InputEvent) bool {
		st := &a.overview.informationScroll
		if ev.Kind == ui.InputScroll && ((ev.DY > 0 && st.Y < st.MaxY) || (ev.DY < 0 && st.Y > 0)) {
			st.Y = max(float32(0), min(st.MaxY, st.Y+ev.DY))
			return true
		}
		return false
	}).Children(func() {
		ui.Box(c).Width(r.w - 40).Height(float32(rows)*(tileH+12) - 12).Children(func() {
			for i, key := range keys {
				top := float32(i/columns) * (tileH + 12)
				if top+tileH < a.overview.informationScroll.Y || top > a.overview.informationScroll.Y+r.h-70 {
					continue
				}
				ui.Box(c).Key("information-" + key).Absolute().Left(float32(i%columns) * (tileW + 12)).Top(top).Width(tileW).Height(tileH).
					Radius(20).Clip().Children(func() {
					a.renderOverviewCard(c, overviewRect{key, 0, 0, tileW, tileH})
				})
			}
		})
	})
}
