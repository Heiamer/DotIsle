// pipeprobe exercises the settings pipe contract the control center relies on:
// each check sends a real command through island.Request and reads the state back.
package main

import (
	"encoding/json"
	"fmt"
	"os"
	"time"

	"dotisle/settings/island"
)

func field(state map[string]any, path ...string) any {
	var cur any = state
	for i, key := range path {
		m, ok := cur.(map[string]any)
		if !ok {
			return nil
		}
		cur = m[key]
		if i == len(path)-1 {
			return cur
		}
	}
	return nil
}

func main() {
	pass, fail := 0, 0
	check := func(name string, ok bool, detail string) {
		if ok {
			pass++
			fmt.Printf("PASS  %s\n", name)
		} else {
			fail++
			fmt.Printf("FAIL  %s  %s\n", name, detail)
		}
	}

	// 1. The island answers state.
	state, err := island.Request("state", nil)
	check("state answers", err == nil && state["success"] == true, fmt.Sprint(err))
	if err != nil {
		os.Exit(1)
	}

	// 2. update SettingsTheme flips the dark flag in the reply settings.
	if _, err := island.Request("update", map[string]any{"key": "SettingsTheme", "value": "Dark"}); err != nil {
		check("update SettingsTheme", false, err.Error())
	} else {
		time.Sleep(120 * time.Millisecond)
		state, _ = island.Request("state", nil)
		check("SettingsTheme=Dark applied", state["settings"].(map[string]any)["SettingsTheme"] == "Dark", "")
		check("settingsDark follows", state["settingsDark"] == true, fmt.Sprint(state["settingsDark"]))
		island.Request("update", map[string]any{"key": "SettingsTheme", "value": "System"})
	}

	// 3. toggleVisibility flips the island window (model.rs Pref("island")).
	island.Request("toggleVisibility", nil)
	time.Sleep(700 * time.Millisecond)
	state, _ = island.Request("state", nil)
	afterHide := state["islandVisible"]
	island.Request("toggleVisibility", nil)
	time.Sleep(700 * time.Millisecond)
	state, _ = island.Request("state", nil)
	afterShow := state["islandVisible"]
	check("toggleVisibility flips island", afterHide != afterShow,
		fmt.Sprintf("hide=%v show=%v", afterHide, afterShow))

	// 4. OffsetMilliseconds round trip (music page ±100ms buttons).
	island.Request("update", map[string]any{"key": "OffsetMilliseconds", "value": -300})
	time.Sleep(120 * time.Millisecond)
	state, _ = island.Request("state", nil)
	check("OffsetMilliseconds applied", field(state, "settings", "OffsetMilliseconds") == float64(-300), fmt.Sprint(field(state, "settings", "OffsetMilliseconds")))
	island.Request("update", map[string]any{"key": "OffsetMilliseconds", "value": 0})

	// 5. todoSave creates a todo the todos list reports.
	payload := map[string]any{"id": "", "title": "管道探针待办", "content": "", "date": "2030-01-01", "time": "09:00", "daily": false, "color": "#F3D79B"}
	reply, err := island.Request("todoSave", payload)
	check("todoSave accepted", err == nil && reply["success"] == true, fmt.Sprint(err, reply))
	time.Sleep(120 * time.Millisecond)
	state, _ = island.Request("state", nil)
	found := false
	if todos, ok := state["todos"].([]any); ok {
		for _, t := range todos {
			if m, ok := t.(map[string]any); ok && m["title"] == "管道探针待办" {
				found = true
				if id, ok := m["id"].(string); ok {
					island.Request("todoDelete", map[string]any{"id": id})
				}
			}
		}
	}
	check("todo appears in state", found, "")

	// 6. timerCreate creates a timer; timerAction removes it.
	timer, err := island.Request("timerCreate", map[string]any{"mode": "pomodoro", "duration": 1500, "focus": 1500, "rest": 300, "rounds": 1})
	check("timerCreate accepted", err == nil && timer["success"] == true, fmt.Sprint(err, timer))
	var timerID string
	if v, ok := timer["id"].(string); ok {
		timerID = v
	}
	time.Sleep(120 * time.Millisecond)
	state, _ = island.Request("state", nil)
	if timers, ok := state["timers"].([]any); ok {
		check("timer appears in state", len(timers) > 0, fmt.Sprint(len(timers)))
		if timerID == "" {
			if t0, ok := timers[0].(map[string]any); ok {
				timerID, _ = t0["id"].(string)
			}
		}
	} else {
		check("timer appears in state", false, "no timers array")
	}
	if timerID != "" {
		// The action carries the timer's current revision; read it from state like
		// the UI does (model.rs timer_action_value).
		state, _ = island.Request("state", nil)
		revision := float64(0)
		if timers, ok := state["timers"].([]any); ok {
			for _, t := range timers {
				if m, ok := t.(map[string]any); ok && m["id"] == timerID {
					if r, ok := m["revision"].(float64); ok {
						revision = r
					}
				}
			}
		}
		island.Request("timerAction", map[string]any{"id": timerID, "action": "delete", "revision": revision})
		// Sweep any timers left by earlier probe runs, then verify by id absence.
		time.Sleep(120 * time.Millisecond)
		state, _ = island.Request("state", nil)
		if timers, ok := state["timers"].([]any); ok {
			for _, t := range timers {
				if m, ok := t.(map[string]any); ok {
					if id, ok := m["id"].(string); ok {
						rev, _ := m["revision"].(float64)
						island.Request("timerAction", map[string]any{"id": id, "action": "delete", "revision": rev})
					}
				}
			}
		}
		time.Sleep(200 * time.Millisecond)
		state, _ = island.Request("state", nil)
		gone := true
		if timers, ok := state["timers"].([]any); ok {
			for _, t := range timers {
				if m, ok := t.(map[string]any); ok && m["id"] == timerID {
					gone = false
				}
			}
		}
		check("timer removed", gone, "")
	}

	// 7. media toggle without a player: the island answers without crashing.
	_, err = island.Request("media", map[string]any{"action": "toggle"})
	check("media toggle answered", err == nil, fmt.Sprint(err))

	// 8. notificationPreview is fire-and-forget.
	_, err = island.Request("notificationPreview", nil)
	check("notificationPreview answered", err == nil, fmt.Sprint(err))

	raw, _ := json.Marshal(map[string]any{"pass": pass, "fail": fail})
	fmt.Println(string(raw))
	if fail > 0 {
		os.Exit(1)
	}
}
