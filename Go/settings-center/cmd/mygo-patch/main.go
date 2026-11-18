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
