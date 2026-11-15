package ui

import (
	"errors"
	"strings"
	"testing"
	"time"

	"github.com/egoist/mygo/ui"
)

func TestCommandFailuresAndModuleExitAcknowledgement(t *testing.T) {
	a := newTestApp(t)
	for _, tc := range []struct {
		reply   map[string]any
		err     error
		message string
	}{
		{map[string]any{"success": false, "message": "模块正在校验"}, nil, "模块正在校验"},
		{nil, errors.New("pipe timeout"), "pipe timeout"},
		{map[string]any{}, nil, "操作未成功"},
	} {
		if a.applyCommandReply("moduleApply", tc.reply, tc.err) {
			t.Fatal("must keep the center open after a failed apply")
		}
		if !strings.Contains(a.toast, tc.message) {
			t.Fatalf("failure was hidden: %q", a.toast)
		}
		if time.Since(a.toastAt) > time.Second {
			t.Fatal("toast must start when the reply arrives")
		}
	}
	if !a.applyCommandReply("moduleApply", map[string]any{"success": true}, nil) {
		t.Fatal("accepted module apply must release the center process")
	}
	if a.applyCommandReply("moduleStage", map[string]any{"success": true}, nil) {
		t.Fatal("staging must keep the center open")
	}
}

func TestRepeatedPendingPageAfterAcknowledgement(t *testing.T) {
	a := newTestApp(t)
	a.ApplyPacket(map[string]any{"pendingPage": "island"}, true, "")
	a.Page = PageTodos
	a.ApplyPacket(map[string]any{"pendingPage": nil}, true, "")
	a.ApplyPacket(map[string]any{"pendingPage": "island"}, true, "")
	if a.Page != PageSettings || a.SettingsTab != TabIslandFirst {
		t.Fatal("a repeated tray request must reopen its destination after pageSeen")
	}
}

func TestMusicUsesNativeSettingsReply(t *testing.T) {
	a := newTestApp(t)
	a.ApplyPacket(map[string]any{
		"title": "测试歌曲", "artist": "测试歌手", "songId": "42",
		"position": float64(60), "duration": float64(240), "isPlaying": false,
		"musicPreview": map[string]any{"lyricFrame": map[string]any{
			"currentLine": "这一句歌词", "currentTranslation": "译文",
		}},
	}, true, "")
	if a.trackTitle() != "测试歌曲" || a.trackArtist() != "测试歌手" {
		t.Fatal("song metadata must come from the actual settings protocol")
	}
	fraction, duration := a.mediaFraction()
	if fraction != 0.25 || duration != 240 {
		t.Fatalf("progress = %v / %v, want 0.25 / 240", fraction, duration)
	}
	a.Palette = BuildPalette(&a.Prefs, a.settings(), a.State)
	tt := ui.NewTester(func(c *ui.Context) { a.deckLyrics(c, &a.Palette, 0, 0, 600, 200) }, 600, 200)
	tt.Frame()
	if !tt.HasText("这一句歌词") || !tt.HasText("译文") {
		t.Fatal("the music card must render the native lyricFrame")
	}
}

func TestTodoModalRenderedOnce(t *testing.T) {
	a := newTestApp(t)
	a.Palette = BuildPalette(&a.Prefs, a.settings(), a.State)
	a.openTodoDialog("")
	t.Cleanup(func() { todoDialogDraft.initialized = false })
	tt := ui.NewTester(func(c *ui.Context) {
		root := ui.Box(c).Fill()
		root.Children(func() {
			a.todosPage(c, 0, 0, 900)
			a.dialogViews(c, 900, 720)
		})
	}, 900, 720)
	tt.Frame()
	count := 0
	for _, text := range tt.Texts() {
		if text == "记下一件事" {
			count++
		}
	}
	if count != 1 {
		t.Fatalf("todo modal rendered %d times, want 1", count)
	}
}

func TestPlayingProgressDoesNotAdvancePastDuration(t *testing.T) {
	a := newTestApp(t)
	a.ApplyPacket(map[string]any{"position": float64(240), "duration": float64(240), "isPlaying": true}, true, "")
	a.packetAt = time.Now().Add(-time.Second)
	f, _ := a.mediaFraction()
	if f != 1 {
		t.Fatalf("completed playback fraction = %v", f)
	}
}
