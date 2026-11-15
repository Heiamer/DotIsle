package ui

import "github.com/egoist/mygo/ui"

type surfaceElevation uint8

const (
	elevationControl surfaceElevation = iota
	elevationCard
	elevationPopover
	elevationDialog
)

// Shadows are static GPU primitives; elevation never adds a backdrop filter.
func elevationStyle(e *ui.Element, p *Palette, level surfaceElevation) {
	e.NoShadow()
	contact, ambient := .055, .11
	if p.Dark {
		contact, ambient = .20, .38
	}
	switch level {
	case elevationControl:
		e.Shadow(0, 1, 2, 0, AlphaCol(0, contact))
	case elevationCard:
		e.Shadow(0, 2, 4, 0, AlphaCol(0, contact)).
			Shadow(0, 8, 20, -5, AlphaCol(0, ambient))
	case elevationPopover:
		e.Shadow(0, 3, 8, -1, AlphaCol(0, contact*1.5)).
			Shadow(0, 16, 38, -6, AlphaCol(0, ambient*1.6))
	case elevationDialog:
		e.Shadow(0, 4, 12, -1, AlphaCol(0, contact*1.8)).
			Shadow(0, 24, 60, -8, AlphaCol(0, ambient*2))
	}
}
