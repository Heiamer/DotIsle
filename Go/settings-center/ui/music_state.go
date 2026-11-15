package ui

// The settings pipe exposes title/artist/position/duration at the top level.
// Lyric data lives in musicPreview.lyricFrame, not the backend's raw snapshot.
func (a *App) hasTrack() bool {
	return stateString(a.State, "", "title") != ""
}

func (a *App) lyricFrame() map[string]any {
	preview, _ := a.State["musicPreview"].(map[string]any)
	frame, _ := preview["lyricFrame"].(map[string]any)
	return frame
}
