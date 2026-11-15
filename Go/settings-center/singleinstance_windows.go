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
