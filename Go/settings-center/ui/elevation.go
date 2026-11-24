package ui

import "github.com/egoist/mygo/ui"

type surfaceElevation uint8

const (
	elevationControl surfaceElevation = iota
	elevationCard
	elevationPopover
	elevationDialog
)

// Shadows are static GPU primitives; elevation follows the top-left 32deg natural daylight.
func elevationStyle(e *ui.Element, p *Palette, level surfaceElevation) {
	e.NoShadow()
	shadowRGB := uint32(0x1E293B)
	contact, ambient := .055, .085
	if p != nil && p.Dark {
		shadowRGB = 0x000000
		contact, ambient = .28, .48
	}
	switch level {
	case elevationControl:
		e.Shadow(0, 1, 2, 0, AlphaCol(shadowRGB, contact)).
			Shadow(0, 3, 8, -2, AlphaCol(shadowRGB, ambient*0.5))
	case elevationCard:
		e.Shadow(0, 1, 3, 0, AlphaCol(shadowRGB, contact)).
			Shadow(1, 10, 26, -8, AlphaCol(shadowRGB, ambient))
	case elevationPopover:
		e.Shadow(0, 3, 8, -1, AlphaCol(shadowRGB, contact*1.4)).
			Shadow(1, 18, 42, -8, AlphaCol(shadowRGB, ambient*1.55))
	case elevationDialog:
		e.Shadow(0, 4, 12, -1, AlphaCol(shadowRGB, contact*1.65)).
			Shadow(2, 26, 64, -10, AlphaCol(shadowRGB, ambient*1.90))
	}
}
