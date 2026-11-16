// Wallpaper pipeline tests: the decode guards, the 2560x1600 cap, the frosted blur
// radius and the import flow, all without a GPU context.
package ui

import (
	"bytes"
	"image"
	"image/color"
	"image/jpeg"
	"image/png"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"dotisle/settings/prefs"
)

// writeImage writes enc-encoded pixels to a temp file and returns its path.
func writeImage(t *testing.T, dir, name string, img image.Image, enc func(*os.File) error) string {
	t.Helper()
	p := filepath.Join(dir, name)
	f, err := os.Create(p)
	if err != nil {
		t.Fatalf("create %s: %v", p, err)
	}
	if err := enc(f); err != nil {
		f.Close()
		t.Fatalf("encode %s: %v", p, err)
	}
	if err := f.Close(); err != nil {
		t.Fatalf("close %s: %v", p, err)
	}
	return p
}

// checker builds a high-contrast board: the blur test needs detail to lose.
func checker(w, h, cell int) *image.NRGBA {
	im := image.NewNRGBA(image.Rect(0, 0, w, h))
	for y := 0; y < h; y++ {
		for x := 0; x < w; x++ {
			on := ((x/cell)+(y/cell))%2 == 0
			v := uint8(30)
			if on {
				v = uint8(230)
			}
			im.Set(x, y, color.NRGBA{R: v, G: v, B: v, A: 255})
		}
	}
	return im
}

// detail is the mean absolute horizontal gradient, the same measure the screenshot
// pass used: it falls when the picture is frosted.
func detail(im *image.NRGBA) float64 {
	total, n := 0, 0
	for y := 0; y < im.Rect.Dy(); y++ {
		for x := 1; x < im.Rect.Dx(); x++ {
			a := int(im.Pix[im.PixOffset(x-1, y)])
			b := int(im.Pix[im.PixOffset(x, y)])
			total += absInt(a - b)
			n++
		}
	}
	if n == 0 {
		return 0
	}
	return float64(total) / float64(n)
}

func absInt(v int) int {
	if v < 0 {
		return -v
	}
	return v
}

func TestDecodeWallpaperGuards(t *testing.T) {
	dir := t.TempDir()
	good := writeImage(t, dir, "good.png", checker(64, 48, 8),
		func(f *os.File) error { return png.Encode(f, checker(64, 48, 8)) })

	if _, err := decodeWallpaper(filepath.Join(dir, "absent.png"), wallpaperMaxW, wallpaperMaxH, false); err == nil ||
		err.Error() != "找不到图片，请重新选择" {
		t.Fatalf("missing file error = %v", err)
	}

	// A directory is not a picture either, and must not panic.
	if _, err := decodeWallpaper(dir, wallpaperMaxW, wallpaperMaxH, false); err == nil {
		t.Fatal("a directory decoded as a picture")
	}

	broken := filepath.Join(dir, "broken.jpg")
	var small bytes.Buffer
	if err := jpeg.Encode(&small, checker(80, 60, 8), nil); err != nil {
		t.Fatalf("encode jpeg: %v", err)
	}
	raw := small.Bytes()
	if err := os.WriteFile(broken, raw[:len(raw)/3], 0o644); err != nil {
		t.Fatalf("write broken jpeg: %v", err)
	}
	if _, err := decodeWallpaper(broken, wallpaperMaxW, wallpaperMaxH, false); err == nil ||
		!strings.Contains(err.Error(), "图片损坏或格式不支持") {
		t.Fatalf("truncated jpeg error = %v", err)
	}

	// background.rs caps the file at 64 MB before decoding it.
	fat := filepath.Join(dir, "fat.png")
	f, err := os.Create(fat)
	if err != nil {
		t.Fatalf("create fat: %v", err)
	}
	if err := f.Truncate(wallpaperMaxBytes + 1); err != nil {
		f.Close()
		t.Fatalf("truncate fat: %v", err)
	}
	f.Close()
	if _, err := decodeWallpaper(fat, wallpaperMaxW, wallpaperMaxH, false); err == nil ||
		err.Error() != "图片超过 64 MB，请选择较小的图" {
		t.Fatalf("oversized file error = %v", err)
	}

	img, err := decodeWallpaper(good, wallpaperMaxW, wallpaperMaxH, false)
	if err != nil {
		t.Fatalf("decode good: %v", err)
	}
	if img.Rect.Dx() != 64 || img.Rect.Dy() != 48 {
		t.Fatalf("good decode size = %v", img.Rect)
	}
}

func TestDecodeWallpaperCapsSize(t *testing.T) {
	dir := t.TempDir()
	// 3000x2000 is over the 2560x1600 cap and must keep its aspect ratio: the height
	// binds first, so the result is 2400x1600.
	big := checker(3000, 2000, 64)
	path := writeImage(t, dir, "big.png", big, func(f *os.File) error { return png.Encode(f, big) })
	img, err := decodeWallpaper(path, wallpaperMaxW, wallpaperMaxH, false)
	if err != nil {
		t.Fatalf("decode big: %v", err)
	}
	w, h := img.Rect.Dx(), img.Rect.Dy()
	if w != 2400 || h != wallpaperMaxH {
		t.Fatalf("capped size = %dx%d, want 2400x1600", w, h)
	}

	exact := checker(wallpaperMaxW, wallpaperMaxH, 64)
	path = writeImage(t, dir, "exact.png", exact, func(f *os.File) error { return png.Encode(f, exact) })
	img, err = decodeWallpaper(path, wallpaperMaxW, wallpaperMaxH, false)
	if err != nil {
		t.Fatalf("decode exact: %v", err)
	}
	if img.Rect.Dx() != wallpaperMaxW || img.Rect.Dy() != wallpaperMaxH {
		t.Fatalf("exact size = %v", img.Rect)
	}
}

func TestDecodeWallpaperCoverModeKeepsSquareSharp(t *testing.T) {
	dir := t.TempDir()
	// A square wallpaper must arrive at 1800×1800 for a 1800×1122 content area:
	// capping it fit-within would leave Cover to stretch 1122 to 1800.
	square := checker(2000, 2000, 64)
	path := writeImage(t, dir, "square.png", square, func(f *os.File) error { return png.Encode(f, square) })
	img, err := decodeWallpaper(path, 1800, 1122, true)
	if err != nil {
		t.Fatalf("decode square: %v", err)
	}
	if w, h := img.Rect.Dx(), img.Rect.Dy(); w != 1800 || h != 1800 {
		t.Fatalf("cover-capped square = %dx%d, want 1800x1800", w, h)
	}
}

func TestFrostedKeepsTheRadiusMeaningful(t *testing.T) {
	src := checker(400, 300, 16)
	sharp := detail(src)
	if sharp < 8 {
		t.Fatalf("fixture is not detailed enough: %.2f", sharp)
	}
	soft := detail(frosted(src, 4, 1))
	softer := detail(frosted(src, 20, 1))
	t.Logf("detail sharp=%.2f blur4=%.2f blur20=%.2f", sharp, soft, softer)
	if soft >= sharp*0.9 {
		t.Fatalf("blur 4 detail %.2f did not fall below the sharp %.2f", soft, sharp)
	}
	if softer >= soft {
		t.Fatalf("blur 20 detail %.2f is not below blur 4 %.2f", softer, soft)
	}
	// A 16px cell averaged over a 2r+1 window at r=10 is nearly flat, so the heavier
	// radius leaves a quarter of the pattern at most.
	if softer > sharp*0.3 {
		t.Fatalf("blur 20 left detail %.2f of the sharp %.2f", softer, sharp)
	}
	// The source must not be modified in place: the cache keeps it for blur 0.
	if detail(src) != sharp {
		t.Fatal("frosted mutated its input")
	}
	// A divisor that clamps a huge radius cannot exceed the image.
	if got := frosted(checker(8, 8, 2), 40, 1); got.Rect.Dx() != 8 {
		t.Fatalf("tiny image resized to %v", got.Rect)
	}
}

func TestFitWithin(t *testing.T) {
	cases := []struct{ w, h, mw, mh, gw, gh int }{
		{3000, 2000, 2560, 1600, 2400, 1600},
		{1000, 4000, 2560, 1600, 400, 1600},
		{6000, 2000, 2560, 1600, 2560, 853},
		{100, 100, 2560, 1600, 100, 100},
		{1, 1, 2560, 1600, 1, 1},
	}
	for _, c := range cases {
		w, h := fitWithin(c.w, c.h, c.mw, c.mh)
		if w != c.gw || h != c.gh {
			t.Fatalf("fitWithin(%d,%d) = %dx%d, want %dx%d", c.w, c.h, w, h, c.gw, c.gh)
		}
	}
}

func TestImportBackgroundStoresPNGAndPrefs(t *testing.T) {
	src := t.TempDir()
	path := writeImage(t, src, "pick.png", checker(120, 90, 10),
		func(f *os.File) error { return png.Encode(f, checker(120, 90, 10)) })

	data := t.TempDir()
	a := newTestApp(t)
	a.Data = data
	a.Prefs.Backgrounds = make([]prefs.Background, 6)

	if err := a.importBackground(path); err != nil {
		t.Fatalf("import: %v", err)
	}
	bg := a.Prefs.Backgrounds[0]
	if !bg.Enabled || bg.Path == "" {
		t.Fatalf("slot 0 not enabled with a path: %+v", bg)
	}
	if !strings.HasPrefix(bg.Path, filepath.Join(data, "Backgrounds")) {
		t.Fatalf("import wrote outside Data/Backgrounds: %s", bg.Path)
	}
	if filepath.Ext(bg.Path) != ".png" {
		t.Fatalf("import kept the source extension: %s", bg.Path)
	}
	stored, err := os.ReadFile(bg.Path)
	if err != nil {
		t.Fatalf("read stored copy: %v", err)
	}
	img, _, err := image.Decode(bytes.NewReader(stored))
	if err != nil {
		t.Fatalf("stored copy is not a picture: %v", err)
	}
	if img.Bounds().Dx() != 120 || img.Bounds().Dy() != 90 {
		t.Fatalf("stored copy resized to %v", img.Bounds())
	}

	// Preferences on disk must already point at the copy.
	prefsPath, _, _ := prefs.Paths(data)
	saved, err := prefs.Load(prefsPath)
	if err != nil {
		t.Fatalf("reload prefs: %v", err)
	}
	if saved.Backgrounds[0].Path != bg.Path || !saved.Backgrounds[0].Enabled {
		t.Fatalf("persisted slot 0 = %+v", saved.Backgrounds[0])
	}

	// A rejected source must leave the slot untouched and store nothing new.
	entries, err := os.ReadDir(filepath.Join(data, "Backgrounds"))
	if err != nil {
		t.Fatalf("read backgrounds dir: %v", err)
	}
	before := len(entries)
	bad := filepath.Join(src, "bad.png")
	if err := os.WriteFile(bad, []byte("not a picture"), 0o644); err != nil {
		t.Fatalf("write bad: %v", err)
	}
	if err := a.importBackground(bad); err == nil {
		t.Fatal("a corrupt file imported")
	}
	if a.Prefs.Backgrounds[0].Path != bg.Path {
		t.Fatal("a failed import changed the slot")
	}
	entries, err = os.ReadDir(filepath.Join(data, "Backgrounds"))
	if err != nil {
		t.Fatalf("read backgrounds dir again: %v", err)
	}
	if len(entries) != before {
		t.Fatalf("a failed import left %d files, want %d", len(entries), before)
	}
}

func TestApplyPacketRoutesPendingPage(t *testing.T) {
	a := newTestApp(t)
	a.Page = PageHome
	// native_sync.rs: every snapshot that carries pendingPage navigates once.
	a.ApplyPacket(map[string]any{"pendingPage": "island"}, true, "")
	if a.Page != PageSettings || a.SettingsTab != TabIslandFirst {
		t.Fatalf("pendingPage=island routed to page %d tab %d", a.Page, a.SettingsTab)
	}
	// The same request must not fight a later manual navigation.
	a.Page = PageTodos
	a.ApplyPacket(map[string]any{"pendingPage": "island"}, true, "")
	if a.Page != PageTodos {
		t.Fatalf("a repeated pendingPage re-navigated to page %d", a.Page)
	}
	a.ApplyPacket(map[string]any{"pendingPage": "todos"}, true, "")
	if a.Page != PageTodos {
		t.Fatalf("pendingPage=todos routed to page %d", a.Page)
	}
	// A transient pipe failure keeps the last snapshot and the current page.
	a.Page = PageWidgets
	a.ApplyPacket(nil, false, "pipe closed")
	if a.Page != PageWidgets || a.Connected {
		t.Fatalf("a nil packet moved the page (%d) or kept connected=%v", a.Page, a.Connected)
	}
}

func TestWallpaperErrorSurfacesWithoutABitmap(t *testing.T) {
	a := newTestApp(t)
	a.Data = t.TempDir()
	a.Prefs.Backgrounds = make([]prefs.Background, 6)
	a.Prefs.Backgrounds[0] = prefs.Background{
		Path: filepath.Join(a.Data, "gone.png"), Enabled: true, Opacity: 1,
		Brightness: 1, Mode: "Cover",
	}
	// Each call site sees the decode error instead of a frozen frame; the slot keeps
	// its picture path so the panel can still show it and the user can pick again.
	bmp, errMsg := a.wallpaper()
	if bmp != nil {
		t.Fatalf("a missing picture produced a bitmap: %v", bmp)
	}
	if errMsg != "找不到图片，请重新选择" {
		t.Fatalf("error message = %q", errMsg)
	}
	// A disabled slot paints nothing even when the file is there.
	dir := t.TempDir()
	good := writeImage(t, dir, "good.png", checker(32, 32, 8),
		func(f *os.File) error { return png.Encode(f, checker(32, 32, 8)) })
	a.Prefs.Backgrounds[0] = prefs.Background{Path: good, Enabled: false, Opacity: 1, Brightness: 1, Mode: "Cover"}
	if bmp, errMsg := a.wallpaper(); bmp != nil || errMsg != "" {
		t.Fatalf("disabled slot = %v, %q", bmp, errMsg)
	}
	// Blur is part of the cache key, so a new radius decodes a new bitmap.
	a.Prefs.Backgrounds[0].Enabled = true
	a.Prefs.Backgrounds[0].Blur = 0
	sharp, errMsg := a.wallpaper()
	if sharp == nil || errMsg != "" {
		t.Fatalf("enabled slot = %v, %q", sharp, errMsg)
	}
	a.Prefs.Backgrounds[0].Blur = 12
	soft, errMsg := a.wallpaper()
	if soft == nil || errMsg != "" {
		t.Fatalf("blurred slot = %v, %q", soft, errMsg)
	}
	if soft == sharp {
		t.Fatal("the blur radius did not change the cached bitmap")
	}
}
