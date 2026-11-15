package ui

import (
	"dotisle/settings/prefs"
	"github.com/egoist/mygo/ui"
	"math"
)

func overviewLayoutSized(keys []string, sizes map[string]prefs.CardSize, x, y, w float32) []overviewRect {
	defaults := overviewLayout(keys, x, y, w)
	out := make([]overviewRect, 0, len(keys))
	// A skyline lets a tall card sit beside two shorter cards without overlaps.
	columns := overviewColumns(w)
	unit := (w - float32(columns-1)*16) / float32(columns)
	skyline := make([]float32, columns)
	for i := range skyline {
		skyline[i] = y
	}
	for i, key := range keys {
		span := int(math.Round(float64((defaults[i].w + 16) / (unit + 16))))
		h := overviewCardHeight
		if size, ok := sizes[key]; ok {
			span = int(math.Round(float64(size.Width * float32(columns))))
			span = max(3, min(columns, span))
			h = max(overviewCardHeight, min(float32(560), size.Height))
		}
		best, bestY := 0, float32(math.MaxFloat32)
		for col := 0; col+span <= columns; col++ {
			yy := y
			for j := col; j < col+span; j++ {
				yy = max(yy, skyline[j])
			}
			if yy < bestY {
				best, bestY = col, yy
			}
		}
		r := overviewRect{key, x + float32(best)*(unit+16), bestY, float32(span)*(unit+16) - 16, h}
		out = append(out, r)
		for col := best; col < best+span; col++ {
			skyline[col] = bestY + h + 16
		}
	}
	return out
}

// cancelOverviewGesture releases transient state before its handles leave the view.
func (a *App) cancelOverviewGesture() {
	a.finishOverviewResize(false)
	a.overview.pressed, a.overview.dragging, a.overview.target = "", "", ""
}

func (a *App) setOverviewView(view string) {
	a.cancelOverviewGesture()
	a.overview.edit = false
	a.overview.config.View = view
	a.Scroll[PageHome] = ui.ScrollState{}
	a.saveOverview()
}

func (a *App) finishOverviewResize(commit bool) {
	st := &a.overview
	if st.resizing == "" {
		return
	}
	if commit {
		if st.config.Sizes == nil {
			st.config.Sizes = map[string]prefs.CardSize{}
		}
		columns := float32(overviewColumns(st.resizeCanvas))
		unit := (st.resizeCanvas + 16) / columns
		span := max(float32(3), min(columns, float32(math.Round(float64((st.resizeW+16)/unit)))))
		st.config.Sizes[st.resizing] = prefs.CardSize{Width: span / columns, Height: float32(math.Round(float64(st.resizeH/8))) * 8}
		a.saveOverview()
	}
	st.resizing = ""
}

func (a *App) overviewResizeHandle(c *ui.Context, r overviewRect, canvas float32) {
	st, s := &a.overview, &a.Palette.Scheme
	if !st.edit {
		return
	}
	x, y := r.x+r.w-34, r.y+r.h-34
	handle := ui.ButtonBase(c).Key("resize-" + r.key).Absolute().Left(x).Top(y).Width(34).Height(34).
		Radius(12).Label("调整大小" + overviewName(r.key)).Cursor(ui.CursorResizeNWSE)
	handle.HandleInput(func(ev ui.InputEvent) bool {
		switch ev.Kind {
		case ui.InputPointerDown:
			if ev.Button != 0 || st.dragging != "" {
				return false
			}
			st.resizing = r.key
			st.resizeStartX = x + ev.X
			st.resizeStartY = y + ev.Y
			st.resizeInitialW, st.resizeInitialH = r.w, r.h
			st.resizeW, st.resizeH = r.w, r.h
			st.resizeCanvas = canvas
			return false
		case ui.InputPointerMove:
			if st.resizing != r.key {
				return false
			}
			minimum := 3*(canvas+16)/float32(overviewColumns(canvas)) - 16
			st.resizeW = max(minimum, min(canvas-r.x+st.resizeOrigin, st.resizeInitialW+x+ev.X-st.resizeStartX))
			st.resizeH = max(overviewCardHeight, min(float32(560), st.resizeInitialH+y+ev.Y-st.resizeStartY))
			return true
		case ui.InputPointerUp:
			if st.resizing == r.key {
				a.finishOverviewResize(!ev.Canceled)
				return false
			}
		}
		return false
	})
	handle.Children(func() {
		for i := 0; i < 3; i++ {
			ui.Box(c).Absolute().Left(13 + float32(i)*4).Top(24 - float32(i)*4).Width(3).Height(3).Radius(1.5).Background(AlphaCol(s.OnSurface, .45)).IgnoreInput(true)
		}
	})
	if st.resizing == r.key {
		ui.Box(c).Absolute().Left(r.x).Top(r.y).Width(st.resizeW).Height(st.resizeH).Radius(22).
			Background(AlphaCol(s.Primary, .035)).Border(2, AlphaCol(s.Primary, .7)).IgnoreInput(true)
	}
}
