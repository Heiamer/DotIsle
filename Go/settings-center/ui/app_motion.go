package ui

import (
	"math"
	"time"

	"github.com/egoist/mygo/ui"
)

// Motion belongs to DotIsle's Animation preference, as in 3.5.1. Do not route
// spatial values through MyGo's system-controlled animation channels.
type appSpring struct {
	value, target, velocity float64
	at                      time.Time
}

func (s *appSpring) jump(value float64, now time.Time) {
	s.value, s.target, s.velocity, s.at = value, value, 0, now
}

func (s *appSpring) settled() bool {
	return math.Abs(s.value-s.target) < .002 && math.Abs(s.velocity) < .02
}

// Closed-form integration and deck constants are from 3.5.1 shared_motion.rs.
// Sampling twice at the same frame time never advances the spring twice.
func (s *appSpring) sample(now time.Time, target, stiffness, damping float64, reduced bool) float32 {
	dt := now.Sub(s.at).Seconds()
	s.at = now
	if reduced {
		s.jump(target, now)
		return float32(target)
	}
	if s.settled() {
		s.value, s.velocity = s.target, 0
	} else if dt > 0 {
		dt = math.Min(dt, 10)
		omega := math.Sqrt(stiffness)
		x, v := s.value-s.target, s.velocity
		e := math.Exp(-damping * omega * dt)
		if damping >= 1 {
			b := v + omega*x
			s.value = s.target + e*(x+b*dt)
			s.velocity = e * (v - omega*b*dt)
		} else {
			wd := omega * math.Sqrt(1-damping*damping)
			b := (v + damping*omega*x) / wd
			c, sn := math.Cos(wd*dt), math.Sin(wd*dt)
			s.value = s.target + e*(x*c+b*sn)
			s.velocity = e * (-damping*omega*(x*c+b*sn) + wd*(-x*sn+b*c))
		}
	}
	s.target = target
	if s.settled() {
		s.jump(target, now)
	}
	return float32(s.value)
}

func appSpringValue(c *ui.Context, e ui.Element, key string, target float32, duration time.Duration, reduced bool) float32 {
	s := ui.Local(e, key, func() appSpring {
		return appSpring{value: float64(target), target: float64(target), at: c.Now()}
	})
	omega := 2 * math.Pi / duration.Seconds()
	value := s.sample(c.Now(), float64(target), omega*omega, 1, reduced)
	if !s.settled() {
		c.AnimationFrame()
	}
	return value
}

type appTween struct {
	value, from, target float32
	start               time.Time
	duration            time.Duration
}

func appTweenValue(c *ui.Context, e ui.Element, key string, initial, target float32, duration time.Duration, reduced bool) float32 {
	s := ui.Local(e, key, func() appTween {
		return appTween{value: initial, from: initial, target: initial, start: c.Now(), duration: duration}
	})
	if reduced {
		s.value, s.from, s.target = target, target, target
		return target
	}
	t := clampF64(float64(c.Now().Sub(s.start))/float64(s.duration), 0, 1)
	s.value = s.from + (s.target-s.from)*motionEase(float32(t))
	if target != s.target {
		s.from, s.target, s.start, s.duration = s.value, target, c.Now(), duration
	}
	if s.value != s.target {
		c.AnimationFrame()
	}
	return s.value
}

func (a *App) reducedMotion() bool {
	return !a.leafBool("Animation", !a.Prefs.ReducedMotion)
}
