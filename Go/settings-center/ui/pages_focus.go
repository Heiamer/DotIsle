// The focus page: timer_surface.rs render() (Original theme), the full-width
// workflow workspace from workflow.rs hero_full/editor_full/running_full, and the
// timer dialogs (TimerPreferences, WorkflowStage, WorkflowTemplate, WorkflowTemplates,
// WorkflowModules) from views.rs dialog / tool_preferences.rs. Timer math mirrors
// dotisle-focus engine.rs.
package ui

import (
	"encoding/json"
	"fmt"
	"strings"
	"time"
	"unicode"

	"dotisle/settings/island"
	"github.com/egoist/mygo/ui"
)

// nanoStamp mirrors the Rust "flow-{nanos}"/"user-{nanos}" id scheme.
func nanoStamp() int64 { return time.Now().UnixNano() }

// ---- wire types (camelCase, dotisle-focus engine.rs) ----

type goStage struct {
	Name    string `json:"name"`
	Seconds int64  `json:"seconds"`
}

type goWorkflow struct {
	Stages     []goStage `json:"stages"`
	StageIndex int       `json:"stageIndex"`
	Waiting    bool      `json:"waiting"`
	TemplateID string    `json:"templateId"`
}

type goTimer struct {
	ID        string      `json:"id"`
	Name      string      `json:"name"`
	Mode      string      `json:"mode"`
	Duration  int64       `json:"duration"`
	Focus     int64       `json:"focus"`
	Rest      int64       `json:"rest"`
	Rounds    int64       `json:"rounds"`
	Round     int64       `json:"round"`
	Phase     string      `json:"phase"`
	ElapsedMs int64       `json:"elapsedMs"`
	Running   bool        `json:"running"`
	Finished  bool        `json:"finished"`
	Workflow  *goWorkflow `json:"workflow"`
	Revision  int64       `json:"revision"`
	Laps      []int64     `json:"laps"`
	CountUp   bool        `json:"countUp"`
}

func (a *App) timerItems() []goTimer {
	if a.State == nil {
		return nil
	}
	raw, ok := a.State["timers"].([]any)
	if !ok {
		return nil
	}
	items := make([]goTimer, 0, len(raw))
	for _, v := range raw {
		buf, err := json.Marshal(v)
		if err != nil {
			continue
		}
		var t goTimer
		if json.Unmarshal(buf, &t) == nil {
			items = append(items, t)
		}
	}
	return items
}

// formatTimerTime is dotisle-focus engine::format_time.
func formatTimerTime(s int64) string {
	if s >= 3600 {
		return fmt.Sprintf("%02d:%02d:%02d", s/3600, s/60%60, s%60)
	}
	return fmt.Sprintf("%02d:%02d", s/60, s%60)
}

func (t *goTimer) stage() *goStage {
	if t.Workflow == nil || t.Workflow.StageIndex < 0 || t.Workflow.StageIndex >= len(t.Workflow.Stages) {
		return nil
	}
	return &t.Workflow.Stages[t.Workflow.StageIndex]
}

// limitMs: workflow stage, pomodoro phase or plain duration, in ms.
func (t *goTimer) limitMs() int64 {
	if t.Mode == "workflow" {
		ms := int64(1000)
		if st := t.stage(); st != nil {
			if st.Seconds > 1 {
				ms = st.Seconds * 1000
			}
		}
		return ms
	}
	if t.Mode == "pomodoro" {
		if t.Phase == "rest" {
			return t.Rest * 1000
		}
		return t.Focus * 1000
	}
	return t.Duration * 1000
}

func (t *goTimer) remainingMs() int64 {
	limit := t.limitMs()
	if t.ElapsedMs >= limit {
		return 0
	}
	return limit - t.ElapsedMs
}

func (t *goTimer) clock() string {
	if t.Mode == "stopwatch" || t.CountUp {
		return formatTimerTime(t.ElapsedMs / 1000)
	}
	return formatTimerTime((t.remainingMs() + 999) / 1000)
}

func (t *goTimer) progress() float64 {
	if t.Mode == "stopwatch" {
		return float64(t.ElapsedMs%60000) / 60000
	}
	return clamp01(float64(t.ElapsedMs) / float64(max64(t.limitMs(), 1)))
}

func (t *goTimer) subtitle() string {
	if t.Finished {
		return "已完成"
	}
	if t.Mode == "workflow" {
		index, total, waiting := 0, 0, false
		stage := "阶段"
		if w := t.Workflow; w != nil {
			index, total, waiting = w.StageIndex, len(w.Stages), w.Waiting
			if st := t.stage(); st != nil {
				stage = st.Name
			}
		}
		state := ""
		switch {
		case waiting:
			state = "等待下一阶段"
		case t.Running:
			state = stage
		default:
			state = stage + " · 已暂停"
		}
		return fmt.Sprintf("流程 · 第 %d / %d 阶段 · %s", index+1, total, state)
	}
	var phase string
	switch {
	case t.Mode == "pomodoro":
		name := "休息"
		if t.Phase == "focus" {
			name = "专注"
		}
		phase = fmt.Sprintf("%s · 第 %d / %d 轮", name, t.Round, t.Rounds)
	case t.Mode == "stopwatch":
		phase = "正向计时"
	default:
		phase = "倒计时"
	}
	if t.Running {
		return phase
	}
	return phase + " · 已暂停"
}

func (t *goTimer) primaryAction() string {
	switch {
	case t.Finished:
		return "restart"
	case t.Mode == "workflow" && t.Workflow != nil && t.Workflow.Waiting:
		return "next"
	case t.Running:
		return "pause"
	}
	return "resume"
}

func max64(a, b int64) int64 {
	if a > b {
		return a
	}
	return b
}

// ---- page-local state (move into App centrally) ----

var focusUI struct {
	mode       int // 0 番茄钟 1 倒计时 2 秒表 3 流程 (Model::mode)
	minutes    int // Model::minutes, default 25
	seconds    int // Model::timer_seconds, 0 = None
	rest       int // Model::rest_seconds, default 300
	rounds     int // Model::timer_rounds, default 4
	selected   string
	modulePage int
}

type wfStageDraft struct {
	Name    string
	Minutes int
}

var workflowUI struct {
	stages       []wfStageDraft
	templateID   string
	templateName string
}

func defaultWorkflowStages() []wfStageDraft {
	return []wfStageDraft{{"专注", 25}, {"休息", 5}, {"复盘", 10}}
}

func init() {
	focusUI.minutes, focusUI.rest, focusUI.rounds = 25, 300, 4
	workflowUI.stages = defaultWorkflowStages()
}

// focus dialog kinds (Model::dialog DialogKind).
const (
	fdNone = iota
	fdTimerPrefs
	fdStage
	fdTemplate
	fdTemplates
	fdModules
	fdModuleEdit
)

var focusDialog struct {
	kind       int
	pLabel0    string
	pSecs      string
	pRest      string
	pRounds    string
	pUp        bool
	pErr       string
	sIndex     int // <0 appends a new stage
	sName      string
	sMins      string
	sErr       string
	tName      string
	tOverwrite bool
	tErr       string
	mIndex     int // <0 creates a module
	mName      string
	mMins      string
	mErr       string
}

// fdClose closes whichever focus dialog is open (Action::CancelModal).
func fdClose() { focusDialog.kind = fdNone }

// ---- workflow draft ops (workflow.rs, exact messages) ----

const (
	wfStageMin    = 2
	wfStageMax    = 12
	wfStageMaxMin = 240
	wfStageStep   = 5
	wfTotalMaxMin = 24 * 60
)

func wfTotalMinutes(stages []wfStageDraft) int {
	total := 0
	for _, s := range stages {
		total += s.Minutes
	}
	return total
}

func wfSummary(stages []wfStageDraft) string {
	return fmt.Sprintf("共 %d 分钟 · %d 个阶段", wfTotalMinutes(stages), len(stages))
}

func wfAddStage(stage wfStageDraft) error {
	if len(workflowUI.stages) >= wfStageMax {
		return fmt.Errorf("流程最多 %d 个阶段", wfStageMax)
	}
	m := stage.Minutes
	if m < 1 {
		m = 1
	}
	if m > wfStageMaxMin {
		m = wfStageMaxMin
	}
	if wfTotalMinutes(workflowUI.stages)+m > wfTotalMaxMin {
		return fmt.Errorf("流程总时长不能超过 24 小时")
	}
	workflowUI.stages = append(workflowUI.stages, wfStageDraft{stage.Name, m})
	return nil
}

func wfDeleteStage(index int) error {
	if index < 0 || index >= len(workflowUI.stages) {
		return fmt.Errorf("阶段不存在")
	}
	if len(workflowUI.stages) <= wfStageMin {
		return fmt.Errorf("流程至少需要 %d 个阶段", wfStageMin)
	}
	workflowUI.stages = append(workflowUI.stages[:index], workflowUI.stages[index+1:]...)
	return nil
}

func wfMoveStage(index, delta int) error {
	if index < 0 || index >= len(workflowUI.stages) {
		return fmt.Errorf("阶段不存在")
	}
	target := index + delta
	if target < 0 {
		return fmt.Errorf("已经是第一个阶段")
	}
	if target >= len(workflowUI.stages) {
		return fmt.Errorf("已经是最后一个阶段")
	}
	stages := workflowUI.stages
	stages[index], stages[target] = stages[target], stages[index]
	return nil
}

func wfNudgeStage(index, delta int) error {
	if index < 0 || index >= len(workflowUI.stages) {
		return fmt.Errorf("阶段不存在")
	}
	next := workflowUI.stages[index].Minutes + delta
	if next < 1 {
		next = 1
	}
	if next > wfStageMaxMin {
		next = wfStageMaxMin
	}
	if next == workflowUI.stages[index].Minutes {
		return nil
	}
	others := 0
	for i, s := range workflowUI.stages {
		if i != index {
			others += s.Minutes
		}
	}
	if others+next > wfTotalMaxMin {
		return fmt.Errorf("流程总时长不能超过 24 小时")
	}
	workflowUI.stages[index].Minutes = next
	return nil
}

// wfApplyStage is workflow.rs apply_stage; index < 0 appends.
func wfApplyStage(index int, name, minutes string) error {
	var mins int
	if _, err := fmt.Sscanf(strings.TrimSpace(minutes), "%d", &mins); err != nil {
		return fmt.Errorf("分钟需要 1–240 的数字")
	}
	if mins < 1 || mins > wfStageMaxMin {
		return fmt.Errorf("分钟需要 1–240")
	}
	name = strings.TrimSpace(name)
	if len([]rune(name)) > 40 {
		return fmt.Errorf("阶段名称不能超过 40 字")
	}
	for _, r := range name {
		if unicode.IsControl(r) {
			return fmt.Errorf("阶段名称不能超过 40 字")
		}
	}
	others := 0
	for i, s := range workflowUI.stages {
		if i != index {
			others += s.Minutes
		}
	}
	if others+mins > wfTotalMaxMin {
		return fmt.Errorf("流程总时长不能超过 24 小时")
	}
	if index >= 0 && index < len(workflowUI.stages) {
		if name == "" {
			name = fmt.Sprintf("阶段 %d", index+1)
		}
		workflowUI.stages[index] = wfStageDraft{name, mins}
		return nil
	}
	if len(workflowUI.stages) >= wfStageMax {
		return fmt.Errorf("流程最多 %d 个阶段", wfStageMax)
	}
	if name == "" {
		name = fmt.Sprintf("阶段 %d", len(workflowUI.stages)+1)
	}
	workflowUI.stages = append(workflowUI.stages, wfStageDraft{name, mins})
	return nil
}

// wfStagePayload is workflow.rs stage_payload, the timerCreate/template stages array.
func wfStagePayload(stages []wfStageDraft) ([]map[string]any, error) {
	if len(stages) < wfStageMin || len(stages) > wfStageMax {
		return nil, fmt.Errorf("流程需要 %d 到 %d 个阶段", wfStageMin, wfStageMax)
	}
	out := make([]map[string]any, 0, len(stages))
	total := 0
	for i, s := range stages {
		if s.Minutes < 1 || s.Minutes > wfStageMaxMin {
			return nil, fmt.Errorf("第 %d 个阶段需要 1–240 分钟", i+1)
		}
		total += s.Minutes
		name := strings.TrimSpace(s.Name)
		if len([]rune(name)) > 40 {
			return nil, fmt.Errorf("阶段名称不能超过 40 字")
		}
		for _, r := range name {
			if unicode.IsControl(r) {
				return nil, fmt.Errorf("阶段名称不能超过 40 字")
			}
		}
		if name == "" {
			name = fmt.Sprintf("阶段 %d", i+1)
		}
		out = append(out, map[string]any{"name": name, "seconds": int64(s.Minutes) * 60})
	}
	if total > wfTotalMaxMin {
		return nil, fmt.Errorf("流程总时长不能超过 24 小时")
	}
	return out, nil
}

// goTemplate mirrors dotisle-focus Template.
type goTemplate struct {
	ID     string    `json:"id"`
	Name   string    `json:"name"`
	Stages []goStage `json:"stages"`
}

func (a *App) templates() []goTemplate {
	if a.State == nil {
		return nil
	}
	raw, ok := a.State["templates"].([]any)
	if !ok {
		return nil
	}
	out := make([]goTemplate, 0, len(raw))
	for _, v := range raw {
		buf, err := json.Marshal(v)
		if err != nil {
			continue
		}
		var t goTemplate
		if json.Unmarshal(buf, &t) == nil {
			out = append(out, t)
		}
	}
	return out
}

// goModule mirrors workflow.rs WorkflowModule.
type goModule struct {
	ID      string
	Name    string
	Minutes int
	Kind    uint8
}

// modules is workflow.rs modules(): native-user data, else the six built-ins.
func (a *App) modules() []goModule {
	raw, ok := a.pref("WorkflowModules").([]any)
	if !ok {
		return []goModule{
			{"builtin-thinking", "整理思路", 5, 0},
			{"builtin-focus", "专注", 25, 1},
			{"builtin-rest", "休息", 5, 2},
			{"builtin-review", "复盘", 10, 3},
			{"builtin-move", "活动一下", 3, 4},
			{"builtin-custom", "自定义", 10, 5},
		}
	}
	out := make([]goModule, 0, len(raw))
	for _, v := range raw {
		item, ok := v.(map[string]any)
		if !ok {
			continue
		}
		id, _ := item["id"].(string)
		name, _ := item["name"].(string)
		minutes, _ := item["minutes"].(float64)
		kind, _ := item["kind"].(float64)
		if id == "" || name == "" {
			continue
		}
		m := int(minutes)
		if m < 1 {
			m = 1
		}
		if m > wfStageMaxMin {
			m = wfStageMaxMin
		}
		k := uint8(kind)
		if k > 5 {
			k = 5
		}
		out = append(out, goModule{id, name, m, k})
	}
	return out
}

// ---- commands ----

// timerAction sends timerAction {id, action, revision} (model.rs timer_action_value).
func (a *App) timerAction(id, action string) {
	for _, t := range a.timerItems() {
		if t.ID == id {
			a.command("timerAction", map[string]any{"id": id, "action": action, "revision": t.Revision})
			return
		}
	}
}

// sendTimerCreate sends timerCreate and selects the created timer from the reply
// (apply_native_reply: self.selected = reply["id"]).
func (a *App) sendTimerCreate(payload map[string]any) {
	a.startAsync(func() {
		reply, err := island.Request("timerCreate", payload)
		a.applyAsync(func() {
			if a.applyCommandReply("timerCreate", reply, err) {
				return
			}
			if err == nil && reply["success"] == true {
				if id, _ := reply["id"].(string); id != "" {
					focusUI.selected = id
				}
			}
			a.Refresh()
		})
	})
}

// ---- the page ----

// focusPage mirrors timer_surface.rs render() + workflow.rs hero_full.
func (a *App) focusPage(c *ui.Context, x, y, w float32) float32 {
	focusUI.mode = max(0, min(3, focusUI.mode))
	p := &a.Palette
	s := &p.Scheme
	timers := a.timerItems()
	var selected *goTimer
	if focusUI.selected != "" {
		for i := range timers {
			if timers[i].ID == focusUI.selected {
				selected = &timers[i]
			}
		}
	}

	// Local height pass, mirroring the Rust cy flow.
	runningWorkflow := focusUI.mode == 3 && selected != nil && selected.Mode == "workflow"
	bottom := float32(0)
	{
		by := float32(64)
		if focusUI.mode == 3 {
			by = a.workflowHeight(c, w, runningWorkflow)
		} else {
			by += timerStudioHeight
			if focusUI.mode == 2 && selected != nil && len(selected.Laps) > 0 {
				by += 40 + float32(min(len(selected.Laps), 10))*56 + 12
			}
			by += 24
		}
		if !runningWorkflow {
			bottom = by + 60 + float32(len(timers))*100
			if len(timers) == 0 {
				bottom += 44
			}
		} else {
			bottom = by
		}
	}

	page := ui.Box(c).Margin(y, 0, 0, x).Width(w).Height(bottom)
	page.Children(func() {
		Segmented(c, p, 0, 0, w, []string{"番茄钟", "倒计时", "秒表", "流程"}, focusUI.mode, func(index int) {
			focusUI.mode, focusUI.selected = index, ""
		})

		if focusUI.mode == 3 {
			a.workflowHeroFull(c, p, selected, 0, 64, w)
		} else {
			a.timerCards(c, p, selected, 0, 64, w)
			ly := float32(64 + timerStudioHeight + 24)
			if focusUI.mode == 2 && selected != nil && len(selected.Laps) > 0 {
				Label(c, "分段记录", 0, ly, 16, s.OnSurface, true, w-8)
				ly += 40
				laps := selected.Laps
				start := len(laps) - 10
				if start < 0 {
					start = 0
				}
				for index := len(laps) - 1; index >= start; index-- {
					previous := int64(0)
					if index > 0 {
						previous = laps[index-1]
					}
					Panel(c, p, 0, ly, w, 48, s.Container)
					Label(c, fmt.Sprintf("第 %d 段", index+1), 20, ly+13, 14, s.OnSurface, false, 120)
					Label(c, fmt.Sprintf("%s · 总计 %s",
						formatTimerTime((laps[index]-previous)/1000),
						formatTimerTime(laps[index]/1000)), 160, ly+13, 14, s.OnSurfaceVariant, false, w-180)
					ly += 56
				}
				ly += 12
			}
			a.timerList(c, p, timers, ly, w)
		}
	})
	a.focusDialogs(c)
	return y + bottom
}

// startTimer is Action::StartTimer for modes 0..2 (model.rs remote_action).
func (a *App) startTimer(_ string) {
	mode := [3]string{"pomodoro", "countdown", "stopwatch"}[max(0, min(focusUI.mode, 2))]
	dur := int64(focusUI.minutes * 60)
	if focusUI.seconds > 0 {
		dur = int64(focusUI.seconds)
	}
	a.sendTimerCreate(map[string]any{
		"mode":     mode,
		"duration": dur,
		"focus":    dur,
		"rest":     int64(focusUI.rest),
		"rounds":   int64(focusUI.rounds),
	})
}

// timerList is the 计时列表 block of timer_surface.rs.
func (a *App) timerList(c *ui.Context, p *Palette, timers []goTimer, ly, w float32) {
	s := &p.Scheme
	Label(c, "计时列表", 0, ly+8, 20, s.OnSurface, true, w-176)
	Button(c, p, "新建计时", w-148, ly, 148, 44, "新建计时", "add", true, len(timers) < 8, func() {
		a.startTimer("")
	})
	ly += 60
	for i := range timers {
		t := &timers[i]
		Panel(c, p, 0, ly, w, 88, s.Container)
		sel := ui.ButtonBase(c).Absolute().Left(0).Top(ly).Width(w - 132).Height(88).
			Label("选择" + t.Name).Cursor(ui.CursorPointer)
		hoverLayer(&sel, ui.Transparent, s.OnSurface, true)
		if sel.Clicked() {
			// Action::SelectTimer re-maps the mode from the timer.
			focusUI.selected = t.ID
			switch t.Mode {
			case "countdown":
				focusUI.mode = 1
			case "stopwatch":
				focusUI.mode = 2
			case "workflow":
				focusUI.mode = 3
			default:
				focusUI.mode = 0
			}
		}
		running := uint32(s.Outline)
		if t.Running {
			running = s.Primary
		}
		ui.Box(c).Absolute().Left(16).Top(ly + 20).Width(4).Height(48).
			Radius(2).Background(Col(running))
		Label(c, t.Name, 36, ly+15, 16, s.OnSurface, true, w-340)
		Label(c, t.subtitle(), 36, ly+49, 12, s.OnSurfaceVariant, false, w-340)
		Label(c, t.clock(), w-300, ly+25, 26, s.Primary, true, 160)
		playIcon := "play_arrow"
		if t.Running {
			playIcon = "pause"
		}
		IconButton(c, p, "切换计时状态", w-120, ly+22, 44, 44, playIcon, false, true, func() {
			a.timerAction(t.ID, t.primaryAction())
		})
		IconButton(c, p, "移除计时器", w-64, ly+22, 44, 44, "delete", false, true, func() {
			a.timerAction(t.ID, "delete")
		})
		ly += 100
	}
	if len(timers) == 0 {
		Label(c, "还没有计时，开始一段属于自己的时间。", 0, ly+8, 13, s.OnSurfaceVariant, false, w)
	}
}

// openTimerPrefs is tool_preferences.rs edit_timer_preferences.
func (a *App) openTimerPrefs() {
	focusDialog.kind = fdTimerPrefs
	focusDialog.pErr = ""
	focusDialog.pUp = a.pref("FocusCountUp") == true
	if focusUI.mode == 0 {
		focusDialog.pLabel0 = "专注时长 · 秒"
	} else {
		focusDialog.pLabel0 = "倒计时时长 · 秒"
	}
	secs := focusUI.seconds
	if secs == 0 {
		secs = focusUI.minutes * 60
	}
	focusDialog.pSecs = fmt.Sprintf("%d", secs)
	focusDialog.pRest = fmt.Sprintf("%d", focusUI.rest)
	focusDialog.pRounds = fmt.Sprintf("%d", focusUI.rounds)
}

// saveTimerPrefs is tool_preferences.rs save_timer_preferences.
func (a *App) saveTimerPrefs() {
	parse := func(s string) (int, bool) {
		var v int
		if _, err := fmt.Sscanf(strings.TrimSpace(s), "%d", &v); err != nil {
			return 0, false
		}
		return v, true
	}
	secs, ok1 := parse(focusDialog.pSecs)
	rest, ok2 := parse(focusDialog.pRest)
	rounds, ok3 := parse(focusDialog.pRounds)
	if !ok1 || !ok2 || !ok3 {
		focusDialog.pErr = "请填写整数"
		return
	}
	limit := 86400
	if focusUI.mode == 0 {
		limit = 14400
	}
	if secs == 0 || secs > limit {
		focusDialog.pErr = "计时时长超出范围：番茄钟最多 4 小时，倒计时最多 24 小时。"
		return
	}
	if rest < 1 || rest > 3600 || rounds < 1 || rounds > 16 {
		focusDialog.pErr = "休息时长需要 1–3600 秒，轮数需要 1–16。"
		return
	}
	focusUI.seconds = secs
	focusUI.minutes = (secs + 59) / 60
	focusUI.rest = rest
	focusUI.rounds = rounds
	focusUI.selected = ""
	a.pushNative("FocusCountUp", focusDialog.pUp)
	fdClose()
}

// ---- workflow workspace (workflow.rs hero_full) ----

// focusMsg holds the last toast message; the toast strip itself is rendered by
// views.rs render() in the Rust app and needs central wiring in the Go shell.
var focusMsg string

// workflowHeight computes the absolute content bottom of the mode-3 branch.
func (a *App) workflowHeight(c *ui.Context, w float32, running bool) float32 {
	_ = c
	if running {
		return 64 + 392
	}
	heroH := float32(196)
	if w >= 860 {
		heroH = 132
	}
	cy := 64 + heroH + 18 + 30 + float32(len(workflowUI.stages))*72 + 6 + 26
	mods := len(a.modules()) + 1 // plus the 自定义阶段 chip
	cols := 3
	if w >= 880 {
		cols = 4
	}
	rows := (mods + cols - 1) / cols
	return cy + float32(rows)*44 + 6
}

// workflowHeroFull is workflow.rs hero_full.
func (a *App) workflowHeroFull(c *ui.Context, p *Palette, t *goTimer, lx, ly, w float32) {
	if t != nil && t.Mode == "workflow" {
		a.runningFull(c, p, t, lx, ly, w)
		return
	}
	a.editorFull(c, p, lx, ly, w)
}

// heroButton is workflow.rs hero_button: pill with radius h/2-8, 13.5px bold label.
func heroButton(c *ui.Context, p *Palette, label string, x, y, w, h float32, icon string, primary, enabled bool, onClick func()) {
	s := &p.Scheme
	bg, fg := uint32(s.High), uint32(s.OnSurfaceVariant)
	if enabled {
		if primary {
			bg, fg = s.Primary, s.OnPrimary
		} else {
			bg, fg = s.Surface, s.OnSurface
		}
	}
	e := ui.ButtonBase(c).Absolute().Left(x).Top(y).Width(w).Height(h).
		Justify(ui.Center).AlignItems(ui.Center).
		Radius(ButtonRadius).Background(Col(bg)).Label(label).Cursor(ui.CursorPointer)
	if !enabled {
		e.Disabled(true)
	}
	hoverLayer(&e, Col(bg), fg, enabled)
	if enabled && onClick != nil && e.Clicked() {
		onClick()
	}
	e.Children(func() {
		if icon != "" {
			if svg := Symbol(icon); svg != nil {
				ui.Icon(c, svg).FontSize(18).TextColor(Col(fg)).Absolute().Left(16).Top((h - 18) / 2)
			}
			ui.Box(c).Absolute().Left(32).Top(0).Width(w - 40).Height(h).
				Justify(ui.Center).AlignItems(ui.Center).Children(func() {
				ui.Text(c, label).Font(textFamily(label)).FontSize(13.5).FixedLineHeight(LineHeight(13.5)).
					TextColor(Col(fg)).FontWeight(500).SingleLine()
			})
			return
		}
		ui.Text(c, label).Font(textFamily(label)).FontSize(13.5).FixedLineHeight(LineHeight(13.5)).
			TextColor(Col(fg)).FontWeight(500).SingleLine()
	})
}

// quickChip is workflow.rs quick_chip: 18px-radius add chip with a leading plus.
func quickChip(c *ui.Context, p *Palette, label string, x, y, w float32, enabled bool, onClick func()) {
	s := &p.Scheme
	bg := uint32(s.Container)
	fg := uint32(s.OnSurface)
	plus := uint32(s.Primary)
	if !enabled {
		bg, fg, plus = s.High, s.OnSurfaceVariant, s.Outline
	}
	e := ui.ButtonBase(c).Absolute().Left(x).Top(y).Width(w).Height(36).
		Label(label).Cursor(ui.CursorPointer).Radius(18).Background(Col(bg))
	if !enabled {
		e.Disabled(true)
	}
	hoverLayer(&e, Col(bg), s.OnSurface, enabled)
	if enabled && onClick != nil && e.Clicked() {
		onClick()
	}
	e.Children(func() {
		if svg := Symbol("add"); svg != nil {
			ui.Icon(c, svg).FontSize(16).TextColor(Col(plus)).Absolute().Left(13).Top(10)
		}
		ui.Box(c).Absolute().Left(34).Top(0).Width(w - 42).Height(36).
			Justify(ui.Center).AlignItems(ui.Center).Children(func() {
			ui.Text(c, label).Font(textFamily(label)).FontSize(12.5).FixedLineHeight(LineHeight(12.5)).TextColor(Col(fg)).SingleLine()
		})
	})
}

// rowStep is workflow.rs row_step: a 2px minus line or a 16px plus icon.
func rowStep(c *ui.Context, p *Palette, x, y, w, h float32, plus bool, label string, enabled bool, onClick func()) {
	s := &p.Scheme
	fg := uint32(s.OnSurface)
	if !enabled {
		fg = s.Outline
	}
	e := ui.ButtonBase(c).Absolute().Left(x).Top(y).Width(w).Height(h).
		Justify(ui.Center).AlignItems(ui.Center).Radius(10).Label(label).Cursor(ui.CursorPointer)
	if !enabled {
		e.Disabled(true)
	}
	hoverLayer(&e, ui.Transparent, s.OnSurface, enabled)
	if enabled && onClick != nil && e.Clicked() {
		onClick()
	}
	e.Children(func() {
		if plus {
			if svg := Symbol("add"); svg != nil {
				ui.Icon(c, svg).FontSize(16).TextColor(Col(fg))
			}
			return
		}
		ui.Box(c).Width(w).Height(h).Draw(func(pc *ui.Painter, r ui.Rect) {
			var path ui.Path
			path.MoveTo(r.X+r.W/2-6, r.Y+r.H/2)
			path.LineTo(r.X+r.W/2+6, r.Y+r.H/2)
			pc.StrokePath(&path, 2, Col(fg))
		})
	})
}

// rowIcon is workflow.rs row_icon: an 18px glyph on a 10px-radius hit box.
func rowIcon(c *ui.Context, p *Palette, x, y, w, h float32, icon, label string, enabled bool, onClick func()) {
	s := &p.Scheme
	fg := uint32(s.OnSurfaceVariant)
	if !enabled {
		fg = s.Outline
	}
	e := ui.ButtonBase(c).Absolute().Left(x).Top(y).Width(w).Height(h).
		Justify(ui.Center).AlignItems(ui.Center).Radius(10).Label(label).Cursor(ui.CursorPointer)
	if !enabled {
		e.Disabled(true)
	}
	hoverLayer(&e, ui.Transparent, s.OnSurfaceVariant, enabled)
	if enabled && onClick != nil && e.Clicked() {
		onClick()
	}
	e.Children(func() {
		if svg := Symbol(icon); svg != nil {
			ui.Icon(c, svg).FontSize(18).TextColor(Col(fg))
		}
	})
}

// rowChevron is workflow.rs row_chevron: a stroked 2px round-cap chevron.
func rowChevron(c *ui.Context, p *Palette, x, y, w, h float32, up bool, label string, enabled bool, onClick func()) {
	s := &p.Scheme
	fg := uint32(s.OnSurfaceVariant)
	if !enabled {
		fg = s.Outline
	}
	e := ui.ButtonBase(c).Absolute().Left(x).Top(y).Width(w).Height(h).
		Radius(8).Label(label).Cursor(ui.CursorPointer)
	if !enabled {
		e.Disabled(true)
	}
	hoverLayer(&e, ui.Transparent, s.OnSurfaceVariant, enabled)
	if enabled && onClick != nil && e.Clicked() {
		onClick()
	}
	e.Children(func() {
		ui.Box(c).Width(w).Height(h).Draw(func(pc *ui.Painter, r ui.Rect) {
			cx, cy := r.X+r.W/2, r.Y+r.H/2
			var path ui.Path
			if up {
				path.MoveTo(cx-4.5, cy+3)
				path.LineTo(cx, cy-3)
				path.LineTo(cx+4.5, cy+3)
			} else {
				path.MoveTo(cx-4.5, cy-3)
				path.LineTo(cx, cy+3)
				path.LineTo(cx+4.5, cy-3)
			}
			pc.StrokePath(&path, 2, Col(fg))
		})
	})
}

// startWorkflow is Model::start_workflow (workflow.rs start_payload).
func (a *App) startWorkflow() {
	stages, err := wfStagePayload(workflowUI.stages)
	if err != nil {
		focusMsg = err.Error()
		return
	}
	name := strings.TrimSpace(workflowUI.templateName)
	if name == "" {
		name = "流程编排"
	} else if r := []rune(name); len(r) > 60 {
		name = string(r[:60])
	}
	payload := map[string]any{"mode": "workflow", "name": name, "stages": stages}
	if workflowUI.templateID != "" {
		payload["templateId"] = workflowUI.templateID
	}
	a.sendTimerCreate(payload)
}

// wfEditCopy is workflow.rs copy_stages_from_timer + Model::workflow_edit_copy.
func (a *App) wfEditCopy(id string) {
	var t *goTimer
	for i := range a.timerItems() {
		if a.timerItems()[i].ID == id {
			t = &a.timerItems()[i]
		}
	}
	if t == nil || t.stage() == nil {
		focusMsg = "这个计时器没有可复制的流程"
		return
	}
	stages := make([]wfStageDraft, 0, len(t.Workflow.Stages))
	for index := t.Workflow.StageIndex; index < len(t.Workflow.Stages); index++ {
		st := t.Workflow.Stages[index]
		minutes := (st.Seconds + 59) / 60
		if index == t.Workflow.StageIndex {
			minutes = (t.remainingMs() + 59999) / 60000
		}
		if minutes < 1 {
			minutes = 1
		}
		if minutes > wfStageMaxMin {
			minutes = wfStageMaxMin
		}
		stages = append(stages, wfStageDraft{st.Name, int(minutes)})
	}
	if len(stages) < wfStageMin {
		stages = append(stages, wfStageDraft{"休息", 5})
	}
	workflowUI.stages = stages
	workflowUI.templateID = ""
	workflowUI.templateName = ""
	focusUI.selected = ""
	focusUI.mode = 3
}

// editorFull is workflow.rs editor_full.
func (a *App) editorFull(c *ui.Context, p *Palette, lx, ly, w float32) {
	s := &p.Scheme
	stages := workflowUI.stages
	library := a.modules()
	wide := w >= 860
	heroH := float32(196)
	if wide {
		heroH = 132
	}
	Panel(c, p, lx, ly, w, heroH, s.PrimaryContainer)
	Label(c, "总时长", lx+32, ly+18, 12, s.OnPrimaryContainer, true, w-560)
	clock := formatTimerTime(int64(wfTotalMinutes(stages)) * 60)
	clockSize := float32(42)
	if wide {
		clockSize = 46
	}
	Label(c, clock, lx+32, ly+36, clockSize, s.OnPrimaryContainer, true, 260)
	label := wfSummary(stages)
	if strings.TrimSpace(workflowUI.templateName) != "" {
		label = fmt.Sprintf("%s · 模板「%s」", label, workflowUI.templateName)
	}
	sumY := ly + float32(82)
	if wide {
		sumY = ly + 96
	}
	Label(c, label, lx+34, sumY, 12, s.OnPrimaryContainer, false, w-72)

	startEnabled := len(stages) >= wfStageMin
	addEnabled := len(stages) < wfStageMax
	if wide {
		cy := ly + heroH/2 - 22
		gap := float32(8)
		rx := lx + w - 28 - 44
		IconButton(c, p, "重置编排", rx, cy, 44, 44, "restart_alt", false, true, a.wfDraftReset)
		rx -= gap + 108
		startX := rx
		rx -= gap + 108
		addX := rx
		rx -= gap + 108
		saveX := rx
		rx -= gap + 96
		libX := rx
		heroButton(c, p, "模板库", libX, cy, 96, 44, "", false, true, func() {
			focusDialog.kind = fdTemplates
		})
		heroButton(c, p, "保存模板", saveX, cy, 108, 44, "", false, true, a.openTemplateDialog)
		heroButton(c, p, "添加阶段", addX, cy, 108, 44, "add", false, addEnabled, func() {
			a.openStageDialog(-1)
		})
		heroButton(c, p, "开始流程", startX, cy, 108, 44, "play_arrow", true, startEnabled, a.startWorkflow)
	} else {
		cy := ly + heroH - 66
		gap := float32(8)
		start := float32(w * 0.30)
		if start < 142 {
			start = 142
		}
		if start > 190 {
			start = 190
		}
		bw := (w - 56 - start - 4*gap) / 4
		heroButton(c, p, "开始流程", lx+28, cy, start, 44, "play_arrow", true, startEnabled, a.startWorkflow)
		heroButton(c, p, "添加阶段", lx+28+start+gap, cy, bw, 44, "", false, addEnabled, func() {
			a.openStageDialog(-1)
		})
		heroButton(c, p, "保存模板", lx+28+start+2*gap+bw, cy, bw, 44, "", false, true, a.openTemplateDialog)
		heroButton(c, p, "模板库", lx+28+start+3*gap+2*bw, cy, bw, 44, "", false, true, func() {
			focusDialog.kind = fdTemplates
		})
		IconButton(c, p, "重置编排", lx+w-28-44, cy, 44, 44, "restart_alt", false, true, a.wfDraftReset)
	}

	cy := ly + heroH + 18
	Label(c, "流程阶段", lx, cy, 16, s.OnSurface, true, 120)
	Label(c, fmt.Sprintf("%d / %d", len(stages), wfStageMax), lx+88, cy+2, 12, s.OnSurfaceVariant, false, 86)
	Button(c, p, "管理模块库", lx+w-152, cy-8, 152, 38, "管理模块库", "settings", true, true, func() {
		focusDialog.kind = fdModules
	})
	Label(c, "点阶段名称可改名称与时长", lx+186, cy+2, 11, s.OnSurfaceVariant, false, w-350)
	cy += 30
	for i, stage := range stages {
		a.stageRowFull(c, p, i, stage, len(stages), lx, cy, w)
		cy += 72
	}
	cy += 6
	Label(c, "快速添加", lx, cy, 15, s.OnSurface, true, 100)
	Label(c, "点一下追加到流程末尾", lx+82, cy+2, 11, s.OnSurfaceVariant, false, 180)
	cy += 26
	cols := 3
	if w >= 880 {
		cols = 4
	}
	gap := float32(8)
	cw := (w - gap*float32(cols-1)) / float32(cols)
	for i, module := range library {
		cx := lx + float32(i%cols)*(cw+gap)
		ry := cy + float32(i/cols)*44
		quickChip(c, p, fmt.Sprintf("%s · %d 分", module.Name, module.Minutes), cx, ry, cw, addEnabled, func(m goModule) func() {
			return func() {
				if err := wfAddStage(wfStageDraft{m.Name, m.Minutes}); err != nil {
					focusMsg = err.Error()
				}
			}
		}(module))
	}
	custom := len(library)
	quickChip(c, p, "自定义阶段", lx+float32(custom%cols)*(cw+gap),
		cy+float32(custom/cols)*44, cw, addEnabled, func() { a.openStageDialog(-1) })
}

// stageRowFull is workflow.rs stage_row_full (ROW_H = 64).
func (a *App) stageRowFull(c *ui.Context, p *Palette, index int, stage wfStageDraft, count int, lx, ly, w float32) {
	s := &p.Scheme
	ui.Box(c).Absolute().Left(lx).Top(ly).Width(w).Height(64).
		Radius(20).Background(Col(s.Surface))
	ui.Box(c).Absolute().Left(lx + 20).Top(ly + 14).Width(36).Height(36).
		Radius(18).Background(Col(s.SecondaryContainer))
	CenteredText(c, fmt.Sprintf("%d", index+1), lx+20, ly+14, 36, 36, 13, s.OnSecondaryContainer, true)
	if index+1 < count {
		ui.Box(c).Absolute().Left(lx + 37.3).Top(ly + 52).Width(1.4).Height(20).
			Background(Col(s.Outline))
	}
	name := strings.TrimSpace(stage.Name)
	if name == "" {
		name = fmt.Sprintf("阶段 %d", index+1)
	}
	editW := w - 296 - 68 - 12
	if editW < 120 {
		editW = 120
	}
	edit := ui.ButtonBase(c).Absolute().Left(lx + 60).Top(ly + 4).Width(editW + 8).Height(56).
		Label("编辑阶段").Cursor(ui.CursorPointer)
	if edit.Clicked() {
		a.openStageDialog(index)
	}
	Label(c, name, lx+68, ly+11, 16, s.OnSurface, true, editW)
	Label(c, fmt.Sprintf("%d 分钟", stage.Minutes), lx+68, ly+37, 12, s.OnSurfaceVariant, false, editW)
	pillX := lx + w - 296
	ui.Box(c).Absolute().Left(pillX).Top(ly + 14).Width(176).Height(36).
		Radius(18).Background(Col(s.High))
	rowStep(c, p, pillX+4, ly+16, 32, 32, false, "减少 5 分钟", stage.Minutes > 1, func() {
		if err := wfNudgeStage(index, -wfStageStep); err != nil {
			focusMsg = err.Error()
		}
	})
	CenteredText(c, fmt.Sprintf("%d 分钟", stage.Minutes), pillX+38, ly+16, 100, 32, 12.5, s.OnSurface, true)
	rowStep(c, p, pillX+176-36, ly+16, 32, 32, true, "增加 5 分钟", stage.Minutes < wfStageMaxMin, func() {
		if err := wfNudgeStage(index, wfStageStep); err != nil {
			focusMsg = err.Error()
		}
	})
	rowChevron(c, p, lx+w-108, ly+8, 28, 24, true, "上移阶段", index > 0, func() {
		if err := wfMoveStage(index, -1); err != nil {
			focusMsg = err.Error()
		}
	})
	rowIcon(c, p, lx+w-44, ly+16, 32, 32, "delete", "删除阶段", count > wfStageMin, func() {
		if err := wfDeleteStage(index); err != nil {
			focusMsg = err.Error()
		}
	})
	rowChevron(c, p, lx+w-108, ly+32, 28, 24, false, "下移阶段", index+1 < count, func() {
		if err := wfMoveStage(index, 1); err != nil {
			focusMsg = err.Error()
		}
	})
}

// runningFull is workflow.rs running_full (392px panel).
func (a *App) runningFull(c *ui.Context, p *Palette, t *goTimer, lx, ly, w float32) {
	s := &p.Scheme
	count, index, waiting := 0, 0, false
	stageName := "阶段"
	if wf := t.Workflow; wf != nil {
		count = len(wf.Stages)
		index = wf.StageIndex
		waiting = wf.Waiting
		if st := t.stage(); st != nil {
			stageName = st.Name
		}
	}
	Panel(c, p, lx, ly, w, 392, s.PrimaryContainer)
	CenteredText(c, "流程进行中", lx+24, ly+20, w-48, 24, 13, s.OnPrimaryContainer, true)
	CenteredText(c, stageName, lx+24, ly+48, w-48, 40, 30, s.OnPrimaryContainer, true)
	CenteredText(c, formatTimerTime((t.remainingMs()+999)/1000), lx+24, ly+94, w-48, 80, 64, s.OnPrimaryContainer, true)
	CenteredText(c, fmt.Sprintf("第 %d / %d 阶段 · 已过 %s", index+1, count,
		formatTimerTime(t.ElapsedMs/1000)), lx+24, ly+178, w-48, 22, 13, s.OnPrimaryContainer, false)
	ui.Box(c).Absolute().Left(lx + 40).Top(ly + 210).Width(w - 80).Height(8).
		Radius(4).Background(Col(s.SecondaryContainer))
	fill := float32(t.progress()) * (w - 80)
	if fill < 0.001 {
		fill = 0.001
	}
	ui.Box(c).Absolute().Left(lx + 40).Top(ly + 210).Width(fill).Height(8).
		Radius(4).Background(Col(s.Primary))
	ui.Box(c).Absolute().Left(lx + 40).Top(ly + 232).Width(w - 80).Height(44).
		Radius(22).Background(Col(s.SecondaryContainer))
	text := "已暂停，继续后重新开始计时。"
	switch {
	case t.Finished:
		text = "流程已结束，可以重新开始或编辑副本。"
	case waiting:
		text = "本阶段完成，准备进入下一阶段。"
	case t.Running:
		text = "正在专注，阶段完成后由你点「下一阶段」。"
	}
	CenteredText(c, text, lx+40, ly+232, w-80, 44, 12.5, s.OnSecondaryContainer, true)
	active := !t.Finished
	gap := float32(8)
	top := ly + 292
	bw := (w - 80 - gap) / 2
	primaryLabel := map[string]string{"pause": "暂停", "resume": "继续", "next": "进入下一阶段"}[t.primaryAction()]
	if primaryLabel == "" {
		primaryLabel = "重新开始"
	}
	primaryIcon := "play_arrow"
	if t.Running {
		primaryIcon = "pause"
	}
	Button(c, p, primaryLabel, lx+40, top, bw, 44, primaryLabel, primaryIcon, false, active, func() {
		a.timerAction(t.ID, t.primaryAction())
	})
	heroButton(c, p, "跳过本阶段", lx+40+bw+gap, top, bw, 44, "", false, active, func() {
		a.timerAction(t.ID, "skip")
	})
	top2 := top + 52
	bw4 := (w - 80 - 3*gap) / 4
	heroButton(c, p, "延长 5 分钟", lx+40, top2, bw4, 44, "", false, active, func() {
		a.timerAction(t.ID, "extend5")
	})
	heroButton(c, p, "结束流程", lx+40+bw4+gap, top2, bw4, 44, "", false, active, func() {
		a.timerAction(t.ID, "end")
	})
	heroButton(c, p, "编辑副本", lx+40+2*(bw4+gap), top2, bw4, 44, "", false, true, func() {
		a.wfEditCopy(t.ID)
	})
	heroButton(c, p, "回到编排", lx+40+3*(bw4+gap), top2, bw4, 44, "", false, true, func() {
		focusUI.mode = 3 // Action::Mode(3) also clears the selection
		focusUI.selected = ""
	})
}

// wfDraftReset is Model::workflow_draft_reset.
func (a *App) wfDraftReset() {
	workflowUI.stages = defaultWorkflowStages()
	workflowUI.templateID = ""
	workflowUI.templateName = ""
}

// ---- workflow dialogs ----

// openStageDialog is Model::workflow_open_stage; index < 0 appends.
func (a *App) openStageDialog(index int) {
	focusDialog.kind = fdStage
	focusDialog.sErr = ""
	focusDialog.sIndex = index
	if index >= 0 && index < len(workflowUI.stages) {
		focusDialog.sName = workflowUI.stages[index].Name
		focusDialog.sMins = fmt.Sprintf("%d", workflowUI.stages[index].Minutes)
	} else {
		focusDialog.sName = ""
		focusDialog.sMins = "25"
	}
}

// submitStageDialog is Model::submit_stage_dialog.
func (a *App) submitStageDialog() {
	if err := wfApplyStage(focusDialog.sIndex, focusDialog.sName, focusDialog.sMins); err != nil {
		focusDialog.sErr = err.Error()
		return
	}
	fdClose()
}

// openTemplateDialog is Model::workflow_open_save.
func (a *App) openTemplateDialog() {
	focusDialog.kind = fdTemplate
	focusDialog.tErr = ""
	focusDialog.tOverwrite = false
	name := workflowUI.templateName
	if strings.TrimSpace(name) == "" {
		name = "我的流程"
	}
	focusDialog.tName = name
}

// submitTemplateDialog is Model::submit_template_dialog, remote branch.
func (a *App) submitTemplateDialog() {
	name := strings.TrimSpace(focusDialog.tName)
	if name == "" {
		focusDialog.tErr = "请填写模板名称"
		return
	}
	if len([]rune(name)) > 60 {
		focusDialog.tErr = "模板名称不能超过 60 字"
		return
	}
	stages, err := wfStagePayload(workflowUI.stages)
	if err != nil {
		focusDialog.tErr = err.Error()
		return
	}
	overwrite := focusDialog.tOverwrite && workflowUI.templateID != ""
	payload := map[string]any{"name": name, "stages": stages}
	if overwrite {
		payload["id"] = workflowUI.templateID
	}
	a.command("workflowTemplateSave", payload)
	workflowUI.templateName = name
	fdClose()
}

// loadTemplate is workflow.rs load_template.
func (a *App) loadTemplate(id string) {
	for _, t := range a.templates() {
		if t.ID == id {
			stages := make([]wfStageDraft, 0, len(t.Stages))
			for _, st := range t.Stages {
				minutes := (st.Seconds + 59) / 60
				if minutes < 1 {
					minutes = 1
				}
				stages = append(stages, wfStageDraft{st.Name, int(minutes)})
			}
			workflowUI.stages = stages
			workflowUI.templateID = t.ID
			workflowUI.templateName = t.Name
			fdClose()
			return
		}
	}
}

// focusDialogs renders the timer/workflow dialogs (views.rs dialog sizes).
func (a *App) focusDialogs(c *ui.Context) {
	switch focusDialog.kind {
	case fdTimerPrefs:
		open := true
		a.dialogCard(c, &open, 560, 496, func(p *Palette) {
			s := &p.Scheme
			Label(c, "计时参数", 28, 24, 20, s.OnSurface, true, 450)
			IconButton(c, p, "取消", 560-68, 20, 44, 44, "close", false, true, func() { open = false })
			labels := [3]string{focusDialog.pLabel0, "番茄钟休息时长 · 秒", "番茄钟轮数 · 1–16"}
			values := [3]*string{&focusDialog.pSecs, &focusDialog.pRest, &focusDialog.pRounds}
			maxs := [3]int{6, 4, 2}
			for i := 0; i < 3; i++ {
				a.dialogField(c, p, 28, 88+float32(i)*84, 560-56, labels[i], values[i], maxs[i])
			}
			Chip(c, p, "番茄钟正向计时", 28, 356, 196, 38, "番茄钟正向计时", focusDialog.pUp, func() {
				focusDialog.pUp = !focusDialog.pUp
			})
			Label(c, "时长参数用于下一次新建计时。", 28, 404, 12, s.OnSurfaceVariant, false, 560-56)
			if focusDialog.pErr != "" {
				Label(c, focusDialog.pErr, 28, 496-104, 12, s.Error, true, 560-56)
			}
			Button(c, p, "取消计时参数", 560-248, 496-70, 96, 44, "取消", "", true, true, func() { open = false })
			Button(c, p, "保存计时参数", 560-140, 496-70, 112, 44, "保存", "", false, true, a.saveTimerPrefs)
		})
		if !open {
			fdClose()
		}
	case fdStage:
		open := true
		a.dialogCard(c, &open, 560, 360, func(p *Palette) {
			s := &p.Scheme
			title := "添加阶段"
			nameLabel := "阶段名称 · 留空自动命名"
			if focusDialog.sIndex >= 0 {
				title = "编辑阶段"
				nameLabel = "阶段名称"
			}
			Label(c, title, 28, 24, 20, s.OnSurface, true, 450)
			IconButton(c, p, "取消", 560-68, 20, 44, 44, "close", false, true, func() { open = false })
			a.dialogField(c, p, 28, 88, 560-56, nameLabel, &focusDialog.sName, 40)
			a.dialogField(c, p, 28, 172, 560-56, "分钟 · 1–240", &focusDialog.sMins, 3)
			if focusDialog.sErr != "" {
				Label(c, focusDialog.sErr, 28, 360-104, 12, s.Error, true, 560-56)
			}
			Button(c, p, "取消阶段", 560-248, 360-70, 96, 44, "取消", "", true, true, func() { open = false })
			Button(c, p, "保存阶段", 560-140, 360-70, 112, 44, "保存阶段", "", false, true, a.submitStageDialog)
		})
		if !open {
			fdClose()
		}
	case fdTemplate:
		open := true
		a.dialogCard(c, &open, 560, 360, func(p *Palette) {
			s := &p.Scheme
			Label(c, "保存流程模板", 28, 24, 20, s.OnSurface, true, 450)
			IconButton(c, p, "取消", 560-68, 20, 44, 44, "close", false, true, func() { open = false })
			a.dialogField(c, p, 28, 88, 560-56, "模板名称", &focusDialog.tName, 60)
			if workflowUI.templateID != "" {
				Chip(c, p, "覆盖已加载模板", 28, 176, 176, 40, "覆盖已加载模板", focusDialog.tOverwrite, func() {
					focusDialog.tOverwrite = !focusDialog.tOverwrite
				})
				hint := "不覆盖时，会另存为一个新模板。"
				if focusDialog.tOverwrite {
					hint = "保存后更新当前模板；正在运行的流程实例不受影响。"
				}
				Label(c, hint, 28, 226, 12, s.OnSurfaceVariant, false, 560-56)
			}
			if focusDialog.tErr != "" {
				Label(c, focusDialog.tErr, 28, 360-104, 12, s.Error, true, 560-56)
			}
			Button(c, p, "取消模板", 560-248, 360-70, 96, 44, "取消", "", true, true, func() { open = false })
			Button(c, p, "保存模板", 560-140, 360-70, 112, 44, "保存模板", "", false, true, a.submitTemplateDialog)
		})
		if !open {
			fdClose()
		}
	case fdTemplates:
		a.templatesDialog(c)
	case fdModules:
		a.modulesDialog(c)
	case fdModuleEdit:
		a.moduleEditDialog(c)
	}
}

// dialogCard is the shared 560-wide dialog surface (views.rs dialog panel).
func (a *App) dialogCard(c *ui.Context, open *bool, dw, dh float32, build func(p *Palette)) {
	p := &a.Palette
	ui.DialogBase(c, open, func(backdrop, panel ui.Element) {
		backdrop.Background(AlphaCol(0, 0.5))
		panel.Width(dw).Height(dh)
		overlayStyle(&panel, p)
		panel.Children(func() { build(p) })
	})
}

// dialogField is the 68px input field of views.rs dialog: high fill, radius 12,
// 11px primary label, 15px value input.
func (a *App) dialogField(c *ui.Context, p *Palette, x, y, w float32, label string, value *string, max int) {
	s := &p.Scheme
	field := ui.Box(c).Absolute().Left(x).Top(y).Width(w).Height(68)
	surfaceStyle(&field, p, s.Container, FieldRadius)
	input := ui.TextInputBase(c, value).Absolute().Left(x + 14).Top(y + 26).
		Width(w - 28).Height(36).Font(UIFont).FontSize(14).TextColor(Col(s.OnSurface)).MaxLength(max)
	// Built on every frame, transparent while unfocused: inserting this box on
	// focus shifted the identity of the panel's later children and dropped the
	// press already bound to the dialog's buttons (kit.go FieldOutline).
	FieldOutline(c, x, y, w, 68, 12, input.Focused(), s.Primary)
	Label(c, label, x+14, y+8, 12, s.OnSurfaceVariant, true, w-28)
}

// templatesDialog is views.rs templates_dialog.
func (a *App) templatesDialog(c *ui.Context) {
	p := &a.Palette
	s := &p.Scheme
	templates := a.templates()
	shown := len(templates)
	if shown > 16 {
		shown = 16
	}
	rows := (shown + 1) / 2
	_, wh := winSize(c)
	dh := 100 + float32(rows)*58 + 76
	if dh < 250 {
		dh = 250
	}
	if dh > wh-40 {
		dh = wh - 40
	}
	const dw = float32(620)
	open := true
	a.dialogCard(c, &open, dw, dh, func(p *Palette) {
		Label(c, "流程模板", 28, 24, 20, s.OnSurface, true, dw-110)
		IconButton(c, p, "关闭模板库", dw-68, 20, 44, 44, "close", false, true, func() { open = false })
		hint := "加载会复制成当前草稿；删除不影响正在运行的流程。"
		if len(templates) == 0 {
			hint = "还没有保存的模板。"
		}
		Label(c, hint, 28, 74, 13, s.OnSurfaceVariant, false, dw-56)
		if len(templates) == 0 {
			Label(c, "编排好阶段后，点「保存为模板」即可在这里复用。", 28, 126, 15, s.OnSurface, false, dw-56)
			return
		}
		cw := (dw - 64) / 2
		for i, t := range templates {
			if i >= 16 {
				break
			}
			cx := 28 + float32(i%2)*(cw+8)
			cy := 104 + float32(i/2)*58
			ui.Box(c).Absolute().Left(cx).Top(cy).Width(cw).Height(50).
				Radius(14).Background(Col(s.High))
			Label(c, t.Name, cx+14, cy+6, 15, s.OnSurface, true, cw-160)
			Label(c, wfSummaryOf(t.Stages), cx+14, cy+28, 11, s.OnSurfaceVariant, false, cw-160)
			Button(c, p, "加载", cx+cw-136, cy+7, 76, 36, "加载", "", true, true, func() {
				a.loadTemplate(t.ID)
			})
			IconButton(c, p, "删除模板", cx+cw-52, cy+7, 40, 36, "delete", false, true, func() {
				a.command("workflowTemplateDelete", t.ID)
			})
		}
		Button(c, p, "关闭模板", dw-140, dh-62, 112, 44, "关闭", "", true, true, func() { open = false })
	})
	if !open {
		fdClose()
	}
}

// wfSummaryOf formats a native template's stage list.
func wfSummaryOf(stages []goStage) string {
	total := int64(0)
	for _, st := range stages {
		total += st.Seconds
	}
	return fmt.Sprintf("共 %d 分钟 · %d 个阶段", (total+59)/60, len(stages))
}

// modulesDialog is tool_preferences.rs modules_dialog (640×480).
func (a *App) modulesDialog(c *ui.Context) {
	p := &a.Palette
	s := &p.Scheme
	const dw, dh = float32(640), float32(480)
	library := a.modules()
	pages := (len(library) + 7) / 8
	if pages < 1 {
		pages = 1
	}
	if focusUI.modulePage > pages-1 {
		focusUI.modulePage = pages - 1
	}
	page := focusUI.modulePage
	open := true
	a.dialogCard(c, &open, dw, dh, func(p *Palette) {
		Label(c, "流程模块库", 28, 24, 26, s.OnSurface, true, 280)
		Button(c, p, "新建模块", dw-248, 24, 152, 40, "新建模块", "add", false, len(library) < 24, func() {
			a.openModuleDialog(-1)
		})
		IconButton(c, p, "关闭模块库", dw-72, 20, 44, 44, "close", false, true, func() { open = false })
		Label(c, "编辑或删除不会改动已经创建的流程。", 28, 74, 13, s.OnSurfaceVariant, false, dw-56)
		cw := (dw - 64) / 2
		for index, module := range library {
			if index < page*8 || index >= page*8+8 {
				continue
			}
			cell := index - page*8
			cx := 28 + float32(cell%2)*(cw+8)
			cy := 112 + float32(cell/2)*64
			ui.Box(c).Absolute().Left(cx).Top(cy).Width(cw).Height(56).
				Radius(16).Background(Col(s.High))
			Label(c, module.Name, cx+14, cy+7, 15, s.OnSurface, true, cw-106)
			Label(c, fmt.Sprintf("%d 分钟", module.Minutes), cx+14, cy+31, 11, s.OnSurfaceVariant, false, cw-106)
			IconButton(c, p, "编辑模块", cx+cw-86, cy+8, 36, 40, "settings", false, true, func() {
				a.openModuleDialog(index)
			})
			IconButton(c, p, "删除模块", cx+cw-46, cy+8, 36, 40, "delete", false, true, func() {
				a.removeModule(index)
			})
		}
		if len(library) == 0 {
			CenteredText(c, "模块库为空，可以新建一个模块。", 28, 178, dw-56, 44, 16, s.OnSurfaceVariant, false)
		}
		Button(c, p, "上一页", 28, dh-68, 100, 40, "上一页", "", true, page > 0, func() {
			focusUI.modulePage = page - 1
		})
		CenteredText(c, fmt.Sprintf("%d / %d", page+1, pages), 140, dh-68, 88, 40, 13, s.OnSurfaceVariant, false)
		Button(c, p, "下一页", 240, dh-68, 100, 40, "下一页", "", true, page+1 < pages, func() {
			focusUI.modulePage = page + 1
		})
		Button(c, p, "完成模块库", dw-140, dh-68, 112, 40, "完成", "", true, true, func() { open = false })
	})
	if !open {
		fdClose()
	}
}

// openModuleDialog is tool_preferences.rs edit_module; index < 0 creates.
func (a *App) openModuleDialog(index int) {
	focusDialog.kind = fdModuleEdit
	focusDialog.mErr = ""
	focusDialog.mIndex = index
	focusDialog.mName = ""
	focusDialog.mMins = "10"
	if index >= 0 {
		if library := a.modules(); index < len(library) {
			focusDialog.mName = library[index].Name
			focusDialog.mMins = fmt.Sprintf("%d", library[index].Minutes)
		}
	}
}

// saveModule is tool_preferences.rs save_module; edits go through WorkflowModules.
func (a *App) saveModule() {
	name := strings.TrimSpace(focusDialog.mName)
	var mins int
	if _, err := fmt.Sscanf(strings.TrimSpace(focusDialog.mMins), "%d", &mins); err != nil {
		focusDialog.mErr = "时长需要填写整数分钟"
		return
	}
	if name == "" || mins < 1 || mins > 240 {
		focusDialog.mErr = "请填写模块名称和 1–240 分钟的时长"
		return
	}
	list := make([]map[string]any, 0, len(a.modules())+1)
	for _, m := range a.modules() {
		list = append(list, map[string]any{"id": m.ID, "name": m.Name, "minutes": m.Minutes, "kind": m.Kind})
	}
	if focusDialog.mIndex >= 0 {
		if focusDialog.mIndex >= len(list) {
			focusDialog.mErr = "模块不存在"
			return
		}
		list[focusDialog.mIndex]["name"] = name
		list[focusDialog.mIndex]["minutes"] = mins
	} else {
		if len(list) >= 24 {
			focusDialog.mErr = "模块库最多保存 24 个模块"
			return
		}
		list = append(list, map[string]any{
			"id": fmt.Sprintf("user-%d", nanoStamp()), "name": name, "minutes": mins, "kind": 5})
	}
	a.pushNative("WorkflowModules", list)
	fdClose()
}

// removeModule is tool_preferences.rs remove_module.
func (a *App) removeModule(index int) {
	list := make([]map[string]any, 0, len(a.modules()))
	for i, m := range a.modules() {
		if i != index {
			list = append(list, map[string]any{"id": m.ID, "name": m.Name, "minutes": m.Minutes, "kind": m.Kind})
		}
	}
	a.pushNative("WorkflowModules", list)
}

// moduleEditDialog is views.rs dialog for DialogKind::WorkflowModule (340px).
func (a *App) moduleEditDialog(c *ui.Context) {
	p := &a.Palette
	s := &p.Scheme
	open := true
	a.dialogCard(c, &open, 560, 340, func(p *Palette) {
		title := "新建模块"
		if focusDialog.mIndex >= 0 {
			title = "编辑模块"
		}
		Label(c, title, 28, 24, 20, s.OnSurface, true, 450)
		IconButton(c, p, "取消", 560-68, 20, 44, 44, "close", false, true, func() { open = false })
		a.dialogField(c, p, 28, 88, 560-56, "模块名称", &focusDialog.mName, 40)
		a.dialogField(c, p, 28, 172, 560-56, "时长 · 分钟", &focusDialog.mMins, 3)
		if focusDialog.mErr != "" {
			Label(c, focusDialog.mErr, 28, 340-104, 12, s.Error, true, 560-56)
		}
		Button(c, p, "取消模块", 560-248, 340-70, 96, 44, "取消", "", true, true, func() { open = false })
		Button(c, p, "保存模块", 560-140, 340-70, 112, 44, "保存", "", false, true, a.saveModule)
	})
	if !open {
		fdClose()
	}
}
