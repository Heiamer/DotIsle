package ui

import (
	"bytes"
	"encoding/base64"
	"image/png"
	"time"

	"dotisle/settings/island"
	"github.com/egoist/mygo/ui"
)

type progressPreviewKey struct {
	Width   int    `json:"width"`
	Bar     string `json:"bar"`
	Thumb   string `json:"thumb"`
	ThumbOn bool   `json:"thumbOn"`
	Glow    bool   `json:"glow"`
	Spin    bool   `json:"spin"`
	Animate bool   `json:"animate"`
	Ink     uint32 `json:"ink"`
	Rail    uint32 `json:"rail"`
}

type progressPreviewState struct {
	key     progressPreviewKey
	bitmap  *ui.Bitmap
	loading bool
	retry   time.Time
}

func (a *App) drawProgressPreview(c *ui.Context, p *Palette, x, y, w float32, thumbOn bool) {
	key := progressPreviewKey{
		Width: min(1200, max(80, int(w+.5))), Bar: a.islandString("ProgressBarStyle", "Normal"),
		Thumb: a.islandString("ProgressBarThumb", "Pill"), ThumbOn: thumbOn,
		Glow: a.islandBool("ProgressBarGlow", true), Spin: thumbOn && a.islandBool("ProgressBarThumbSpin", false),
		Animate: !a.reducedMotion(), Ink: p.Scheme.Primary, Rail: p.Scheme.OutlineVariant,
	}
	state := &a.progressPreview
	if !a.Connected {
		state.bitmap = nil
		Label(c, "连接灵动岛后显示预览", x+8, y+4, 12, p.Scheme.OnSurfaceVariant, false, w-16)
		return
	}
	if state.key == key && state.bitmap != nil {
		ui.Image(c, state.bitmap).Absolute().Left(x).Top(y).Width(w).Height(28).Label("播放进度条预览").IgnoreInput(true)
		return
	}
	label := "正在读取原生预览…"
	if state.key == key && time.Now().Before(state.retry) {
		label = "原生预览暂不可用"
	} else if a.Win != nil && a.centerVisible && !state.loading {
		state.loading = true
		a.startAsync(func() {
			reply, err := island.Request("playbackProgressPreview", key)
			var bitmap *ui.Bitmap
			if err == nil && reply["success"] == true {
				encoded, _ := reply["png"].(string)
				if data, err := base64.StdEncoding.DecodeString(encoded); err == nil {
					if img, err := png.Decode(bytes.NewReader(data)); err == nil {
						bitmap = ui.NewBitmap(img)
					}
				}
			}
			a.applyAsync(func() {
				state.loading = false
				if a.Page != PageSettings || a.SettingsTab != 133 || !a.Connected {
					return
				}
				state.key, state.bitmap = key, bitmap
				state.retry = time.Now().Add(3 * time.Second)
			})
		})
	}
	Label(c, label, x+8, y+4, 12, p.Scheme.OnSurfaceVariant, false, w-16)
}
