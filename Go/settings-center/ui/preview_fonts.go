package ui

import (
	"path/filepath"
)

var previewFontsLoaded = map[string]bool{}

// Optional preview fonts are only decoded when the user actually selects one.
// HarmonyOS Sans is the only eagerly registered interface family.
func ensurePreviewFont(family string) {
	if previewFontsLoaded[family] {
		return
	}
	previewFontsLoaded[family] = true
	files := map[string]string{
		"Google Sans Flex": "GoogleSansFlex-Regular.ttf", "Noto Sans SC": "NotoSansSC.ttf",
		"HarmonyOS Sans SC": "HarmonyOS_Sans_SC_Regular.ttf",
		"Noto Sans JP":      "NotoSansJP.ttf", "Space Mono": "SpaceMono-Regular.ttf",
		"Source Serif 4": "SourceSerif4.ttf", "Noto Serif SC": "NotoSerifSC.ttf",
		"Space Grotesk": "SpaceGrotesk.ttf", "Doto": "Doto.ttf", "Fusion Pixel": "FusionPixel12.ttf",
	}
	if name := files[family]; name != "" {
		_ = registerFontFile(filepath.Join(assetsRoot(), "Fonts", name), "")
	}
}
