//go:build dotisle_mygo_fontfile && (amd64 || arm64)

package ui

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/egoist/mygo/ui"
)

func registerFontFile(path, family string) error {
	path, err := filepath.Abs(path)
	if err != nil {
		return err
	}
	info, err := os.Stat(path)
	if err != nil {
		return err
	}
	if !info.Mode().IsRegular() || info.Size() <= 0 || info.Size() > 64<<20 {
		return fmt.Errorf("invalid font file: %s", path)
	}
	return ui.RegisterFontFile(path, family)
}
