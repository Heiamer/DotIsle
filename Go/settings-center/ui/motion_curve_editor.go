package ui

import (
	"encoding/json"
	"fmt"
	"math"

	"github.com/egoist/mygo/ui"
)

type motionCurveDraft struct {
	points   [4]float64
	dragging int
	before   [4]float64
}

func (a *App) configuredMotionCurve() ([4]float64, bool) {
	points := [4]float64{.2, 0, 0, 1}
	value := a.islandSetting("MotionCurve", nil)
	if value == nil {
		return points, false
	}
	raw, err := json.Marshal(value)
	if err != nil || json.Unmarshal(raw, &points) != nil {
		return [4]float64{.2, 0, 0, 1}, false
	}
	return points, true
}

func (a *App) motionCurveEditor(c *ui.Context, p *Palette, x, y, w float32) float32 {
	points, custom := a.configuredMotionCurve()
	if a.curveDraft == nil {
		a.curveDraft = &motionCurveDraft{points: points, dragging: -1}
	}
	draft := a.curveDraft
	if draft.dragging < 0 {
		draft.points = points
	}
	s := &p.Scheme
	Panel(c, p, x, y, w, 244, s.Container)
	graphX, graphY, graphW, graphH := x+44, y+28, w-88, float32(158)
	ui.Box(c).Key("motion-curve-graph").Absolute().Left(graphX).Top(graphY).Width(graphW).Height(graphH).Draw(func(pc *ui.Painter, r ui.Rect) {
		var grid, path ui.Path
		for i := 0; i <= 4; i++ {
			d := float32(i) / 4
			grid.MoveTo(r.X+d*r.W, r.Y)
			grid.LineTo(r.X+d*r.W, r.Y+r.H)
			grid.MoveTo(r.X, r.Y+d*r.H)
			grid.LineTo(r.X+r.W, r.Y+d*r.H)
		}
		pc.StrokePath(&grid, 1, AlphaCol(s.OnSurface, .06))
		point := func(x, y float64) (float32, float32) { return r.X + float32(x)*r.W, r.Y + (1-float32(y))*r.H }
		p0x, p0y := point(0, 0)
		p1x, p1y := point(draft.points[0], draft.points[1])
		p2x, p2y := point(draft.points[2], draft.points[3])
		p3x, p3y := point(1, 1)
		var handles ui.Path
		handles.MoveTo(p0x, p0y)
		handles.LineTo(p1x, p1y)
		handles.MoveTo(p3x, p3y)
		handles.LineTo(p2x, p2y)
		pc.StrokePath(&handles, 1, AlphaCol(s.OnSurfaceVariant, .5))
		path.MoveTo(p0x, p0y)
		if custom || draft.dragging >= 0 {
			path.CubeTo(p1x, p1y, p2x, p2y, p3x, p3y)
		} else {
			stiffness := a.islandFloat("SpringStiffness", 420)
			mass := a.islandFloat("SpringMass", .8)
			damping := a.islandFloat("SpringDamping", 30)
			z := math.Max(1, damping/(2*math.Sqrt(stiffness*mass)))
			for i := 1; i <= 100; i++ {
				t := float64(i) / 100
				value := 1 - (1+12*t)*math.Exp(-12*t)
				if z > 1.000001 {
					aa := -12 * (z - math.Sqrt(z*z-1))
					bb := -12 * (z + math.Sqrt(z*z-1))
					value = 1 - (bb*math.Exp(aa*t)-aa*math.Exp(bb*t))/(bb-aa)
				}
				px, py := point(t, value)
				path.LineTo(px, py)
			}
		}
		pc.StrokePath(&path, 3, Col(s.Primary))
	})
	for i := 0; i < 2; i++ {
		index := i
		handle := ui.ButtonBase(c).Key(fmt.Sprint("curve-handle-", i)).Absolute().Left(graphX+float32(draft.points[i*2])*graphW-12).Top(graphY+(1-float32(draft.points[i*2+1]))*graphH-12).Width(24).Height(24).Radius(12).Label(fmt.Sprintf("曲线控制点%d", i+1)).Background(Col(s.Primary)).Border(5, Col(s.Container))
		if dx, dy, active := handle.Dragged(); active && (dx != 0 || dy != 0) {
			if draft.dragging < 0 {
				draft.before = draft.points
				draft.dragging = index
			}
			draft.points[i*2] = clamp01(draft.points[i*2] + float64(dx/graphW))
			draft.points[i*2+1] = clamp01(draft.points[i*2+1] - float64(dy/graphH))
		} else if draft.dragging == index && !active && !handle.Pressed() {
			draft.dragging = -1
			if draft.points != draft.before {
				a.pushNative("MotionCurve", draft.points)
			}
		}
	}
	text := "原版弹簧 · 拖动控制点可切换为自定义曲线"
	if custom {
		text = fmt.Sprintf("自定义曲线 · %.2f, %.2f, %.2f, %.2f", points[0], points[1], points[2], points[3])
	}
	Label(c, text, x+24, y+208, 12, s.OnSurfaceVariant, false, w-48)
	y += 260
	for i, preset := range []struct {
		name  string
		value any
	}{{"原版弹簧", nil}, {"轻快", [4]float64{.2, 0, 0, 1}}, {"柔和", [4]float64{.4, 0, .2, 1}}, {"线性", [4]float64{0, 0, 1, 1}}} {
		v := preset.value
		selected := !custom && v == nil
		if value, ok := v.([4]float64); ok {
			selected = custom && points == value
		}
		cw := (w - 24) / 4
		Chip(c, p, "动效曲线"+preset.name, x+float32(i)*(cw+8), y, cw, 38, preset.name, selected, func() { a.pushNative("MotionCurve", v) })
	}
	return y + 58
}
