package prefs

import (
	"encoding/json"
	"os"
	"slices"
	"strings"
)

type Overview struct {
	View         string              `json:"view"`
	ScrollPlugin bool                `json:"scrollPlugin"`
	Cards        []string            `json:"cards"`
	ScrollPages  []string            `json:"scrollPages"`
	Information  []string            `json:"information"`
	Sizes        map[string]CardSize `json:"sizes,omitempty"`
}

// Width is a fraction of the overview canvas, so a saved layout also fits the
// expanded canvas when the sidebar is hidden. Height uses logical pixels.
type CardSize struct {
	Width  float32 `json:"width"`
	Height float32 `json:"height"`
}

var OverviewCardIDs = []string{"music", "clock", "todos", "focus", "quick", "stats", "notifications", "widgets",
	"widget:calendar", "widget:system", "widget:note", "widget:anniversary", "widget:weather", "widget:today",
	"widget:schedule", "widget:screenTime", "widget:battery", "widget:storage", "widget:worldClock", "widget:launcher", "widget:focusStats",
	"wireless", "agenda", "cpu", "memory", "week", "aiQuota"}

func DefaultOverview() Overview {
	return Overview{View: "cards", Cards: []string{"music", "widget:calendar", "widget:screenTime", "todos", "focus", "quick"},
		ScrollPages: []string{"music", "focus", "todos", "notifications", "information"},
		Information: []string{"clock", "widget:weather", "widget:system"}}
}

// Scroll pages and the information mosaic have independent selections. Old
// layouts gain defaults without changing their existing card arrangement.
func InformationCardIDs() []string {
	return append([]string{"clock", "stats"}, OverviewCardIDs[8:]...)
}
func ScrollPageIDs() []string { return append([]string{"information"}, OverviewCardIDs...) }
func cleanOverviewKeys(keys, allowed, fallback []string) []string {
	known, seen := map[string]bool{}, map[string]bool{}
	for _, key := range allowed {
		known[key] = true
	}
	clean := []string{}
	for _, key := range keys {
		if (known[key] || ValidPluginKey(key)) && !seen[key] && len(clean) < 64 {
			clean = append(clean, key)
			seen[key] = true
		}
	}
	if len(clean) == 0 {
		return append([]string{}, fallback...)
	}
	return clean
}

func NormalizeOverview(p Overview) Overview {
	// The scroll deck ships as an opt-in built-in plugin; without it the
	// overview always uses the card grid.
	if p.View != "scroll" || !p.ScrollPlugin {
		p.View = "cards"
	}
	d := DefaultOverview()
	if len(p.Sizes) == 0 && (slices.Equal(p.Cards, []string{"music", "clock", "todos", "focus", "quick"}) || slices.Equal(p.Cards, []string{"music", "widget:calendar", "widget:system", "todos", "focus", "quick"})) {
		p.Cards = append([]string{}, d.Cards...)
	}
	p.Cards = cleanOverviewKeys(p.Cards, OverviewCardIDs, d.Cards)
	p.ScrollPages = cleanOverviewKeys(p.ScrollPages, ScrollPageIDs(), d.ScrollPages)
	p.Information = cleanOverviewKeys(p.Information, InformationCardIDs(), d.Information)
	clean := map[string]CardSize{}
	for _, key := range p.Cards {
		if size, ok := p.Sizes[key]; ok {
			if !(size.Width >= .24 && size.Width <= 1 && size.Height >= 264 && size.Height <= 560) {
				continue
			}
			clean[key] = size
		}
	}
	p.Sizes = nil
	if len(clean) > 0 {
		p.Sizes = clean
	}
	return p
}

func LoadOverview(path string) (Overview, error) {
	p := DefaultOverview()
	raw, err := os.ReadFile(path)
	if os.IsNotExist(err) {
		return p, nil
	}
	if err != nil {
		return p, err
	}
	if len(raw) > 64<<10 {
		return p, ErrPrefs("概览配置过大，已保留原文件")
	}
	if err = json.Unmarshal(raw, &p); err != nil {
		return DefaultOverview(), err
	}
	return NormalizeOverview(p), nil
}

func SaveOverview(path string, p Overview) error { return atomicSave(path, NormalizeOverview(p)) }

// Plugin IDs remain inert when a plugin is removed, so reinstall restores layout.
func ValidPluginKey(key string) bool {
	id, ok := strings.CutPrefix(key, "plugin:")
	if !ok || len(id) == 0 || len(id) > 64 || strings.HasPrefix(id, ".") || strings.HasSuffix(id, ".") || strings.Contains(id, "..") {
		return false
	}
	for _, c := range id {
		if !(c >= 'a' && c <= 'z' || c >= '0' && c <= '9' || c == '-' || c == '.') {
			return false
		}
	}
	return true
}
