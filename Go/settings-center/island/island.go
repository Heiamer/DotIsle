// Package island is the Go port of the Rust settings transport (lyric-backend io.rs
// + dotisle-app remote.rs): the newline-delimited JSON named pipe, its retry rules and
// the island lifecycle. Mutations are never replayed once delivered; only read-only
// state requests may retry.
package island

import (
	"bufio"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"reflect"
	"strings"
	"sync"
	"syscall"
	"time"
	"unsafe"

	"golang.org/x/sys/windows"
)

const defaultPipe = "LyricIsland.Native.Settings.v1"

func pipeName() string {
	if test := os.Getenv("LYRICISLAND_TEST_PIPE"); test != "" {
		return test
	}
	return defaultPipe
}

func wakeName() string {
	return `Local\DotIsle.SettingsWake.` + pipeName()
}

// State is one snapshot of the island reply, kept raw plus the fields every page reads.
type State struct {
	Version   string
	ProcessID uint32
	Settings  map[string]any
	Raw       map[string]any
	Err       string
	Exited    bool // The connected native process exited; its UI host must stop too.
}

// Packet mirrors remote.rs Packet: the latest state plus the reply to a command.
type Packet struct {
	State *State
	Reply map[string]any
}

var (
	kernel32          = windows.NewLazySystemDLL("kernel32.dll")
	procWaitNamedPipe = kernel32.NewProc("WaitNamedPipeW")
)

// waitNamedPipe returns true when the pipe instance can accept a client within ms.
func waitNamedPipe(name string, ms uint32) bool {
	p, err := windows.UTF16PtrFromString(name)
	if err != nil {
		return false
	}
	r, _, _ := procWaitNamedPipe.Call(uintptr(unsafe.Pointer(p)), uintptr(ms))
	return r != 0
}

// pipeReadTimeout bounds one reply read; a stalled island must not freeze the pump.
var pipeReadTimeout = 8 * time.Second

// PumpTrace appends one pump diagnostic line when DOTISLE_TRACE_UI is set.
func PumpTrace(format string, args ...any) {
	if os.Getenv("DOTISLE_TRACE_UI") == "" {
		return
	}
	f, err := os.OpenFile(os.Getenv("TEMP")+`\pump-trace.txt`, os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0o644)
	if err != nil {
		return
	}
	fmt.Fprintf(f, format+"\n", args...)
	_ = f.Close()
}

// openPipeOverlapped opens the pipe with FILE_FLAG_OVERLAPPED so os.File deadlines
// work: a plain os.OpenFile handle is non-overlapped on Windows and SetReadDeadline
// is silently unsupported there, which let a silent server block ReadBytes forever.
func openPipeOverlapped(name string) (*os.File, error) {
	p, err := windows.UTF16PtrFromString(name)
	if err != nil {
		return nil, err
	}
	h, err := windows.CreateFile(p, windows.GENERIC_READ|windows.GENERIC_WRITE, 0, nil,
		windows.OPEN_EXISTING, windows.FILE_FLAG_OVERLAPPED, 0)
	if err != nil {
		return nil, err
	}
	return os.NewFile(uintptr(h), name), nil
}

// request performs one pipe round trip with the Rust client's retry rules.
func request(command string, value any) (map[string]any, error) {
	readonly := command == "state"
	attempts := 2
	if readonly {
		attempts = 4
	}
	var last error
	for round := 0; round < attempts; round++ {
		if round > 0 {
			time.Sleep(120 * time.Millisecond)
		}
		out, delivered, err := tryPipe(command, value)
		if err == nil {
			return out, nil
		}
		last = err
		// A failed write means nothing was delivered: retrying even a mutation is safe.
		// After delivery only read-only commands may retry.
		if delivered && !readonly {
			return nil, err
		}
	}
	return nil, last
}

// tryPipe connects, writes one request line and reads one reply line. delivered
// reports whether the request reached the server in full.
func tryPipe(command string, value any) (map[string]any, bool, error) {
	name := `\\.\pipe\` + pipeName()
	deadline := time.Now().Add(2 * time.Second)
	var file *os.File
	for {
		if waitNamedPipe(name, 100) {
			f, err := openPipeOverlapped(name)
			if err == nil {
				file = f
				break
			}
			// Errors other than the transient busy set (2/231/535) are fatal.
			if !errors.Is(err, windows.ERROR_FILE_NOT_FOUND) &&
				!errors.Is(err, windows.ERROR_PIPE_BUSY) &&
				!errors.Is(err, windows.ERROR_PIPE_CONNECTED) {
				return nil, false, err
			}
		}
		if time.Now().After(deadline) {
			return nil, false, fmt.Errorf("灵动岛未连接，请先启动主程序。")
		}
		time.Sleep(15 * time.Millisecond)
	}
	defer file.Close()
	req := map[string]any{"version": 1, "command": command, "value": value}
	line, err := json.Marshal(req)
	if err != nil {
		return nil, false, err
	}
	line = append(line, '\n')
	if err := file.SetDeadline(time.Now().Add(pipeReadTimeout)); err != nil {
		return nil, false, err
	}
	n, err := file.Write(line)
	if err != nil {
		return nil, n > 0, err
	}
	delivered := true
	reader := bufio.NewReaderSize(file, 1<<16)
	response, err := reader.ReadBytes('\n')
	if err != nil {
		return nil, delivered, err
	}
	var out map[string]any
	if err := json.Unmarshal(response, &out); err != nil {
		return nil, delivered, err
	}
	return out, delivered, nil
}

// islandRunning mirrors io.rs island_running: probe the single-instance mutex.
func islandRunning() bool {
	test := os.Getenv("LYRICISLAND_TEST_PIPE")
	name := "Local.LyricIsland.Native.Vello.v3"
	if strings.HasPrefix(test, "LyricIsland.Settings.test.") {
		name = "Local.LyricIsland.Native.Test." + test
	}
	p, err := windows.UTF16PtrFromString(name)
	if err != nil {
		return false
	}
	h, err := windows.CreateMutex(nil, false, p)
	if err != nil {
		// The mutex already exists: the island holds it, so it is running.
		if err == windows.ERROR_ALREADY_EXISTS {
			windows.CloseHandle(h)
			return true
		}
		// CreateMutex fails with ACCESS_DENIED when another process owns it too.
		return err == windows.ERROR_ACCESS_DENIED
	}
	windows.CloseHandle(h)
	return false
}

func spawnIsland(data string) {
	exe, err := os.Executable()
	if err == nil {
		if dir := parentDir(exe); dir != "" {
			if _, err := os.Stat(dir + `\LyricIsland.exe`); err == nil {
				startIsland(dir+`\LyricIsland.exe`, data)
				return
			}
		}
	}
	// Dev-tree fallback. The shipped layout keeps Center.exe beside LyricIsland.exe
	// (checked first), but running from Go\settings-center the island lives under
	// DOTISLE_MODULE_DIR (Rust\target\release), which the e2e tooling always sets.
	if dir := os.Getenv("DOTISLE_MODULE_DIR"); dir != "" {
		if _, err := os.Stat(dir + `\LyricIsland.exe`); err == nil {
			startIsland(dir+`\LyricIsland.exe`, data)
		}
	}
}

func startIsland(path, data string) {
	cmd := exec.Command(path, "--from-settings", "--data-dir", data)
	cmd.SysProcAttr = &syscall.SysProcAttr{CreationFlags: 0x08000000} // CREATE_NO_WINDOW
	_ = cmd.Start()
}

func parentDir(path string) string {
	i := strings.LastIndexByte(path, '\\')
	if i <= 0 {
		return ""
	}
	return path[:i]
}

// DataDir resolves the island data directory like dotisle-app main.rs:
// --data-dir > LYRICISLAND_NATIVE_DATA > Portable.json/Installed.json > %APPDATA%.
func DataDir(flag string) string {
	if flag != "" {
		return flag
	}
	if env := os.Getenv("LYRICISLAND_NATIVE_DATA"); env != "" {
		return env
	}
	exe, err := os.Executable()
	if err == nil {
		dir := parentDir(exe)
		if dir != "" {
			if _, err := os.Stat(dir + `\Portable.json`); err == nil {
				return dir + `\Data`
			}
			if raw, err := os.ReadFile(dir + `\Installed.json`); err == nil {
				var installed struct {
					DataDirectory string `json:"dataDirectory"`
				}
				if json.Unmarshal(raw, &installed) == nil && installed.DataDirectory != "" {
					return installed.DataDirectory
				}
			}
		}
	}
	appdata := os.Getenv("APPDATA")
	if appdata == "" {
		appdata = `C:\Users\` + os.Getenv("USERNAME") + `\AppData\Roaming`
	}
	return appdata + `\DotIsle\V3`
}

// openWake opens (or creates) the auto-reset wake event shared with the island.
func openWake() windows.Handle {
	p, err := windows.UTF16PtrFromString(wakeName())
	if err != nil {
		return 0
	}
	h, err := windows.OpenEvent(windows.SYNCHRONIZE, false, p)
	if err == nil {
		return h
	}
	// The island may not have created it yet; create it so both sides share one.
	ch, cerr := windows.CreateEvent(nil, 0, 0, p)
	if cerr != nil {
		return 0
	}
	return ch
}

// Run connects to the island, keeps a fresh state flowing and reports packets on a
// channel across island restarts. Commands use Request separately from the state pump.
func Run(data string) <-chan *State {
	out := make(chan *State, 1)
	go worker(data, out)
	return out
}

func worker(data string, out chan<- *State) {
	defer close(out)
	PumpTrace("worker start data=%s", data)
	// Connect immediately. A missing island is started once, exactly like remote.rs.
	if _, err := request("state", nil); err != nil {
		PumpTrace("initial state err=%v running=%v", err, islandRunning())
		if islandRunning() {
			deadline := time.Now().Add(5 * time.Second)
			for time.Now().Before(deadline) {
				if _, err := request("state", nil); err == nil {
					break
				}
				time.Sleep(150 * time.Millisecond)
			}
		} else {
			spawnIsland(data)
		}
	}
	wake := openWake()
	if wake != 0 {
		defer windows.CloseHandle(wake)
	}
	var pid uint32
	var previous map[string]any
	lastError := ""
	for {
		state, err := request("state", nil)
		delay := 600 * time.Millisecond
		if err != nil {
			PumpTrace("state err=%v", err)
			if lastError != err.Error() {
				out <- &State{Err: err.Error()}
			}
			lastError = err.Error()
		} else {
			s := decode(state)
			pid = s.ProcessID
			mm, _ := state["moduleManagement"].(map[string]any)
			PumpTrace("state ok pid=%d phase=%v", pid, mm["phase"])
			delay = pollInterval(state)
			if lastError != "" || !sameSnapshot(previous, state) {
				out <- s
			}
			previous, lastError = state, ""
		}
		// Lyric frames and commands wake immediately. Idle snapshots need no
		// 90ms polling; playback keeps the original bound.
		if wake != 0 {
			windows.WaitForSingleObject(wake, uint32(delay/time.Millisecond))
		} else {
			time.Sleep(delay)
		}
		if pid != 0 {
			if !processAlive(pid) {
				PumpTrace("island pid %d exited; stopping its center and widgets", pid)
				out <- &State{Err: "主程序已退出。", Exited: true}
				return
			}
		}
	}
}

func decode(raw map[string]any) *State {
	s := &State{Raw: raw, Settings: map[string]any{}}
	if v, ok := raw["version"].(string); ok {
		s.Version = v
	}
	if v, ok := raw["processId"].(float64); ok {
		s.ProcessID = uint32(v)
	}
	if v, ok := raw["settings"].(map[string]any); ok {
		s.Settings = v
	}
	return s
}

func processAlive(pid uint32) bool {
	h, err := windows.OpenProcess(windows.SYNCHRONIZE, false, pid)
	if err != nil {
		return false
	}
	defer windows.CloseHandle(h)
	event, _ := windows.WaitForSingleObject(h, 0)
	// A process handle signals when the process exits: alive == WAIT_TIMEOUT.
	return event == uint32(windows.WAIT_TIMEOUT)
}

// Request exposes a one-shot command for callers outside the worker loop.
var commandMu sync.Mutex

func Request(command string, value any) (map[string]any, error) {
	commandMu.Lock()
	defer commandMu.Unlock()
	return request(command, value)
}

// Parsed snapshots are immutable. Only unused renderer counters are excluded;
// lyrics, commands, errors, settings and pending navigation always participate.
func sameSnapshot(previous, next map[string]any) bool {
	if previous == nil || next == nil {
		return false
	}
	visibleLen := func(m map[string]any) int {
		n := len(m)
		for _, k := range []string{"renderFrames", "glassCaptures"} {
			if _, ok := m[k]; ok {
				n--
			}
		}
		return n
	}
	if visibleLen(previous) != visibleLen(next) {
		return false
	}
	for key, value := range next {
		if key == "renderFrames" || key == "glassCaptures" {
			continue
		}
		old, ok := previous[key]
		if !ok || !reflect.DeepEqual(old, value) {
			return false
		}
	}
	return true
}

func pollInterval(state map[string]any) time.Duration {
	if playing, _ := state["isPlaying"].(bool); playing {
		return 90 * time.Millisecond
	}
	if timers, ok := state["timers"].([]any); ok {
		for _, raw := range timers {
			if item, ok := raw.(map[string]any); ok && item["running"] == true {
				return 150 * time.Millisecond
			}
		}
	}
	return 600 * time.Millisecond
}
