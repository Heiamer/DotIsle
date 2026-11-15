// Package prefs reads and writes the settings process's local JSON stores with the
// same schemas and defaults as dotisle-domain (preferences schema 3, todos, timers).
package prefs

import (
	"encoding/json"
	"math"
	"os"
	"path/filepath"
)

type Background struct {
	Path       string  `json:"path"`
	Enabled    bool    `json:"enabled"`
	Opacity    float64 `json:"opacity"`
	Brightness float64 `json:"brightness"`
	Blur       float64 `json:"blur"`
	Mode       string  `json:"mode"`
}

func defaultBackground() Background {
	return Background{Opacity: 0.3, Brightness: 1, Mode: "Cover"}
}

// Preferences mirrors dotisle_domain::preferences::Preferences (camelCase, schema 3).
type Preferences struct {
	Backgrounds        []Background `json:"backgrounds"`
	Schema             uint32       `json:"schema"`
	Dark               bool         `json:"dark"`
	Monet              bool         `json:"monet"`
	Seed               uint32       `json:"seed"`
	Translation        bool         `json:"translation"`
	WordHighlight      bool         `json:"wordHighlight"`
	Online             bool         `json:"online"`
	IncludeBrowser     bool         `json:"includeBrowser"`
	OffsetMs           int          `json:"offsetMs"`
	ReducedMotion      bool         `json:"reducedMotion"`
	Island             bool         `json:"island"`
	AlwaysOnTop        bool         `json:"alwaysOnTop"`
	ReminderSound      bool         `json:"reminderSound"`
	LocalFolders       []string     `json:"localFolders"`
	ControlCenterTheme string       `json:"controlCenterTheme"`
	FontScope          string       `json:"fontScope"`
}

func Default() Preferences {
	return Preferences{
		Backgrounds:        []Background{defaultBackground(), defaultBackground(), defaultBackground(), defaultBackground(), defaultBackground(), defaultBackground()},
		Schema:             3,
		Monet:              true,
		Seed:               0x496b55,
		Translation:        true,
		WordHighlight:      true,
		Online:             true,
		AlwaysOnTop:        true,
		ReminderSound:      true,
		ControlCenterTheme: "Original",
		FontScope:          "Both",
	}
}

// Load mirrors Preferences::load, including the repair and migration rules.
func Load(path string) (Preferences, error) {
	p := Default()
	raw, err := os.ReadFile(path)
	if os.IsNotExist(err) {
		return p, nil
	}
	if err != nil {
		return p, err
	}
	if len(raw) > 1024*1024 {
		return p, ErrPrefs("偏好文件过大，原文件已保留")
	}
	if err := json.Unmarshal(raw, &p); err != nil {
		return Default(), ErrPrefs("偏好读取失败，原文件已保留：" + err.Error())
	}
	if p.Schema != 3 {
		return Default(), ErrPrefs("不支持此偏好版本，原文件已保留")
	}
	for len(p.Backgrounds) < 6 {
		p.Backgrounds = append(p.Backgrounds, defaultBackground())
	}
	p.Backgrounds = p.Backgrounds[:6]
	for i := range p.Backgrounds {
		b := &p.Backgrounds[i]
		b.Opacity = clamp(finite(b.Opacity, 0.3), 0, 1)
		b.Brightness = clamp(finite(b.Brightness, 1), 0, 2)
		b.Blur = clamp(finite(b.Blur, 0), 0, 40)
		if b.Mode != "Contain" {
			b.Mode = "Cover"
		}
	}
	switch p.ControlCenterTheme {
	case "Nocturne", "Nothing":
		p.ControlCenterTheme = "Original"
	case "Swiss":
		p.ControlCenterTheme = "Gallery"
	case "Porcelain":
		p.ControlCenterTheme = "Miuix"
	}
	if p.OffsetMs < -10000 {
		p.OffsetMs = -10000
	}
	if p.OffsetMs > 10000 {
		p.OffsetMs = 10000
	}
	p.Seed &= 0xffffff
	return p, nil
}

// Save writes atomically: temp file + rename, like lyric-backend storage::atomic_write.
func Save(path string, p *Preferences) error {
	return atomicSave(path, p)
}

func atomicSave(path string, value any) error {
	raw, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	f, err := os.CreateTemp(filepath.Dir(path), ".preferences-*.tmp")
	if err != nil {
		return err
	}
	tmp := f.Name()
	defer os.Remove(tmp)
	if _, err = f.Write(raw); err != nil {
		f.Close()
		return err
	}
	if err = f.Sync(); err != nil {
		f.Close()
		return err
	}
	if err = f.Close(); err != nil {
		return err
	}
	return os.Rename(tmp, path)
}

type ErrPrefs string

func (e ErrPrefs) Error() string { return string(e) }

func finite(v, def float64) float64 {
	if !math.IsInf(v, 0) && !math.IsNaN(v) {
		return v
	}
	return def
}

func clamp(v, lo, hi float64) float64 {
	if v < lo {
		return lo
	}
	if v > hi {
		return hi
	}
	return v
}

// Paths returns the store paths inside the island data directory.
func Paths(data string) (prefs, todos, timers string) {
	return filepath.Join(data, "preferences.json"),
		filepath.Join(data, "todos.json"),
		filepath.Join(data, "timers.json")
}
