// Shared App helpers for the page bodies: snapshot accessors, timer presentation
// (port of the timers.rs display functions) and todo list views.
package ui

import (
	"math"
	"time"
)

// clipH is the content viewport height, set by the root view each frame so the
// deck can size its cards like the Vello clip did.
var clipH float64 = 560

// deckPos is the ring coordinate (card_ring.rs); deckTarget and deckVel drive the
// spring toward it; deckDragY tracks the pressed pointer.
var (
	deckPos        float64
	deckTarget     float64
	deckVel        float64
	deckSpringIdle = true
	deckDragY      float64
	deckLastInput  time.Time
)

// deckMinutes is the focus duration chosen on the focus card (model.rs minutes).
var deckMinutes = 25

// currentTimer returns the selected or first timer snapshot, like Model::current_timer.
func (a *App) currentTimer() map[string]any {
	if a.State == nil {
		return nil
	}
	timers, _ := a.State["timers"].([]any)
	if len(timers) == 0 {
		return nil
	}
	first, _ := timers[0].(map[string]any)
	return first
}

// timerClock formats the remaining time as mm:ss (or hh:mm:ss past an hour).
func timerClock(timer map[string]any) string {
	remainingMs, ok := stateFloat(timer, "remainingMs")
	if !ok {
		if v, ok := stateFloat(timer, "remaining"); ok {
			remainingMs = v * 1000
		} else {
			return ""
		}
	}
	total := int(math.Floor(remainingMs / 1000))
	h, m, s := total/3600, (total/60)%60, total%60
	if h > 0 {
		return twoDigits(h) + ":" + twoDigits(m) + ":" + twoDigits(s)
	}
	return twoDigits(m) + ":" + twoDigits(s)
}

// timerProgress is the elapsed share of the current stage.
func timerProgress(timer map[string]any) float64 {
	elapsed, eok := stateFloat(timer, "elapsedMs")
	total, tok := stateFloat(timer, "stageMs")
	if !eok || !tok || total <= 0 {
		return 0
	}
	return clampF64(elapsed/total, 0, 1)
}

// timerPrimaryAction ports timers.rs primary_action: pause while running, resume
// while paused, next at a stage boundary, restart when finished.
func timerPrimaryAction(timer map[string]any) string {
	if finished := stateBool(timer, "finished"); finished {
		return "restart"
	}
	if stateBool(timer, "stageEnded") {
		return "next"
	}
	if stateBool(timer, "running") {
		return "pause"
	}
	return "resume"
}

func timerSubtitle(timer map[string]any) string {
	if s := stateString(timer, "", "subtitle"); s != "" {
		return s
	}
	return "按自己的节奏，专心做完这一段。"
}

func timerStatusLine(timer map[string]any) string {
	switch {
	case stateBool(timer, "finished"):
		return "这一段，已经完成。"
	case stateBool(timer, "running"):
		return "正在专注，按自己的节奏来。"
	default:
		return "已经暂停，准备好时再继续。"
	}
}

// quickFocusValue builds the timerCreate payload for the focus card (model.rs:
// mode pomodoro, duration minutes*60, default rest and rounds).
func (a *App) quickFocusValue() map[string]any {
	seconds := deckMinutes * 60
	return map[string]any{
		"mode":     "pomodoro",
		"duration": seconds,
		"focus":    seconds,
		"rest":     300,
		"rounds":   1,
	}
}

// sortTodos orders by date then time, both as ISO-like strings.
func sortTodos(items []map[string]any) {
	for i := 1; i < len(items); i++ {
		for j := i; j > 0; j-- {
			a, _ := items[j-1]["date"].(string)
			b, _ := items[j]["date"].(string)
			if a > b {
				items[j-1], items[j] = items[j], items[j-1]
				continue
			}
			ta, _ := items[j-1]["time"].(string)
			tb, _ := items[j]["time"].(string)
			if a == b && ta > tb {
				items[j-1], items[j] = items[j], items[j-1]
			}
		}
	}
}

func (a *App) widgetCount() int {
	if a.State == nil {
		return 0
	}
	list, _ := a.State["desktopWidgets"].([]any)
	return len(list)
}

// homeWidgetKind maps the six deck entries to Kind variant names.
func homeWidgetKind(index int) string {
	kinds := [6]string{"Clock", "Calendar", "Todos", "Focus", "Music", "System"}
	return kinds[index]
}

// homeWidgetDefaultSize mirrors Kind::default_size (clock/calendar small).
func homeWidgetDefaultSize(index int) string {
	if index <= 1 {
		return "small"
	}
	return "medium"
}

func twoDigits(v int) string {
	return string([]byte{byte('0' + (v/10)%10), byte('0' + v%10)})
}

func math8Round(v float64) float64 { return math.Round(v) }

func clampF32(v, lo, hi float32) float32 {
	if v < lo {
		return lo
	}
	if v > hi {
		return hi
	}
	return v
}

func (a *App) pendingTodos() int { return len(a.todoItems()) - a.doneTodoCount() }

// doneTodoCount counts todos already marked done (deck header).
func (a *App) doneTodoCount() int {
	n := 0
	now, _ := a.todoNow()
	for _, t := range a.todoItems() {
		if todoDone(t, now) {
			n++
		}
	}
	return n
}

// packetFreshness reports seconds since the last island snapshot arrived.
func (a *App) packetFreshness() time.Duration { return time.Since(a.packetAt) }
