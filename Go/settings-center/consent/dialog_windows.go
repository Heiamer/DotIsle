//go:build windows

package consent

import (
	"encoding/binary"
	"runtime"
	"strings"
	"unsafe"

	"golang.org/x/sys/windows"
)

// A fixed-size modal dialog keeps the full document scrollable on small screens.
// Its default decision is rejection; closing it never records acceptance.
func documentDialog() bool {
	var template []byte
	word := func(v uint16) { template = binary.LittleEndian.AppendUint16(template, v) }
	dword := func(v uint32) { template = binary.LittleEndian.AppendUint32(template, v) }
	wide := func(s string) {
		v, _ := windows.UTF16FromString(s)
		for _, c := range v {
			word(c)
		}
	}
	dword(0x80c808c0) // WS_POPUP | CAPTION | SYSMENU | DS_CENTER | DS_SETFONT
	dword(0)
	word(4)
	for _, v := range []uint16{0, 0, 450, 280, 0, 0} {
		word(v)
	}
	wide("点屿 · 使用声明与免责条款")
	word(10)
	wide("Microsoft YaHei UI")
	control := func(style uint32, x, y, w, h, id, class uint16, title string) {
		for len(template)%4 != 0 {
			template = append(template, 0)
		}
		dword(style)
		dword(0)
		for _, v := range []uint16{x, y, w, h, id, 0xffff, class} {
			word(v)
		}
		wide(title)
		word(0)
	}
	control(0x50a10844, 10, 10, 430, 218, 1001, 0x81, "")
	control(0x50000000, 10, 234, 430, 12, 1002, 0x82, "请阅读全文，滚动至文末后才能同意。")
	control(0x58010000, 210, 252, 110, 18, 6, 0x80, "同意并继续")
	control(0x50010001, 330, 252, 110, 18, 7, 0x80, "不同意并退出")
	user := windows.NewLazySystemDLL("user32.dll")
	end := user.NewProc("EndDialog")
	type scrollInfo struct {
		Size, Mask uint32
		Min, Max   int32
		Page       uint32
		Pos, Track int32
	}
	callback := windows.NewCallback(func(hwnd, msg, wp, lp uintptr) uintptr {
		switch msg {
		case 0x110: // WM_INITDIALOG
			user.NewProc("SetDlgItemTextW").Call(hwnd, 1001, lp)
			button, _, _ := user.NewProc("GetDlgItem").Call(hwnd, 7)
			user.NewProc("SetFocus").Call(button)
			user.NewProc("SetTimer").Call(hwnd, 1, 120, 0)
			return 0
		case 0x113: // WM_TIMER
			edit, _, _ := user.NewProc("GetDlgItem").Call(hwnd, 1001)
			info := scrollInfo{Mask: 0x17}
			info.Size = uint32(unsafe.Sizeof(info))
			ok, _, _ := user.NewProc("GetScrollInfo").Call(edit, 1, uintptr(unsafe.Pointer(&info)))
			if ok != 0 && int64(info.Pos)+int64(info.Page) > int64(info.Max) {
				button, _, _ := user.NewProc("GetDlgItem").Call(hwnd, 6)
				user.NewProc("EnableWindow").Call(button, 1)
				text, _ := windows.UTF16PtrFromString("已到文末，请决定是否同意。")
				user.NewProc("SetDlgItemTextW").Call(hwnd, 1002, uintptr(unsafe.Pointer(text)))
				user.NewProc("KillTimer").Call(hwnd, 1)
			}
			return 1
		case 0x111: // WM_COMMAND
			if id := wp & 0xffff; id == 6 || id == 7 || id == 2 {
				if id == 6 {
					button, _, _ := user.NewProc("GetDlgItem").Call(hwnd, 6)
					enabled, _, _ := user.NewProc("IsWindowEnabled").Call(button)
					if enabled == 0 {
						return 1
					}
				}
				end.Call(hwnd, id)
				return 1
			}
		case 0x10: // WM_CLOSE
			end.Call(hwnd, 2)
			return 1
		}
		return 0
	})
	text, _ := windows.UTF16PtrFromString(strings.ReplaceAll(strings.ReplaceAll(Text, "\r\n", "\n"), "\n", "\r\n"))
	result, _, _ := user.NewProc("DialogBoxIndirectParamW").Call(0, uintptr(unsafe.Pointer(&template[0])), 0, callback, uintptr(unsafe.Pointer(text)))
	runtime.KeepAlive(template)
	runtime.KeepAlive(text)
	return result == 6
}
