// Embedded HarmonyOS Sans and control icons accompany the runtime's native preview fonts.
package ui

import (
	"embed"
	"log"
	"os"
	"path/filepath"
	"sync"

	"github.com/egoist/mygo/ui"
)

// assetsRoot finds the Assets directory: next to the exe when installed, else the
// repository root while developing from the source tree.
var assetDirectory = sync.OnceValue(func() string {
	var candidates []string
	if exe, err := os.Executable(); err == nil {
		candidates = append(candidates, filepath.Join(filepath.Dir(exe), "Assets"))
	}
	if dir, err := os.Getwd(); err == nil {
		for i := 0; i < 5; i++ {
			candidates = append(candidates, filepath.Join(dir, "Assets"))
			dir = filepath.Dir(dir)
		}
	}
	for _, candidate := range candidates {
		if st, err := os.Stat(filepath.Join(candidate, "Fonts", "HarmonyOS_Sans_SC_Regular.ttf")); err == nil && st.Mode().IsRegular() {
			return candidate
		}
	}
	return ""
})

func assetsRoot() string { return assetDirectory() }

var transportSVGs = map[string]*ui.SVG{}

// TransportSymbol loads the theme's own prev/play/next artwork. Reef keeps the
// shipped Material Symbols, like the island; every other theme reads its SVGs
// from the shared Assets folder so the preview matches the real control.
func TransportSymbol(theme, name string) *ui.SVG {
	fallback := map[string]string{"prev": "skip_previous", "play": "play_arrow", "next": "skip_next"}[name]
	if theme == "Reef" {
		return Symbol(fallback)
	}
	key := theme + "/" + name
	if svg, ok := transportSVGs[key]; ok {
		return svg
	}
	root := assetsRoot()
	if root == "" {
		return Symbol(fallback)
	}
	raw, err := os.ReadFile(filepath.Join(root, "TransportIcons", theme, name+".svg"))
	if err != nil {
		return Symbol(fallback)
	}
	svg := ui.MustParseSVG(raw)
	transportSVGs[key] = svg
	return svg
}

//go:embed assets/icons/*.svg
var controlIcons embed.FS

// Small Latin fallbacks stay embedded. The CJK faces are shared with the native
// island in Assets/Fonts, without a second 24 MB copy inside the executable.
//
//go:embed assets/fonts/HarmonyOS_Sans_Regular.ttf
var uiSansRegular []byte

//go:embed assets/fonts/HarmonyOS_Sans_Medium.ttf
var uiSansMedium []byte

//go:embed assets/fonts/HarmonyOS_Sans_Bold.ttf
var uiSansBold []byte

var uiFontFaces = []struct {
	name string
	face string
	data []byte
}{
	{"HarmonyOS_Sans_Regular.ttf", "HarmonyOS Sans", uiSansRegular},
	{"HarmonyOS_Sans_Medium.ttf", "HarmonyOS Sans", uiSansMedium},
	{"HarmonyOS_Sans_Bold.ttf", "HarmonyOS Sans", uiSansBold},
	{"HarmonyOS_Sans_SC_Regular.ttf", "HarmonyOS Sans SC", nil},
	{"HarmonyOS_Sans_SC_Medium.ttf", "HarmonyOS Sans SC", nil},
	{"HarmonyOS_Sans_SC_Bold.ttf", "HarmonyOS Sans SC", nil},
}

var fontsRegistered bool

// RegisterFonts uses the same complete font files as the native island.
func RegisterFonts() {
	if fontsRegistered {
		return
	}
	fontsRegistered = true
	for _, f := range uiFontFaces {
		var err error
		if len(f.data) != 0 {
			err = ui.RegisterFont(f.data, f.face)
		} else {
			err = registerFontFile(filepath.Join(assetsRoot(), "Fonts", f.name), f.face)
		}
		if err != nil {
			log.Printf("register %s: %v", f.name, err)
		} else if len(f.data) == 0 && f.name == "HarmonyOS_Sans_SC_Regular.ttf" {
			previewFontsLoaded[f.face] = true
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
