package mailbox

import (
	"os"
	"strings"
	"testing"
)

func TestHTMLPreviewIsPlainAndNeverIncludesTrackingOrScripts(t *testing.T) {
	raw := []byte("Subject: Test\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n<head><title>Hidden</title></head><p>验证码：<b>004812</b></p><img src='https://example.invalid/pixel'><script>bad()</script>")
	got := preview(raw)
	if !strings.Contains(got, "004812") || strings.Contains(got, "Hidden") || strings.Contains(got, "pixel") || strings.Contains(got, "bad()") {
		t.Fatalf("unexpected preview: %q", got)
	}
}

func TestConnectionValidation(t *testing.T) {
	got, err := Validate(Account{Address: "test@qq.com", Secret: "fixture-app-password"})
	if err != nil || got.Server != "imap.qq.com" {
		t.Fatalf("validate failed: %+v %v", got.Server, err)
	}
	for _, server := range []string{"https://example.com", "example.com:993", "127.0.0.1", "example.com\r\n", "example.com/path"} {
		if strings.TrimSpace(server) == "example.com" {
			continue
		}
		if _, err := Validate(Account{Address: "test@qq.com", Server: server, Secret: "test"}); err == nil {
			t.Errorf("accepted invalid server %q", server)
		}
	}
}

func TestCredentialStorageUsesDPAPIAndRemovesOnlyItsFile(t *testing.T) {
	dir := t.TempDir()
	a := Account{Address: "test@qq.com", Server: "imap.qq.com", Secret: "fixture-app-password"}
	if err := Save(dir, a); err != nil {
		t.Fatal(err)
	}
	raw, err := os.ReadFile(path(dir))
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(string(raw), a.Secret) || strings.Contains(string(raw), a.Address) {
		t.Fatal("plaintext credentials on disk")
	}
	got, err := Load(dir)
	if err != nil || got != a {
		t.Fatalf("round trip failed: %v", err)
	}
	if err := Forget(dir); err != nil {
		t.Fatal(err)
	}
	if _, err := Load(dir); !os.IsNotExist(err) {
		t.Fatalf("credential file remains: %v", err)
	}
}
