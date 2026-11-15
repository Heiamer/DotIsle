package main

import (
	"encoding/json"
	"os"
	"path/filepath"

	"dotisle/settings/desktop"
	"dotisle/settings/themes"
	"dotisle/settings/ui"
)

// Health runs before the single-instance gate and never starts the UI or core.
func verifyCenter(output string, capture bool) error {
	if err := os.MkdirAll(output, 0755); err != nil {
		return err
	}
	_, err := desktop.Layout(nil)
	if err == nil {
		_, err = themes.SchemeOf(0x0088ff, false)
	}
	if err == nil && capture {
		err = ui.Verify(filepath.Join(output, "control-center"))
	}
	if err == nil && capture {
		err = desktop.Verify(filepath.Join(output, "desktop"))
	}
	report := map[string]any{"success": err == nil, "frontend": "go", "desktop": "C ABI 1"}
	if err != nil {
		report["error"] = err.Error()
	}
	raw, _ := json.MarshalIndent(report, "", "  ")
	writeErr := os.WriteFile(filepath.Join(output, "report.json"), raw, 0644)
	if err != nil {
		return err
	}
	return writeErr
}
