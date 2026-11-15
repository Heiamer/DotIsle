package prefs

import (
	"os"
	"path/filepath"
	"testing"
)

func TestSaveCreatesDataDirectory(t *testing.T) {
	path := filepath.Join(t.TempDir(), "fresh", "Data", "preferences.json")
	p := Default()
	p.Dark = true
	if err := Save(path, &p); err != nil {
		t.Fatal(err)
	}
	got, err := Load(path)
	if err != nil || !got.Dark {
		t.Fatalf("saved preferences not restored: %v", err)
	}
}

func TestPartialBackgroundKeepsRustDefaults(t *testing.T) {
	path := filepath.Join(t.TempDir(), "preferences.json")
	if err := os.WriteFile(path, []byte(`{"schema":3,"backgrounds":[{"path":"image.png","enabled":true}]}`), 0600); err != nil {
		t.Fatal(err)
	}
	p, err := Load(path)
	if err != nil {
		t.Fatal(err)
	}
	if p.Backgrounds[0].Brightness != 1 || p.Backgrounds[0].Opacity != 0.3 {
		t.Fatalf("partial background lost defaults: %+v", p.Backgrounds[0])
	}
}

func TestUnreadablePreferencesReportsError(t *testing.T) {
	if _, err := Load(t.TempDir()); err == nil {
		t.Fatal("read failure must not be treated as a missing file")
	}
}
