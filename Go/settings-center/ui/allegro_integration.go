package ui

import (
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"sync"
	"syscall"
	"time"
	"unsafe"

	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
	"golang.org/x/sys/windows"
)

type allegroSettings struct {
	Enabled         bool   `json:"enabled"`
	Edge            string `json:"edge"`
	CustomModifiers uint32 `json:"custom_modifiers"`
	CustomKey       uint32 `json:"custom_key"`
	EditModifiers   uint32 `json:"edit_modifiers"`
	EditKey         uint32 `json:"edit_key"`
	Hotkey          string `json:"hotkey"`
	ShowHotkeyHint  bool   `json:"show_hotkey_hint"`
	TutorialVersion int    `json:"tutorial_version"`
	OnboardingDone  bool   `json:"onboarding_done"`
}
type allegroCard struct {
	ID      uint64   `json:"id"`
	Title   string   `json:"title"`
	Kind    string   `json:"kind"`
	Size    string   `json:"size"`
	Acrylic int      `json:"acrylic"`
	Stack   []string `json:"stack"`
	Page    int      `json:"page"`
	X       float64  `json:"x"`
	Y       float64  `json:"y"`
	Note    string   `json:"note_text"`
}
type allegroBoard struct {
	Cards []allegroCard `json:"cards"`
}
type allegroMutation struct {
	file   string
	change func([]byte) ([]byte, error)
}
type allegroController struct {
	previews chan string
	live     chan []byte
	queue    chan allegroMutation
	stop     chan struct{}
	done     chan struct{}
	once     sync.Once
}

func defaultAllegroSettings() allegroSettings {
	return allegroSettings{Enabled: true, Edge: "right", Hotkey: "alt_grave", ShowHotkeyHint: true}
}
func atomicAllegro(path string, value []byte) error {
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		return err
	}
	tmp := fmt.Sprintf("%s.center-%d.tmp", path, time.Now().UnixNano())
	if err := os.WriteFile(tmp, value, 0600); err != nil {
		return err
	}
	defer os.Remove(tmp)
	from, _ := windows.UTF16PtrFromString(tmp)
	to, _ := windows.UTF16PtrFromString(path)
	return windows.MoveFileEx(from, to, windows.MOVEFILE_REPLACE_EXISTING|windows.MOVEFILE_WRITE_THROUGH)
}
func (a *App) StartAllegro() {
	exe, err := os.Executable()
	if err != nil {
		return
	}
	engine := filepath.Join(filepath.Dir(exe), "DotIsle-Allegro.exe")
	if _, err = os.Stat(engine); err != nil {
		return
	}
	a.AllegroAvailable = true
	a.allegroSettings = defaultAllegroSettings()
	if raw, err := os.ReadFile(filepath.Join(a.Data, "allegro-settings.json")); err == nil {
		_ = json.Unmarshal(raw, &a.allegroSettings)
	}
	ctl := &allegroController{previews: make(chan string, 64), live: make(chan []byte, 1), queue: make(chan allegroMutation, 32), stop: make(chan struct{}), done: make(chan struct{})}
	a.allegro = ctl
	go func() {
		defer close(ctl.done)
		cmd := exec.Command(engine, "--integrated", "--keep-alive", "--data-dir", a.Data)
		cmd.Dir = filepath.Dir(engine)
		cmd.SysProcAttr = &syscall.SysProcAttr{CreationFlags: 0x08000000}
		logFile, _ := os.OpenFile(filepath.Join(a.Data, "allegro-host.log"), os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0600)
		if logFile != nil {
			defer logFile.Close()
			cmd.Stdout = logFile
			cmd.Stderr = logFile
		}
		for _, name := range []string{"allegro-desktop.json", "allegro-sidebar.json"} {
			path := filepath.Join(a.Data, name)
			if _, err := os.Stat(path); os.IsNotExist(err) {
				_ = atomicAllegro(path, []byte(`{"cards":[]}`))
			}
		}
		if _, err := os.Stat(filepath.Join(a.Data, "allegro-settings.json")); os.IsNotExist(err) {
			raw, _ := json.Marshal(defaultAllegroSettings())
			_ = atomicAllegro(filepath.Join(a.Data, "allegro-settings.json"), raw)
		}
		if err := cmd.Start(); err != nil {
			mygo.RunOnMain(func() { a.allegroError = "侧边栏启动失败：" + err.Error(); a.Refresh() })
			return
		}
		job, jobErr := windows.CreateJobObject(nil, nil)
		if jobErr == nil {
			info := windows.JOBOBJECT_EXTENDED_LIMIT_INFORMATION{}
			info.BasicLimitInformation.LimitFlags = windows.JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
			_, jobErr = windows.SetInformationJobObject(job, windows.JobObjectExtendedLimitInformation, uintptr(unsafe.Pointer(&info)), uint32(unsafe.Sizeof(info)))
			if jobErr == nil {
				process, err := windows.OpenProcess(windows.PROCESS_SET_QUOTA|windows.PROCESS_TERMINATE, false, uint32(cmd.Process.Pid))
				if err == nil {
					jobErr = windows.AssignProcessToJobObject(job, process)
					windows.CloseHandle(process)
				} else {
					jobErr = err
				}
			}
			defer windows.CloseHandle(job)
		}
		if jobErr != nil {
			cmd.Process.Kill()
			cmd.Wait()
			mygo.RunOnMain(func() { a.allegroError = "侧边栏进程管理初始化失败：" + jobErr.Error(); a.Refresh() })
			return
		}
		childDone := make(chan error, 1)
		go func() { childDone <- cmd.Wait() }()
		defer func() {
			if cmd.Process != nil {
				_ = cmd.Process.Kill()
			}
		}()
		ticker := time.NewTicker(500 * time.Millisecond)
		defer ticker.Stop()
		var previous string
		var live, written []byte
		publish := func() {
			settings := defaultAllegroSettings()
			desktop, side := allegroBoard{}, allegroBoard{}
			raw, _ := os.ReadFile(filepath.Join(a.Data, "allegro-settings.json"))
			_ = json.Unmarshal(raw, &settings)
			readBoard := func(name string, b *allegroBoard) {
				v, _ := os.ReadFile(filepath.Join(a.Data, name))
				_ = json.Unmarshal(v, b)
			}
			readBoard("allegro-desktop.json", &desktop)
			readBoard("allegro-sidebar.json", &side)
			raw, _ = json.Marshal([]any{settings, desktop, side})
			var status struct {
				Settings       allegroSettings `json:"settings"`
				Registered     bool            `json:"registered"`
				EditRegistered bool            `json:"edit_registered"`
			}
			statusRaw, _ := os.ReadFile(filepath.Join(a.Data, "allegro-host-status.json"))
			_ = json.Unmarshal(statusRaw, &status)
			hotkeyError := ""
			if len(statusRaw) > 0 && status.Settings == settings && settings.Enabled && !status.Registered {
				hotkeyError = "侧边栏热键已被占用，请录入其他组合键。"
			}
			if len(statusRaw) > 0 && status.Settings == settings && !status.EditRegistered {
				hotkeyError += " 桌面编辑热键已被占用，请录入其他组合键。"
			}
			key := string(raw) + hotkeyError
			open, openErr := os.ReadFile(filepath.Join(a.Data, "allegro-center-request.json"))
			if openErr == nil {
				_ = os.Remove(filepath.Join(a.Data, "allegro-center-request.json"))
			}
			if key == previous && openErr != nil {
				return
			}
			previous = key
			mygo.RunOnMain(func() {
				a.allegroSettings = settings
				a.allegroError = hotkeyError
				a.allegroDesktop = desktop
				a.allegroSidebar = side
				if openErr == nil {
					a.Page = PageSettings
					a.SettingsTab = 10
					if string(open) == "tutorial" {
						a.allegroTutorialOpen = true
						a.allegroTutorial = 0
					}
					if a.ShowCenter != nil {
						a.ShowCenter()
					}
				}
				a.Refresh()
			})
		}
		publish()
		for {
			select {
			case <-ctl.stop:
				return
			case err := <-childDone:
				mygo.RunOnMain(func() { a.allegroError = fmt.Sprintf("侧边栏已退出：%v，请重启点屿", err); a.Refresh() })
				return
			case m := <-ctl.queue:
				path := filepath.Join(a.Data, m.file)
				old, _ := os.ReadFile(path)
				next, err := m.change(old)
				if err == nil {
					err = atomicAllegro(path, next)
				}
				if err != nil {
					mygo.RunOnMain(func() { a.allegroError = "保存失败：" + err.Error(); a.Refresh() })
				} else {
					mygo.RunOnMain(func() { a.allegroError = ""; a.Refresh() })
				}
				publish()
			case name := <-ctl.previews:
				raw, err := os.ReadFile(filepath.Join(filepath.Dir(engine), "Assets", "Allegro", name))
				if err == nil {
					mygo.RunOnMain(func() {
						b, err := ui.DecodeBitmap(raw)
						if err != nil {
							return
						}
						if a.allegroPreviews == nil {
							a.allegroPreviews = map[string]*ui.Bitmap{}
						}
						if len(a.allegroPreviews) >= 32 {
							for key := range a.allegroPreviews {
								delete(a.allegroPreviews, key)
								delete(a.allegroPreviewPending, key)
								break
							}
						}
						a.allegroPreviews[name] = b
						a.Refresh()
					})
				}
			case live = <-ctl.live:
			case <-ticker.C:
				if string(live) != string(written) {
					if atomicAllegro(filepath.Join(a.Data, "allegro-live.json"), live) == nil {
						written = live
					}
				}
				publish()
			}
		}
	}()
}
func (a *App) StopAllegro() {
	if a.allegro != nil {
		a.allegro.once.Do(func() { close(a.allegro.stop) })
		select {
		case <-a.allegro.done:
		case <-time.After(3 * time.Second):
		}
	}
}
func (a *App) allegroChange(change func(*allegroSettings)) {
	if a.allegro == nil {
		return
	}
	// Persist a field-level delta so concurrent native edits retain their values.
	old, next := a.allegroSettings, a.allegroSettings
	change(&next)
	a.allegroSettings = next
	ctl := a.allegro
	m := allegroMutation{"allegro-settings.json", func(raw []byte) ([]byte, error) {
		s := defaultAllegroSettings()
		_ = json.Unmarshal(raw, &s)
		ov, nv, sv := reflect.ValueOf(old), reflect.ValueOf(next), reflect.ValueOf(&s).Elem()
		for i := 0; i < ov.NumField(); i++ {
			if !reflect.DeepEqual(ov.Field(i).Interface(), nv.Field(i).Interface()) {
				sv.Field(i).Set(nv.Field(i))
			}
		}
		return json.MarshalIndent(s, "", "  ")
	}}
	select {
	case ctl.queue <- m:
	default:
		a.allegroError = "设置正在保存，请稍后重试"
	}
}
func (a *App) allegroBoardChange(change func(*allegroBoard)) {
	if a.allegro == nil {
		return
	}
	m := allegroMutation{"allegro-desktop.json", func(raw []byte) ([]byte, error) {
		var b allegroBoard
		if len(raw) > 0 {
			if err := json.Unmarshal(raw, &b); err != nil {
				return nil, err
			}
		}
		change(&b)
		return json.MarshalIndent(b, "", "  ")
	}}
	select {
	case a.allegro.queue <- m:
	default:
		a.pushToast("组件正在保存，请稍后重试")
	}
}

var allegroHotkeys = [][2]string{{"Alt + `", "alt_grave"}, {"Alt + Space", "alt_space"}, {"Ctrl + Space", "ctrl_space"}, {"Alt + Q", "alt_q"}, {"Ctrl + Shift + D", "ctrl_shift_d"}}

func (a *App) allegroHotkeyChoices(c *ui.Context, p *Palette, x, y, w float32) {
	for i, opt := range allegroHotkeys {
		xx, yy := x+float32(i%3)*(w/3), y+float32(i/3)*38
		Chip(c, p, "sidebar-hotkey-"+opt[1], xx, yy, w/3-6, 32, opt[0], a.allegroSettings.CustomKey == 0 && a.allegroSettings.Hotkey == opt[1], func() {
			next := a.allegroSettings
			next.Hotkey = opt[1]
			next.CustomKey = 0
			next.CustomModifiers = 0
			if next.hotkeyLabel() == next.editHotkeyLabel() {
				a.allegroCaptureError = "不能与桌面编辑热键相同。"
				return
			}
			a.allegroCaptureError = ""
			a.allegroChange(func(s *allegroSettings) { s.Hotkey = opt[1]; s.CustomKey = 0; s.CustomModifiers = 0 })
		})
	}
	a.allegroCustomHotkey(c, p, x, y+80, w)
}
func (a *App) allegroSettingsBody(c *ui.Context, p *Palette, x, y, w float32) float32 {
	y = leafHeading(c, p, "效率侧边栏", x, y, w)
	if !a.AllegroAvailable {
		Label(c, "侧边栏引擎未随运行包提供，请使用完整运行包。", x, y, 14, p.Scheme.Error, false, w)
		return y + 80
	}
	settingCard(c, x, y, w, 80, p.Scheme.Container)
	settingToggleRow(c, p, "allegro-enable", "启用效率侧边栏", "使用热键打开；关闭后不再响应侧边栏热键。", x+16, y, w-32, a.allegroSettings.Enabled, func() { a.allegroChange(func(s *allegroSettings) { s.Enabled = !s.Enabled }) })
	y += 96
	y = leafHeading(c, p, "侧边栏热键", x, y, w)
	a.allegroHotkeyChoices(c, p, x+16, y, w-32)
	y += 150
	y = leafHeading(c, p, "桌面编辑热键", x, y, w)
	a.allegroEditHotkey(c, p, x+16, y, w-32)
	y += 80
	y = leafChoices(c, p, x, y, w, "展开方向", [][2]string{{"左侧", "left"}, {"右侧", "right"}}, a.allegroSettings.Edge, func(v string) { a.allegroChange(func(s *allegroSettings) { s.Edge = v }) })
	y = leafAction(c, p, x, y, w, "侧边栏与组件教程", "搜索、组件库、拖动、编辑与堆叠。", "重新查看", func() { a.allegroTutorialOpen = true; a.allegroTutorial = 0 })
	if a.allegroError != "" {
		Label(c, a.allegroError, x+16, y, 13, p.Scheme.Error, false, w-32)
		y += 56
	}
	return y
}
func (a *App) allegroTutorialView(c *ui.Context, w, h float32) {
	lessons := [][3]string{{"侧边栏，随时呼出", "使用设置的热键展开或收起侧边栏。点击搜索框可查找应用或计算；Esc 收起面板。", "sidebar-light.png"}, {"把常用组件放在身边", "从组件库添加或拖出卡片，滚轮浏览；同尺寸卡片可以堆叠，滚轮切换堆叠内容。", "sidebar-drawer-light.png"}, {"桌面也能自由整理", a.allegroSettings.editHotkeyLabel() + " 进入桌面编辑，拖动卡片对齐网格。右键可改尺寸、材质、顺序与删除。", "widgets-light.png"}}
	i := min(a.allegroTutorial, len(lessons)-1)
	lesson := lessons[i]
	p := &a.Palette
	s := &p.Scheme
	ui.Overlay(c, func() {
		ui.Box(c).Absolute().Fill().Modal().Role(ui.RoleDialog).Label("侧边栏使用教程").Background(AlphaCol(0, .4)).Children(func() {
			pw, ph := min(float32(720), w-32), min(float32(610), h-32)
			panel := ui.Box(c).Absolute().Left((w - pw) / 2).Top((h - ph) / 2).Width(pw).Height(ph)
			overlayStyle(&panel, p)
			panel.Children(func() {
				Label(c, fmt.Sprintf("侧边栏与组件 · %d / 3", i+1), 24, 20, 13, s.Primary, true, pw-48)
				Label(c, lesson[0], 24, 54, 22, s.OnSurface, true, pw-48)
				if bmp := a.allegroBitmap(lesson[2]); bmp != nil {
					ui.Image(c, bmp).Absolute().Left(24).Top(100).Width(pw - 48).Height(ph - 264).Fit(ui.Contain).IgnoreInput(true)
				}
				ui.Box(c).Absolute().Left(24).Top(ph - 146).Width(pw - 48).Children(func() {
					ui.Text(c, lesson[1]).Font(textFamily(lesson[1])).FontSize(14).TextColor(Col(s.OnSurfaceVariant))
				})
				Button(c, p, "sidebar-tutorial-prev", 24, ph-64, 112, 38, "上一步", "", true, i > 0, func() { a.allegroTutorial-- })
				label := "下一步"
				if i == 2 {
					label = "继续设置"
				}
				Button(c, p, "sidebar-tutorial-next", pw-158, ph-64, 134, 38, label, "", false, true, func() {
					if i < 2 {
						a.allegroTutorial++
					} else {
						a.allegroTutorialOpen = false
						a.allegroChange(func(s *allegroSettings) { s.TutorialVersion = 1 })
						a.cityPromptAsked = true
						a.cityPrompt = &cityPromptState{text: a.configuredWeatherCity()}
					}
				})
			})
		})
	})
}
func (a *App) allegroBitmap(name string) *ui.Bitmap {
	if b := a.allegroPreviews[name]; b != nil {
		return b
	}
	if a.allegro != nil && !a.allegroPreviewPending[name] {
		if a.allegroPreviewPending == nil {
			a.allegroPreviewPending = map[string]bool{}
		}
		select {
		case a.allegro.previews <- name:
			a.allegroPreviewPending[name] = true
		default:
		}
	}
	return nil
}
func (a *App) allegroGallery(c *ui.Context, x, y, w float32) float32 {
	p := &a.Palette
	s := &p.Scheme
	Label(c, "小组件", x, y, 18, s.OnSurface, true, w)
	Label(c, "与侧边栏共用的组件库 · 预览为示例内容，添加后在桌面编辑", x, y+32, 12, s.OnSurfaceVariant, false, w)
	y += 76
	cw := (w - 20) / 2
	ch := float32(306)
	for i, e := range allegroCatalog {
		xx, yy := x+float32(i%2)*(cw+20), y+float32(i/2)*(ch+20)
		if yy+ch < a.scrollY() || yy > a.scrollY()+float32(clipH) {
			continue
		}
		appleWidgetSurface(c, p, overviewRect{"", xx, yy, cw, ch})
		size := a.widgetSizeOf(e.kind)
		mode := "light"
		if a.Palette.Dark {
			mode = "dark"
		}
		if bmp := a.allegroBitmap(e.kind + "-" + size + "-" + mode + ".png"); bmp != nil {
			ui.Image(c, bmp).Absolute().Left(xx + 16).Top(yy + 12).Width(cw - 32).Height(184).Fit(ui.Contain).IgnoreInput(true)
		}
		Label(c, e.label, xx+20, yy+208, 15, s.OnSurface, true, cw-40)
		for j, sz := range widgetSizes {
			Chip(c, p, "allegro-size-"+e.kind+sz.key, xx+18+float32(j)*46, yy+252, 42, 32, sz.label, size == sz.key, func() { a.setWidgetSize(e.kind, sz.key) })
		}
		Button(c, p, "allegro-add-"+e.kind, xx+cw-126, yy+250, 108, 36, "添加到桌面", "add", false, len(a.allegroDesktop.Cards) < 128, func() {
			a.allegroBoardChange(func(b *allegroBoard) {
				id := uint64(time.Now().UnixMilli())
				b.Cards = append(b.Cards, allegroCard{ID: id, Kind: e.kind, Title: e.label, Size: map[string]string{"small": "Small", "medium": "Medium", "large": "Large"}[size], Acrylic: 1, Stack: []string{}, X: 32 + float64(len(b.Cards)%4)*192, Y: 32 + float64(len(b.Cards)/4)*192})
			})
		})
	}
	y += float32((len(allegroCatalog)+1)/2)*(ch+20) + 12
	Label(c, fmt.Sprintf("桌面上的组件 · %d", len(a.allegroDesktop.Cards)), x, y, 18, s.OnSurface, true, w)
	y += 44
	for _, card := range a.allegroDesktop.Cards {
		Label(c, card.Title, x+16, y+12, 14, s.OnSurface, true, w-300)
		Button(c, p, fmt.Sprint("allegro-resize-", card.ID), x+w-270, y, 120, 36, "切换尺寸", "", true, true, func() {
			a.allegroBoardChange(func(b *allegroBoard) {
				for i := range b.Cards {
					if b.Cards[i].ID == card.ID {
						b.Cards[i].Size = map[string]string{"Small": "Medium", "Medium": "Large", "Large": "Small"}[b.Cards[i].Size]
					}
				}
			})
		})
		Button(c, p, fmt.Sprint("allegro-remove-", card.ID), x+w-138, y, 120, 36, "移除", "", true, true, func() {
			a.allegroBoardChange(func(b *allegroBoard) {
				for i := range b.Cards {
					if b.Cards[i].ID == card.ID {
						b.Cards = append(b.Cards[:i], b.Cards[i+1:]...)
						break
					}
				}
			})
		})
		y += 54
	}
	if a.allegroError != "" {
		Label(c, a.allegroError, x, y, 13, s.Error, false, w)
		y += 48
	}
	return y
}

func (a *App) publishAllegroLive(state map[string]any) {
	if a.allegro == nil {
		return
	}
	raw, err := json.Marshal(state)
	if err != nil {
		return
	}
	select {
	case a.allegro.live <- raw:
	default:
		select {
		case <-a.allegro.live:
		default:
		}
		select {
		case a.allegro.live <- raw:
		default:
		}
	}
}
