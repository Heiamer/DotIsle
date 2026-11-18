package ui

import (
	"bytes"
	"dotisle/settings/island"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"image"
	"image/png"
	"time"

	"github.com/egoist/mygo/ui"
)

type layoutPreviewFrame struct {
	Width, Height, Padding float64
	Elements               []musicElement
	Glass, HasTrack        bool
	Bitmap                 *ui.Bitmap
}

type layoutPreviewKey struct {
	Draft, Appearance, Track, Lyrics string
	Second                           int
}

func (a *App) onLayoutEditor() bool {
	return a.Page == PageSettings && a.SettingsTab == TabLayoutEditor
}

func (a *App) requestLayoutPreview(e *musicEditor) {
	if a.Win == nil || !a.centerVisible || !a.Connected || e.PreviewLoad || e.Pending {
		return
	}
	preview, _ := a.State["musicPreview"].(map[string]any)
	frame := a.lyricFrame()
	elapsed, _ := preview["elapsed"].(float64)
	appearance := []any{preview["width"], a.State["settingsDark"]}
	for _, key := range []string{
		"FontSize", "IslandFontFamilyRender", "IslandMaterial", "IslandSizeMode", "MiniMaterial",
		"IslandColorSource", "IslandDynamicSeedColor", "DynamicColorHighContrast", "ExpandedMonetEnabled",
		"ExpandedCoverOverlay", "ExpandedTextColor", "ExpandedComponentColor", "LyricProgressColor",
		"AcrylicCardFrost", "AcrylicCardOpacity", "Translation", "TranslationButton", "Motion",
		"MusicExpandedLayout", "ProgressBarStyle", "ProgressBarThumb", "ProgressBarThumbEnabled",
	} {
		appearance = append(appearance, a.pref(key))
	}
	key := layoutPreviewKey{
		Draft: layoutJSON(e.Draft), Appearance: layoutJSON(appearance),
		Track:  layoutJSON([]any{a.State["title"], a.State["artist"], a.State["isPlaying"], preview["duration"], preview["coverPath"]}),
		Lyrics: layoutJSON([]any{frame["currentLine"], frame["currentTranslation"], frame["nextLine"]}),
		Second: int(elapsed),
	}
	if e.Preview != nil && e.PreviewKey == key {
		return
	}
	interval := time.Second
	if key.Draft != e.PreviewKey.Draft || key.Appearance != e.PreviewKey.Appearance || key.Track != e.PreviewKey.Track || key.Lyrics != e.PreviewKey.Lyrics {
		interval = 220 * time.Millisecond
	}
	if e.PreviewError != "" {
		interval = 3 * time.Second
	}
	if time.Since(e.PreviewAt) < interval {
		return
	}
	e.PreviewLoad, e.PreviewAt = true, time.Now()
	a.startAsync(func() {
		reply, err := island.Request("musicLayoutPreview", json.RawMessage(key.Draft))
		var next *layoutPreviewFrame
		var img image.Image
		if err == nil {
			next, img, err = decodeLayoutPreview(reply)
		}
		a.applyAsync(func() {
			e.PreviewLoad = false
			if !a.onLayoutEditor() || !a.centerVisible || !a.Connected || a.musicEditor != e {
				return
			}
			if err != nil {
				e.Preview, e.PreviewError = nil, err.Error()
				return
			}
			if e.Preview != nil {
				next.Bitmap = e.Preview.Bitmap
				next.Bitmap.Update(img)
			} else {
				next.Bitmap = ui.NewBitmap(img)
			}
			e.Preview, e.PreviewKey, e.PreviewError = next, key, ""
		})
	})
}

func decodeLayoutPreview(reply map[string]any) (*layoutPreviewFrame, image.Image, error) {
	if reply["success"] != true {
		return nil, nil, fmt.Errorf("%s", stateString(reply, "原生预览不可用", "message"))
	}
	encoded, _ := reply["png"].(string)
	data, err := base64.StdEncoding.DecodeString(encoded)
	if err != nil {
		return nil, nil, err
	}
	img, err := png.Decode(bytes.NewReader(data))
	if err != nil {
		return nil, nil, err
	}
	next := new(layoutPreviewFrame)
	for k, dst := range map[string]*float64{"width": &next.Width, "height": &next.Height, "padding": &next.Padding} {
		*dst, _ = reply[k].(float64)
	}
	if next.Width <= 0 || next.Height <= 0 || next.Padding < 0 {
		return nil, nil, fmt.Errorf("原生预览尺寸无效")
	}
	next.Glass, _ = reply["glass"].(bool)
	next.HasTrack, _ = reply["hasTrack"].(bool)
	if err := json.Unmarshal([]byte(layoutJSON(reply["elements"])), &next.Elements); err != nil {
		return nil, nil, err
	}
	return next, img, nil
}

func (e *musicEditor) editorElements() []musicElement {
	if e.Preview == nil {
		return e.Geometry
	}
	// Persist X/width in the existing 400 DIP design coordinates, not the island's width.
	out := make([]musicElement, len(e.Preview.Elements))
	for i, element := range e.Preview.Elements {
		element.X *= 400 / e.Preview.Width
		element.W *= 400 / e.Preview.Width
		out[i] = element
	}
	return out
}

func (a *App) drawLayoutPreview(c *ui.Context, p *Palette, e *musicEditor, x, y, w, h float32) {
	frame := e.Preview
	if frame == nil {
		if a.Win == nil {
			// The headless tester can exercise the layout handles without a live pipe.
			frame = &layoutPreviewFrame{Width: 400, Height: e.Height, Elements: e.Geometry}
		} else {
			label := "正在读取原生预览…"
			if !a.Connected {
				label = "灵动岛未连接"
			} else if e.PreviewError != "" {
				label = e.PreviewError
			}
			Label(c, label, x+32, y+h/2-10, 13, p.Scheme.OnSurfaceVariant, false, w-64)
			return
		}
	}
	width, height, pad := float32(frame.Width), float32(frame.Height), float32(frame.Padding)
	cw, ch := width+2*pad, height+2*pad
	scale := min((w-52)/cw, (h-16)/max(ch, 1))
	ui.Box(c).Key("music-layout-canvas").Absolute().Left(x+(w-scale*cw)/2).Top(y+(h-scale*ch)/2).
		Width(cw).Height(ch).TransformOrigin(0, 0).Scale(scale, scale).Label("灵动岛布局实时预览").Children(func() {
		if frame.Bitmap != nil {
			ui.Image(c, frame.Bitmap).Absolute().Fill().IgnoreInput(true)
		}
		for _, element := range frame.Elements {
			node := ui.ButtonBase(c).Key(fmt.Sprintf("music-layout-%d", element.ID)).Absolute().
				Left(pad + float32(element.X)).Top(pad + float32(element.Y)).Width(float32(element.W)).Height(float32(element.H)).
				Label("布局元素" + element.Label).Radius(5).FocusRing(false)
			if e.Selected == element.ID {
				node.Border(1.5, Col(p.Scheme.Primary))
			}
			if !e.Pending {
				if node.Pressed() {
					e.Selected, e.Tab, e.Scroll = element.ID, 0, ui.ScrollState{}
				}
				dx, dy, dragged := node.Dragged()
				designScale := 400 / width
				element.X *= float64(designScale)
				e.drag(element, dx*designScale, dy, dragged || node.Pressed())
			}
		}
	})
}
