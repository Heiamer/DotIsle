// Neutral design tokens and responsive geometry for the native control center.
// Dynamic island and widget palettes are owned by their existing Rust modules.
package ui

import (
	"dotisle/settings/prefs"
	"dotisle/settings/themes"
)

// Layout constants (center_layout.rs + views.rs), logical pixels.
const (
	TitlebarH   = 52
	TitleY      = 68
	SubtitleY   = 100
	ContentTop  = 132
	EditorTop   = 68
	TodoActionY = 68
	NavBottom   = 22
	NavHeight   = 72
	WindowW     = 1200
	WindowH     = 800
)

// Settings tabs (settings_hub.rs routing).
const (
	TabRoot          = 255
	TabGroupFirst    = 240
	TabLayoutEditor  = 135
	TabIslandFirst   = 128
	TabIslandLast    = 134
	TabModuleManager = 9
)

type Palette struct {
	Scheme              themes.Scheme
	Dark                bool
	HighContrast        bool
	ReducedMotion       bool
	BackgroundProminent themes.Scheme // placeholder for per-card variants
}

// stateAlpha is the panel hairline stroke alpha (light 0.075 / dark 0.045).
func (p *Palette) stateAlpha() float64 {
	if p.Dark {
		return 0.045
	}
	return 0.075
}

func mix(a, b uint32, t float64) uint32 {
	c := func(n uint32, v uint32) float64 { return float64((v >> n) & 255) }
	ch := func(n uint32) float64 { return c(n, a) + (c(n, b)-c(n, a))*t }
	rr := uint32(ch(16)+0.5) << 16
	gg := uint32(ch(8)+0.5) << 8
	bb := uint32(ch(0) + 0.5)
	return rr | gg | bb
}

func parseHexColor(s string) (uint32, bool) {
	if len(s) == 0 {
		return 0, false
	}
	if s[0] == '#' {
		s = s[1:]
	}
	if len(s) != 6 {
		return 0, false
	}
	var v uint32
	for i := 0; i < 6; i++ {
		d := s[i]
		var n uint32
		switch {
		case d >= '0' && d <= '9':
			n = uint32(d - '0')
		case d >= 'a' && d <= 'f':
			n = uint32(d-'a') + 10
		case d >= 'A' && d <= 'F':
			n = uint32(d-'A') + 10
		default:
			return 0, false
		}
		v = v<<4 | n
	}
	return v, true
}

// resolveSettingsDark mirrors native_sync.rs:158-162 — prefs.dark follows the
// island's SettingsTheme and settingsDark snapshot fields.
func resolveSettingsDark(p *prefs.Preferences, settings map[string]any, settingsDark any) bool {
	theme, _ := settings["SettingsTheme"].(string)
	switch theme {
	case "Dark":
		return true
	case "Light":
		return false
	default:
		if b, ok := settingsDark.(bool); ok {
			return b
		}
		return p.Dark
	}
}

// BuildPalette keeps the control center neutral regardless of legacy Monet seeds.
// The island and desktop widgets continue to use their own native color settings.
func BuildPalette(p *prefs.Preferences, settings map[string]any, state map[string]any) Palette {
	dark := resolveSettingsDark(p, settings, state["settingsDark"])
	high, _ := settings["DynamicColorHighContrast"].(bool)
	s := fallbackScheme(dark)
	if enabled, _ := settings["ControlCenterDynamicColorEnabled"].(bool); enabled {
		s = pastelScheme(centerColorSeed(settings, state, dark), dark)
	}
	if high {
		s.OnSurfaceVariant = s.OnSurface
		s.OutlineVariant = s.Outline
	}
	reduced := p.ReducedMotion
	if enabled, ok := settings["Animation"].(bool); ok {
		reduced = !enabled
	}
	return Palette{Scheme: s, Dark: dark, HighContrast: high, ReducedMotion: reduced, BackgroundProminent: s}
}

// Effective neutral palette with user-requested gray and blue screenshot overrides.
func fallbackScheme(dark bool) themes.Scheme {
	if dark {
		return themes.Scheme{
			Primary: 0x008af3, OnPrimary: 0xfcfcfc,
			PrimaryContainer: 0x181818, OnPrimaryContainer: 0xfcfcfc,
			SecondaryContainer: 0x282828, OnSecondaryContainer: 0xfcfcfc,
			TertiaryContainer: 0x272727, OnTertiaryContainer: 0xfcfcfc,
			Surface: 0x060606, Container: 0x181818, High: 0x232323,
			OnSurface: 0xfcfcfc, OnSurfaceVariant: 0xa0a0a0,
			Outline: 0x2a2a2a, OutlineVariant: 0x292929, Error: 0xff6157,
		}
	}
	return themes.Scheme{
		Primary: 0x008af3, OnPrimary: 0xfcfcfc,
		PrimaryContainer: 0xffffff, OnPrimaryContainer: 0x181818,
		SecondaryContainer: 0xebebeb, OnSecondaryContainer: 0x181818,
		TertiaryContainer: 0xeaeaea, OnTertiaryContainer: 0x181818,
		Surface: 0xf5f5f5, Container: 0xffffff, High: 0xefefef,
		OnSurface: 0x181818, OnSurfaceVariant: 0x727272,
		Outline: 0xdedede, OutlineVariant: 0xe4e4e4, Error: 0xf04438,
	}
}

// The overlay menu never reserves a permanent sidebar column.
func (a *App) sidebarWidth(_ float32) float32 { return 0 }
