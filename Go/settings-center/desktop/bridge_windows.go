//go:build windows

// Package desktop embeds the desktop renderer through a small, versioned C ABI.
// Go owns the state pump; Rust owns only widget windows on its pinned UI thread.
package desktop

import (
	"encoding/json"
	"fmt"
	"golang.org/x/sys/windows"
	"os"
	"path/filepath"
	"runtime"
	"sync"
	"time"
	"unsafe"
)

var library struct {
	sync.Mutex
	dll                                 *windows.DLL
	run, snapshot, stop, layout, verify *windows.Proc
}

func load() error {
	library.Lock()
	defer library.Unlock()
	if library.dll != nil {
		return nil
	}
	exe, err := os.Executable()
	if err != nil {
		return err
	}
	path := os.Getenv("DOTISLE_DESKTOP_DLL")
	if path == "" {
		path = filepath.Join(filepath.Dir(exe), "dotisle_desktop.dll")
	}
	dll, err := windows.LoadDLL(path)
	if err != nil {
		return fmt.Errorf("桌面组件模块未能加载：%w", err)
	}
	abi, err := dll.FindProc("dotisle_desktop_abi")
	if err != nil {
		dll.Release()
		return err
	}
	version, _, _ := abi.Call()
	if version != 1 {
		dll.Release()
		return fmt.Errorf("桌面组件接口版本不兼容：%d", version)
	}
	procs := make([]*windows.Proc, 5)
	for i, name := range []string{"dotisle_desktop_run", "dotisle_desktop_snapshot", "dotisle_desktop_stop", "dotisle_desktop_layout", "dotisle_desktop_verify"} {
		procs[i], err = dll.FindProc(name)
		if err != nil {
			dll.Release()
			return err
		}
	}
	library.dll = dll
	library.run, library.snapshot, library.stop, library.layout = procs[0], procs[1], procs[2], procs[3]
	library.verify = procs[4]
	return nil
}

type Host struct {
	data     string
	started  bool
	done     chan error
	finished bool
}

func hasStandaloneAllegro() bool {
	if os.Getenv("DOTISLE_FORCE_INPROC_DESKTOP") == "1" {
		return false
	}
	exe, err := os.Executable()
	if err != nil {
		return false
	}
	dir := filepath.Dir(exe)
	for _, name := range []string{"DotIsle-Allegro.exe", "DotIsle.Allegro.exe"} {
		if _, err := os.Stat(filepath.Join(dir, name)); err == nil {
			return true
		}
	}
	return false
}

func New(data string) *Host { return &Host{data: data} }
func (h *Host) Apply(state map[string]any) error {
	if h.finished {
		return nil
	}
	if !h.started && hasStandaloneAllegro() {
		// 存在独立 DotIsle-Allegro.exe 进程时交由外置引擎托管桌面画布，避免进程内双开冲突。
		return nil
	}
	if h.started {
		select {
		case err := <-h.done:
			h.finished = true
			return err
		default:
		}
	}
	if !h.started {
		items, _ := state["desktopWidgets"].([]any)
		if len(items) == 0 {
			return nil
		}
	}
	if err := load(); err != nil {
		return err
	}
	raw, err := json.Marshal(state)
	if err != nil {
		return err
	}
	code, _, _ := library.snapshot.Call(uintptr(unsafe.Pointer(&raw[0])), uintptr(len(raw)))
	runtime.KeepAlive(raw)
	if code != 0 {
		return fmt.Errorf("桌面组件状态同步失败：%d", code)
	}
	if !h.started {
		h.started = true
		h.done = make(chan error, 1)
		path := []byte(h.data)
		go func() {
			runtime.LockOSThread()
			defer runtime.UnlockOSThread()
			code, _, _ := library.run.Call(uintptr(unsafe.Pointer(&path[0])), uintptr(len(path)))
			h.done <- fmt.Errorf("桌面组件运行已结束（代码 %d），请重新启动控制中心", code)
			runtime.KeepAlive(path)
		}()
	}
	return nil
}
func (h *Host) Stop() {
	if h.started && !h.finished {
		library.stop.Call()
		select {
		case <-h.done:
			h.finished = true
		case <-time.After(5 * time.Second):
		}
	}
}

// Layout uses the same validation and geometry as the native music island.
func Layout(value any) (map[string]any, error) {
	if err := load(); err != nil {
		return nil, err
	}
	raw, err := json.Marshal(value)
	if err != nil {
		return nil, err
	}
	out := make([]byte, 65536)
	n, _, _ := library.layout.Call(uintptr(unsafe.Pointer(&raw[0])), uintptr(len(raw)), uintptr(unsafe.Pointer(&out[0])), uintptr(len(out)))
	runtime.KeepAlive(raw)
	if int(n) <= 0 || n > uintptr(len(out)) {
		return nil, fmt.Errorf("布局接口返回无效长度：%d", int(n))
	}
	var result map[string]any
	if err = json.Unmarshal(out[:n], &result); err != nil {
		return nil, err
	}
	if message, _ := result["error"].(string); message != "" {
		return nil, fmt.Errorf("%s", message)
	}
	return result, nil
}

// Verify captures every widget kind and size with the shipped Rust renderer.
func Verify(output string) error {
	if err := load(); err != nil {
		return err
	}
	raw := []byte(output)
	if len(raw) == 0 {
		return fmt.Errorf("验收输出目录为空")
	}
	code, _, _ := library.verify.Call(uintptr(unsafe.Pointer(&raw[0])), uintptr(len(raw)))
	runtime.KeepAlive(raw)
	if code != 0 {
		return fmt.Errorf("桌面组件渲染验收失败：%d", code)
	}
	return nil
}
