package ui

import (
	"dotisle/settings/themes"
	"github.com/egoist/mygo/ui"
	"math"
)

// The opt-in beta uses hue from the source, with bounded saturation and lightness.
// Text and controls keep readable contrast; large surfaces carry the pastel tint.
func pastelScheme(seed uint32, dark bool) themes.Scheme {
	h := colorHue(seed)
	s := fallbackScheme(dark)
	if dark {
		s.Surface, s.Container, s.High = hueColor(h, .12, .09), hueColor(h, .14, .14), hueColor(h, .16, .20)
		s.Primary, s.OnPrimary = hueColor(h, .42, .80), hueColor(h, .20, .13)
		s.SecondaryContainer, s.OnSecondaryContainer = hueColor(h, .23, .25), 0xf3f4f6
		s.PrimaryContainer, s.OnPrimaryContainer = hueColor(h, .25, .24), 0xf3f4f6
		s.Outline, s.OutlineVariant = hueColor(h, .13, .39), hueColor(h, .12, .27)
	} else {
		s.Surface, s.Container, s.High = hueColor(h, .40, .93), hueColor(h, .32, .985), hueColor(h, .38, .955)
		s.Primary, s.OnPrimary = hueColor(h, .40, .32), 0xffffff
		s.SecondaryContainer, s.OnSecondaryContainer = hueColor(h, .44, .87), 0x25272c
		s.PrimaryContainer, s.OnPrimaryContainer = hueColor(h, .44, .87), 0x25272c
		s.Outline, s.OutlineVariant = hueColor(h, .18, .66), hueColor(h, .25, .83)
	}
	s.TertiaryContainer, s.OnTertiaryContainer = s.SecondaryContainer, s.OnSecondaryContainer
	return s
}

func colorHue(rgb uint32) float64 {
	r, g, b := float64(rgb>>16&255)/255, float64(rgb>>8&255)/255, float64(rgb&255)/255
	hi, lo := max(r, g, b), min(r, g, b)
	d := hi - lo
	if d < .04 {
		return 210
	}
	h := (g - b) / d
	if hi == g {
		h = (b-r)/d + 2
	} else if hi == b {
		h = (r-g)/d + 4
	}
	return math.Mod(h*60+360, 360)
}

func hueColor(h, s, l float64) uint32 {
	chroma := (1 - math.Abs(2*l-1)) * s
	x := chroma * (1 - math.Abs(math.Mod(h/60, 2)-1))
	m := l - chroma/2
	var r, g, b float64
	switch {
	case h < 60:
		r, g = chroma, x
	case h < 120:
		r, g = x, chroma
	case h < 180:
		g, b = chroma, x
	case h < 240:
		g, b = x, chroma
	case h < 300:
		r, b = x, chroma
	default:
		r, b = chroma, x
	}
	return uint32(math.Round((r+m)*255))<<16 | uint32(math.Round((g+m)*255))<<8 | uint32(math.Round((b+m)*255))
}

func centerColorSeed(settings, state map[string]any, dark bool) uint32 {
	custom := uint32(0x9abfe8)
	if text, ok := settings["ControlCenterCustomColor"].(string); ok {
		if rgb, valid := parseHexColor(text); valid {
			custom = rgb
		}
	}
	switch settings["ControlCenterColorSource"] {
	case "Custom":
		return custom
	case "System":
		if value, ok := stateFloat(state, "systemAccent"); ok && value >= 0 && value <= 0xffffffff {
			return uint32(value) & 0xffffff
		}
	default:
		if value, ok := stateFloat(state, "artworkSeed"); ok && value >= 0 && value <= 0xffffffff {
			return uint32(value) & 0xffffff
		}
		if (&App{State: state}).coverBitmap() != nil && coverCache.seed != 0 {
			return coverCache.seed
		}
	}
	return custom
}

func (a *App) pastelSettings(c *ui.Context, p *Palette, x, y, w float32) float32 {
	on := a.leafBool("ControlCenterDynamicColorEnabled", false)
	Panel(c, p, x, y, w, 88, p.Scheme.Container)
	appearanceToggleRow(c, p, "ControlCenterDynamicColorEnabled", x, y, w, 88, "动态取色", "低饱和马卡龙色彩，随歌曲封面或系统配色变化", on, func() { a.pushNative("ControlCenterDynamicColorEnabled", !on) })
	textY := y + (88-LineHeight(14)-4-LineHeight(12))/2
	ui.Box(c).Absolute().Left(x + 102).Top(textY).Width(38).Height(LineHeight(14)).Radius(7).Background(Col(p.Scheme.SecondaryContainer)).IgnoreInput(true)
	CenteredText(c, "Beta", x+102, textY, 38, LineHeight(14), 10, p.Scheme.OnSecondaryContainer, true).IgnoreInput(true)
	y += 108
	if !on {
		return y
	}
	y = leafChoices(c, p, x, y, w, "取色来源", [][2]string{{"歌曲封面", "Content"}, {"系统强调色", "System"}, {"自定义", "Custom"}}, a.leafString("ControlCenterColorSource", "Content"), func(source string) { a.pushNative("ControlCenterColorSource", source) })
	if a.leafString("ControlCenterColorSource", "Content") == "Custom" {
		y = leafChoices(c, p, x, y, w, "马卡龙色彩", [][2]string{{"淡蓝", "#9ABFE8"}, {"淡绿", "#A9D8C2"}, {"淡粉", "#E4B4CD"}}, a.leafString("ControlCenterCustomColor", "#9ABFE8"), func(color string) { a.pushNative("ControlCenterCustomColor", color) })
	}
	return y
}
