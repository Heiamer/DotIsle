package ui

import "testing"

func TestCanIdleExitPreservesTrayWork(t *testing.T) {
	for _, test := range []struct {
		name   string
		change func(*App)
	}{
		{"visible", func(a *App) { a.centerVisible = true }},
		{"disconnected", func(a *App) { a.Connected = false }},
		{"unknown widgets", func(a *App) { delete(a.State, "desktopWidgets") }},
		{"live widget", func(a *App) { a.State["desktopWidgets"] = []any{map[string]any{"id": "note"}} }},
		{"queued page", func(a *App) { a.State["pendingPage"] = "layout" }},
		{"pending save", func(a *App) { a.asyncRequests.Add(1) }},
		{"todo draft", func(a *App) { a.todoDialog = true }},
		{"update review", func(a *App) { a.updateNotes = &updateNotes{} }},
		{"reminder draft", func(a *App) { a.reminderFields[0] = leafEditField{seeded: true, text: "10", lastNative: "5"} }},
		{"layout draft", func(a *App) { a.musicEditor = &musicEditor{Baseline: layoutJSON(musicLayout{Enabled: true})} }},
	} {
		t.Run(test.name, func(t *testing.T) {
			a := &App{Connected: true, State: map[string]any{"desktopWidgets": []any{}}}
			if !a.CanIdleExit() {
				t.Fatal("idle center without work must be releasable")
			}
			test.change(a)
			if a.CanIdleExit() {
				t.Fatal("center can exit while work is still active")
			}
		})
	}
}
