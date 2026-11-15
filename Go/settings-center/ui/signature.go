// The handwritten "Dotisle" wordmark (dotisle-render brand_signature.rs): the seven
// filled letter outlines and pen routes are ported verbatim. The write-on fills each
// letter's outline clipped by a fat round-capped pen mask along its route — the same
// construction as the Rust draw(), with the stroke-to-region conversion done by hand
// (MyGo has no stroke-to-path).
package ui

import (
	"fmt"
	"math"
	"os"
	"strconv"

	"github.com/egoist/mygo/ui"
)

// Pen timing (brand_signature.rs).
const (
	sigWrite = 1.12
	sigHold  = 0.18
	sigExit  = 0.18
)

// sigSlow returns the DOTISLE_SIG_SLOW multiplier (e2e capture hook): it
// stretches the reveal so the scripted shots — which start ~2.5s after launch —
// can catch mid-write frames. Both the duration and the ink clock use it.
func sigSlow() float64 {
	if v := os.Getenv("DOTISLE_SIG_SLOW"); v != "" {
		if n, err := strconv.ParseFloat(v, 64); err == nil && n > 0 {
			return n
		}
	}
	return 1
}

func signatureDuration(reduced bool) float64 {
	if reduced {
		return 0.24
	}
	return (sigWrite + sigHold + sigExit) * sigSlow()
}

// sigInk returns (write progress 0..1, cover alpha 0..1).
func sigInk(elapsed float64, reduced bool) (float64, float64) {
	// The slow-mo hook divides the wall clock first, so every phase below
	// works on the unscaled pen timeline (0.24s when reduced).
	if !reduced {
		elapsed /= sigSlow()
	}
	if reduced {
		if elapsed >= 0.24 {
			return 1, 0
		}
		return 1, 1 - clampF64((elapsed-0.08)/0.16, 0, 1)
	}
	if elapsed >= sigWrite+sigHold+sigExit {
		return 1, 0
	}
	exit := clampF64((elapsed-sigWrite-sigHold)/sigExit, 0, 1)
	return clampF64(elapsed/sigWrite, 0, 1), 1 - float64(exit)
}

type sigPoint struct{ x, y float64 }

type sigGlyph struct {
	outline [][]sigPoint // separate closed contours preserve the letter counters
	strokes [][]sigPoint
	length  float64
}

var sigPen struct {
	glyphs []sigGlyph
	length float64
	built  bool
}

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

func polyLength(strokes [][]sigPoint) float64 {
	total := 0.0
	for _, pts := range strokes {
		for i := 1; i < len(pts); i++ {
			dx, dy := pts[i].x-pts[i-1].x, pts[i].y-pts[i-1].y
			total += math.Sqrt(dx*dx + dy*dy)
		}
	}
	return total
}

// sigBuild parses the seven outlines and pen routes once.
func sigBuild() {
	if sigPen.built {
		return
	}
	sigPen.built = true
	for i, route := range signatureRoutes {
		strokes := sigParsePath(route)
		outline := sigParsePath(signatureOutlines[i])
		g := sigGlyph{outline: outline, strokes: strokes, length: polyLength(strokes)}
		sigPen.glyphs = append(sigPen.glyphs, g)
		sigPen.length += g.length
	}
}

func flatten(strokes [][]sigPoint) []sigPoint {
	var pts []sigPoint
	for _, s := range strokes {
		pts = append(pts, s...)
	}
	return pts
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

// sigStrokeRegion builds the fat round-capped pen region along a partial polyline.
// width is the full stroke width in letter units; remaining caps the travel.
func sigStrokeRegion(strokes [][]sigPoint, width float64, remaining float64, tr func(sigPoint) (float32, float32)) *ui.Path {
	half := width / 2
	path := ui.Path{}
	addQuad := func(a, b sigPoint) {
		dx, dy := b.x-a.x, b.y-a.y
		l := math.Sqrt(dx*dx + dy*dy)
		if l < 1e-9 {
			return
		}
		px, py := -dy/l*half, dx/l*half
		x0, y0 := px, py
		x1, y1 := -px, -py
		first := true
		for _, pt := range []sigPoint{
			{a.x + x0, a.y + y0}, {b.x + x0, b.y + y0},
			{b.x + x1, b.y + y1}, {a.x + x1, a.y + y1},
		} {
			if first {
				path.MoveTo(tr(pt))
				first = false
			} else {
				path.LineTo(tr(pt))
			}
		}
		path.LineTo(tr(sigPoint{a.x + x0, a.y + y0}))
	}
	addDisc := func(center sigPoint) {
		steps := 16
		first := true
		for k := 0; k <= steps; k++ {
			a := float64(k) / float64(steps) * 2 * math.Pi
			px := float32(center.x + half*math.Cos(a))
			py := float32(center.y + half*math.Sin(a))
			if first {
				path.MoveTo(tr(sigPoint{float64(px), float64(py)}))
				first = false
			} else {
				path.LineTo(tr(sigPoint{float64(px), float64(py)}))
			}
		}
	}
	for _, pts := range strokes {
		if remaining <= 0 {
			break
		}
		for i := 1; i < len(pts); i++ {
			if remaining <= 0 {
				break
			}
			seg := pts[i-1]
			next := pts[i]
			dx, dy := next.x-seg.x, next.y-seg.y
			l := math.Sqrt(dx*dx + dy*dy)
			if l <= 0 {
				continue
			}
			if remaining >= l {
				addQuad(seg, next)
				remaining -= l
			} else {
				t := remaining / l
				tip := sigPoint{seg.x + dx*t, seg.y + dy*t}
				addQuad(seg, tip)
				addDisc(tip)
				remaining = 0
			}
		}
		if remaining > 0 {
			addDisc(pts[len(pts)-1])
		}
	}
	return &path
}

// drawSignature renders the wordmark cover: completed letters as filled outlines,
// the letter being written masked by the pen region — the Rust draw() construction.
// The about card uses the exact opening outlines without an idle animation.
func drawWordmark(c *ui.Context, x, y, w, h float32, rgb uint32) {
	sigBuild()
	ui.Box(c).Absolute().Left(x).Top(y).Width(w).Height(h).Label("Dotisle 手写字标").IgnoreInput(true).
		Draw(func(pc *ui.Painter, r ui.Rect) {
			scale := float64(min(r.W/328, r.H/106))
			tr := func(pt sigPoint) (float32, float32) {
				return r.X + r.W/2 + float32((pt.x-164)*scale), r.Y + r.H/2 + float32((pt.y-74)*scale)
			}
			for _, glyph := range sigPen.glyphs {
				pc.FillPath(sigPathPoly(glyph.outline, tr), Col(rgb))
			}
		})
}

func (a *App) drawSignature(c *ui.Context, x, y, w, h float32, elapsed float64, reduced bool) {
	sigBuild()
	if elapsed <= 0 || elapsed > signatureDuration(reduced)+0.05 {
		return
	}
	progress, inkAlpha := sigInk(elapsed, reduced)
	if inkAlpha <= 0 {
		return
	}
	// The parent cover owns the fade, so ink is composited exactly once.
	ink := Col(a.Palette.Scheme.OnSurface)
	scale := float64(minF(w/328, h/106))
	if scale <= 0 {
		return
	}
	tr := func(pt sigPoint) (float32, float32) {
		return float32(float64(x) + float64(w)/2 + (pt.x-164)*scale),
			float32(float64(y) + float64(h)/2 + (pt.y-74)*scale)
	}
	remaining := sigPen.length * progress
	for _, glyph := range sigPen.glyphs {
		if remaining <= 0 {
			break
		}
		outline := sigPathPoly(glyph.outline, tr)
		if progress >= 1 || remaining >= glyph.length {
			ui.Box(c).Fill().Draw(func(pc *ui.Painter, r ui.Rect) {
				pc.FillPath(outline, ink)
			})
		} else {
			mask := sigStrokeRegion(glyph.strokes, 18, remaining, tr)
			ui.Box(c).Fill().Draw(func(pc *ui.Painter, r ui.Rect) {
				pc.ClipPath(mask, func() {
					pc.FillPath(outline, ink)
				})
			})
		}
		remaining -= glyph.length
	}
}

// DebugSignature prints parse statistics for the wordmark (sigdebug tool).
func DebugSignature() {
	sigBuild()
	fmt.Printf("glyphs=%d totalLength=%.1f\n", len(sigPen.glyphs), sigPen.length)
	for i, g := range sigPen.glyphs {
		minX, minY := 1e9, 1e9
		maxX, maxY := -1e9, -1e9
		for _, p := range flatten(g.outline) {
			minX = math.Min(minX, p.x)
			minY = math.Min(minY, p.y)
			maxX = math.Max(maxX, p.x)
			maxY = math.Max(maxY, p.y)
		}
		fmt.Printf("glyph %d: outlinePts=%d bbox=[%.1f,%.1f..%.1f,%.1f] strokes=%d len=%.1f\n",
			i, len(flatten(g.outline)), minX, minY, maxX, maxY, len(g.strokes), g.length)
	}
}
