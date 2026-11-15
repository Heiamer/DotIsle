//go:build windows

package island

import (
	"fmt"
	"os"
	"os/exec"
	"strings"
	"testing"
	"time"

	"golang.org/x/sys/windows"
)

// Regression: the alive check used to treat WAIT_OBJECT_0 (process exited) as
// alive, so the pump worker quit after its first snapshot and the UI froze.
func TestProcessAliveSelfAndDeadChild(t *testing.T) {
	if !processAlive(uint32(os.Getpid())) {
		t.Fatal("current process must be reported alive")
	}
	cmd := exec.Command("cmd", "/c", "exit 0")
	if err := cmd.Run(); err != nil {
		t.Fatal(err)
	}
	if processAlive(uint32(cmd.ProcessState.Pid())) {
		t.Fatal("exited child must be reported dead")
	}
}

func testPipeName(t *testing.T) string {
	t.Helper()
	name := fmt.Sprintf("LyricIsland.Native.Settings.test.islandgo-%d-%d", os.Getpid(), time.Now().UnixNano())
	t.Setenv("LYRICISLAND_TEST_PIPE", name)
	return `\\.\pipe\` + name
}

func createTestPipe(t *testing.T, name string) windows.Handle {
	t.Helper()
	p, err := windows.UTF16PtrFromString(name)
	if err != nil {
		t.Fatal(err)
	}
	h, err := windows.CreateNamedPipe(p, windows.PIPE_ACCESS_DUPLEX,
		windows.PIPE_TYPE_BYTE|windows.PIPE_READMODE_BYTE|windows.PIPE_WAIT, 1, 4096, 4096, 0, nil)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { windows.CloseHandle(h) })
	return h
}

// Regression: a non-overlapped pipe handle ignores SetReadDeadline on Windows, so a
// server that accepts but never replies used to block the pump forever.
func TestSilentPipeServerReadTimesOut(t *testing.T) {
	name := testPipeName(t)
	h := createTestPipe(t, name)
	go func() {
		_ = windows.ConnectNamedPipe(h, nil)
		time.Sleep(2 * time.Second)
	}()
	old := pipeReadTimeout
	pipeReadTimeout = 300 * time.Millisecond
	defer func() { pipeReadTimeout = old }()
	start := time.Now()
	_, delivered, err := tryPipe("state", nil)
	if err == nil {
		t.Fatal("expected a timeout error from the silent server")
	}
	if !delivered {
		t.Fatal("the request line should have been delivered")
	}
	if time.Since(start) > 3*time.Second {
		t.Fatal("read blocked past the deadline")
	}
}

func TestEchoPipeRoundTripOverlapped(t *testing.T) {
	name := testPipeName(t)
	h := createTestPipe(t, name)
	go func() {
		if err := windows.ConnectNamedPipe(h, nil); err != nil && err != windows.ERROR_PIPE_CONNECTED {
			return
		}
		buf := make([]byte, 4096)
		var n uint32
		if windows.ReadFile(h, buf, &n, nil) != nil {
			return
		}
		resp := []byte("{\"success\":true,\"echo\":1}\n")
		var written uint32
		if windows.WriteFile(h, resp, &written, nil) != nil {
			return
		}
		_ = windows.FlushFileBuffers(h)
		_ = windows.DisconnectNamedPipe(h)
	}()
	out, delivered, err := tryPipe("state", nil)
	if err != nil || !delivered {
		t.Fatalf("round trip failed: delivered=%v err=%v", delivered, err)
	}
	if out["success"] != true {
		t.Fatalf("unexpected reply: %v", out)
	}
}

// The same deadline must cover writes: a connected server which stops reading
// can otherwise block before the client ever reaches its bounded reply read.
func TestSilentPipeServerWriteTimesOut(t *testing.T) {
	name := testPipeName(t)
	h := createTestPipe(t, name)
	go func() {
		_ = windows.ConnectNamedPipe(h, nil)
		time.Sleep(2 * time.Second)
	}()
	old := pipeReadTimeout
	pipeReadTimeout = 300 * time.Millisecond
	defer func() { pipeReadTimeout = old }()
	start := time.Now()
	_, _, err := tryPipe("update", map[string]any{"payload": strings.Repeat("x", 8<<20)})
	if !os.IsTimeout(err) {
		t.Fatalf("expected a write timeout, got %v", err)
	}
	if time.Since(start) > 3*time.Second {
		t.Fatal("write blocked past its deadline")
	}
}
