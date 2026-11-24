// Control center uses OS fallbacks plus on-demand shared fonts from Assets/Fonts
// without embedding a duplicate multi-megabyte font payload inside the executable.
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
		if st, err := os.Stat(filepath.Join(candidate, "Sounds", "notification-soft.wav")); err == nil && st.Mode().IsRegular() {
			return candidate
		}
		if st, err := os.Stat(filepath.Join(candidate, "Fonts", "NotoSansSC.ttf")); err == nil && st.Mode().IsRegular() {
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

var sharedFonts = []struct {
	file   string
	family string
}{
	{"GoogleSansFlex-Regular.ttf", "Google Sans Flex"},
	{"GoogleSansFlex-Medium.ttf", "Google Sans Flex"},
	{"GoogleSansFlex-SemiBold.ttf", "Google Sans Flex"},
	{"GoogleSansFlex-Bold.ttf", "Google Sans Flex"},
	{"NotoSansSC.ttf", "Noto Sans SC"},
}

var fontsRegistered bool

// RegisterFonts loads the shared Google Sans Flex and Noto Sans SC faces
// directly from Assets/Fonts without duplicating fonts inside the center binary.
func RegisterFonts() {
	if fontsRegistered {
		return
	}
	fontsRegistered = true
	root := assetsRoot()
	if root == "" {
		return
	}
	for _, f := range sharedFonts {
		p := filepath.Join(root, "Fonts", f.file)
		if st, err := os.Stat(p); err != nil || !st.Mode().IsRegular() {
			continue
		}
		if err := registerFontFile(p, f.family); err != nil {
			log.Printf("register %s: %v", f.file, err)
		} else if f.file == "NotoSansSC.ttf" {
			previewFontsLoaded[f.family] = true
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
