// Rossela wordmark shared with About, README and the native renderer.
package ui

import (
	"fmt"
	"github.com/egoist/mygo/ui"
	"github.com/egoist/mygo/ui/shader"
	"math"
	"os"
	"strconv"
	"sync"
)

// Pure typography "Light & Refraction" entrance: directional light shaft + caustics, Snell lens refraction, chromatic dispersion convergence, internal prismatic caustic sweep, and i-dot crystal glint.
const (
	sigWrite      = 0.96
	sigHold       = 0.30
	sigExit       = 0.24
	openingLightW = 104
	openingLightH = 65
)

var openingLightShader = shader.MustCompile(`package main

var Dark float
var Progress float
var BaseRGB vec3
var AccentRGB vec3

func Fragment(dst vec4, pos vec2) vec4 {
	size := rectSize()
	uv := pos / max(size, vec2(1.0, 1.0))

	col := BaseRGB
	topFade := smoothstep(0.0, 0.14, uv.y)
	sideFade := smoothstep(0.0, 0.08, uv.x) * (1.0 - smoothstep(0.92, 1.0, uv.x))
	edgeFade := topFade * (0.45 + 0.55*sideFade)

	beamPos := -0.24 + 1.46*clamp(Progress/0.90, 0.0, 1.0)
	beamEnv := smoothstep(0.0, 0.14, Progress) * (1.0 - smoothstep(0.76, 1.0, Progress))
	settleEnv := smoothstep(0.32, 0.78, Progress) * (1.0 - smoothstep(0.90, 1.0, Progress)*0.35)

	s := uv.x*0.84 + uv.y*0.34 - beamPos
	cy := (uv.y - 0.46) * 2.15
	centerFocus := exp(-(cy * cy) / 0.52)
	bend := 0.052 * sin((uv.y-0.46)*5.0)

	beam := exp(-(s * s) / 0.024) * beamEnv
	causticCore := exp(-(s * s) / 0.0045) * centerFocus * beamEnv
	sCool := s - 0.034 - bend
	causticCool := exp(-(sCool * sCool) / 0.011) * centerFocus * beamEnv
	sWarm := s + 0.032 + bend*0.85
	causticWarm := exp(-(sWarm * sWarm) / 0.011) * centerFocus * beamEnv

	pC := (uv - vec2(0.50, 0.46)) * vec2(1.15, 1.9)
	halo := exp(-dot(pC, pC) / 0.24) * settleEnv

	if Dark >= 0.5 {
		coolCol := mix(vec3(0.42, 0.80, 1.0), AccentRGB, 0.16)
		col += vec3(0.92, 0.96, 1.0) * beam * 0.12 * edgeFade
		col += vec3(1.0) * causticCore * 0.17 * edgeFade
		col += coolCol * causticCool * 0.14 * edgeFade
		col += vec3(1.0, 0.82, 0.56) * causticWarm * 0.09 * edgeFade
		col += vec3(0.88, 0.92, 0.98) * halo * 0.055 * edgeFade
	} else {
		col = mix(col, BaseRGB*vec3(0.952, 0.960, 0.976), 0.34*edgeFade*(1.0-clamp(beam*0.7, 0.0, 0.7)))
		col = mix(col, vec3(1.0), clamp((beam*0.65+causticCore*0.90+halo*0.30)*edgeFade, 0.0, 0.92))
		coolCol := mix(vec3(0.80, 0.92, 1.0), mix(vec3(1.0), AccentRGB, 0.30), 0.15)
		col = mix(col, coolCol, clamp(causticCool*0.56*edgeFade, 0.0, 0.44))
		col = mix(col, vec3(1.0, 0.93, 0.82), clamp(causticWarm*0.44*edgeFade, 0.0, 0.34))
	}

	dither := (fract(sin(dot(pos, vec2(12.9898, 78.233)))*43758.5453) - 0.5) * (1.0 / 255.0) * topFade
	col = clamp(col+vec3(dither), 0.0, 1.0)
	return vec4(col, 1.0)
}
`)

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

type sigGlyphPiece struct {
	contours [][]sigPoint
	px, py   float64
}

type sigGlyphSpec struct {
	start, dur     float64
	dx, dy         float64
	rotDeg, scale0 float64
	isDot          bool
}

var sigGlyphSpecs = [8]sigGlyphSpec{
	{start: 0.00, dur: 0.44, dx: -8, dy: 14, rotDeg: -4.2, scale0: 0.88},             // D
	{start: 0.10, dur: 0.36, dx: -6, dy: 12, rotDeg: -3.5, scale0: 0.85},             // o
	{start: 0.17, dur: 0.40, dx: -7, dy: 14, rotDeg: -3.8, scale0: 0.87},             // t
	{start: 0.25, dur: 0.34, dx: -5, dy: 11, rotDeg: -3.2, scale0: 0.85},             // i stem
	{start: 0.32, dur: 0.38, dx: -6, dy: 13, rotDeg: -3.5, scale0: 0.86},             // s
	{start: 0.39, dur: 0.38, dx: -6, dy: 14, rotDeg: -3.5, scale0: 0.87},             // l
	{start: 0.46, dur: 0.36, dx: -5, dy: 11, rotDeg: -3.2, scale0: 0.86},             // e
	{start: 0.55, dur: 0.30, dx: 3, dy: -16, rotDeg: 6.0, scale0: 0.28, isDot: true}, // i dot
}

func (a *App) resetOpeningLight() {
	a.openingCanvas = nil
	a.openingLightValid = false
}

func (a *App) paintOpeningLight(c *ui.Context, w, h float32, t float64, reduced bool) {
	if reduced || opticalReduced(c, &a.Palette) || w <= 0 || h <= 0 {
		return
	}
	if a.openingCanvas == nil {
		a.openingCanvas = ui.NewCanvas(openingLightW, openingLightH)
		a.openingLightValid = false
	}
	dark := a.Palette.Dark
	darkF := float32(0)
	if dark {
		darkF = 1
	}
	progress := float32(clampF64(t/(sigWrite+sigHold), 0, 1))
	baseCol := a.Palette.Scheme.Surface
	accentCol := a.Palette.Scheme.Primary
	needRedraw := !a.openingLightValid ||
		a.openingLightDark != dark ||
		a.openingLightBase != baseCol ||
		a.openingLightAccent != accentCol ||
		abs32(progress-a.openingLightProg) >= 0.015 ||
		(progress >= 1 && a.openingLightProg < 1) ||
		(progress <= 0 && a.openingLightProg > 0)

	baseRGB := rgbVec3(baseCol)
	accentRGB := rgbVec3(accentCol)

	ui.Box(c).Absolute().Left(0).Top(0).Width(w).Height(h).IgnoreInput(true).
		Draw(func(pc *ui.Painter, r ui.Rect) {
			if needRedraw {
				a.openingCanvas.Draw(pc, func(cp *ui.Painter) {
					openingLightShader.Draw(cp, ui.Rect{W: openingLightW, H: openingLightH}, shader.Options{
						Uniforms: map[string]any{
							"Dark":      darkF,
							"Progress":  progress,
							"BaseRGB":   baseRGB,
							"AccentRGB": accentRGB,
						},
					})
				})
				a.openingLightValid = true
				a.openingLightDark = dark
				a.openingLightBase = baseCol
				a.openingLightAccent = accentCol
				a.openingLightProg = progress
			}
			pc.Image(a.openingCanvas.Bitmap(), r, ui.Cover)
		})
}

func sigPrismGradientStops(center float64, dark bool, env float64) []ui.GradientStop {
	coolRGB, coolA := uint32(0x38BDF8), 0.20*env
	coreRGB, coreA := uint32(0xFFFFFF), 0.56*env
	warmRGB, warmA := uint32(0xF59E0B), 0.14*env
	if dark {
		coolRGB, coolA = 0x7DD3FC, 0.24*env
		coreRGB, coreA = 0xFFFFFF, 0.62*env
		warmRGB, warmA = 0xFDE68A, 0.18*env
	}
	type bandPt struct {
		at  float64
		rgb uint32
		a   float64
	}
	pts := []bandPt{
		{center - 0.17, coolRGB, 0},
		{center - 0.09, coolRGB, coolA},
		{center, coreRGB, coreA},
		{center + 0.09, warmRGB, warmA},
		{center + 0.17, warmRGB, 0},
	}
	sampleAt := func(x float64) ui.Color {
		if x <= pts[0].at || x >= pts[len(pts)-1].at {
			return ui.Transparent
		}
		for i := 0; i < len(pts)-1; i++ {
			a, b := pts[i], pts[i+1]
			if x >= a.at && x <= b.at {
				span := b.at - a.at
				f := 0.0
				if span > 1e-6 {
					f = (x - a.at) / span
				}
				rgb := mix(a.rgb, b.rgb, f)
				al := a.a + (b.a-a.a)*f
				return AlphaCol(rgb, al)
			}
		}
		return ui.Transparent
	}
	stops := []ui.GradientStop{{Color: sampleAt(0), At: 0}}
	for _, p := range pts {
		if p.at > 0.0005 && p.at < 0.9995 {
			stops = append(stops, ui.GradientStop{Color: AlphaCol(p.rgb, p.a), At: float32(p.at)})
		}
	}
	stops = append(stops, ui.GradientStop{Color: sampleAt(1), At: 1})
	return stops
}

func sigDrawStarGlint(pc *ui.Painter, cx, cy, arm, rotRad float32, c ui.Color) {
	waist := arm * 0.13
	cosR := float32(math.Cos(float64(rotRad)))
	sinR := float32(math.Sin(float64(rotRad)))
	rotPt := func(dx, dy float32) (float32, float32) {
		return cx + dx*cosR - dy*sinR, cy + dx*sinR + dy*cosR
	}
	var p ui.Path
	pts := [8][2]float32{
		{0, -arm}, {waist, -waist},
		{arm, 0}, {waist, waist},
		{0, arm}, {-waist, waist},
		{-arm, 0}, {-waist, -waist},
	}
	for i, pt := range pts {
		x, y := rotPt(pt[0], pt[1])
		if i == 0 {
			p.MoveTo(x, y)
		} else {
			p.LineTo(x, y)
		}
	}
	p.Close()
	pc.FillPath(&p, c)
}

var wordmarkOnce sync.Once
var wordmarkContours [][][]sigPoint
var wordmarkPieces []sigGlyphPiece

func sigPiecePivot(contours [][]sigPoint, baselineBias float64) (float64, float64) {
	first := true
	var minX, maxX, minY, maxY float64
	for _, pts := range contours {
		for _, pt := range pts {
			if first {
				minX, maxX, minY, maxY, first = pt.x, pt.x, pt.y, pt.y, false
				continue
			}
			minX, maxX = min(minX, pt.x), max(maxX, pt.x)
			minY, maxY = min(minY, pt.y), max(maxY, pt.y)
		}
	}
	if first {
		return 0, 0
	}
	return (minX + maxX) / 2, minY + baselineBias*(maxY-minY)
}

func sigEaseOutQuint(t float64) float64 {
	u := 1 - clampF64(t, 0, 1)
	return 1 - u*u*u*u*u
}

func sigEaseOutCubic(t float64) float64 {
	u := 1 - clampF64(t, 0, 1)
	return 1 - u*u*u
}

func sigDotSpring(t float64) float64 {
	u := clampF64(t, 0, 1)
	if u <= 0 {
		return 0
	}
	if u >= 1 {
		return 1
	}
	return 1 - math.Exp(-4.6*u)*math.Cos(1.5*math.Pi*u)
}

func wordmarkBuild() {
	wordmarkOnce.Do(func() {
		for _, outline := range signatureOutlines {
			wordmarkContours = append(wordmarkContours, sigParsePath(outline))
		}
		makePiece := func(contours [][]sigPoint, bias float64) sigGlyphPiece {
			px, py := sigPiecePivot(contours, bias)
			return sigGlyphPiece{contours: contours, px: px, py: py}
		}
		for i, c := range wordmarkContours {
			if i == 3 {
				stem := c
				if len(c) > 0 {
					stem = c[:1]
				}
				wordmarkPieces = append(wordmarkPieces, makePiece(stem, 0.62))
				continue
			}
			wordmarkPieces = append(wordmarkPieces, makePiece(c, 0.62))
		}
		if len(wordmarkContours) > 3 && len(wordmarkContours[3]) > 1 {
			wordmarkPieces = append(wordmarkPieces, makePiece(wordmarkContours[3][1:2], 0.50))
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
	_, alpha := sigInk(elapsed, reduced)
	if elapsed < 0 || alpha <= 0 {
		return
	}
	if reduced || a.Palette.HighContrast {
		drawWordmark(c, x, y, w, h, a.Palette.Scheme.OnSurface)
		return
	}
	wordmarkBuild()
	pad := float32(36)
	t := elapsed / sigSlow()
	inkRGB := a.Palette.Scheme.OnSurface
	dark := a.Palette.Dark

	ui.Box(c).Absolute().Left(x - pad).Top(y - pad).Width(w + pad*2).Height(h + pad*2).Label("Dotisle Rossela 字标").IgnoreInput(true).
		Draw(func(pc *ui.Painter, r ui.Rect) {
			baseScale := float64(min(w/signatureWidth, h/signatureHeight))
			cx0 := float64(r.X + r.W/2)
			cy0 := float64(r.Y + r.H/2)

			macroT := sigEaseOutCubic(clampF64(t/0.85, 0, 1))
			macroScale := 0.965 + 0.035*macroT
			macroY := 5.0 * (1 - macroT)
			if t > sigWrite+sigHold {
				exitP := float64(motionEase(float32(clampF64((t-sigWrite-sigHold)/sigExit, 0, 1))))
				macroScale *= 1 + 0.025*exitP
				macroY -= 8.0 * exitP
			}

			sheenActive := t > 0.42 && t < 1.08
			hasSheen := false
			var sheenPath ui.Path
			front := -0.18 + 1.48*sigEaseOutCubic(clampF64(t/0.94, 0, 1))

			for i, piece := range wordmarkPieces {
				if i >= len(sigGlyphSpecs) {
					break
				}
				spec := sigGlyphSpecs[i]
				u := clampF64((t-spec.start)/spec.dur, 0, 1)
				if u <= 0 {
					continue
				}
				var moveP, scP, fillA float64
				if spec.isDot {
					moveP = sigDotSpring(u)
					scP = moveP
					fillA = sigEaseOutCubic(clampF64(u/0.45, 0, 1))
				} else {
					moveP = sigEaseOutQuint(u)
					scP = moveP
					fillA = sigEaseOutCubic(clampF64((u-0.06)/0.56, 0, 1))
				}
				tx := spec.dx * (1 - moveP)
				ty := spec.dy * (1 - moveP)
				rad := spec.rotDeg * (1 - moveP) * math.Pi / 180
				cosR, sinR := math.Cos(rad), math.Sin(rad)
				sc := spec.scale0 + (1-spec.scale0)*scP
				warpDecay := max(0.0, 1.0-moveP)

				tr := func(pt sigPoint) (float32, float32) {
					lx := (pt.x - piece.px) * sc
					ly := (pt.y - piece.py) * sc
					rx := lx*cosR - ly*sinR + piece.px + tx
					ry := lx*sinR + ly*cosR + piece.py + ty
					proj := (rx + (ry-float64(signatureHeight)*0.5)*0.28) / float64(signatureWidth)
					d := (proj - front) / 0.22
					lensWarp := math.Exp(-d*d*1.8) * d * 7.2 * warpDecay
					rx += lensWarp * 0.95
					ry += lensWarp * -0.31
					gx := cx0 + (rx-float64(signatureWidth)/2)*baseScale*macroScale
					gy := cy0 + (ry-float64(signatureHeight)/2)*baseScale*macroScale + macroY*baseScale
					return float32(gx), float32(gy)
				}

				disp := 1.0 - sigEaseOutCubic(clampF64((u-0.04)/0.76, 0, 1))
				if disp > 0.01 && fillA > 0.01 {
					dispPx := float32((4.2 * disp) * baseScale)
					offX, offY := dispPx*0.94, dispPx*-0.34
					coolCol := AlphaCol(0x0284C7, 0.30*disp*fillA)
					warmCol := AlphaCol(0xEA580C, 0.24*disp*fillA)
					if dark {
						coolCol = AlphaCol(0x7DD3FC, 0.46*disp*fillA)
						warmCol = AlphaCol(0xFCD34D, 0.38*disp*fillA)
					}
					pc.FillPath(sigPathPoly(piece.contours, func(pt sigPoint) (float32, float32) {
						px, py := tr(pt)
						return px + offX, py + offY
					}), coolCol)
					pc.FillPath(sigPathPoly(piece.contours, func(pt sigPoint) (float32, float32) {
						px, py := tr(pt)
						return px - offX, py - offY
					}), warmCol)
				}

				path := sigPathPoly(piece.contours, tr)
				if fillA > 0.002 {
					pc.FillPath(path, AlphaCol(inkRGB, fillA))
				}
				if sheenActive && fillA > 0.35 {
					for _, pts := range piece.contours {
						for j, pt := range pts {
							px, py := tr(pt)
							if j == 0 {
								sheenPath.MoveTo(px, py)
							} else {
								sheenPath.LineTo(px, py)
							}
						}
						if len(pts) > 0 {
							sheenPath.Close()
							hasSheen = true
						}
					}
				}
			}

			if sheenActive && hasSheen {
				sheenU := clampF64((t-0.42)/0.66, 0, 1)
				env := sigEaseOutCubic(clampF64(sheenU/0.16, 0, 1)) * (1 - sigEaseOutCubic(clampF64((sheenU-0.78)/0.22, 0, 1)))
				if env > 0.004 {
					center := -0.22 + 1.44*sigEaseOutCubic(sheenU)
					pc.FillPathGradient(&sheenPath, ui.LinearGradient{
						Angle: 68,
						Stops: sigPrismGradientStops(center, dark, env),
					})
				}
			}

			if t > 0.66 && t < 0.96 && len(wordmarkPieces) > 7 {
				glintA := math.Sin(math.Pi * clampF64((t-0.66)/0.30, 0, 1))
				if glintA > 0.01 {
					dot := wordmarkPieces[7]
					dotX := float32(cx0 + (dot.px-float64(signatureWidth)/2)*baseScale*macroScale + 1.8*baseScale)
					dotY := float32(cy0 + (dot.py-float64(signatureHeight)/2)*baseScale*macroScale + (macroY-1.8)*baseScale)
					arm := float32((4.6 + 2.4*glintA) * baseScale)
					haloCol := AlphaCol(0x38BDF8, 0.22*glintA)
					coreCol := AlphaCol(0xFFFFFF, 0.72*glintA)
					if dark {
						haloCol = AlphaCol(0x7DD3FC, 0.24*glintA)
						coreCol = AlphaCol(0xFFFFFF, 0.82*glintA)
					}
					const glintRot = float32(-14.0 * math.Pi / 180.0)
					sigDrawStarGlint(pc, dotX, dotY, arm*1.3, glintRot, haloCol)
					sigDrawStarGlint(pc, dotX, dotY, arm, glintRot, coreCol)
				}
			}
		})
}
func DebugSignature() {
	wordmarkBuild()
	fmt.Printf("Rossela glyphs=%d bounds=%.1fx%.1f\n", len(wordmarkContours), signatureWidth, signatureHeight)
}
