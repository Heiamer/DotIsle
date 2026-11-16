//go:build !windows

package ui

import (
	"github.com/egoist/mygo/ui"
	"os"
)

func registerFontFile(path, family string) error {
	data, err := os.ReadFile(path)
	if err != nil {
		return err
	}
	return ui.RegisterFont(data, family)
}
