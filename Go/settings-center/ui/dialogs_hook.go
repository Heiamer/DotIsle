// Central glue for the dialog entries and native pickers: helpers used by the page
// files plus the render hook that shows whichever dialog flag is set.
package ui

import (
	"fmt"
	"strings"
	"time"

	"github.com/egoist/mygo/ui"
)

// NUL-terminated double-NUL filter strings for GetOpenFileNameW.
const (
	filterImages    = "图片文件\x00*.png;*.jpg;*.jpeg;*.bmp\x00所有文件\x00*.*\x00\x00"
	filterModuleDLL = "模块 DLL\x00*.dll\x00所有文件\x00*.*\x00\x00"
)

// settingsColorHex reads a settings color key with a fallback.
func (a *App) settingsColorHex(key, def string) string {
	v, _ := a.settings()[key].(string)
	if v == "" {
		return "#" + def
	}
	return v
}

// hexValue normalizes a hex string into the picker's uint32 form.
func hexValue(hex string) uint32 {
	v, ok := parseHexColor(hex)
	if !ok {
		return 0
	}
	return v
}

// pickBackgroundImage opens the image dialog and runs the import flow.
func (a *App) pickBackgroundImage() {
	path := openFileDialog(a.Win.NativeHandle(), "选择背景图片", filterImages, "")
	if path == "" {
		return
	}
	if err := a.importBackground(path); err != nil {
		a.pushToast(fmt.Sprintf("背景导入失败：%v", err))
	}
}

// pickLocalFolder appends one folder to LocalFolders (Action::Paths).
func (a *App) pickLocalFolder() {
	folder := openFolderDialog(a.Win.NativeHandle(), "选择本地歌词文件夹")
	if folder == "" {
		return
	}
	for _, existing := range a.Prefs.LocalFolders {
		if strings.EqualFold(existing, folder) {
			return
		}
	}
	a.Prefs.LocalFolders = append(a.Prefs.LocalFolders, folder)
	a.savePrefs()
	a.pushNative("LocalFolders", a.Prefs.LocalFolders)
}

// pickModuleDLL stages a replacement module DLL (platform::choose_module flow).
func (a *App) pickModuleDLL(id string) {
	path := openFileDialog(a.Win.NativeHandle(), "选择替换 DLL（"+id+"）", filterModuleDLL, "")
	if path == "" {
		return
	}
	a.command("moduleStage", map[string]any{"id": id, "path": path})
}

// dialogViews renders whichever modal the page layer requested. Call from the root
// after the page content so dialogs paint on top.
func (a *App) dialogViews(c *ui.Context, w, h float32) {
	if a.gestureTutorial != nil && a.gestureTutorial.mandatory {
		a.gestureTutorialView(c, w, h)
		return
	}
	if a.wechatWarning != nil {
		a.wechatWarningView(c, w, h)
		return
	}
	a.considerInstalledNotes()
	if a.updateNotes != nil {
		a.updateNotesView(c, w, h)
		return
	}
	if a.AllegroAvailable && a.centerVisible && a.Connected && a.gestureTutorial == nil && !a.allegroSettings.OnboardingDone && !a.cityPromptAsked && !a.signaturePending && a.signatureStart.IsZero() && !a.allegroTutorialOpen {
		a.allegroTutorialOpen = true
		a.allegroTutorial = 0
	}
	if a.allegroTutorialOpen {
		a.allegroTutorialView(c, w, h)
		return
	}
	a.maybePromptCity()
	if a.cityPrompt != nil {
		a.cityPromptView(c)
		return
	}
	if a.picker != nil {
		a.colorPickerView(c, w, h)
	}
	if a.showRuleDialog {
		a.ruleDialog(c, w, h, a.ruleEditID)
	}
	if a.showQuickReminder {
		a.quickReminderDialog(c, w, h)
	}
	if a.widgetEditor != nil {
		a.widgetEditorView(c)
	}
	if a.overviewEditor != nil {
		a.overviewEditorView(c)
	}
	if a.todoDialog {
		a.todoDialogView(c, w, h)
	}
	if a.fontPicker != nil {
		a.fontPickerView(c, w, h)
	}
	a.gestureTutorialView(c, w, h)
}

// StartSignature marks the wordmark cover start (called once on window ready).
func (a *App) StartSignature() {
	a.LoadStartupSettings()
	a.signaturePending = a.signatureEnabled()
	a.Refresh()
}

// toastView draws the transient message in MyGo's overlay lane, with the
// palette's own surface. The default toast paints the theme's inverse colour,
// which showed up as a black block in the light control center after a save.
// The viewport is built even when the message just expired: MyGo keeps a
// toast for four seconds, and handing it back to the default look in that
// last second showed the black block again.
func (a *App) toastView(c *ui.Context, p *Palette) {
	if a.toast != "" {
		if time.Since(a.toastAt) > 3*time.Second {
			c.CloseToast(a.toast)
			a.toast = ""
		} else {
			c.Toast(a.toast)
		}
	}
	s := &p.Scheme
	ui.ToastViewportBase(c, func(viewport ui.Element, toasts []ui.Toast) {
		viewport.Bottom(26).AlignItems(ui.Center).Gap(8)
		for _, t := range toasts {
			parts := ui.ToastBase(c, t)
			box := parts.Root.Padding(10, 16).Radius(14).
				Background(Col(s.Container)).Border(1, Col(s.OutlineVariant)).
				Shadow(0, 6, 20, 0, ui.RGBA(0, 0, 0, 0.18))
			box.Children(func() {
				ui.Text(c, t.Title).Font(UIFont).FontSize(13).TextColor(Col(s.OnSurface))
			})
		}
	})
}

// pushToast records a transient message for the next frame.
func (a *App) pushToast(msg string) {
	a.toast = msg
	a.toastAt = time.Now()
	a.Refresh()
}
