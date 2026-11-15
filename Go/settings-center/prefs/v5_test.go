package prefs

import (
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

func TestOldOverviewGainsIndependentScrollLayout(t *testing.T) {
	path := filepath.Join(t.TempDir(), "center-overview.json")
	if err := os.WriteFile(path, []byte(`{"view":"scroll","cards":["focus","music"]}`), 0600); err != nil {
		t.Fatal(err)
	}
	p, err := LoadOverview(path)
	if err != nil || !reflect.DeepEqual(p.Cards, []string{"focus", "music"}) || !reflect.DeepEqual(p.ScrollPages, DefaultOverview().ScrollPages) {
		t.Fatal("legacy layout changed", err)
	}
	p.ScrollPages = []string{"information", "music", "information", "unknown"}
	p.Information = []string{"widget:calendar", "widget:weather"}
	if err = SaveOverview(path, p); err != nil {
		t.Fatal(err)
	}
	restored, err := LoadOverview(path)
	if err != nil || !reflect.DeepEqual(restored.ScrollPages, []string{"information", "music"}) || !reflect.DeepEqual(restored.Information, p.Information) {
		t.Fatal("independent layout round trip failed", err)
	}
}
