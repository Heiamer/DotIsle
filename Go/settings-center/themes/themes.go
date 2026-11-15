// Package themes calls the island's own dotisle_themes.dll over its stable C/JSON ABI
// (dotisle-module-api), so the Go front-end resolves palettes with exactly the same
// algorithm as the Vello UI — no duplicated color math.
//
// Win64 ABI notes: dotisle_service_call returns a 16-byte Buffer, which arrives through
// a hidden pointer as the first argument; dotisle_service_free takes the Buffer by
// pointer, as the ABI passes aggregates larger than 8 bytes by reference.
package themes

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"syscall"
	"unsafe"
)

type buffer struct {
	Ptr *byte
	Len uintptr
}

type library struct {
	module *syscall.DLL
	call   *syscall.Proc
	free   *syscall.Proc
}

var (
	mu       sync.Mutex
	lib      *library
	libErr   error
	loadedID string
)

func moduleDir() string {
	if dir := os.Getenv("DOTISLE_MODULE_DIR"); dir != "" {
		return dir
	}
	exe, err := os.Executable()
	if err != nil {
		return ""
	}
	return filepath.Dir(exe)
}

// open loads dotisle_themes.dll once and validates its manifest the way
// dotisle_module_api does. Callers must hold mu.
func open() (*library, error) {
	dir := moduleDir()
	if dir == "" {
		return nil, fmt.Errorf("模块目录不可用")
	}
	path := filepath.Join(dir, "dotisle_themes.dll")
	module, err := syscall.LoadDLL(path)
	if err != nil {
		return nil, fmt.Errorf("无法加载 %s：%v", path, err)
	}
	call, err := module.FindProc("dotisle_service_call")
	if err != nil {
		return nil, fmt.Errorf("模块缺少服务接口")
	}
	free, err := module.FindProc("dotisle_service_free")
	if err != nil {
		return nil, fmt.Errorf("模块缺少服务接口")
	}
	l := &library{module: module, call: call, free: free}
	// The manifest must say themes/ABI 1 before any call goes through.
	var manifest map[string]any
	if err := l.invoke(map[string]any{"method": "describe"}, &manifest); err != nil {
		return nil, err
	}
	if manifest["abi"] != float64(1) || manifest["id"] != "themes" {
		return nil, fmt.Errorf("模块与 CORE 不兼容")
	}
	return l, nil
}

func get() (*library, error) {
	mu.Lock()
	defer mu.Unlock()
	if lib != nil || libErr != nil {
		return lib, libErr
	}
	lib, libErr = open()
	return lib, libErr
}

func (l *library) invoke(request map[string]any, out any) error {
	input, err := json.Marshal(request)
	if err != nil {
		return err
	}
	if len(input) > 4*1024*1024 {
		return fmt.Errorf("模块请求过大")
	}
	var result buffer
	// Hidden return pointer first, then (ptr, len).
	r, _, callErr := l.call.Call(
		uintptr(unsafe.Pointer(&result)),
		uintptr(unsafe.Pointer(&input[0])),
		uintptr(len(input)),
	)
	if r == 0 && callErr != syscall.EINVAL {
		// Non-zero returns are the norm; a zero pointer means failure.
		if result.Ptr == nil {
			return fmt.Errorf("模块返回空缓冲区")
		}
	}
	defer func() {
		if result.Ptr != nil {
			l.free.Call(uintptr(unsafe.Pointer(&result)))
		}
	}()
	if result.Len > 4*1024*1024 {
		return fmt.Errorf("模块响应过大")
	}
	reply := make([]byte, result.Len)
	copy(reply, unsafe.Slice(result.Ptr, result.Len))
	var envelope struct {
		Ok    bool            `json:"ok"`
		Value json.RawMessage `json:"value"`
		Error string          `json:"error"`
	}
	if err := json.Unmarshal(reply, &envelope); err != nil {
		return err
	}
	if !envelope.Ok {
		return fmt.Errorf("%s", envelope.Error)
	}
	if out == nil {
		return nil
	}
	return json.Unmarshal(envelope.Value, out)
}

// Call sends {"method":..., "args":...} and fills out with reply.value.
func Call(method string, args map[string]any, out any) error {
	l, err := get()
	if err != nil {
		return err
	}
	mu.Lock()
	defer mu.Unlock()
	return l.invoke(map[string]any{"method": method, "args": args}, out)
}

// Scheme is the 16-role palette the Vello widgets consume.
type Scheme struct {
	Primary              uint32 `json:"primary"`
	OnPrimary            uint32 `json:"on_primary"`
	PrimaryContainer     uint32 `json:"primary_container"`
	OnPrimaryContainer   uint32 `json:"on_primary_container"`
	SecondaryContainer   uint32 `json:"secondary_container"`
	OnSecondaryContainer uint32 `json:"on_secondary_container"`
	TertiaryContainer    uint32 `json:"tertiary_container"`
	OnTertiaryContainer  uint32 `json:"on_tertiary_container"`
	Surface              uint32 `json:"surface"`
	Container            uint32 `json:"container"`
	High                 uint32 `json:"high"`
	OnSurface            uint32 `json:"on_surface"`
	OnSurfaceVariant     uint32 `json:"on_surface_variant"`
	Outline              uint32 `json:"outline"`
	OutlineVariant       uint32 `json:"outline_variant"`
	Error                uint32 `json:"error"`
}

// Roles is the full resolve_center role set before the neutral override.
type Roles struct {
	Primary                uint32 `json:"primary"`
	PrimaryContainer       uint32 `json:"primary_container"`
	SecondaryContainer     uint32 `json:"secondary_container"`
	TertiaryContainer      uint32 `json:"tertiary_container"`
	Surface                uint32 `json:"surface"`
	OnSurface              uint32 `json:"on_surface"`
	OnSurfaceVariant       uint32 `json:"on_surface_variant"`
	Outline                uint32 `json:"outline"`
	OutlineVariant         uint32 `json:"outline_variant"`
	SurfaceContainer       uint32 `json:"surface_container"`
	SurfaceContainerHigh   uint32 `json:"surface_container_high"`
	SurfaceContainerLow    uint32 `json:"surface_container_low"`
	SurfaceContainerLowest uint32 `json:"surface_container_lowest"`
}

// SchemeOf mirrors dotisle_domain::dynamic_color::scheme(seed, dark).
func SchemeOf(seed uint32, dark bool) (Scheme, error) {
	var s Scheme
	err := Call("scheme", map[string]any{"seed": seed, "dark": dark}, &s)
	return s, err
}

// RolesOf mirrors dynamic_color::roles(seed, dark, content, high_contrast).
func RolesOf(seed uint32, dark, content, highContrast bool) (Roles, error) {
	var r Roles
	err := Call("roles", map[string]any{
		"seed":          seed,
		"dark":          dark,
		"content":       content,
		"high_contrast": highContrast,
	}, &r)
	return r, err
}

// ResolveCenter mirrors dynamic_color::resolve_center — the control center palette source.
func ResolveCenter(requested string, content, system any, custom uint32, dark, high bool) (map[string]any, error) {
	var r map[string]any
	err := Call("resolve_center", map[string]any{
		"requested": requested,
		"content":   content,
		"system":    system,
		"custom":    custom,
		"dark":      dark,
		"high":      high,
	}, &r)
	return r, err
}

// RolesUi maps a resolved role set through themes "roles.ui" into a widget Scheme.
func RolesUi(roles map[string]any) (Scheme, error) {
	var s Scheme
	err := Call("roles.ui", map[string]any{"self": roles}, &s)
	return s, err
}

// Mix blends two role sets (themes "roles.mix") for palette transitions.
func RolesMix(self, other map[string]any, t float64) (map[string]any, error) {
	var r map[string]any
	err := Call("roles.mix", map[string]any{"self": self, "other": other, "t": t}, &r)
	return r, err
}
