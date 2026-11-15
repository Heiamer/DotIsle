package ui

import (
	"encoding/json"
	"io"
	"os"
	"path/filepath"
)

// LoadStartupSettings reads the persisted opening switch once, before the first
// frame. It never waits for the pipe; later snapshots override this fallback.
func (a *App) LoadStartupSettings() {
	if a.startupSettingsLoaded {
		return
	}
	a.startupSettingsLoaded = true
	a.startupSignatureEnabled = true
	file, err := os.Open(filepath.Join(a.Data, "settings.json"))
	if err != nil {
		return
	}
	defer file.Close()
	raw, err := io.ReadAll(io.LimitReader(file, 1024*1024+1))
	if err != nil || len(raw) > 1024*1024 {
		return
	}
	var settings map[string]any
	if json.Unmarshal(raw, &settings) != nil {
		return
	}
	if enabled, ok := settings["StartupSignatureEnabled"].(bool); ok {
		a.startupSignatureEnabled = enabled
	}
}

func (a *App) signatureEnabled() bool {
	if enabled, ok := a.settings()["StartupSignatureEnabled"].(bool); ok {
		return enabled
	}
	return !a.startupSettingsLoaded || a.startupSignatureEnabled
}
