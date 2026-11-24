package prefs

import (
	"encoding/json"
	"os"
)

type ReleaseNotesReceipt struct {
	Version string `json:"version"`
}

func ReadReleaseNotesReceipt(path string) ReleaseNotesReceipt {
	var receipt ReleaseNotesReceipt
	if raw, err := os.ReadFile(path); err == nil && len(raw) <= 1024 {
		_ = json.Unmarshal(raw, &receipt)
	}
	return receipt
}

func SaveReleaseNotesReceipt(path, version string) error {
	return atomicSave(path, ReleaseNotesReceipt{Version: version})
}
