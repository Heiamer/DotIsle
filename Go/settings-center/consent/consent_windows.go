//go:build windows

// Package consent gates all normal startup before any data workers run.
package consent

import (
	"crypto/sha256"
	_ "embed"
	"encoding/hex"
	"encoding/json"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"time"
	"unsafe"

	"golang.org/x/sys/windows"
)

const Revision = "2026-11-17.1"

//go:embed disclaimer.zh-CN.txt
var Text string

func appVersion() string {
	path, err := os.Executable()
	if err == nil {
		for _, relative := range []string{"Assets/RuntimeFiles.json", "Portable.json"} {
			if b, err := os.ReadFile(filepath.Join(filepath.Dir(path), relative)); err == nil {
				var v struct {
					Version string `json:"version"`
				}
				if json.Unmarshal(b, &v) == nil && v.Version != "" {
					return v.Version
				}
			}
		}
	}
	return "5.0.8"
}

type record struct {
	AppVersion string `json:"appVersion"`
	Revision   string `json:"revision"`
	SHA256     string `json:"sha256"`
	Accepted   bool   `json:"accepted"`
	AcceptedAt int64  `json:"acceptedAtUnix"`
}

func digest() string {
	sum := sha256.Sum256([]byte(strings.ReplaceAll(Text, "\r\n", "\n")))
	return hex.EncodeToString(sum[:])
}
func accepted(data string) bool {
	b, err := os.ReadFile(filepath.Join(data, "consent.json"))
	if err != nil {
		return false
	}
	var r record
	// Consent is a one-time local decision; upgrades and document edits keep it.
	return json.Unmarshal(b, &r) == nil && r.Accepted && r.AcceptedAt > 0
}
func save(data string) error {
	if err := os.MkdirAll(data, 0700); err != nil {
		return err
	}
	b, err := json.MarshalIndent(record{appVersion(), Revision, digest(), true, time.Now().Unix()}, "", "  ")
	if err != nil {
		return err
	}
	f, err := os.CreateTemp(data, ".consent-*.tmp")
	if err != nil {
		return err
	}
	name := f.Name()
	defer os.Remove(name)
	if _, err = f.Write(b); err != nil {
		f.Close()
		return err
	}
	if err = f.Sync(); err != nil {
		f.Close()
		return err
	}
	if err = f.Close(); err != nil {
		return err
	}
	old, _ := windows.UTF16PtrFromString(name)
	dest, _ := windows.UTF16PtrFromString(filepath.Join(data, "consent.json"))
	return windows.MoveFileEx(old, dest, windows.MOVEFILE_REPLACE_EXISTING|windows.MOVEFILE_WRITE_THROUGH)
}
func message(text, title string, flags uintptr) uintptr {
	p, _ := windows.UTF16PtrFromString(text)
	t, _ := windows.UTF16PtrFromString(title)
	result, _, _ := windows.NewLazySystemDLL("user32.dll").NewProc("MessageBoxW").Call(0, uintptr(unsafe.Pointer(p)), uintptr(unsafe.Pointer(t)), flags)
	return result
}

func Ensure(data string) bool {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	// Serialize the center and island prompts for the same data directory.
	absolute, err := filepath.Abs(data)
	if err != nil {
		return false
	}
	sum := sha256.Sum256([]byte(strings.ToLower(strings.ReplaceAll(absolute, "/", "\\"))))
	name, _ := windows.UTF16PtrFromString("Local.DotIsle.Consent." + hex.EncodeToString(sum[:]))
	lock, err := windows.CreateMutex(nil, false, name)
	if err != nil {
		return false
	}
	defer windows.CloseHandle(lock)
	wait, err := windows.WaitForSingleObject(lock, windows.INFINITE)
	if err != nil || (wait != windows.WAIT_OBJECT_0 && wait != windows.WAIT_ABANDONED) {
		return false
	}
	defer windows.ReleaseMutex(lock)
	if accepted(data) {
		return true
	}
	if !documentDialog() {
		return false
	}
	if err := save(data); err != nil {
		message("无法保存本机同意记录，软件不会启动。请检查数据目录是否可写后重试。", "点屿 · 无法保存确认", 0x10010)
		return false
	}
	return true
}
