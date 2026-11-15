package ui

import (
	"log"
	"os"
	"unsafe"

	"golang.org/x/sys/windows"
)

var setBackdropAttribute = windows.NewLazySystemDLL("dwmapi.dll").NewProc("DwmSetWindowAttribute")
var getBackdropAttribute = windows.NewLazySystemDLL("dwmapi.dll").NewProc("DwmGetWindowAttribute")

// Mica's color mode belongs to DWM, independently of the custom UI palette.
func (a *App) syncBackdropTheme() {
	if a.Win == nil {
		return
	}
	hwnd := a.Win.NativeHandle()
	if hwnd == 0 || (a.backdropWindow == hwnd && a.backdropDark == a.Palette.Dark) {
		return
	}
	value := int32(0)
	if a.Palette.Dark {
		value = 1
	}
	if setBackdropAttribute.Find() == nil {
		setBackdropAttribute.Call(hwnd, 20, uintptr(unsafe.Pointer(&value)), 4)
	}
	if a.backdropWindow != hwnd {
		kind := int32(0)
		result := uintptr(0x80004005)
		if getBackdropAttribute.Find() == nil {
			result, _, _ = getBackdropAttribute.Call(hwnd, 38, uintptr(unsafe.Pointer(&kind)), 4)
		}
		a.backdropChecked = true
		a.backdropMica = result == 0 && kind == 2
		if os.Getenv("DOTISLE_BACKDROP_DIAGNOSTICS") != "" {
			log.Printf("backdrop hwnd=%d type=%d hresult=%#x mica=%t", hwnd, kind, uint32(result), a.backdropMica)
		}
	}
	a.backdropWindow, a.backdropDark = hwnd, a.Palette.Dark
}
