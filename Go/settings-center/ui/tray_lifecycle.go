package ui

import "slices"

func (a *App) DetachCenterWindow() {
	a.Win = nil
	a.contentMounted = false
	a.backdropWindow, a.backdropChecked = 0, false
}

// CanIdleExit preserves widget ownership and edits not yet stored by the island.
func (a *App) CanIdleExit() bool {
	widgets, known := a.State["desktopWidgets"].([]any)
	if a.mail.configured || a.centerVisible || !a.Connected || !known || len(widgets) != 0 || a.asyncRequests.Load() != 0 ||
		stateString(a.State, "", "pendingPage") != "" {
		return false
	}
	if a.todoDialog || a.showQuickReminder || a.showRuleDialog || a.picker != nil || a.fontPicker != nil ||
		a.widgetEditor != nil || a.overviewEditor != nil || a.updateNotes != nil ||
		a.updateReviewLoading || a.updateDownloading || a.overview.edit || focusDialog.kind != fdNone ||
		(a.cityPrompt != nil && a.cityPrompt.text != "") {
		return false
	}
	if draft := a.curveDraft; draft != nil && draft.dragging >= 0 {
		return false
	}
	if e := a.musicEditor; e != nil && (e.Pending || layoutJSON(e.Draft) != e.Baseline) {
		return false
	}
	for _, f := range leafEditFields {
		if f.seeded && f.text != f.lastNative {
			return false
		}
	}
	for _, f := range a.reminderFields {
		if f.seeded && f.text != f.lastNative {
			return false
		}
	}
	return focusUI.minutes == 25 && focusUI.rest == 300 && focusUI.rounds == 4 && focusUI.seconds == 0 &&
		workflowUI.templateID == "" && slices.Equal(workflowUI.stages, defaultWorkflowStages())
}
