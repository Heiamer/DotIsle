// Wallpaper support (background.rs): load the slot image, frost it with the blur
// radius, paint it Contain/Cover at the configured opacity and brightness, and lay the
// title-protecting surface veil over the top.
package ui

import (
	"errors"
	"fmt"
	"image"
	"image/color"
	"image/png"
	"math"
	"os"
	"path/filepath"
	"strconv"
	"time"

	"dotisle/settings/prefs"
	"github.com/egoist/mygo/ui"
	xdraw "golang.org/x/image/draw"
)

// decode() caps in background.rs: a 64 MB file, a 16384 source limit the thumbnail cap
// makes irrelevant, and a 2560x1600 stored copy. The user-facing strings are the
// Rust ones so the two UIs report the same failures.
const (
	wallpaperMaxBytes = 64 << 20
	wallpaperMaxW     = 2560
	wallpaperMaxH     = 1600
	// blurWorkW: the blur passes run on a smaller copy. The picture is frosted anyway,
	// so the downsample is invisible while it keeps three separable passes off the
	// main thread's critical path.
	blurWorkW = 1600
)

// wallpaperCache is background.rs Cache: the decoded and frosted picture of one slot.
// The key holds the radius too, because changing it changes pixels; err keeps a failed
// decode from retrying on every frame.
var wallpaperCache = struct {
	key  string
	bmp  *ui.Bitmap
	err  string
	when time.Time
}{}

// backgroundSlot is the active wallpaper (themes.rs background_slot: the Original
// slot after the theme cleanup).
func (a *App) backgroundSlot() prefs.Background {
	if len(a.Prefs.Backgrounds) == 0 {
		return prefs.Background{}
	}
	return a.Prefs.Backgrounds[0]
}

// wallpaper returns the bitmap for the active slot plus the last decode error, both
// cached per (path, blur, display size) pair. The error surfaces in the settings
// panel status line exactly where background.rs shows Cache::error.
func (a *App) wallpaper() (*ui.Bitmap, string) {
	bg := a.backgroundSlot()
	if !bg.Enabled || bg.Path == "" {
		wallpaperCache.key, wallpaperCache.bmp, wallpaperCache.err = "", nil, ""
		return nil, ""
	}
	maxW, maxH := a.wallpaperPixelCap()
	key := bg.Path + "|" + strconv.FormatFloat(bg.Blur, 'f', 2, 64) +
		"|" + strconv.Itoa(maxW) + "x" + strconv.Itoa(maxH)
	if wallpaperCache.key == key {
		return wallpaperCache.bmp, wallpaperCache.err
	}
	bmp, err := loadWallpaper(bg.Path, bg.Blur, maxW, maxH)
	msg := ""
	if err != nil {
		msg = err.Error()
	}
	wallpaperCache.key, wallpaperCache.bmp, wallpaperCache.err = key, bmp, msg
	wallpaperCache.when = time.Now()
	if bmp != nil {
		trimDecodedImageMemory()
	}
	return bmp, msg
}

// wallpaperPixelCap bounds the decoded picture to the physical pixels of the
// content area: the window is fixed at 1200×800 DIPs, so a 2560×1600 decode
// spent memory on pixels no display ever showed and on halved levels.
func (a *App) wallpaperPixelCap() (int, int) {
	scale := 1.0
	if a.Win != nil {
		if dpi := displayDPI(a.Win.NativeHandle()); dpi >= 96 {
			scale = float64(dpi) / 96
		}
	}
	w := int(math.Ceil(float64(WindowW) * scale))
	h := int(math.Ceil(float64(WindowH-TitlebarH) * scale))
	return min(w, wallpaperMaxW), min(h, wallpaperMaxH)
}

// paintBackground fills the surface below the titlebar with the wallpaper
// (background.rs paint): opacity layer, Contain/Cover fit, brightness overlay and
// the top veil. Called inside the root's children, before the titlebar.
func (a *App) paintBackground(c *ui.Context, w, h float32) {
	bg := a.backgroundSlot()
	bmp, _ := a.wallpaper()
	if bmp == nil {
		return
	}
	s := &a.Palette.Scheme
	rTop := float32(TitlebarH)
	regionH := h - rTop

	// One translucent layer carries the opacity; the image fits inside it.
	layer := ui.Box(c).Absolute().Left(0).Top(rTop).Width(w).Height(regionH).
		Opacity(float32(bg.Opacity))
	layer.Children(func() {
		ui.Image(c, bmp).Fill().Fit(fitOf(bg.Mode))
		// Brightness dims with black and lifts with white (background.rs). The overlay
		// must be absolute: a Fill child after the image would be laid out below it,
		// outside the layer, and the slider would do nothing.
		if bg.Brightness < 1 {
			ui.Box(c).Absolute().Fill().Background(AlphaCol(0x000000, 1-bg.Brightness))
		} else if bg.Brightness > 1 {
			ui.Box(c).Absolute().Fill().Background(AlphaCol(0xFFFFFF, bg.Brightness-1))
		}
	})

	// Title veil: the surface color fading out across the first 112px under the
	// content top (background.rs veil, two-stop approximation).
	fadeStart := float32(ContentTop) - rTop
	fadeEnd := float32(ContentTop) + 112 - rTop
	if fadeEnd > regionH {
		fadeEnd = regionH
	}
	if fadeEnd <= fadeStart {
		return
	}
	ui.Box(c).Absolute().Left(0).Top(rTop + fadeStart).Width(w).Height(fadeEnd - fadeStart).
		Draw(func(pc *ui.Painter, r ui.Rect) {
			pc.FillGradient(r, ui.LinearGradient{
				From:  AlphaCol(s.Surface, 0.9),
				To:    AlphaCol(s.Surface, 0),
				Angle: 180,
			}, 0)
		})
}

func fitOf(mode string) ui.Fit {
	if mode == "Contain" {
		return ui.Contain
	}
	return ui.Cover
}

// loadWallpaper decodes the slot image, caps it to the display's pixels and frosts
// it, then uploads it once: background.rs decode + frosted + refresh_background.
func loadWallpaper(path string, blur float64, maxW, maxH int) (*ui.Bitmap, error) {
	src, err := decodeWallpaper(path, maxW, maxH, true)
	if err != nil {
		return nil, err
	}
	if blur > 0 {
		scale := 1.0
		if w := src.Rect.Dx(); w > blurWorkW {
			h := int(math.Round(float64(src.Rect.Dy()) * float64(blurWorkW) / float64(w)))
			if h < 1 {
				h = 1
			}
			src = scaleNRGBA(src, blurWorkW, h)
			scale = float64(blurWorkW) / float64(w)
		}
		src = frosted(src, blur, scale)
	}
	return ui.NewBitmap(src), nil
}

// decodeWallpaper validates and reads one source image with the given caps,
// returning the capped copy as straight-alpha NRGBA. cover keeps the whole
// content area filled: a square picture capped fit-within would be upscaled
// horizontally by Cover and look soft on a high-DPI screen.
func decodeWallpaper(path string, maxW, maxH int, cover bool) (*image.NRGBA, error) {
	info, err := os.Stat(path)
	if err != nil || info.IsDir() {
		return nil, errors.New("找不到图片，请重新选择")
	}
	if info.Size() > wallpaperMaxBytes {
		return nil, errors.New("图片超过 64 MB，请选择较小的图")
	}
	file, err := os.Open(path)
	if err != nil {
		return nil, errors.New("无法打开图片")
	}
	defer file.Close()
	// The decoders for PNG, JPEG, GIF, BMP and WebP are registered by mygo's own
	// DecodeBitmap imports, so plain image.Decode covers the same set as image::.
	img, _, err := image.Decode(file)
	if err != nil {
		return nil, errors.New("图片损坏或格式不支持，请选择 PNG、JPEG 或 BMP")
	}
	b := img.Bounds()
	if b.Dx() == 0 || b.Dy() == 0 {
		return nil, errors.New("图片尺寸无效")
	}
	if b.Dx() > maxW || b.Dy() > maxH {
		w, h := fitWithin(b.Dx(), b.Dy(), maxW, maxH)
		if cover {
			w, h = coverWithin(b.Dx(), b.Dy(), maxW, maxH)
		}
		return scaledImage(img, w, h), nil
	}
	return toNRGBA(img), nil
}

// Scale the decoded source directly. Converting a full-size JPEG to NRGBA
// first would allocate four bytes for every source pixel just to discard it.
func scaledImage(src image.Image, w, h int) *image.NRGBA {
	if m, ok := src.(*image.NRGBA); ok && m.Rect.Min == (image.Point{}) {
		return scaleNRGBA(m, w, h)
	}
	if src.Bounds().Dx() == w && src.Bounds().Dy() == h {
		return toNRGBA(src)
	}
	dst := image.NewNRGBA(image.Rect(0, 0, w, h))
	xdraw.BiLinear.Scale(dst, dst.Bounds(), src, src.Bounds(), xdraw.Src, nil)
	return dst
}

// frosted approximates background.rs frosted(): three separable box passes reach a
// gaussian closely enough that the radius slider reads the same. scale is the
// work-image ratio, so the radius keeps its source-pixel meaning.
func frosted(src *image.NRGBA, blur, scale float64) *image.NRGBA {
	r := int(math.Round(blur * scale / 2))
	if r < 1 {
		r = 1
	}
	w, h := src.Rect.Dx(), src.Rect.Dy()
	if maxR := (w - 1) / 2; r > maxR {
		r = maxR
	}
	if maxR := (h - 1) / 2; r > maxR {
		r = maxR
	}
	if r < 1 {
		return src
	}
	dst := image.NewNRGBA(image.Rect(0, 0, w, h))
	copy(dst.Pix, src.Pix)
	tmp := make([]uint8, len(dst.Pix))
	for i := 0; i < 3; i++ {
		blurRows(dst.Pix, tmp, w, h, r)
		blurCols(dst.Pix, tmp, w, h, r)
	}
	return dst
}

// blurRows averages horizontal windows of 2r+1 clamped samples, in place.
func blurRows(pix, tmp []uint8, w, h, r int) {
	copy(tmp, pix)
	window := 2*r + 1
	for y := 0; y < h; y++ {
		row := y * w * 4
		for c := 0; c < 4; c++ {
			sum := 0
			for x := -r; x <= r; x++ {
				sum += int(tmp[row+clampIdx(x, w)*4+c])
			}
			for x := 0; x < w; x++ {
				pix[row+x*4+c] = uint8(sum / window)
				sum += int(tmp[row+clampIdx(x+r+1, w)*4+c])
				sum -= int(tmp[row+clampIdx(x-r, w)*4+c])
			}
		}
	}
}

// blurCols does the same down the columns, which makes the box separable.
func blurCols(pix, tmp []uint8, w, h, r int) {
	copy(tmp, pix)
	window := 2*r + 1
	for x := 0; x < w; x++ {
		col := x * 4
		stride := w * 4
		for c := 0; c < 4; c++ {
			sum := 0
			for y := -r; y <= r; y++ {
				sum += int(tmp[clampIdx(y, h)*stride+col+c])
			}
			for y := 0; y < h; y++ {
				pix[y*stride+col+c] = uint8(sum / window)
				sum += int(tmp[clampIdx(y+r+1, h)*stride+col+c])
				sum -= int(tmp[clampIdx(y-r, h)*stride+col+c])
			}
		}
	}
}

func clampIdx(v, n int) int {
	if v < 0 {
		return 0
	}
	if v >= n {
		return n - 1
	}
	return v
}

// fitWithin scales w,h down to fit inside mw,mh keeping the aspect ratio
// (image::thumbnail semantics: never upscales).
func fitWithin(w, h, mw, mh int) (int, int) {
	if w <= 0 || h <= 0 {
		return w, h
	}
	sw := float64(mw) / float64(w)
	sh := float64(mh) / float64(h)
	s := sw
	if sh < s {
		s = sh
	}
	if s >= 1 {
		return w, h
	}
	nw := int(math.Round(float64(w) * s))
	nh := int(math.Round(float64(h) * s))
	if nw < 1 {
		nw = 1
	}
	if nh < 1 {
		nh = 1
	}
	return nw, nh
}

// coverWithin scales w,h down until both sides cover mw,mh, keeping the aspect
// ratio (image::thumbnail is the fit counterpart): a square wallpaper becomes
// 1800×1800 for a 1800×1122 area instead of 1122×1122 that Cover would stretch.
func coverWithin(w, h, mw, mh int) (int, int) {
	if w <= 0 || h <= 0 {
		return w, h
	}
	sw := float64(mw) / float64(w)
	sh := float64(mh) / float64(h)
	s := sw
	if sh > s {
		s = sh
	}
	if s >= 1 {
		return w, h
	}
	nw := max(1, int(math.Round(float64(w)*s)))
	nh := max(1, int(math.Round(float64(h)*s)))
	return nw, nh
}

// toNRGBA converts any decoded image into straight-alpha NRGBA, with fast paths for
// the two types the standard decoders return.
func toNRGBA(img image.Image) *image.NRGBA {
	switch m := img.(type) {
	case *image.NRGBA:
		return m
	case *image.RGBA:
		b := m.Bounds()
		dst := image.NewNRGBA(image.Rect(0, 0, b.Dx(), b.Dy()))
		for y := 0; y < b.Dy(); y++ {
			i := m.PixOffset(b.Min.X, b.Min.Y+y)
			o := dst.PixOffset(0, y)
			for x := 0; x < b.Dx(); x++ {
				a := uint32(m.Pix[i+3])
				if a == 0 || a == 255 {
					dst.Pix[o], dst.Pix[o+1], dst.Pix[o+2], dst.Pix[o+3] = m.Pix[i], m.Pix[i+1], m.Pix[i+2], m.Pix[i+3]
				} else {
					dst.Pix[o] = uint8(uint32(m.Pix[i]) * 255 / a)
					dst.Pix[o+1] = uint8(uint32(m.Pix[i+1]) * 255 / a)
					dst.Pix[o+2] = uint8(uint32(m.Pix[i+2]) * 255 / a)
					dst.Pix[o+3] = uint8(a)
				}
				i += 4
				o += 4
			}
		}
		return dst
	}
	b := img.Bounds()
	dst := image.NewNRGBA(image.Rect(0, 0, b.Dx(), b.Dy()))
	for y := 0; y < b.Dy(); y++ {
		for x := 0; x < b.Dx(); x++ {
			c := color.NRGBAModel.Convert(img.At(b.Min.X+x, b.Min.Y+y)).(color.NRGBA)
			o := dst.PixOffset(x, y)
			dst.Pix[o], dst.Pix[o+1], dst.Pix[o+2], dst.Pix[o+3] = c.R, c.G, c.B, c.A
		}
	}
	return dst
}

// scaleNRGBA resizes with bilinear sampling.
func scaleNRGBA(src *image.NRGBA, w, h int) *image.NRGBA {
	sw, sh := src.Rect.Dx(), src.Rect.Dy()
	if w == sw && h == sh {
		return src
	}
	dst := image.NewNRGBA(image.Rect(0, 0, w, h))
	sx, sy := float64(sw)/float64(w), float64(sh)/float64(h)
	for y := 0; y < h; y++ {
		fy := (float64(y)+0.5)*sy - 0.5
		y0 := clampIdx(int(math.Floor(fy)), sh)
		y1 := clampIdx(y0+1, sh)
		wy := fy - math.Floor(fy)
		for x := 0; x < w; x++ {
			fx := (float64(x)+0.5)*sx - 0.5
			x0 := clampIdx(int(math.Floor(fx)), sw)
			x1 := clampIdx(x0+1, sw)
			wx := fx - math.Floor(fx)
			i00, i10 := src.PixOffset(x0, y0), src.PixOffset(x1, y0)
			i01, i11 := src.PixOffset(x0, y1), src.PixOffset(x1, y1)
			o := dst.PixOffset(x, y)
			for c := 0; c < 4; c++ {
				top := float64(src.Pix[i00+c])*(1-wx) + float64(src.Pix[i10+c])*wx
				bot := float64(src.Pix[i01+c])*(1-wx) + float64(src.Pix[i11+c])*wx
				dst.Pix[o+c] = uint8(math.Round(top*(1-wy) + bot*wy))
			}
		}
	}
	return dst
}

// importBackground mirrors background.rs import_background: validate the source by
// decoding it with the same caps, store the capped copy as PNG inside
// Data/Backgrounds, point slot 0 at it and persist with a rollback.
func (a *App) importBackground(src string) error {
	// Store at the background.rs cap, not the current display's: the stored copy
	// must still fill the window after a move to a higher-DPI screen.
	img, err := decodeWallpaper(src, wallpaperMaxW, wallpaperMaxH, false)
	if err != nil {
		return err
	}
	dir := filepath.Join(a.Data, "Backgrounds")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return errors.New("无法保存图片目录")
	}
	dst := filepath.Join(dir, fmt.Sprintf("theme-0-%d.png", time.Now().UnixNano()))
	f, err := os.Create(dst)
	if err != nil {
		return errors.New("无法保存背景图片")
	}
	encoded := png.Encode(f, img)
	closed := f.Close()
	if encoded != nil || closed != nil {
		os.Remove(dst)
		return errors.New("无法保存背景图片")
	}
	old := a.Prefs
	if len(a.Prefs.Backgrounds) == 0 {
		a.Prefs.Backgrounds = make([]prefs.Background, 6)
	}
	a.Prefs.Backgrounds[0].Path = dst
	a.Prefs.Backgrounds[0].Enabled = true
	if err := a.savePrefsErr(); err != nil {
		a.Prefs = old
		os.Remove(dst)
		return err
	}
	a.Refresh()
	trimDecodedImageMemory()
	return nil
}

// savePrefsErr persists preferences and reports the failure: root.savePrefs ignores
// it, but the import flow rolls back like background.rs persist().
func (a *App) savePrefsErr() error {
	path, _, _ := prefs.Paths(a.Data)
	return prefs.Save(path, &a.Prefs)
}
