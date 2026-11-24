package prefs

import (
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

func TestOverviewMigrationAndRoundTrip(t *testing.T) {
	path := filepath.Join(t.TempDir(), "center-overview.json")
	p, err := LoadOverview(path)
	if err != nil || !reflect.DeepEqual(p, DefaultOverview()) {
		t.Fatal("missing store did not use defaults")
	}
	p = Overview{View: "scroll", ScrollPlugin: true, Cards: []string{"quick", "music", "quick", "unknown", "widget:system"}}
	if err = SaveOverview(path, p); err != nil {
		t.Fatal(err)
	}
	restored, err := LoadOverview(path)
	if err != nil || restored.View != "scroll" || !reflect.DeepEqual(restored.Cards, []string{"quick", "music", "widget:system"}) {
		t.Fatalf("bad round trip: %+v %v", restored, err)
	}
	if NormalizeOverview(Overview{View: "scroll"}).View != "cards" {
		t.Fatal("scroll view must be opt-in")
	}
	raw := []byte(`{"view":"invalid","cards":[]}`)
	if err = os.WriteFile(path, raw, 0600); err != nil {
		t.Fatal(err)
	}
	restored, err = LoadOverview(path)
	if err != nil || !reflect.DeepEqual(restored, DefaultOverview()) {
		t.Fatal("invalid layout not repaired")
	}
}

func TestLegacyDefaultOverviewMigratesButCustomLayoutSurvives(t *testing.T) {
	legacy := []string{"music", "clock", "todos", "focus", "quick"}
	p := NormalizeOverview(Overview{Cards: legacy})
	if !reflect.DeepEqual(p.Cards, DefaultOverview().Cards) {
		t.Fatal("legacy defaults did not migrate", p.Cards)
	}
	sizes := map[string]CardSize{"music": {Width: .5, Height: 336}}
	p = NormalizeOverview(Overview{Cards: legacy, Sizes: sizes})
	if !reflect.DeepEqual(p.Cards, legacy) || !reflect.DeepEqual(p.Sizes, sizes) {
		t.Fatal("custom sizes lost", p)
	}
	custom := []string{"quick", "music", "clock"}
	p = NormalizeOverview(Overview{Cards: custom})
	if !reflect.DeepEqual(p.Cards, custom) {
		t.Fatal("custom order lost", p.Cards)
	}
}
