package ui

import (
	"math"

	"github.com/egoist/mygo/ui"
)

// opticalReduced 检查系统级透明/高对比度无障碍降级开关。
func opticalReduced(c *ui.Context, p *Palette) bool {
	if c != nil {
		pref := c.Preferences()
		if pref.ReduceTransparency || pref.HighContrast {
			return true
		}
	}
	return p != nil && p.HighContrast
}

// opticalFlowAngle 返回与自然光场呼吸微调同步的镜面棱光角度。
func (a *App) opticalFlowAngle(base float32) float32 {
	if a == nil {
		return base
	}
	return base + float32(math.Sin(a.lightField.phase*0.42)*12.0)
}

// opticalFresnelConic 生成无彩色浮法光学玻璃棱边：顺应左上受光面高光与右下极细玻璃厚度收边。
func opticalFresnelConic(dark bool) ui.ConicGradient {
	if dark {
		return ui.ConicGradient{
			Angle: 315,
			Oklab: true,
			Stops: []ui.GradientStop{
				{Color: AlphaCol(0xFFFFFF, 0.20), At: 0.00},
				{Color: AlphaCol(0xFFFFFF, 0.08), At: 0.25},
				{Color: AlphaCol(0xFFFFFF, 0.025), At: 0.55},
				{Color: AlphaCol(0xFFFFFF, 0.09), At: 0.80},
				{Color: AlphaCol(0xFFFFFF, 0.20), At: 1.00},
			},
		}
	}
	return ui.ConicGradient{
		Angle: 315,
		Oklab: true,
		Stops: []ui.GradientStop{
			{Color: AlphaCol(0xFFFFFF, 0.98), At: 0.00},
			{Color: AlphaCol(0xFFFFFF, 0.68), At: 0.24},
			{Color: AlphaCol(0x1E293B, 0.15), At: 0.54},
			{Color: AlphaCol(0xFFFFFF, 0.74), At: 0.78},
			{Color: AlphaCol(0xFFFFFF, 0.98), At: 1.00},
		},
	}
}

// opticalPrismFlowConic 保持无彩色极简曜光高光弧，绝不引入廉价彩虹色散边。
func opticalPrismFlowConic(dark bool, angle float32) ui.ConicGradient {
	if dark {
		return ui.ConicGradient{
			Angle: angle,
			Oklab: true,
			Stops: []ui.GradientStop{
				{Color: AlphaCol(0xFFFFFF, 0.28), At: 0.00},
				{Color: AlphaCol(0xFFFFFF, 0.11), At: 0.28},
				{Color: AlphaCol(0xFFFFFF, 0.04), At: 0.55},
				{Color: AlphaCol(0xFFFFFF, 0.14), At: 0.80},
				{Color: AlphaCol(0xFFFFFF, 0.28), At: 1.00},
			},
		}
	}
	return ui.ConicGradient{
		Angle: angle,
		Oklab: true,
		Stops: []ui.GradientStop{
			{Color: AlphaCol(0xFFFFFF, 0.98), At: 0.00},
			{Color: AlphaCol(0xE2E8F0, 0.65), At: 0.25},
			{Color: AlphaCol(0x111827, 0.09), At: 0.54},
			{Color: AlphaCol(0xFFFFFF, 0.78), At: 0.78},
			{Color: AlphaCol(0xFFFFFF, 0.98), At: 1.00},
		},
	}
}

func opticalCardFill(p *Palette) ui.Color {
	if p != nil && p.Dark {
		return AlphaCol(0x17191E, 0.82)
	}
	return AlphaCol(0xFFFFFF, 0.86)
}

func opticalOverlayFill(p *Palette) ui.Color {
	if p != nil && p.Dark {
		return AlphaCol(0x15171D, 0.96)
	}
	return AlphaCol(0xF6F7FA, 0.96)
}

// paintCardSpecular 绘制克制的左上纯净日光整卡斜照、顶部受光棱线与纯白丝绸悬停反光。
func paintCardSpecular(pc *ui.Painter, box ui.Rect, radius float32, dark bool, px, py float32, hovered bool) {
	if box.W <= 4 || box.H <= 4 {
		return
	}
	inner := ui.Rect{X: box.X + 1, Y: box.Y + 1, W: box.W - 2, H: box.H - 2}
	ir := max(float32(0), radius-1)
	pc.Clip(inner, ir, func() {
		topFrom, topTo := AlphaCol(0xFFFFFF, 0.30), AlphaCol(0xFFFFFF, 0)
		rimLine := AlphaCol(0xFFFFFF, 0.94)
		if dark {
			topFrom = AlphaCol(0xFFFFFF, 0.045)
			rimLine = AlphaCol(0xFFFFFF, 0.16)
		}
		// 铺满完整内区，避免大卡中部 36% 高度处出现水平渐隐截断线
		pc.FillGradient(inner, ui.LinearGradient{
			From:  topFrom,
			To:    topTo,
			Angle: 150,
		}, 0)
		lineInset := min(radius+2, inner.W*0.25)
		if inner.W > lineInset*2 {
			pc.Line(inner.X+lineInset, inner.Y, inner.X+inner.W-lineInset, inner.Y, 1, rimLine)
		}
		if hovered && box.W > 0 && box.H > 0 {
			spotCol := AlphaCol(0xFFFFFF, 0.36)
			if dark {
				spotCol = AlphaCol(0xFFFFFF, 0.07)
			}
			rad := min(max(box.W, box.H)*0.44, float32(190))
			pc.FillRadialGradient(inner, ui.RadialGradient{
				Center: ui.Point{X: clamp01F(px / box.W), Y: clamp01F(py / box.H)},
				Radius: rad,
				Stops: []ui.GradientStop{
					{Color: spotCol, At: 0},
					{Color: AlphaCol(0xFFFFFF, 0), At: 1},
				},
			}, 0)
		}
	})
}

// opticalCardStyle 将元件配置为 L1 物理光学浮法玻璃卡。
func opticalCardStyle(c *ui.Context, e *ui.Element, p *Palette, radius float32) {
	if opticalReduced(c, p) {
		e.Background(Col(p.Scheme.Container)).Radius(radius).Border(1, Col(p.Scheme.Outline)).NoShadow()
		elevationStyle(e, p, elevationCard)
		return
	}
	dark := p != nil && p.Dark
	fill := opticalCardFill(p)
	e.Background(fill).Radius(radius).
		Border(1, AlphaCol(0xFFFFFF, 0.80)).
		BorderConicGradient(opticalFresnelConic(dark)).
		NoShadow()
	elevationStyle(e, p, elevationCard)
	if !dark {
		e.InsetShadow(0, 1, 0, 0, AlphaCol(0xFFFFFF, 0.94))
	} else {
		e.InsetShadow(0, 1, 0, 0, AlphaCol(0xFFFFFF, 0.16))
	}
	px, py, over := e.PointerPosition()
	e.Draw(func(pc *ui.Painter, r ui.Rect) {
		if r.W <= 4 || r.H <= 4 {
			return
		}
		paintCardSpecular(pc, r, radius, dark, px, py, over)
	})
}

// opticalPillStyle 为胶囊与小型交互控件配置清透浮法微玻璃底盘。
func opticalPillStyle(c *ui.Context, e *ui.Element, p *Palette, radius float32, tint ui.Color, interactive, specularRim bool, angle float32) {
	if opticalReduced(c, p) {
		bg := Col(p.Scheme.Container)
		if tint.A > 0 {
			bg = tint
		}
		e.Background(bg).Radius(radius).Border(1, Col(p.Scheme.Outline)).NoShadow()
		elevationStyle(e, p, elevationControl)
		return
	}
	fill := AlphaCol(0xFFFFFF, 0.62)
	if p != nil && p.Dark {
		fill = AlphaCol(0x1C1F26, 0.78)
	}
	if tint.A > 0 {
		fill = tint
	}
	e.Background(fill).Radius(radius).NoShadow()
	if specularRim {
		e.Border(1, AlphaCol(0xFFFFFF, 0.80)).BorderConicGradient(opticalPrismFlowConic(p.Dark, angle))
	} else {
		e.Border(1, AlphaCol(0xFFFFFF, 0.78)).BorderConicGradient(opticalFresnelConic(p.Dark))
	}
	if p == nil || !p.Dark {
		e.InsetShadow(0, 1, 0, 0, AlphaCol(0xFFFFFF, 0.96))
	} else {
		e.InsetShadow(0, 1, 0, 0, AlphaCol(0xFFFFFF, 0.16))
	}
	elevationStyle(e, p, elevationControl)
}

// opticalOverlayStyle 将悬浮面板、抽屉与命令面板配置为 L3 磨砂建筑玻璃层。
func opticalOverlayStyle(c *ui.Context, e *ui.Element, p *Palette, radius float32, specularRim bool, angle float32) {
	if opticalReduced(c, p) {
		e.Background(Col(p.Scheme.Container)).Radius(radius).Border(1, Col(p.Scheme.Outline)).NoShadow()
		elevationStyle(e, p, elevationDialog)
		return
	}
	e.Background(opticalOverlayFill(p)).Radius(radius).NoShadow()
	if specularRim {
		e.Border(1, AlphaCol(0xFFFFFF, 0.85)).BorderConicGradient(opticalPrismFlowConic(p.Dark, angle))
	} else {
		e.Border(1, AlphaCol(0xFFFFFF, 0.80)).BorderConicGradient(opticalFresnelConic(p.Dark))
	}
	if !p.Dark {
		e.InsetShadow(0, 1, 0, 0, AlphaCol(0xFFFFFF, 0.96))
	} else {
		e.InsetShadow(0, 1, 0, 0, AlphaCol(0xFFFFFF, 0.18))
	}
	elevationStyle(e, p, elevationDialog)
}
