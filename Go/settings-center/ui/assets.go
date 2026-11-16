// Embedded HarmonyOS Sans and control icons accompany the runtime's native preview fonts.
package ui

import (
	"embed"
	"log"
	"os"
	"path/filepath"

	"github.com/egoist/mygo/ui"
)

// assetsRoot finds the Assets directory: next to the exe when installed, else the
// repository root while developing from the source tree.
func assetsRoot() string {
	var candidates []string
	if exe, err := os.Executable(); err == nil {
		candidates = append(candidates, filepath.Join(filepath.Dir(exe), "Assets"))
	}
	candidates = append(candidates,
		`C:\Users\miku\Documents\Trae-CN\temp_20261113_01\src\DotIsle-5.0.0-Source\Assets`,
		filepath.Join(".", "Assets"),
	)
	for _, c := range candidates {
		if st, err := os.Stat(c); err == nil && st.IsDir() {
			return c
		}
	}
	return ""
}

//go:embed assets/icons/*.svg
var controlIcons embed.FS

// The faces are embedded as byte slices, not read from controlFonts: the slice
// points into the executable's read-only data, while ReadFile copied every face
// (24 MB with the SC weights) onto the Go heap at startup. RegisterFont's DWrite
// loader copies the data itself, so nothing here needs a heap copy.
//
//go:embed assets/fonts/HarmonyOS_Sans_Regular.ttf
var uiSansRegular []byte

//go:embed assets/fonts/HarmonyOS_Sans_Medium.ttf
var uiSansMedium []byte

//go:embed assets/fonts/HarmonyOS_Sans_Bold.ttf
var uiSansBold []byte

//go:embed assets/fonts/HarmonyOS_Sans_SC_Regular.ttf
var uiSansSCRegular []byte

//go:embed assets/fonts/HarmonyOS_Sans_SC_Medium.ttf
var uiSansSCMedium []byte

//go:embed assets/fonts/HarmonyOS_Sans_SC_Bold.ttf
var uiSansSCBold []byte

var uiFontFaces = []struct {
	name string
	face string
	data []byte
}{
	{"HarmonyOS_Sans_Regular.ttf", "HarmonyOS Sans", uiSansRegular},
	{"HarmonyOS_Sans_Medium.ttf", "HarmonyOS Sans", uiSansMedium},
	{"HarmonyOS_Sans_Bold.ttf", "HarmonyOS Sans", uiSansBold},
	{"HarmonyOS_Sans_SC_Regular.ttf", "HarmonyOS Sans SC", uiSansSCRegular},
	{"HarmonyOS_Sans_SC_Medium.ttf", "HarmonyOS Sans SC", uiSansSCMedium},
	{"HarmonyOS_Sans_SC_Bold.ttf", "HarmonyOS Sans SC", uiSansSCBold},
}

var fontsRegistered bool

// RegisterFonts embeds the control center's HarmonyOS Sans faces. Native island
// preview faces remain available for the user's independent font selection.
func RegisterFonts() {
	if fontsRegistered {
		return
	}
	fontsRegistered = true
	for _, f := range uiFontFaces {
		if err := ui.RegisterFont(f.data, f.face); err != nil {
			log.Printf("register %s: %v", f.name, err)
		}
	}
}

var svgCache = map[string]*ui.SVG{}

// Symbol returns one of the redrawn control icons, parsed once.
func Symbol(name string) *ui.SVG {
	if svg, ok := svgCache[name]; ok {
		return svg
	}
	raw, err := controlIcons.ReadFile("assets/icons/" + name + ".svg")
	if err != nil {
		return nil
	}
	svg := ui.MustParseSVG(raw)
	svgCache[name] = svg
	return svg
}
