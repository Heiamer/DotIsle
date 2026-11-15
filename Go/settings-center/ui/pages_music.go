// The music page (views.rs fn music) and its transport, at Original-theme geometry.
package ui

import (
	"dotisle/settings/island"
	"os"
	"time"

	"github.com/egoist/mygo/ui"
)

var coverCache = struct {
	path  string
	bmp   *ui.Bitmap
	frost *ui.Bitmap
	seed  uint32
	at    time.Time
}{}

// coverBitmap loads the island's cover preview, refreshed when the path changes.
func (a *App) coverBitmap() *ui.Bitmap {
	if a.State == nil {
		return nil
	}
	preview, _ := a.State["musicPreview"].(map[string]any)
	if preview == nil {
		return nil
	}
	path, _ := preview["coverPath"].(string)
	if path == "" {
		return nil
	}
	if coverCache.path == path && coverCache.bmp != nil {
		return coverCache.bmp
	}
	raw, err := os.ReadFile(path)
	if err != nil {
		return nil
	}
	bmp, err := ui.DecodeBitmap(raw)
	if err != nil {
		return nil
	}
	coverCache.path, coverCache.bmp, coverCache.at = path, bmp, time.Now()
	coverCache.frost, coverCache.seed = frostAlbum(raw)
	return bmp
}

func (a *App) str(stateKey ...string) string {
	cur := a.State
	for _, key := range stateKey {
		if cur == nil {
			return ""
		}
		next, ok := cur[key].(map[string]any)
		if !ok {
			v, _ := cur[key].(string)
			return v
		}
		cur = next
	}
	return ""
}

// stateFloat reads a nested numeric snapshot field.
func stateFloat(root map[string]any, path ...string) (float64, bool) {
	var cur any = root
	for i, key := range path {
		m, ok := cur.(map[string]any)
		if !ok {
			return 0, false
		}
		cur = m[key]
		if i == len(path)-1 {
			if f, ok := cur.(float64); ok {
				return f, true
			}
		}
	}
	return 0, false
}

func stateBool(root map[string]any, path ...string) bool {
	var cur any = root
	for i, key := range path {
		m, ok := cur.(map[string]any)
		if !ok {
			return false
		}
		cur = m[key]
		if i == len(path)-1 {
			b, _ := cur.(bool)
			return b
		}
	}
	return false
}

func stateString(root map[string]any, def string, path ...string) string {
	var cur any = root
	for i, key := range path {
		m, ok := cur.(map[string]any)
		if !ok {
			return def
		}
		cur = m[key]
		if i == len(path)-1 {
			if s, ok := cur.(string); ok && s != "" {
				return s
			}
		}
	}
	return def
}

// interpolatedPosition extrapolates the packet position while playing, like the
// island's PlaybackProgress, bounded to 0.35s of age.
func (a *App) interpolatedPosition() float64 {
	if a.State == nil {
		return 0
	}
	pos, ok := stateFloat(a.State, "position")
	if !ok {
		return 0
	}
	if !stateBool(a.State, "isPlaying") {
		return pos
	}
	age := time.Since(a.packetAt).Seconds()
	if age > 0.35 {
		age = 0.35
	}
	return pos + age
}

// mediaFraction is the seek slider fraction with duration guarding.
func (a *App) mediaFraction() (float64, float64) {
	if a.State == nil {
		return 0, 0
	}
	duration, _ := stateFloat(a.State, "duration")
	pos := a.interpolatedPosition()
	if duration <= 0 {
		return 0, duration
	}
	return clamp01(pos / duration), duration
}

// seekTo sends the absolute-second seek (model.rs seek: canSeek guard, media seek).
func (a *App) seekTo(fraction, duration float64) {
	if duration <= 0 || !stateBool(a.State, "canSeek") {
		return
	}
	a.command("media", map[string]any{"action": "seek", "seconds": fraction * duration})
}

// mediaCommand sends a transport action when a track is loaded.
func (a *App) mediaCommand(action string) {
	if a.State == nil || !stateBool(a.State, "isPlaying") && action == "toggle" {
		// toggle is always allowed; the island decides.
	}
	a.command("media", map[string]any{"action": action})
}

// musicPage mirrors views.rs fn music at Original geometry.
func (a *App) musicPage(c *ui.Context, x, y, w float32) {
	p := &a.Palette
	s := &p.Scheme
	hasTrack := a.hasTrack()

	Panel(c, p, x, y, w, 300, s.PrimaryContainer)
	a.recordDisc(c, x+24, y+24, 252, 252)
	px := x + 310
	avail := w - 340
	status := "等待播放器"
	if a.ConnErr != "" {
		status = "等待播放器"
	} else if hasTrack {
		status = "正在播放"
	}
	Label(c, status, px, y+25, 12, s.OnPrimaryContainer, true, avail)
	Label(c, a.trackTitle(), px, y+60, 36, s.OnPrimaryContainer, true, avail)
	Label(c, a.trackArtist(), px, y+113, 16, s.OnPrimaryContainer, false, avail)

	// Progress with the interpolated position; the knob previews the drag and
	// commits the absolute-second seek on release.
	fraction, duration := a.mediaFraction()
	Seekbar(c, p, "播放进度", px, y+164, avail, 28, fraction,
		hasTrack && stateBool(a.State, "canSeek"), func(f float64) {
			a.seekTo(f, duration)
		})
	a.playerControls(c, px+avail/2-88, y+218, true)

	body := float32(320)
	if hasTrack && !stateBool(a.State, "canSeek") {
		statusText := stateString(a.State, "播放器尚未提供静默进度通道", "seekStatus")
		Panel(c, p, x, y+body, w, 64, s.Container)
		Label(c, "播放进度", x+24, y+body+12, 15, s.OnSurface, true, 200)
		Label(c, statusText, x+24, y+body+38, 12, s.OnSurfaceVariant, false, w-280)
		if src := stateString(a.State, "", "player"); equalFold(src, "cloudmusic.exe") &&
			statusText == "播放器尚未提供静默进度通道" {
			Button(c, p, "查看连接状态", x+w-216, y+body+10, 192, 44,
				"查看连接状态", "", true, true, func() {
					a.command("neteaseEnableSilent", nil)
				})
		}
		body += 88
	}

	// Lyric card with the word-scan highlight clip and translation.
	Panel(c, p, x, y+body, w, 194, s.Container)
	Label(c, "歌词", x+26, y+body+22, 13, s.OnSurfaceVariant, true, 100)
	line := stateString(a.lyricFrame(), "把耳朵交给音乐，把此刻留给自己。", "currentLine")
	Label(c, line, x+26, y+body+56, 28, s.OnSurface, true, w-52)
	if a.Prefs.WordHighlight && hasTrack {
		scan, _ := stateFloat(a.lyricFrame(), "wordScanProgress")
		if scan > 0 {
			ui.Box(c).Absolute().Left(x + 26).Top(y + body + 54).
				Width(float32(float64(w-52) * scan)).Height(45).Clip().Children(func() {
				ui.Text(c, line).FontSize(28).FixedLineHeight(LineHeight(28)).
					TextColor(Col(s.Primary)).FontWeight(500).SingleLine()
			})
		}
	}
	if a.Prefs.Translation {
		Label(c, stateString(a.lyricFrame(), "", "currentTranslation"),
			x+28, y+body+105, 14, s.OnSurfaceVariant, false, w-56)
	}
	Label(c, stateString(a.lyricFrame(), "歌词会在歌曲播放后自动匹配", "nextLine"),
		x+28, y+body+147, 18, s.OnSurfaceVariant, false, w-56)

	// Offset card: ±100 ms lyric sync (Action::Offset).
	oy := y + body + 214
	Panel(c, p, x, oy, w, 72, s.High)
	Label(c, "歌词同步", x+24, oy+13, 15, s.OnSurface, true, 200)
	Label(c, "向左提前，向右延后 · 每次 100 ms", x+24, oy+39, 12, s.OnSurfaceVariant, false, w-300)
	IconButton(c, p, "歌词提前 100 毫秒", x+w-238, oy+14, 44, 44, "arrow_back", false, true, func() {
		a.shiftOffset(-100)
	})
	CenteredText(c, formatOffset(a.Prefs.OffsetMs), x+w-187, oy+14, 120, 44, 15, s.Primary, true)
	IconButton(c, p, "歌词延后 100 毫秒", x+w-60, oy+14, 44, 44, "arrow_forward", false, true, func() {
		a.shiftOffset(100)
	})
}

func (a *App) shiftOffset(delta int) {
	a.Prefs.OffsetMs += delta
	if a.Prefs.OffsetMs < -10000 {
		a.Prefs.OffsetMs = -10000
	}
	if a.Prefs.OffsetMs > 10000 {
		a.Prefs.OffsetMs = 10000
	}
	a.savePrefs()
	a.pushNative("OffsetMilliseconds", a.Prefs.OffsetMs)
}

func formatOffset(v int) string {
	sign := "+"
	if v < 0 {
		sign = "-"
	}
	return sign + itoa(abs(v)) + " ms"
}

func abs(v int) int {
	if v < 0 {
		return -v
	}
	return v
}

func itoa(v int) string {
	if v == 0 {
		return "0"
	}
	var buf [12]byte
	i := len(buf)
	for v > 0 {
		i--
		buf[i] = byte('0' + v%10)
		v /= 10
	}
	return string(buf[i:])
}

func equalFold(a, b string) bool {
	if len(a) != len(b) {
		return false
	}
	for i := 0; i < len(a); i++ {
		ca, cb := a[i], b[i]
		if ca >= 'A' && ca <= 'Z' {
			ca += 32
		}
		if cb >= 'A' && cb <= 'Z' {
			cb += 32
		}
		if ca != cb {
			return false
		}
	}
	return true
}

// recordDisc is views.rs record: the 15px-radius cover, or the tonal placeholder.
func (a *App) recordDisc(c *ui.Context, x, y, w, h float32) {
	p := &a.Palette
	s := &p.Scheme
	if bmp := a.coverBitmap(); bmp != nil {
		ui.Image(c, bmp).Absolute().Left(x).Top(y).Width(w).Height(h).
			Fit(ui.Cover).Radius(15)
		return
	}
	ui.Box(c).Absolute().Left(x).Top(y).Width(w).Height(h).
		Radius(15).Background(Col(s.SecondaryContainer)).Children(func() {
		size := float32(float64(minF(w, h)) * 0.32)
		if svg := Symbol("music_note"); svg != nil {
			ui.Icon(c, svg).FontSize(size).TextColor(Col(s.OnSurfaceVariant)).
				Absolute().Left((w - size) / 2).Top((h - size) / 2)
		}
	})
}

func minF(a, b float32) float32 {
	if a < b {
		return a
	}
	return b
}

// playerControls is views.rs player_controls: Reef transport at 64px play.
func (a *App) playerControls(c *ui.Context, x, y float32, large bool) {
	p := &a.Palette
	s := &p.Scheme
	playing := stateBool(a.State, "isPlaying")
	hasTrack := a.hasTrack()
	size := float32(52)
	if large {
		size = 64
	}
	pad := (size - 44) / 2
	enabled := hasTrack

	// Prev (Reef: bare icon).
	prev := ui.ButtonBase(c).Absolute().Left(x).Top(y + pad).Width(44).Height(44).
		Justify(ui.Center).AlignItems(ui.Center).Radius(22).Label("上一首")
	if !enabled {
		prev.Disabled(true)
	}
	hoverLayer(&prev, ui.Transparent, s.OnSurface, enabled)
	if enabled && prev.Clicked() {
		a.mediaCommand("previous")
	}
	prev.Children(func() {
		ui.Icon(c, Symbol("skip_previous")).FontSize(22).TextColor(Col(s.OnSurface))
	})

	// Play/pause (Reef: accent fill, radius morphs square while playing).
	var playGlyph string
	if playing {
		playGlyph = "pause"
	} else {
		playGlyph = "play_arrow"
	}
	radius := size * 0.5
	if playing {
		radius = size * 0.28
	}
	play := ui.ButtonBase(c).Absolute().Left(x + 56).Top(y).Width(size).Height(size).
		Justify(ui.Center).AlignItems(ui.Center).Radius(radius).
		Background(Col(s.Primary)).Label(map[bool]string{true: "暂停播放", false: "开始播放"}[playing])
	if !enabled {
		play.Disabled(true)
	}
	hoverLayer(&play, Col(s.Primary), s.OnPrimary, enabled)
	if enabled && play.Clicked() {
		a.mediaCommand("toggle")
	}
	play.Children(func() {
		ui.Icon(c, Symbol(playGlyph)).FontSize(24).TextColor(Col(s.OnPrimary))
	})

	// Next (Reef: bare icon).
	next := ui.ButtonBase(c).Absolute().Left(x + size + 68).Top(y + pad).Width(44).Height(44).
		Justify(ui.Center).AlignItems(ui.Center).Radius(22).Label("下一首")
	if !enabled {
		next.Disabled(true)
	}
	hoverLayer(&next, ui.Transparent, s.OnSurface, enabled)
	if enabled && next.Clicked() {
		a.mediaCommand("next")
	}
	next.Children(func() {
		ui.Icon(c, Symbol("skip_next")).FontSize(22).TextColor(Col(s.OnSurface))
	})
}

// keep the island import used (cover path reads run through the worker state).
var _ = island.Request
