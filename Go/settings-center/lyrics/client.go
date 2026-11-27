package lyrics

import (
	"bytes"
	"encoding/json"
	"errors"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"time"
)

type client struct {
	http *http.Client
}
type obj = map[string]any

var hosts = map[string]bool{
	"music.163.com": true,
	"c.y.qq.com":    true, "lrclib.net": true,
	"api.qishui.com": true, "beta-luna.douyin.com": true,
	"lyrics.kugou.com": true, "mobilecdn.kugou.com": true,
}

func allowed(u *url.URL) bool {
	h := strings.ToLower(u.Hostname())
	if u.Scheme == "http" && u.User == nil && (u.Port() == "" || u.Port() == "80") {
		return h == "mobilecdn.kugou.com" && u.Path == "/api/v3/search/song" || h == "lyrics.kugou.com" && (u.Path == "/search" || u.Path == "/download")
	}
	return u.Scheme == "https" && u.User == nil && (u.Port() == "" || u.Port() == "443") && hosts[h]
}

func newClient() *client {
	t := http.DefaultTransport.(*http.Transport).Clone()
	t.Proxy = nil // Match the original lyric-source transport.
	t.MaxIdleConns = 24
	t.MaxConnsPerHost = 8
	return &client{http: &http.Client{Transport: t, Timeout: 10 * time.Second, CheckRedirect: func(req *http.Request, via []*http.Request) error {
		if len(via) >= 3 || !allowed(req.URL) || !strings.EqualFold(req.URL.Hostname(), via[0].URL.Hostname()) {
			return http.ErrUseLastResponse
		}
		return nil
	}}}
}

func escape(s string) string { return strings.ReplaceAll(url.QueryEscape(s), "+", "%20") }
func number(v any) float64 {
	n, _ := strconv.ParseFloat(text(v), 64)
	if !(n >= 0 && n <= 86400000) {
		return 0
	}
	return n
}
func text(v any) string {
	switch v := v.(type) {
	case string:
		return v
	case json.Number:
		return v.String()
	}
	return ""
}
func object(v any) obj { m, _ := v.(map[string]any); return m }
func field(v any, keys ...string) any {
	for _, k := range keys {
		v = object(v)[k]
	}
	return v
}
func array(v any) []any { a, _ := v.([]any); return a }
func names(v any, keys ...string) []string {
	var out []string
	for _, a := range array(v) {
		for _, k := range keys {
			if n := text(field(a, k)); n != "" {
				out = append(out, n)
				break
			}
		}
	}
	return out
}
func pick(values ...any) any {
	for _, v := range values {
		if v != nil && v != "" {
			return v
		}
	}
	return nil
}
func numericID(id string) bool {
	if id == "" || len(id) > 24 {
		return false
	}
	for _, r := range id {
		if r < '0' || r > '9' {
			return false
		}
	}
	return true
}

func parseJSON(b []byte) (any, error) {
	if !json.Valid(b) {
		return nil, errors.New("invalid lyric JSON")
	}
	d := json.NewDecoder(bytes.NewReader(b))
	d.UseNumber()
	var v any
	err := d.Decode(&v)
	return v, err
}

const neteaseRef = "https://music.163.com/"
const qqRef = "https://y.qq.com/"
const sodaRef = "https://music.douyin.com/"
