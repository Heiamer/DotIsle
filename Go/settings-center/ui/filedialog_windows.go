//go:build windows

// Native file and folder pickers (platform.rs choose_background / choose_module /
// the local-folder picker): comdlg32 GetOpenFileNameW for files and
// SHBrowseForFolderW for directories, both without cgo.
package ui

import (
	"fmt"
	"syscall"
	"unsafe"
)

const (
	ofnAllowMultiSelect = 0x200
	ofnPathMustExist    = 0x800
	ofnFileMustExist    = 0x1000
	ofnExplorer         = 0x00080000
	ofnHideReadOnly     = 0x4
	bifReturnOnlyFsDirs = 0x1
	bifEditBox          = 0x10
	bifNewDialogStyle   = 0x40
)

type openFileName struct {
	StructSize    uint32
	Owner         uintptr
	Instance      uintptr
	Filter        *uint16
	CustomFilter  *uint16
	MaxCustFilter uint32
	FilterIndex   uint32
	File          *uint16
	MaxFile       uint32
	FileTitle     *uint16
	MaxFileTitle  uint32
	InitialDir    *uint16
	Title         *uint16
	Flags         uint32
	FileOffset    uint16
	FileExtension uint16
	DefExt        *uint16
	CustData      uintptr
	FnHook        uintptr
	TemplateName  *uint16
	PvReserved    uintptr
	DwReserved    uint32
	FlagsEx       uint32
}

var (
	comdlg32                = syscall.NewLazyDLL("comdlg32.dll")
	procGetOpenFileName     = comdlg32.NewProc("GetOpenFileNameW")
	shell32                 = syscall.NewLazyDLL("shell32.dll")
	procSHBrowseForFolder   = shell32.NewProc("SHBrowseForFolderW")
	procSHGetPathFromIDList = shell32.NewProc("SHGetPathFromIDListW")
	ole32                   = syscall.NewLazyDLL("ole32.dll")
	procCoTaskMemFree       = ole32.NewProc("CoTaskMemFree")
)

// openFileDialog shows a file picker; filter is "desc\0*.ext;*.ext2\0" double-NUL
// terminated. Returns "" when cancelled.
func openFileDialog(owner uintptr, title, filter, initialDir string) string {
	utf16 := func(s string) *uint16 {
		p, _ := syscall.UTF16PtrFromString(s)
		return p
	}
	var filterBuf []uint16
	for _, r := range filter {
		filterBuf = append(filterBuf, uint16(r))
	}
	filterBuf = append(filterBuf, 0, 0)
	fileBuf := make([]uint16, 1024)
	initial := utf16(initialDir)
	var initPtr *uint16
	if initialDir != "" {
		initPtr = initial
	}
	ofn := openFileName{
		StructSize: uint32(unsafe.Sizeof(openFileName{})),
		Owner:      owner,
		Filter:     &filterBuf[0],
		File:       &fileBuf[0],
		MaxFile:    uint32(len(fileBuf)),
		InitialDir: initPtr,
		Title:      utf16(title),
		Flags:      ofnExplorer | ofnFileMustExist | ofnPathMustExist | ofnHideReadOnly,
	}
	r, _, _ := procGetOpenFileName.Call(uintptr(unsafe.Pointer(&ofn)))
	if r == 0 {
		return ""
	}
	return syscall.UTF16ToString(fileBuf)
}

type browseInfo struct {
	Owner       uintptr
	Root        uintptr
	DisplayName *uint16
	Title       *uint16
	Flags       uint32
	Callback    uintptr
	Param       uintptr
	Image       int32
}

// openFolderDialog shows the shell folder picker; returns "" when cancelled.
func openFolderDialog(owner uintptr, title string) string {
	titlePtr, _ := syscall.UTF16PtrFromString(title)
	display := make([]uint16, 260)
	bi := browseInfo{
		Owner:       owner,
		DisplayName: &display[0],
		Title:       titlePtr,
		Flags:       bifReturnOnlyFsDirs | bifEditBox | bifNewDialogStyle,
	}
	pidl, _, _ := procSHBrowseForFolder.Call(uintptr(unsafe.Pointer(&bi)))
	if pidl == 0 {
		return ""
	}
	pathBuf := make([]uint16, 260)
	ok, _, _ := procSHGetPathFromIDList.Call(pidl, uintptr(unsafe.Pointer(&pathBuf[0])))
	procCoTaskMemFree.Call(pidl)
	if ok == 0 {
		return ""
	}
	return syscall.UTF16ToString(pathBuf)
}

var _ = fmt.Sprintf
