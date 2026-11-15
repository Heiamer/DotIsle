package island

import (
	"testing"
	"time"
)

func TestSnapshotFilteringKeepsNavigationAndLyrics(t *testing.T) {
	before := map[string]any{"renderFrames": float64(1), "glassCaptures": float64(2), "title": "歌曲", "pendingRequestId": float64(1), "musicPreview": map[string]any{"lyricFrame": "第一句"}}
	next := map[string]any{"renderFrames": float64(9), "glassCaptures": float64(4), "title": "歌曲", "pendingRequestId": float64(1), "musicPreview": map[string]any{"lyricFrame": "第一句"}}
	if !sameSnapshot(before, next) {
		t.Fatal("renderer-only counters caused a redraw")
	}
	next["pendingRequestId"] = float64(2)
	if sameSnapshot(before, next) {
		t.Fatal("repeated destination request suppressed")
	}
	next["pendingRequestId"] = float64(1)
	next["musicPreview"] = map[string]any{"lyricFrame": "第二句"}
	if sameSnapshot(before, next) {
		t.Fatal("lyric update suppressed")
	}
	delete(next, "title")
	if sameSnapshot(before, next) {
		t.Fatal("removed field suppressed")
	}
}

func TestAdaptivePollingKeepsPlaybackAndTimerCadence(t *testing.T) {
	state := map[string]any{}
	if pollInterval(state) != 600*time.Millisecond {
		t.Fatal("idle cadence")
	}
	state["timers"] = []any{map[string]any{"running": true}}
	if pollInterval(state) != 150*time.Millisecond {
		t.Fatal("timer cadence")
	}
	state["isPlaying"] = true
	if pollInterval(state) != 90*time.Millisecond {
		t.Fatal("playback lost original cadence")
	}
}
