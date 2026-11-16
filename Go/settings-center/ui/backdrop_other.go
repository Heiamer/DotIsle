//go:build !windows

package ui

func (a *App) syncBackdropTheme() {}

// displayDPI has no window DPI outside Windows.
func displayDPI(hwnd uintptr) int { return 96 }
