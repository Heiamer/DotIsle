// Package main prepares a private, patched copy of the pinned MyGo module.
package main

import (
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"go/format"
	"log"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
)

const modulePath = "github.com/egoist/mygo"
const moduleVersion = "v0.9.0"

type edit struct{ from, to string }

var patches = map[string][]edit{
	"internal/gpu/d3d11/device/device.go": {
		{`"fmt"
	"syscall"`, `"fmt"
    "sync/atomic"
	"syscall"`},
		{`func New() (device, context uintptr, software bool, err error) {
	return create(0, hardware, warp)
}`, `func New() (device, context uintptr, software bool, err error) {
    if hardwareOnly.Load() { return create(0, hardware) }
	return create(0, hardware, warp)
}

var hardwareOnly atomic.Bool

// RequireHardware disables WARP for both window rendering and native chrome.
func RequireHardware() { hardwareOnly.Store(true) }`},
	},
	"ui/window.go": {
		{`type Content struct {
	view func(*context)
}`, `type Content struct {
	view func(*context)
    gpuFailure func(error)
    gpuPresent func(time.Time)
}`},
		{`h := &windowHost{conn: conn}`, `h := &windowHost{conn: conn, gpuFailure: v.gpuFailure, gpuPresent: v.gpuPresent}`},
		{`gpu  gpuRenderer`, `gpu  gpuRenderer
    gpuFailure func(error)
    gpuPresent func(time.Time)
    gpuErr error
    gpuFailed bool`},
		{`if h.detached {
		return false // the window closed: it draws nothing more`, `if h.detached || h.gpuFailed {
		return false // no further frames after closing or a terminal GPU error`},
		{`defer h.ready()
	if h.render(s) {`, `if h.render(s) {
        defer h.ready()`},
		{`h.path = "drawn in memory"
	damage := h.soft.Render(s)`, `if h.gpuFailure != nil {
        // Don't show a blank first frame or execute the software rasterizer.
        h.gpuFailed = true
        err := h.gpuErr
        if err == nil { err = errors.New("no hardware renderer") }
        if h.conn.Post != nil { h.conn.Post(func() { h.gpuFailure(err) }) } else { h.gpuFailure(err) }
        return
    }
    defer h.ready()
	h.path = "drawn in memory"
	damage := h.soft.Render(s)`},
		{`log.Printf("mygo: drawing without the GPU: %v", err)
		g.Release()`, `h.gpuErr = err
		log.Printf("mygo: GPU frame failed: %v", err)
		g.Release()`},
		{`if os.Getenv("MYGO_GPU") == "0" || n == (platform.SurfaceNative{}) {
		return
	}
	r, err := newGPU(n)`, `if n == (platform.SurfaceNative{}) || (h.gpuFailure == nil && os.Getenv("MYGO_GPU") == "0") {
		return
	}
	r, err := newGPU(n)
    if h.gpuFailure != nil && err == nil && r != nil && software(r) {
        r.Release()
        r = nil
        err = errors.New("software D3D11 renderer rejected")
    }`},
		{`log.Printf("mygo: drawing without the GPU: %v", err)
		if retry`, `h.gpuErr = err
		log.Printf("mygo: GPU renderer unavailable: %v", err)
		if retry`},
		{`h.gpu, h.gpuSince = r, time.Now()`, `h.gpu, h.gpuSince = r, time.Now()
    h.gpuErr = nil
    if h.gpuPresent != nil {
        if p, ok := r.(interface{ SetPresentObserver(func(time.Time)) }); ok { p.SetPresentObserver(h.gpuPresent) }
    }`},
		{`func (h *windowHost) requestFrame() {
	h.frameAsked`, `func (h *windowHost) requestFrame() {
    if h.gpuFailed { return }
	h.frameAsked`},
	},
	"internal/gpu/transform.go": {
		{`const idleMaskBytes = 32 << 20`, `// DotIsle: keep two MiB of old clip masks per builder. Current masks are untouched.
const idleMaskBytes = 2 << 20`},
	},
	"internal/gpu/d3d11/d3d11.go": {
		{`"os"
	"sync"`, `"os"
	"sort"
	"sync"`},
		{`if r.frame-r.swept >= sweepEvery {`, `if r.presentObserver != nil && uint32(hr) == 0 && !r.dev.warp { r.presentObserver(time.Now()) }
    r.trimIdleTextures()
	if r.frame-r.swept >= sweepEvery {`},
		{`type Renderer struct {`, `type Renderer struct {
    presentObserver func(time.Time)`},
		{`// presentParams returns`, `// SetPresentObserver reports successful, non-occluded hardware presents.
func (r *Renderer) SetPresentObserver(fn func(time.Time)) { r.presentObserver = fn }

// DotIsle: cap textures from previous pages even if another scheduled frame
// never comes. Do not evict anything in the current scene or touch the
// DXGI buffers, atlases, or shared video frames.
func (r *Renderer) trimIdleTextures() {
	const idleBytes = 6 << 20
	type entry struct {
		id, frame uint64
		bytes int
		canvas bool
	}
	size := func(t *texture) int { return t.w*t.h*4 }
	var bytes int
	for _, tex := range r.images {
		if tex.lastFrame != r.frame { bytes += size(tex) }
	}
	for id, tex := range r.canvases {
		if tex.lastFrame == r.frame { continue }
		c := tex.canvas.Value()
		if c == nil || c.Transient {
			freeCanvas(tex)
			delete(r.canvases, id)
		} else {
			bytes += size(&tex.texture)
		}
	}
	if bytes <= idleBytes { return }
	stale := make([]entry, 0, len(r.images)+len(r.canvases))
	for id, tex := range r.images {
		if tex.lastFrame != r.frame { stale = append(stale, entry{id, tex.lastFrame, size(tex), false}) }
	}
	for id, tex := range r.canvases {
		if tex.lastFrame != r.frame { stale = append(stale, entry{id, tex.lastFrame, size(&tex.texture), true}) }
	}
	sort.Slice(stale, func(i, j int) bool { return stale[i].frame < stale[j].frame })
	for _, e := range stale {
		if bytes <= idleBytes { break }
		if e.canvas {
			freeCanvas(r.canvases[e.id])
			delete(r.canvases, e.id)
		} else {
			freeTexture(r.images[e.id])
			delete(r.images, e.id)
		}
		bytes -= e.bytes
	}
}

// presentParams returns`},
	},
	"internal/text/dwrite_windows.go": {
		{`set, err := e.fileSet(file)`, `return e.registerFile(file, family)
}

// RegisterFontFile adds a local font without copying it into a private buffer.
func (s *System) RegisterFontFile(path, family string) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	e, ok := s.engine().(*dwrite)
	if !ok || e.factory5 == 0 {
		return errors.New("mygo: adding fonts needs Windows 10 version 1703 or later")
	}
	name, err := syscall.UTF16PtrFromString(path)
	if err != nil {
		return err
	}
	const factoryCreateFontFileReference = 7 // IDWriteFactory::CreateFontFileReference
	var file uintptr
	hr := call(e.factory, factoryCreateFontFileReference, uintptr(unsafe.Pointer(name)), 0, uintptr(unsafe.Pointer(&file)))
	runtime.KeepAlive(name)
	if failed(hr) {
		return fmt.Errorf("mygo: cannot open font file (HRESULT %#08x)", uint32(hr))
	}
	if err := e.registerFile(file, family); err != nil {
		return err
	}
	s.fontsChanged()
	return nil
}

// registerFile takes ownership of the font file reference.
func (e *dwrite) registerFile(file uintptr, family string) error {
	set, err := e.fileSet(file)`},
	},
	"internal/text/layout.go": {
		{`	// The system's families may hold the app's under their own names.`, `	s.fontsChanged()
	return nil
}

func (s *System) fontsChanged() {
	// The system's families may hold the app's under their own names.`},
		{`clear(s.marks)
	return nil
}

// A FontFamily`, `clear(s.marks)
}

// A FontFamily`},
	},
}

const fontAPI = `//go:build windows && (amd64 || arm64)

package ui

import "github.com/egoist/mygo/internal/text"

// RegisterFontFile registers a font by absolute file path. Keep the file in
// place for the lifetime of the process.
func RegisterFontFile(path, family string) error {
	return text.Shared().RegisterFontFile(path, family)
}
`

const hardwareAPI = `//go:build windows && (amd64 || arm64)

package ui

import (
    "time"
    "github.com/egoist/mygo/internal/gpu/d3d11/device"
)

// RequireHardwareRendering rejects both software D3D and memory rasterization
// for this content. Call before creating the window; WARP is disabled in the
// process so native chrome cannot independently fall back to it.
func (c *Content) RequireHardwareRendering(onFailure func(error), onPresent func(time.Time)) {
    if onFailure == nil { panic("mygo: missing hardware failure handler") }
    device.RequireHardware()
    c.gpuFailure = onFailure
    c.gpuPresent = onPresent
}
`

const hardwareTests = `package ui

import (
    "errors"
    "testing"
)

func TestDotIsleHardwareOnly(t *testing.T) {
    t.Run("hardware ignores software override", func(t *testing.T) {
        r := &testGPU{}
        h, s, frame := gpuHost(t, func() (gpuRenderer, error) { return r, nil })
        t.Setenv("MYGO_GPU", "0")
        h.gpuFailure = func(err error) { t.Fatal(err) }
        frame()
        if r.frames != 1 || s.pixels != 0 || !h.shown { t.Fatal("hardware did not present") }
    })
    t.Run("no device", func(t *testing.T) {
        calls := 0
        h, s, frame := gpuHost(t, func() (gpuRenderer, error) { return nil, errors.New("no device") })
        h.gpuFailure = func(error) { calls++ }
        frame()
        frame()
        if s.pixels != 0 || h.shown || calls != 1 { t.Fatal("failed window presented or retried") }
    })
    t.Run("removed device cannot fall back to WARP", func(t *testing.T) {
        hw, sw := &testGPU{}, &softGPU{}
        lost := false
        h, s, frame := gpuHost(t, func() (gpuRenderer, error) {
            if lost { return sw, nil }
            return hw, nil
        })
        calls := 0
        h.gpuFailure = func(error) { calls++ }
        frame()
        hw.fail, lost = true, true
        frame()
        frame()
        if hw.frames != 1 || hw.released != 1 || sw.frames != 0 || sw.released != 1 || s.pixels != 0 || calls != 1 {
            t.Fatal("device loss fell back or did not stop the window")
        }
    })
}
`

func main() {
	goExe := flag.String("go", "go", "Go toolchain used for the build")
	out := flag.String("out", "", "path for the generated .mod file")
	flag.Parse()
	if err := prepare(*goExe, *out); err != nil {
		log.Fatal(err)
	}
}

func prepare(goExe, out string) error {
	if filepath.Ext(out) != ".mod" || filepath.Base(out) == "go.mod" {
		return errors.New("-out must name an alternate .mod file")
	}
	cmd := exec.Command(goExe, "mod", "download", "-json", modulePath)
	cmd.Stderr = os.Stderr
	raw, err := cmd.Output()
	if err != nil {
		return err
	}
	var mod struct{ Dir, Version string }
	if err := json.Unmarshal(raw, &mod); err != nil {
		return err
	}
	if mod.Version != moduleVersion || mod.Dir == "" {
		return fmt.Errorf("MyGo patch requires %s %s, got %s", modulePath, moduleVersion, mod.Version)
	}
	out, err = filepath.Abs(out)
	if err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(out), 0755); err != nil {
		return err
	}
	generated, err := os.MkdirTemp(filepath.Dir(out), "mygo-"+moduleVersion+"-")
	if err != nil {
		return err
	}
	if err := os.CopyFS(generated, os.DirFS(mod.Dir)); err != nil {
		return err
	}
	write := func(rel, source string) error {
		file := filepath.Join(generated, rel)
		if err := os.MkdirAll(filepath.Dir(file), 0755); err != nil {
			return err
		}
		buf, err := format.Source([]byte(source))
		if err != nil {
			return fmt.Errorf("format %s: %w", rel, err)
		}
		return os.WriteFile(file, buf, 0644)
	}
	for rel, edits := range patches {
		buf, err := os.ReadFile(filepath.Join(mod.Dir, rel))
		if err != nil {
			return err
		}
		source := strings.ReplaceAll(string(buf), "\r\n", "\n")
		for _, patch := range edits {
			if strings.Count(source, patch.from) != 1 {
				return fmt.Errorf("MyGo patch anchor not unique in %s: %q", rel, patch.from)
			}
			source = strings.Replace(source, patch.from, patch.to, 1)
		}
		if err := write(rel, source); err != nil {
			return err
		}
	}
	if err := write("ui/dotisle_font_file_windows.go", fontAPI); err != nil {
		return err
	}
	if err := write("ui/dotisle_hardware_windows.go", hardwareAPI); err != nil {
		return err
	}
	if err := write("ui/dotisle_hardware_test.go", hardwareTests); err != nil {
		return err
	}
	for _, suffix := range []string{".mod", ".sum"} {
		buf, err := os.ReadFile("go" + suffix)
		if err != nil {
			return err
		}
		if err := os.WriteFile(strings.TrimSuffix(out, ".mod")+suffix, buf, 0644); err != nil {
			return err
		}
	}
	editMod := exec.Command(goExe, "mod", "edit", "-modfile="+out, "-replace="+modulePath+"="+filepath.ToSlash(generated))
	editMod.Stdout, editMod.Stderr = os.Stdout, os.Stderr
	return editMod.Run()
}
