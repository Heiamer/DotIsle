package main

import (
	"encoding/json"
	"os"
	"time"
)

// Opt-in timings contain no settings, paths, media or notification contents.
var startupClock = time.Now()
var startupTracePath = os.Getenv("DOTISLE_STARTUP_TRACE")
var startupTimes = map[string]float64{}

func startupMark(phase string) {
	if startupTracePath == "" {
		return
	}
	startupTimes[phase] = float64(time.Since(startupClock).Microseconds()) / 1000
	if raw, err := json.Marshal(startupTimes); err == nil {
		_ = os.WriteFile(startupTracePath, raw, 0600)
	}
}
