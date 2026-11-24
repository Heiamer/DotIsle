package ui

import (
	"bufio"
	"io"
	"os"
	"path/filepath"
	"strings"

	"dotisle/settings/prefs"
	"github.com/egoist/mygo/ui"
)

type updateNotes struct {
	version, channel, source string
	paragraphs               []string
	scroll                   ui.ScrollState
}

func normalVersion(version string) string { return strings.TrimPrefix(strings.TrimSpace(version), "v") }

func (a *App) releaseVersion() string {
	return normalVersion(stateString(a.State, stateString(a.State, "", "version"), "bundleVersion"))
}

// Notes are plain text in the native dialog; never execute remote markup or links.
func notesParagraphs(notes string) []string {
	var paragraphs []string
	for _, line := range strings.Split(notes, "\n") {
		line = strings.TrimSpace(line)
		line = strings.TrimLeft(line, "# ")
		line = strings.ReplaceAll(line, "**", "")
		line = strings.ReplaceAll(line, "`", "")
		if strings.HasPrefix(line, "- ") || strings.HasPrefix(line, "* ") {
			line = "• " + line[2:]
		}
		if line != "" {
			paragraphs = append(paragraphs, line)
		}
	}
	return paragraphs
}

func bundledNotes(dir, version string) []string {
	for _, name := range []string{"ReleaseNotes.md", "发布说明.md", "CHANGELOG.md"} {
		file, err := os.Open(filepath.Join(dir, name))
		if err != nil {
			continue
		}
		scanner := bufio.NewScanner(io.LimitReader(file, 256*1024))
		var section strings.Builder
		matched := false
		for scanner.Scan() {
			line := strings.TrimSpace(strings.TrimPrefix(scanner.Text(), "\ufeff"))
			heading := strings.TrimLeft(line, "#")
			if level := len(line) - len(heading); level == 1 || level == 2 {
				if matched {
					break
				}
				heading = strings.TrimSpace(strings.TrimPrefix(strings.TrimSpace(heading), "点屿 DotIsle "))
				first, _, _ := strings.Cut(heading, " ")
				matched = normalVersion(first) == version
				continue
			}
			if matched {
				section.WriteString(line + "\n")
				if section.Len() > 24*1024 {
					break
				}
			}
		}
		file.Close()
		if notes := notesParagraphs(section.String()); len(notes) != 0 {
			return notes
		}
	}
	return nil
}

func (a *App) notesReceiptPath() string { return filepath.Join(a.Data, "center-release-notes.json") }

func (a *App) considerInstalledNotes() {
	if !a.centerVisible || !a.Connected || a.updateNotes != nil || a.releaseVersion() == "" ||
		a.updateReviewLoading || a.signaturePending || !a.signatureStart.IsZero() || a.paletteOpen || a.navigationOpen || a.overview.manage ||
		a.cityPrompt != nil || a.picker != nil || a.showRuleDialog || a.showQuickReminder || a.widgetEditor != nil || a.overviewEditor != nil || a.todoDialog || a.fontPicker != nil || a.gestureTutorial != nil {
		return
	}
	version := a.releaseVersion()
	if version == a.notesCheckedVersion || a.Data == "" {
		return
	}
	a.notesCheckedVersion = version
	if prefs.ReadReleaseNotesReceipt(a.notesReceiptPath()).Version != version {
		a.openInstalledNotes()
	}
}

func (a *App) openInstalledNotes() {
	if version := a.releaseVersion(); version != "" {
		exe, _ := os.Executable()
		a.updateNotes = &updateNotes{version: version, paragraphs: bundledNotes(filepath.Dir(exe), version)}
		a.Refresh()
	}
}

func (a *App) openUpdateReview() {
	if a.updateReviewLoading || a.updateDownloading || a.Win == nil {
		return
	}
	a.updateReviewLoading = true
	a.command("updateReview", nil)
}

func (a *App) dismissUpdateNotes() {
	if a.updateNotes == nil || a.updateDownloading {
		return
	}
	notes := a.updateNotes
	a.updateNotes = nil
	if notes.channel == "" && a.Data != "" {
		if err := prefs.SaveReleaseNotesReceipt(a.notesReceiptPath(), notes.version); err != nil {
			a.pushToast("更新内容已关闭，无法保存已读记录。")
		}
	}
	a.Refresh()
}

func (a *App) confirmUpdateDownload() {
	notes := a.updateNotes
	if notes == nil || notes.channel == "" || a.updateDownloading || !a.Connected || a.Win == nil {
		return
	}
	a.updateDownloading = true
	a.command("updateDownload", map[string]any{"version": notes.version, "channel": notes.channel})
}

func (a *App) updateNotesView(c *ui.Context, w, h float32) {
	notes := a.updateNotes
	p := &a.Palette
	s := &p.Scheme
	dw, dh := min(w-48, float32(580)), min(h-56, float32(482))
	open := true
	title, meta := "本次更新", "版本 "+normalVersion(notes.version)
	if notes.channel != "" {
		title = "发现新版本"
		if notes.channel == "beta" {
			meta += "  ·  测试渠道"
		} else {
			meta += "  ·  正式渠道"
		}
		if notes.source != "" {
			meta += "  ·  " + notes.source
		}
	}
	ui.DialogBase(c, &open, func(backdrop, panel ui.Element) {
		backdrop.Background(AlphaCol(0, .36))
		panel.Width(dw).Height(dh).Label(title)
		overlayStyle(&panel, p)
		panel.Children(func() {
			Label(c, title, 28, 24, 22, s.OnSurface, true, dw-100)
			Label(c, meta, 28, 63, 13, s.OnSurfaceVariant, false, dw-56)
			IconButton(c, p, "关闭更新内容", dw-66, 18, 40, 40, "close", false, !a.updateDownloading, func() { open = false })
			ui.Scroll(c).Scrollbars(false).Key("release-notes-body").Absolute().Left(28).Top(103).Width(dw - 56).Height(dh - 185).
				TrackScroll(&notes.scroll).Label("更新内容").Children(func() {
				paragraphs := notes.paragraphs
				if len(paragraphs) == 0 {
					paragraphs = []string{"这个版本没有提供更新说明。"}
				}
				ui.Box(c).Width(dw - 72).Gap(10).Children(func() {
					for _, text := range paragraphs {
						ui.Text(c, text).Width(dw - 72).Font(textFamily(text)).FontSize(14).FixedLineHeight(23).TextColor(Col(s.OnSurface))
					}
				})
			})
			if notes.channel == "" {
				Button(c, p, "已读更新内容", dw-136, dh-62, 108, 40, "知道了", "", false, true, func() { open = false })
				return
			}
			Button(c, p, "稍后更新", dw-270, dh-62, 102, 40, "稍后", "", true, !a.updateDownloading, func() { open = false })
			label := "确认下载"
			if a.updateDownloading {
				label = "正在启动…"
			}
			Button(c, p, "确认下载更新", dw-156, dh-62, 128, 40, label, "", false, !a.updateDownloading && a.Connected, a.confirmUpdateDownload)
		})
	})
	if !open {
		a.dismissUpdateNotes()
	}
}
