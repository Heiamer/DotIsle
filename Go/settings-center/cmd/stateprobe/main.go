// stateprobe dumps the parts of the island state answer that the motion page
// needs: the nested Motion object, whether stale flat motion keys exist, and
// one write path per side (update MotionDuration, todoSave).
package main

import (
	"encoding/json"
	"fmt"
	"os"
	"time"

	"dotisle/settings/island"
)

func main() {
	state, err := island.Request("state", nil)
	if err != nil {
		fmt.Println("STATE ERR:", err)
		os.Exit(1)
	}
	settings, _ := state["settings"].(map[string]any)
	dump := map[string]any{
		"processId":          state["processId"],
		"success":            state["success"],
		"settingsDark":       state["settingsDark"],
		"islandVisible":      state["islandVisible"],
		"Motion":             settings["Motion"],
		"flatMotionDuration": settings["MotionDuration"],
		"flatSpringMass":     settings["SpringMass"],
		"MotionDuration":     settings["MotionDuration"],
	}
	out, _ := json.MarshalIndent(dump, "", "  ")
	fmt.Println("=== initial state ===")
	fmt.Println(string(out))

	// Write path 1: the flat wire key must land in the nested Motion object.
	if _, err := island.Request("update", map[string]any{"key": "MotionDuration", "value": 910}); err != nil {
		fmt.Println("UPDATE MotionDuration ERR:", err)
	} else {
		time.Sleep(300 * time.Millisecond)
		state, _ = island.Request("state", nil)
		settings, _ = state["settings"].(map[string]any)
		motion, _ := settings["Motion"].(map[string]any)
		fmt.Println("=== after update MotionDuration=910 ===")
		out, _ = json.MarshalIndent(motion, "", "  ")
		fmt.Println(string(out))
		island.Request("update", map[string]any{"key": "MotionDuration", "value": 850})
		time.Sleep(300 * time.Millisecond)
	}

	// Write path 2: todoSave round trip.
	payload := map[string]any{"id": "", "title": "stateprobe待办", "content": "", "date": "2030-01-01", "time": "09:00", "daily": false, "color": "#F3D79B"}
	reply, err := island.Request("todoSave", payload)
	if err != nil {
		fmt.Println("TODOSAVE ERR:", err)
	} else {
		out, _ = json.Marshal(reply)
		fmt.Println("=== todoSave reply ===")
		fmt.Println(string(out))
		time.Sleep(300 * time.Millisecond)
		state, _ = island.Request("state", nil)
		if todos, ok := state["todos"].([]any); ok {
			fmt.Println("=== todos count:", len(todos), "===")
		} else {
			fmt.Println("=== todos field type:", fmt.Sprintf("%T", state["todos"]), "===")
		}
	}
	os.Exit(0)
}
