// Geometry and minimalist architectural glass surfaces.
package ui

import (
	"fmt"
	"strings"

	"github.com/egoist/mygo/ui"
)

const (
	UIFont                = "Google Sans Flex, Segoe UI Variable Text, Microsoft YaHei UI, sans-serif"
	CardRadius    float32 = 24
	FieldRadius   float32 = 12
	ButtonRadius  float32 = 24
	OverlayRadius float32 = 32
)

// surfaceStyle 将常规控件、小卡片与文本框统一为极简无彩色超白/哑光黑玻璃。
func surfaceStyle(e *ui.Element, p *Palette, rgb uint32, radius float32) {
	e.Radius(radius).NoShadow()
	level := elevationControl
	if radius >= 20 {
		level = elevationCard
	}
	elevationStyle(e, p, level)
	if p != nil && p.HighContrast {
		e.Background(Col(rgb)).Border(1, Col(p.Scheme.Outline))
		return
	}
	fill := AlphaCol(rgb, 0.86)
	if p == nil || !p.Dark {
		if rgb == 0xFFFFFF || rgb == 0xFCFCFD || (p != nil && rgb == p.Scheme.Container) {
			fill = AlphaCol(0xFFFFFF, 0.86)
		}
		e.Background(fill).
			Border(1, AlphaCol(0xFFFFFF, 0.85)).
			BorderConicGradient(opticalFresnelConic(false)).
			InsetShadow(0, 1, 0, 0, AlphaCol(0xFFFFFF, 0.96))
	} else {
		if p != nil && rgb == p.Scheme.Container {
			fill = AlphaCol(0x17191E, 0.80)
		}
		e.Background(fill).
			Border(1, AlphaCol(0xFFFFFF, 0.14)).
			BorderConicGradient(opticalFresnelConic(true)).
			InsetShadow(0, 1, 0, 0, AlphaCol(0xFFFFFF, 0.14))
	}
}

// overlayStyle 为弹窗、悬浮面板提供高对比极简建筑玻璃表面。
func overlayStyle(e *ui.Element, p *Palette) {
	e.Radius(OverlayRadius).NoShadow()
	elevationStyle(e, p, elevationDialog)
	if p != nil && p.HighContrast {
		e.Background(Col(p.Scheme.Container)).Border(1, Col(p.Scheme.Outline))
		return
	}
	dark := p != nil && p.Dark
	e.Background(opticalOverlayFill(p)).
		Border(1, AlphaCol(0xFFFFFF, 0.85)).
		BorderConicGradient(opticalFresnelConic(dark))
	if dark {
		e.InsetShadow(0, 1, 0, 0, AlphaCol(0xFFFFFF, 0.18))
	} else {
		e.InsetShadow(0, 1, 0, 0, AlphaCol(0xFFFFFF, 0.96))
	}
}

func fieldStyle(e *ui.Element, p *Palette) {
	surfaceStyle(e, p, p.Scheme.Container, FieldRadius)
	e.Font(UIFont).FontSize(14).TextColor(Col(p.Scheme.OnSurface)).
		FocusRingColor(Col(p.Scheme.OnSurface)).FocusRingOffset(2)
}

// sliderVisual 绘制精密仪器推子滑块：6px 细轨 + 石墨/纯白进度 + 18px 精磨光学圆珠。
func sliderVisual(c *ui.Context, p *Palette, w, h float32, fraction float64, enabled, pressed bool) {
	s := &p.Scheme
	trackFill := AlphaCol(0x111827, 0.09)
	fillRGB := uint32(0x16181D)
	if p.Dark {
		trackFill = AlphaCol(0xFFFFFF, 0.14)
		fillRGB = 0xF3F4F6
	}
	if p.HighContrast {
		trackFill = Col(s.SecondaryContainer)
		fillRGB = s.Primary
	}
	fillCol := Col(fillRGB)
	if !enabled {
		trackFill = AlphaCol(s.SecondaryContainer, .45)
		fillCol = AlphaCol(fillRGB, .40)
	}
	cy := h / 2
	frac := float32(clamp01(fraction))
	knobSize := float32(18)
	if pressed {
		knobSize = 16
	}
	tx := float32(9) + max(float32(0), w-18)*frac
	ui.Box(c).Absolute().Left(0).Top(cy - 3).Width(w).Height(6).Radius(3).Background(trackFill)
	ui.Box(c).Absolute().Left(0).Top(cy - 3).Width(tx).Height(6).Radius(3).Background(fillCol)

	knob := ui.Box(c).Absolute().Left(tx-knobSize/2).Top(cy-knobSize/2).Width(knobSize).Height(knobSize).
		Radius(knobSize/2).Background(Col(0xFFFFFF)).
		Border(1, AlphaCol(0x111827, 0.14))
	if !p.Dark {
		knob.Shadow(0, 1, 3, 0, AlphaCol(0x111827, .18))
	} else {
		knob.Shadow(0, 1, 4, 0, AlphaCol(0x000000, .45))
	}
}

// Segmented 使用极简液态无彩色分段胶囊；同页多个同选项控件会自动按坐标隔离状态。
func Segmented(c *ui.Context, p *Palette, x, y, w float32, labels []string, selected int, onPick func(int)) {
	SegmentedKeyed(c, p, fmt.Sprintf("%.0f-%.0f-%s", x, y, strings.Join(labels, "|")), x, y, w, labels, selected, onPick)
}

// SegmentedKeyed 允许显式传入唯一 ID 隔离外轨、滑块和每个选项按钮，彻底避免同页多个相同选项分段控件串位或点击失效。
func SegmentedKeyed(c *ui.Context, p *Palette, id string, x, y, w float32, labels []string, selected int, onPick func(int)) {
	if len(labels) == 0 {
		return
	}
	s := &p.Scheme
	segKey := id
	if segKey == "" {
		segKey = strings.Join(labels, "|")
	}
	track := ui.Box(c).Key("seg-" + segKey).Absolute().Left(x).Top(y).Width(w).Height(40).Radius(20)
	var borderCol ui.Color
	if p.HighContrast {
		track.Background(Col(s.SecondaryContainer))
		borderCol = Col(s.Outline)
	} else if p.Dark {
		track.Background(AlphaCol(0xFFFFFF, 0.06)).
			InsetShadow(0, 1, 2, 0, AlphaCol(0x000000, 0.20))
		borderCol = AlphaCol(0xFFFFFF, 0.09)
	} else {
		track.Background(AlphaCol(0x111827, 0.048)).
			InsetShadow(0, 1, 2, 0, AlphaCol(0x111827, 0.04))
		borderCol = AlphaCol(0xFFFFFF, 0.72)
	}
	track.Children(func() {
		ui.Box(c).Absolute().Fill().Radius(20).Border(1, borderCol).IgnoreInput(true)
		tw := (w - 8) / float32(len(labels))
		if selected >= 0 && selected < len(labels) {
			marker := ui.Box(c).Key("seg-marker-" + segKey).Absolute().Left(4).Top(4).Width(tw).Height(32).Radius(16).IgnoreInput(true)
			if p.HighContrast {
				marker.Background(Col(s.Container)).Border(1, Col(s.Outline))
			} else if !p.Dark {
				marker.Background(Col(0xFFFFFF)).Shadow(0, 1, 3, 0, AlphaCol(0x111827, 0.10))
			} else {
				marker.Background(AlphaCol(0xFFFFFF, 0.15)).Shadow(0, 1, 3, 0, AlphaCol(0x000000, 0.24))
			}
			marker.Translate(appSpringValue(c, marker, "segment-x", float32(selected)*tw, motionSettle, p.ReducedMotion), 0)
		}
		for i, label := range labels {
			bg, fg := ui.Transparent, s.OnSurfaceVariant
			if i == selected {
				fg = s.OnSurface
			}
			tab := ui.ButtonBase(c).Key(fmt.Sprintf("seg-tab-%s-%d", segKey, i)).Absolute().Left(4 + float32(i)*tw).Top(4).Width(tw).Height(32).
				Radius(16).Background(bg).Justify(ui.Center).AlignItems(ui.Center).Label(label).Cursor(ui.CursorPointer)
			hoverLayer(&tab, bg, s.OnSurface, true)
			if tab.Clicked() && onPick != nil {
				onPick(i)
			}
			tab.Children(func() {
				ui.Text(c, label).Font(textFamily(label)).FontSize(14).FontWeight(500).TextColor(Col(fg)).SingleLine()
			})
		}
	})
}

func CheckControl(c *ui.Context, p *Palette, label string, x, y float32, selected bool, onClick func()) {
	s := &p.Scheme
	hit := ui.ButtonBase(c).Absolute().Left(x).Top(y).Width(36).Height(36).Radius(12).Label(label).Cursor(ui.CursorPointer)
	if hit.Clicked() && onClick != nil {
		onClick()
	}
	hit.Children(func() {
		box := ui.Box(c).Absolute().Left(10).Top(10).Width(16).Height(16).Radius(6)
		surfaceStyle(&box, p, s.Container, 6)
		checkFG := s.OnPrimary
		if selected {
			if p.HighContrast {
				box.Background(Col(s.Primary))
			} else if p.Dark {
				box.Background(Col(0xF3F4F6))
				checkFG = 0x111316
			} else {
				box.Background(Col(0x16181D))
				checkFG = 0xFFFFFF
			}
		} else {
			box.Border(1, Col(s.Outline))
		}
		box.Children(func() {
			if selected {
				ui.Icon(c, Symbol("check")).FontSize(11).TextColor(Col(checkFG)).Absolute().Left(2.5).Top(2.5)
			}
		})
	})
}
