// Rossela wordmark shared with About, README and the native renderer.
package ui

import (
	"fmt"
	"github.com/egoist/mygo/ui"
	"os"
	"strconv"
	"sync"
)

// One quiet ink reveal, a brief rest, then a shared crossfade into the page.
const (
	sigWrite = 1.00
	sigHold  = .28
	sigExit  = .22
)

func sigSlow() float64 {
	if n, err := strconv.ParseFloat(os.Getenv("DOTISLE_SIG_SLOW"), 64); err == nil && n > 0 {
		return n
	}
	return 1
}
func signatureDuration(reduced bool) float64 {
	if reduced {
		return .24
	}
	return (sigWrite + sigHold + sigExit) * sigSlow()
}
func sigInk(elapsed float64, reduced bool) (float64, float64) {
	if reduced {
		return 1, 1 - float64(motionEase(float32(clampF64((elapsed-.08)/.16, 0, 1))))
	}
	elapsed /= sigSlow()
	return clampF64(elapsed/sigWrite, 0, 1), 1 - float64(motionEase(float32(clampF64((elapsed-sigWrite-sigHold)/sigExit, 0, 1))))
}

type sigPoint struct{ x, y float64 }

// sigParsePath parses the M/C/L/Z subset both the routes and outlines use.
func sigParsePath(svg string) [][]sigPoint {
	var strokes [][]sigPoint
	var cur []sigPoint
	pos := sigPoint{}
	read := func(from int) (float64, int) {
		v := 0.0
		div := 0.0
		neg := false
		started := false
		i := from
		// SVG paths separate coordinates with spaces/commas; skip them first.
		for i < len(svg) && (svg[i] == ' ' || svg[i] == ',' || svg[i] < 0x20) {
			i++
		}
		for ; i < len(svg); i++ {
			ch := svg[i]
			if ch == '-' {
				if started {
					break
				}
				neg = true
				started = true
				continue
			}
			if ch >= '0' && ch <= '9' {
				v = v*10 + float64(ch-'0')
				started = true
				if div > 0 {
					div *= 10
				}
				continue
			}
			if ch == '.' {
				if div == 0 {
					div = 1
				}
				continue
			}
			break
		}
		if div > 1 {
			v /= div
		}
		if neg {
			v = -v
		}
		return v, i
	}
	i := 0
	for i < len(svg) {
		ch := svg[i]
		switch ch {
		case 'M':
			x, n := read(i + 1)
			y, m := read(n)
			i = m
			if len(cur) > 0 {
				strokes = append(strokes, cur)
				cur = nil
			}
			pos = sigPoint{x, y}
			cur = append(cur, pos)
		case 'C':
			var pts [3]sigPoint
			off := i + 1
			for k := 0; k < 3; k++ {
				x, n := read(off)
				y, m := read(n)
				off = m
				pts[k] = sigPoint{x, y}
			}
			i = off
			for t := 1; t <= 48; t++ {
				tt := float64(t) / 48
				u := 1 - tt
				cur = append(cur, sigPoint{
					u*u*u*pos.x + 3*u*u*tt*pts[0].x + 3*u*tt*tt*pts[1].x + tt*tt*tt*pts[2].x,
					u*u*u*pos.y + 3*u*u*tt*pts[0].y + 3*u*tt*tt*pts[1].y + tt*tt*tt*pts[2].y,
				})
			}
			pos = pts[2]
		case 'L':
			x, n := read(i + 1)
			y, m := read(n)
			i = m
			pos = sigPoint{x, y}
			cur = append(cur, pos)
		case 'Z':
			i++
			if len(cur) > 0 {
				strokes = append(strokes, cur)
				cur = nil
			}
		default:
			i++
		}
	}
	if len(cur) > 0 {
		strokes = append(strokes, cur)
	}
	return strokes
}

// sigPathPoly keeps every contour closed with its original nonzero winding.
func sigPathPoly(contours [][]sigPoint, tr func(sigPoint) (float32, float32)) *ui.Path {
	path := ui.Path{}
	for _, pts := range contours {
		for i, pt := range pts {
			px, py := tr(pt)
			if i == 0 {
				path.MoveTo(px, py)
			} else {
				path.LineTo(px, py)
			}
		}
		if len(pts) > 0 {
			path.Close()
		}
	}
	return &path
}

var wordmarkOnce sync.Once
var wordmarkContours [][][]sigPoint

func wordmarkBuild() {
	wordmarkOnce.Do(func() {
		for _, outline := range signatureOutlines {
			wordmarkContours = append(wordmarkContours, sigParsePath(outline))
		}
	})
}
func drawWordmark(c *ui.Context, x, y, w, h float32, rgb uint32) {
	wordmarkBuild()
	ui.Box(c).Absolute().Left(x).Top(y).Width(w).Height(h).Label("Dotisle Rossela 字标").IgnoreInput(true).
		Draw(func(pc *ui.Painter, r ui.Rect) {
			scale := min(r.W/signatureWidth, r.H/signatureHeight)
			tr := func(pt sigPoint) (float32, float32) {
				return r.X + r.W/2 + (float32(pt.x)-signatureWidth/2)*scale, r.Y + r.H/2 + (float32(pt.y)-signatureHeight/2)*scale
			}
			for _, outline := range wordmarkContours {
				pc.FillPath(sigPathPoly(outline, tr), Col(rgb))
			}
		})
}
func (a *App) drawSignature(c *ui.Context, x, y, w, h float32, elapsed float64, reduced bool) {
	progress, alpha := sigInk(elapsed, reduced)
	if elapsed < 0 || alpha <= 0 {
		return
	}
	pose := float32(1)
	if !reduced {
		pose = motionEase(float32(clampF64(elapsed/sigSlow()/.42, 0, 1)))
	}
	layer := ui.Box(c).Absolute().Left(x).Top(y).Width(w).Height(h).IgnoreInput(true).
		Translate(0, 8*(1-pose)).Scale(.97+.03*pose, .97+.03*pose)
	if progress < 1 {
		edge := float32(progress * 1.16)
		layer.MaskLinearGradient(ui.LinearGradient{Angle: 90, Stops: []ui.GradientStop{
			{Color: ui.RGB(0, 0, 0), At: 0}, {Color: ui.RGB(0, 0, 0), At: max(float32(0), edge-.16)},
			{Color: ui.Transparent, At: min(float32(1), edge)}, {Color: ui.Transparent, At: 1},
		}})
	}
	layer.Children(func() { drawWordmark(c, 0, 0, w, h, a.Palette.Scheme.OnSurface) })
}
func DebugSignature() {
	wordmarkBuild()
	fmt.Printf("Rossela glyphs=%d bounds=%.1fx%.1f\n", len(wordmarkContours), signatureWidth, signatureHeight)
}
