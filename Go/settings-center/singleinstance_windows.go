//go:build windows

// Single-instance guard for the control center window: a named mutex keyed by the
// data directory. A second launch simply exits; the island re-opens the running
// window through pendingPage.
package main

import (
	"golang.org/x/sys/windows"
)

func acquireSingleInstance(data string) bool {
	name := "Local.DotIsle.Center." + sanitizeMutexKey(data)
	p, err := windows.UTF16PtrFromString(name)
	if err != nil {
		return true // unusable name: run without the guard rather than not at all
	}
	_, err = windows.CreateMutex(nil, false, p)
	if err == nil {
		return true
	}
	return err != windows.ERROR_ALREADY_EXISTS
}

func sanitizeMutexKey(data string) string {
	out := make([]rune, 0, len(data))
	for _, r := range data {
		switch {
		case r >= 'A' && r <= 'Z':
			out = append(out, r+'a'-'A')
		case (r >= 'a' && r <= 'z') || (r >= '0' && r <= '9'):
			out = append(out, r)
		default:
			out = append(out, '-')
		}
	}
	return string(out)
}

// centerShowEvent is signalled by a second launch so the running center raises its
// existing window instead of the process silently disappearing.
func centerShowEvent(data string) string {
	return "Local.DotIsle.Center.Show." + sanitizeMutexKey(data)
}

func signalCenterShow(data string) {
	p, err := windows.UTF16PtrFromString(centerShowEvent(data))
	if err != nil {
		return
	}
	h, err := windows.CreateEvent(nil, 0, 0, p)
	if err != nil {
		return
	}
	defer windows.CloseHandle(h)
	_ = windows.SetEvent(h)
}

// watchCenterShow raises the window whenever another launch targets this data dir.
func watchCenterShow(data string, show func()) {
	p, err := windows.UTF16PtrFromString(centerShowEvent(data))
	if err != nil {
		return
	}
	h, err := windows.CreateEvent(nil, 0, 0, p)
	if err != nil {
		return
	}
	go func() {
		defer windows.CloseHandle(h)
		for {
			event, err := windows.WaitForSingleObject(h, windows.INFINITE)
			if err != nil || event != uint32(windows.WAIT_OBJECT_0) {
				return
			}
			show()
		}
	}()
}
