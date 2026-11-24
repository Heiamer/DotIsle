package ui

import (
	"github.com/egoist/mygo/ui"
	"github.com/egoist/mygo/ui/shader"
)

const (
	lightFieldCanvasW = 200
	lightFieldCanvasH = 134
)

// lightFieldShader 渲染“静光与光学玻璃”方向性自然建筑光场：
// 浅色态模拟左上 32° 展馆自然窗光与极克制空气漫射；深色态呈现纯粹碳黑石墨与顶光微芒。
var lightFieldShader = shader.MustCompile(`package main

var Dark float
var Phase float
var BaseTL vec3
var BaseBR vec3
var KeyLight vec3
var AirTint vec3
var WarmBounce vec3
var Intensity float

func Fragment(dst vec4, pos vec2) vec4 {
	size := rectSize()
	uv := pos / max(size, vec2(1.0, 1.0))

	// 1. 顺光方向对角基底渐变（左上受光，右下沉稳）
	diag := clamp(uv.x*0.42+uv.y*0.58, 0.0, 1.0)
	col := mix(BaseTL, BaseBR, diag)

	// 2. 左上 32° 主自然窗光柔和漫射团 + 克制斜向流动建筑天光波带（为上层光学玻璃提供通透折射基底）
	pSun := (uv - vec2(0.14+0.035*sin(Phase*0.45), 0.06+0.025*cos(Phase*0.38))) * vec2(0.85, 1.15)
	sunWash := exp(-dot(pSun, pSun) / 0.34)

	// 顶部偏右一缕极轻薄的天光空气透视
	pSky := (uv - vec2(0.68+0.04*cos(Phase*0.32), 0.10+0.025*sin(Phase*0.41))) * vec2(0.95, 1.35)
	skyWash := exp(-dot(pSky, pSky) / 0.28)

	// 右下极远端微弱石膏/环境反射光，平衡构图重心
	pBounce := (uv - vec2(0.86-0.03*sin(Phase*0.29), 0.88)) * vec2(1.0, 1.2)
	bounceWash := exp(-dot(pBounce, pBounce) / 0.24)

	// 柔和斜向建筑天光波带，在呼吸中缓慢漫游，让玻璃透射产生自然光学纵深
	beamDistA := (uv.x*0.72 - uv.y*0.58 + 0.12 + 0.05*sin(Phase*0.38)) / 0.26
	beamA := exp(-(beamDistA * beamDistA))
	beamDistB := (uv.x*0.55 + uv.y*0.78 - 0.92 + 0.04*cos(Phase*0.31)) / 0.32
	beamB := exp(-(beamDistB * beamDistB))

	if Dark >= 0.5 {
		// 深色暗室：纯正碳黑底板上仅叠加极克制银灰顶光漫射，绝不发藏青或蓝紫
		col += KeyLight * (sunWash*0.24 + beamA*0.12) * Intensity
		col += AirTint * (skyWash*0.14 + beamB*0.06) * Intensity
		col += WarmBounce * bounceWash * (0.08 * Intensity)
	} else {
		// 浅色展馆：纯净无彩色冷雾石膏底面上落下一缕自然窗光，清晰托出透明光学玻璃的质感
		col = mix(col, KeyLight, clamp((sunWash*0.54+beamA*0.30)*Intensity, 0.0, 0.68))
		col = mix(col, AirTint, clamp((skyWash*0.26+beamB*0.16)*Intensity, 0.0, 0.34))
		col = mix(col, WarmBounce, clamp(bounceWash*0.18*Intensity, 0.0, 0.22))
		// 底部与右下极克制沉稳阴影压暗（约 6% 向 BaseBR 过渡），塑造建筑空间景深
		depthMask := smoothstep(0.35, 1.0, uv.y*0.65+uv.x*0.35)
		col = mix(col, BaseBR, depthMask*0.10)
	}

	// 4. 极微弱有序抖动消除 8-bit 梯度色带
	dither := (fract(sin(dot(pos, vec2(12.9898, 78.233)))*43758.5453) - 0.5) * (1.0 / 255.0)
	col = clamp(col+vec3(dither), 0.0, 1.0)
	return vec4(col, 1.0)
}
`)

type lightFieldState struct {
	canvas   *ui.Canvas
	phase    float64
	hasLast  bool
	lastDark bool
	lastSeed uint32
}

func rgbVec3(rgb uint32) []float32 {
	return []float32{
		float32((rgb>>16)&255) / 255.0,
		float32((rgb>>8)&255) / 255.0,
		float32(rgb&255) / 255.0,
	}
}

func mixRGB(a, b uint32, t float64) uint32 {
	return mix(a, b, clamp01(t))
}

// resolveLightFieldPalette 解析克制极简的建筑自然光场色板，音乐封面仅注入极雅致的微弱环境色温。
func (a *App) resolveLightFieldPalette() (baseTL, baseBR, keyLight, airTint, warmBounce uint32, intensity float32, seed uint32) {
	dark := a.Palette.Dark
	if dark {
		baseTL, baseBR = 0x161922, 0x090A0E
		keyLight, airTint, warmBounce = 0x303644, 0x242A36, 0x21222A
		intensity = 0.78
	} else {
		// 白昼展馆：极简微凉晨雾石膏基底，沉稳半阶使通透光学玻璃产生清晰立体反差
		baseTL, baseBR = 0xE5E9F1, 0xCFD7E4
		keyLight, airTint, warmBounce = 0xFFFFFF, 0xEDF2F8, 0xF0ECE6
		intensity = 0.92
	}

	var accentSeed uint32
	if a.leafBool("ControlCenterDynamicColorEnabled", false) {
		accentSeed = centerColorSeed(a.settings(), a.State, dark)
	} else if a.coverBitmap() != nil && coverCache.seed != 0 {
		accentSeed = coverCache.seed
	} else if v, ok := stateFloat(a.State, "artworkSeed"); ok && v > 0 && v <= 0xffffffff {
		accentSeed = uint32(v) & 0xffffff
	}
	if accentSeed != 0 {
		h := colorHue(accentSeed)
		if dark {
			airTint = mixRGB(airTint, hueColor(h, 0.36, 0.24), 0.24)
		} else {
			airTint = mixRGB(airTint, hueColor(h, 0.32, 0.90), 0.22)
		}
		seed = accentSeed
	}
	return
}

// paintLightField 在底层绘制自然建筑方向光场，确保无头测试静态稳定且尊重无障碍降级开关。
func (a *App) paintLightField(c *ui.Context, w, h float32) {
	if opticalReduced(c, &a.Palette) || w <= 0 || h <= 0 {
		return
	}

	st := &a.lightField
	if st.canvas == nil {
		st.canvas = ui.NewCanvas(lightFieldCanvasW, lightFieldCanvasH)
	}

	// The small retained canvas only changes on palette updates, not on mouse movement.
	st.phase = 0.85
	baseTL, baseBR, keyLight, airTint, warmBounce, intensity, seed := a.resolveLightFieldPalette()
	dark := a.Palette.Dark

	needRedraw := !st.hasLast || st.lastDark != dark || st.lastSeed != seed

	darkF := float32(0)
	if dark {
		darkF = 1
	}
	opacity := float32(1)
	if a.materialActive(c) {
		if dark {
			opacity = 0.86
		} else {
			opacity = 0.84
		}
	}

	box := ui.Box(c).Key("lightfield-bg").Absolute().Left(0).Top(0).Width(w).Height(h).
		Opacity(opacity).IgnoreInput(true)
	box.Draw(func(pc *ui.Painter, r ui.Rect) {
		if needRedraw {
			st.canvas.Draw(pc, func(cp *ui.Painter) {
				lightFieldShader.Draw(cp, ui.Rect{W: lightFieldCanvasW, H: lightFieldCanvasH}, shader.Options{
					Uniforms: map[string]any{
						"Dark":       darkF,
						"Phase":      float32(st.phase),
						"BaseTL":     rgbVec3(baseTL),
						"BaseBR":     rgbVec3(baseBR),
						"KeyLight":   rgbVec3(keyLight),
						"AirTint":    rgbVec3(airTint),
						"WarmBounce": rgbVec3(warmBounce),
						"Intensity":  intensity,
					},
				})
			})
			st.hasLast = true
			st.lastDark = dark
			st.lastSeed = seed
		}
		pc.Image(st.canvas.Bitmap(), r, ui.Cover)
	})
}

func clamp01F(v float32) float32 {
	if v < 0 {
		return 0
	}
	if v > 1 {
		return 1
	}
	return v
}

func abs32(v float32) float32 {
	if v < 0 {
		return -v
	}
	return v
}
