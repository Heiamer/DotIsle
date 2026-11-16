//go:build windows

package consent

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

func TestAcceptanceSurvivesApplicationAndDocumentUpdates(t *testing.T) {
	data := t.TempDir()
	if accepted(data) {
		t.Fatal("missing agreement accepted")
	}
	if err := save(data); err != nil {
		t.Fatal(err)
	}
	if !accepted(data) {
		t.Fatal("explicit saved agreement rejected")
	}
	b, err := os.ReadFile(filepath.Join(data, "consent.json"))
	if err != nil {
		t.Fatal(err)
	}
	var r record
	if err = json.Unmarshal(b, &r); err != nil {
		t.Fatal(err)
	}
	for _, change := range []func(*record){func(r *record) { r.AppVersion = "old-version" }, func(r *record) { r.Revision = "old" }, func(r *record) { r.SHA256 = "old-text" }} {
		old := r
		change(&old)
		b, _ := json.Marshal(old)
		if err := os.WriteFile(filepath.Join(data, "consent.json"), b, 0600); err != nil {
			t.Fatal(err)
		}
		if !accepted(data) {
			t.Fatal("previous explicit agreement was forgotten after an update")
		}
	}
	for _, change := range []func(*record){func(r *record) { r.Accepted = false }, func(r *record) { r.AcceptedAt = 0 }} {
		bad := r
		change(&bad)
		b, _ := json.Marshal(bad)
		if err := os.WriteFile(filepath.Join(data, "consent.json"), b, 0600); err != nil {
			t.Fatal(err)
		}
		if accepted(data) {
			t.Fatal("missing or incomplete agreement accepted")
		}
	}
}
