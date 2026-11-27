package ui

import (
	"fmt"
	"github.com/egoist/mygo/ui"
	"golang.org/x/sys/windows"
	"runtime"
	"strings"
)

func sidebarKey(vk uint32) string {
	if vk >= 0x41 && vk <= 0x5a || vk >= 0x30 && vk <= 0x39 {
		return string(rune(vk))
	}
	if vk >= 0x70 && vk <= 0x7b {
		return fmt.Sprintf("F%d", vk-0x6f)
	}
	return map[uint32]string{0x20: "Space", 0xc0: "`", 0xbd: "-", 0xbb: "=", 0xbc: ",", 0xbe: ".", 0xbf: "/", 0xba: ";", 0xde: "'", 0xdb: "[", 0xdd: "]", 0xdc: "\\", 0x24: "Home", 0x23: "End", 0x21: "PageUp", 0x22: "PageDown", 0x25: "Left", 0x26: "Up", 0x27: "Right", 0x28: "Down", 0x2d: "Insert", 0x2e: "Delete", 0x0d: "Enter", 0x09: "Tab", 0x08: "Backspace"}[vk]
}
func sidebarChord(mods, key uint32) string {
	parts := []string{}
	for _, v := range []struct {
		bit  uint32
		name string
	}{{2, "Ctrl"}, {1, "Alt"}, {4, "Shift"}, {8, "Win"}} {
		if mods&v.bit != 0 {
			parts = append(parts, v.name)
		}
	}
	return strings.Join(append(parts, sidebarKey(key)), " + ")
}
func (s allegroSettings) hotkeyLabel() string {
	if s.CustomKey != 0 {
		return sidebarChord(s.CustomModifiers, s.CustomKey)
	}
	for _, v := range allegroHotkeys {
		if s.Hotkey == v[1] {
			return v[0]
		}
	}
	return "Alt + `"
}
func (s allegroSettings) editHotkeyLabel() string {
	if s.EditKey == 0 {
		return "Alt + E"
	}
	return sidebarChord(s.EditModifiers, s.EditKey)
}
func captureSidebarKey(ev ui.InputEvent) (uint32, uint32) {
	mods := uint32(0)
	for _, v := range []struct {
		ui  ui.Modifiers
		win uint32
	}{{ui.Alt, 1}, {ui.Ctrl, 2}, {ui.Shift, 4}, {ui.Super, 8}} {
		if ev.Mods&v.ui != 0 {
			mods |= v.win
		}
	}
	key := ev.PhysicalKey
	if key == ui.KeyUnknown {
		key = ev.Key
	}
	letters := []ui.Key{ui.KeyA, ui.KeyB, ui.KeyC, ui.KeyD, ui.KeyE, ui.KeyF, ui.KeyG, ui.KeyH, ui.KeyI, ui.KeyJ, ui.KeyK, ui.KeyL, ui.KeyM, ui.KeyN, ui.KeyO, ui.KeyP, ui.KeyQ, ui.KeyR, ui.KeyS, ui.KeyT, ui.KeyU, ui.KeyV, ui.KeyW, ui.KeyX, ui.KeyY, ui.KeyZ}
	for i, k := range letters {
		if key == k {
			return mods, 0x41 + uint32(i)
		}
	}
	for i, k := range []ui.Key{ui.Key0, ui.Key1, ui.Key2, ui.Key3, ui.Key4, ui.Key5, ui.Key6, ui.Key7, ui.Key8, ui.Key9} {
		if key == k {
			return mods, 0x30 + uint32(i)
		}
	}
	for i, k := range []ui.Key{ui.KeyF1, ui.KeyF2, ui.KeyF3, ui.KeyF4, ui.KeyF5, ui.KeyF6, ui.KeyF7, ui.KeyF8, ui.KeyF9, ui.KeyF10, ui.KeyF11, ui.KeyF12} {
		if key == k {
			return mods, 0x70 + uint32(i)
		}
	}
	return mods, map[ui.Key]uint32{ui.KeySpace: 0x20, ui.KeyBackquote: 0xc0, ui.KeyMinus: 0xbd, ui.KeyEqual: 0xbb, ui.KeyComma: 0xbc, ui.KeyPeriod: 0xbe, ui.KeySlash: 0xbf, ui.KeySemicolon: 0xba, ui.KeyQuote: 0xde, ui.KeyBracketLeft: 0xdb, ui.KeyBracketRight: 0xdd, ui.KeyBackslash: 0xdc, ui.KeyHome: 0x24, ui.KeyEnd: 0x23, ui.KeyPageUp: 0x21, ui.KeyPageDown: 0x22, ui.KeyLeft: 0x25, ui.KeyUp: 0x26, ui.KeyRight: 0x27, ui.KeyDown: 0x28, ui.KeyInsert: 0x2d, ui.KeyDelete: 0x2e, ui.KeyEnter: 0x0d, ui.KeyTab: 0x09, ui.KeyBackspace: 0x08}[key]
}
func availableSidebarChord(mods, key uint32) bool {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	user := windows.NewLazySystemDLL("user32.dll")
	register := user.NewProc("RegisterHotKey")
	unregister := user.NewProc("UnregisterHotKey")
	ok, _, _ := register.Call(0, 0x5b60, uintptr(mods|0x4000), uintptr(key))
	if ok == 0 {
		return false
	}
	unregister.Call(0, 0x5b60)
	return true
}
func (a *App) recordSidebarHotkey(ev ui.InputEvent) bool { return a.recordAllegroHotkey(ev, false) }
func (a *App) recordEditHotkey(ev ui.InputEvent) bool    { return a.recordAllegroHotkey(ev, true) }
func (a *App) recordAllegroHotkey(ev ui.InputEvent, edit bool) bool {
	if ev.Kind != ui.InputKeyDown {
		return false
	}
	if ev.Key == ui.KeyEscape {
		a.allegroRecording = false
		a.allegroCaptureError = ""
		return true
	}
	mods, key := captureSidebarKey(ev)
	if key == 0 {
		return true
	}
	if mods == 0 {
		a.allegroCaptureError = "请同时按 Ctrl、Alt、Shift 或 Win 中至少一个修饰键。"
		return true
	}
	other, current := a.allegroSettings.editHotkeyLabel(), a.allegroSettings.hotkeyLabel()
	if edit {
		other, current = current, other
	}
	if sidebarChord(mods, key) == other {
		a.allegroCaptureError = "侧边栏与桌面编辑不能使用相同的组合键。"
		return true
	}
	same := sidebarChord(mods, key) == current
	if !same && !availableSidebarChord(mods, key) {
		a.allegroCaptureError = "这个组合已被占用或由系统保留，请换一个。"
		return true
	}
	a.allegroChange(func(s *allegroSettings) {
		if edit {
			s.EditModifiers = mods
			s.EditKey = key
		} else {
			s.CustomModifiers = mods
			s.CustomKey = key
		}
	})
	a.allegroRecording = false
	a.allegroCaptureError = ""
	return true
}
func (a *App) allegroCustomHotkey(c *ui.Context, p *Palette, x, y, w float32) {
	label := "自定义 · " + a.allegroSettings.hotkeyLabel()
	if a.allegroRecording && !a.allegroRecordingEdit {
		label = "请按下组合键…（Esc 取消）"
	}
	e := Button(c, p, "录入侧边栏热键", x, y, w, 36, label, "keyboard", true, true, func() {
		a.allegroRecording = !a.allegroRecording || a.allegroRecordingEdit
		a.allegroRecordingEdit = false
		a.allegroCaptureError = ""
	})
	if a.allegroRecording && !a.allegroRecordingEdit {
		e.AutoFocus().HandleInput(a.recordSidebarHotkey)
	}
	if a.allegroCaptureError != "" && !a.allegroRecordingEdit {
		Label(c, a.allegroCaptureError, x, y+40, 11, p.Scheme.Error, false, w)
	}
}

func (a *App) allegroEditHotkey(c *ui.Context, p *Palette, x, y, w float32) {
	label := "自定义 · " + a.allegroSettings.editHotkeyLabel()
	if a.allegroRecording && a.allegroRecordingEdit {
		label = "请按下组合键…（Esc 取消）"
	}
	e := Button(c, p, "录入桌面编辑热键", x, y, w, 36, label, "keyboard", true, true, func() {
		a.allegroRecording = !a.allegroRecording || !a.allegroRecordingEdit
		a.allegroRecordingEdit = true
		a.allegroCaptureError = ""
	})
	if a.allegroRecording && a.allegroRecordingEdit {
		e.AutoFocus().HandleInput(a.recordEditHotkey)
	}
	if a.allegroCaptureError != "" && a.allegroRecordingEdit {
		Label(c, a.allegroCaptureError, x, y+40, 11, p.Scheme.Error, false, w)
	}
}
