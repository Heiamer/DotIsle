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

//go:embed assets/fonts/HarmonyOS_*.ttf
var controlFonts embed.FS

var fontsRegistered bool

// RegisterFonts embeds the control center's HarmonyOS Sans faces. Native island
// preview faces remain available for the user's independent font selection.
func RegisterFonts() {
	if fontsRegistered {
		return
	}
	fontsRegistered = true
	for _, family := range []string{"HarmonyOS_Sans", "HarmonyOS_Sans_SC"} {
		alias := "HarmonyOS Sans"
		if family == "HarmonyOS_Sans_SC" {
			alias = "HarmonyOS Sans SC"
		}
		for _, weight := range []string{"Regular", "Medium", "Bold"} {
			name := family + "_" + weight + ".ttf"
			raw, err := controlFonts.ReadFile("assets/fonts/" + name)
			if err != nil {
				log.Printf("read UI font %s: %v", name, err)
				continue
			}
			if err = ui.RegisterFont(raw, alias); err != nil {
				log.Printf("register %s: %v", name, err)
			}
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
