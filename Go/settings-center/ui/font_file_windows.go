//go:build windows && (!dotisle_mygo_fontfile || (!amd64 && !arm64))

package ui

import (
	"fmt"
	"os"
	"unsafe"

	"github.com/ebitengine/purego"
	"github.com/egoist/mygo/ui"
	"golang.org/x/sys/windows"
)

func registerFontFile(path, family string) error {
	file, err := os.Open(path)
	if err != nil {
		return err
	}
	defer file.Close()
	info, err := file.Stat()
	if err != nil {
		return err
	}
	if info.Size() <= 0 || info.Size() > 64<<20 {
		return fmt.Errorf("invalid font size: %d", info.Size())
	}
	mapping, err := windows.CreateFileMapping(windows.Handle(file.Fd()), nil, windows.PAGE_READONLY, 0, 0, nil)
	if err != nil {
		return err
	}
	defer windows.CloseHandle(mapping)
	// A typed native return keeps the mapped address a pointer throughout;
	// converting a stored uintptr to a Go pointer violates checkptr rules.
	var mapView func(windows.Handle, uint32, uint32, uint32, uintptr) unsafe.Pointer
	purego.RegisterFunc(&mapView, windows.NewLazySystemDLL("kernel32.dll").NewProc("MapViewOfFile").Addr())
	view := mapView(mapping, windows.FILE_MAP_READ, 0, 0, uintptr(info.Size()))
	if view == nil {
		return windows.GetLastError()
	}
	defer windows.UnmapViewOfFile(uintptr(view))
	// MyGo's Windows DirectWrite loader copies synchronously before returning.
	// No pointer into this mapping survives RegisterFont, so it can close here.
	return ui.RegisterFont(unsafe.Slice((*byte)(view), int(info.Size())), family)
}
