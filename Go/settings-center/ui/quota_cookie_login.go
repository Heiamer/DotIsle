// SPDX-License-Identifier: Apache-2.0
// Isolated provider sign-ins, based on BrimDeck MiMoAccount/DoubaoQuotaAccount.
package ui

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strconv"
	"strings"
	"time"

	"golang.org/x/net/websocket"
)

type quotaCookieEdition struct {
	home, endpoint string
	doubao         bool
}

func quotaCookieProvider(kind string) quotaCookieEdition {
	switch kind {
	case "mimo":
		return quotaCookieEdition{"https://mimo-server-cn.xiaomimimo.com/api/user/xiaomi/me", "https://mimo-server-cn.xiaomimimo.com/api/user/xiaomi/subscription/self", false}
	case "doubao":
		return quotaCookieEdition{"https://www.doubao.com/chat/", "https://www.doubao.com/alice/commerce/sale/subscription/quota/summary/", true}
	case "dola":
		return quotaCookieEdition{"https://www.dola.com/chat/", "https://www.dola.com/alice/commerce/sale/subscription/overview/", true}
	}
	return quotaCookieEdition{}
}
func quotaChromium() string {
	for _, e := range []string{"ProgramFiles(x86)", "ProgramFiles", "LOCALAPPDATA"} {
		dir := os.Getenv(e)
		if !filepath.IsAbs(dir) {
			continue
		}
		for _, sub := range []string{`Microsoft\Edge\Application\msedge.exe`, `Google\Chrome\Application\chrome.exe`} {
			p := filepath.Join(dir, sub)
			if st, err := os.Stat(p); err == nil && st.Mode().IsRegular() {
				return p
			}
		}
	}
	return ""
}
func removeQuotaLoginProfile(root, path string) {
	// Only this random, direct temporary child created by MkdirTemp, never a
	// normal browser folder or an unvalidated path from DevTools.
	parent, err := filepath.EvalSymlinks(root)
	if err != nil {
		return
	}
	target, err := filepath.EvalSymlinks(path)
	if err != nil || !strings.EqualFold(filepath.Dir(target), parent) || !strings.HasPrefix(filepath.Base(target), "DotIsle-provider-signin-") {
		return
	}
	for i := 0; i < 8; i++ {
		if os.RemoveAll(target) == nil {
			return
		}
		time.Sleep(250 * time.Millisecond)
	}
}
func quotaDebugPort(profile string) int {
	p := filepath.Join(profile, "DevToolsActivePort")
	if st, err := os.Stat(p); err != nil || st.Size() > 1024 {
		return 0
	}
	b, err := os.ReadFile(p)
	if err != nil {
		return 0
	}
	line, _, ok := strings.Cut(string(b), "\n")
	if !ok {
		return 0
	}
	n, err := strconv.Atoi(strings.TrimSpace(line))
	if err != nil || n < 1 || n > 65535 {
		return 0
	}
	return n
}

type quotaLoginCookie struct{ Name, Value, Domain string }

func quotaCookieHeader(endpoint string, rows []quotaLoginCookie) (header, csrf string) {
	u, err := url.Parse(endpoint)
	if err != nil {
		return "", ""
	}
	host := u.Hostname()
	selected := map[string]quotaLoginCookie{}
	for _, c := range rows {
		domain := strings.TrimPrefix(c.Domain, ".")
		if domain == "" || (host != domain && !(strings.HasPrefix(c.Domain, ".") && strings.HasSuffix(host, "."+domain))) {
			continue
		}
		if !quotaLoginField(c.Name, 128) || strings.ContainsAny(c.Name, "=;") || !quotaLoginField(c.Value, 4096) || strings.Contains(c.Value, ";") {
			continue
		}
		if old, ok := selected[c.Name]; !ok || len(domain) > len(strings.TrimPrefix(old.Domain, ".")) {
			selected[c.Name] = c
		}
	}
	names := make([]string, 0, len(selected))
	for k := range selected {
		names = append(names, k)
	}
	slices.Sort(names)
	parts := make([]string, 0, len(names))
	for _, k := range names {
		parts = append(parts, k+"="+selected[k].Value)
	}
	csrf = selected["passport_csrf_token"].Value
	if csrf == "" {
		csrf = selected["passport_csrf_token_default"].Value
	}
	return strings.Join(parts, ";"), csrf
}
func quotaCookieProbe(ctx context.Context, e quotaCookieEdition, cookie, csrf string) bool {
	if len(cookie) > 5700 || len(cookie) == 0 {
		return false
	}
	if e.doubao {
		if !strings.Contains(";"+cookie, ";sessionid=") && !strings.Contains(";"+cookie, ";sessionid_ss=") && !strings.Contains(";"+cookie, ";sid_tt=") {
			return false
		}
		u, _ := url.Parse(e.home)
		var doc struct {
			Code *int
			Data map[string]json.RawMessage
		}
		return quotaLoginFetch(ctx, quotaLoginHTTP, http.MethodPost, e.endpoint, `{"product_line":"membership"}`, map[string]string{"Cookie": cookie, "Referer": e.home, "Origin": "https://" + u.Host, "x-csrf-token": csrf}, &doc) == nil &&
			(doc.Code == nil || *doc.Code == 0) && (doc.Data["member_info"] != nil || doc.Data["window_limit_section"] != nil || doc.Data["enterprise_window_limit_section"] != nil)
	}
	var doc struct {
		Code *int
		Data map[string]json.RawMessage
	}
	return quotaLoginFetch(ctx, quotaLoginHTTP, http.MethodGet, e.endpoint, "", map[string]string{"Cookie": cookie}, &doc) == nil && doc.Code != nil && *doc.Code == 0 && doc.Data != nil
}
func quotaCookiesThroughTab(ctx context.Context, port int, e quotaCookieEdition, origin *url.URL) []quotaLoginCookie {
	u, _ := url.Parse(e.home)
	authority := fmt.Sprintf("127.0.0.1:%d", port)
	local := &http.Client{Timeout: 2 * time.Second, Transport: &http.Transport{}, CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse }}
	defer local.CloseIdleConnections()
	var tabs []struct{ Type, URL, WebSocketDebuggerURL string }
	if quotaLoginFetch(ctx, local, http.MethodGet, "http://"+authority+"/json/list", "", nil, &tabs) != nil {
		return nil
	}
	for _, tab := range tabs {
		page, err := url.Parse(tab.URL)
		if err != nil || tab.Type != "page" || page.Scheme != "https" || page.Host != u.Host || page.User != nil {
			continue
		}
		target, err := url.Parse(tab.WebSocketDebuggerURL)
		if err != nil || target.Scheme != "ws" || target.Host != authority || target.User != nil || target.RawQuery != "" || target.Fragment != "" || !strings.HasPrefix(target.Path, "/devtools/page/") {
			continue
		}
		// A random .invalid HTTPS origin is explicitly permitted only in this
		// disposable profile. Ordinary page origins are never allowed.
		config := &websocket.Config{Location: target, Origin: origin, Version: websocket.ProtocolVersionHybi13}
		connection, err := config.DialContext(ctx)
		if err != nil {
			continue
		}
		rows := func() []quotaLoginCookie {
			defer connection.Close()
			connection.MaxPayloadBytes = 256 * 1024
			_ = connection.SetDeadline(time.Now().Add(2 * time.Second))
			command := map[string]any{"id": 1, "method": "Network.getCookies", "params": map[string]any{"urls": []string{e.endpoint}}}
			if websocket.JSON.Send(connection, command) != nil {
				return nil
			}
			for i := 0; i < 4; i++ {
				var msg struct {
					ID     int
					Result struct{ Cookies []quotaLoginCookie }
				}
				if websocket.JSON.Receive(connection, &msg) != nil {
					return nil
				}
				if msg.ID == 1 {
					return msg.Result.Cookies
				}
			}
			return nil
		}()
		if len(rows) > 0 {
			return rows
		}
	}
	return nil
}
func quotaCookieLogin(ctx context.Context, kind string, notify func()) (quotaSession, error) {
	browser := quotaChromium()
	if browser == "" {
		return quotaSession{}, errors.New("找不到 Microsoft Edge 或 Google Chrome。")
	}
	e := quotaCookieProvider(kind)
	if e.home == "" {
		return quotaSession{}, errQuotaLogin
	}
	root := os.TempDir()
	path, err := os.MkdirTemp(root, "DotIsle-provider-signin-")
	if err != nil {
		return quotaSession{}, errQuotaLogin
	}
	defer removeQuotaLoginProfile(root, path)
	origin := &url.URL{Scheme: "https", Host: strings.ToLower(filepath.Base(path)) + ".invalid"}
	cmd := exec.Command(browser, "--remote-allow-origins="+origin.String(), "--user-data-dir="+path, "--remote-debugging-address=127.0.0.1", "--remote-debugging-port=0", "--no-first-run", "--no-default-browser-check", "--disable-background-mode", "--new-window", e.home)
	if cmd.Start() != nil {
		return quotaSession{}, errQuotaLogin
	}
	done := make(chan struct{})
	go func() { _ = cmd.Wait(); close(done) }()
	defer func() {
		select {
		case <-done:
			return
		default:
		}
		_ = cmd.Process.Kill()
		<-done
	}()
	notify()
	limit := time.Now().Add(22 * time.Second)
	port := 0
	for port == 0 && time.Now().Before(limit) && quotaWait(ctx, 400*time.Millisecond) {
		select {
		case <-done:
			return quotaSession{}, errQuotaLogin
		default:
		}
		port = quotaDebugPort(path)
	}
	if port == 0 {
		return quotaSession{}, errQuotaLogin
	}
	var tried string
	for quotaWait(ctx, 1200*time.Millisecond) {
		select {
		case <-done:
			return quotaSession{}, errors.New("登录窗口已关闭，未保存登录。")
		default:
		}
		cookie, csrf := quotaCookieHeader(e.endpoint, quotaCookiesThroughTab(ctx, port, e, origin))
		if cookie == tried {
			continue
		}
		tried = cookie
		if quotaCookieProbe(ctx, e, cookie, csrf) {
			return quotaSession{Token: cookie}, nil
		}
	}
	return quotaSession{}, errQuotaLogin
}
