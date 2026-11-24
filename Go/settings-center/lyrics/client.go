package lyrics

import (
	"bytes"
	"context"
	"crypto/aes"
	"crypto/md5"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"time"
)

const bodyLimit = 2 << 20

type client struct{ http *http.Client }
type obj = map[string]any

var hosts = map[string]bool{
	"interface.music.163.com": true, "music.163.com": true,
	"u.y.qq.com": true, "c.y.qq.com": true, "lrclib.net": true,
	"api.qishui.com": true, "beta-luna.douyin.com": true,
	"lyrics.kugou.com": true, "mobilecdn.kugou.com": true,
	"search.kuwo.cn": true, "m.kuwo.cn": true,
	"jadeite.migu.cn": true,
	"api.amll.dev":    true,
}

func miguHost(host string) bool {
	return host == "migu.cn" || host == "migumusic.cn" || strings.HasSuffix(host, ".migu.cn") || strings.HasSuffix(host, ".migumusic.cn")
}

func allowed(u *url.URL) bool {
	h := strings.ToLower(u.Hostname())
	return u.Scheme == "https" && u.User == nil && (u.Port() == "" || u.Port() == "443") && (hosts[h] || miguHost(h))
}

func newClient() *client {
	t := http.DefaultTransport.(*http.Transport).Clone()
	t.Proxy = nil // Match the original lyric-source transport.
	t.MaxIdleConns = 24
	t.MaxConnsPerHost = 8
	return &client{&http.Client{Transport: t, Timeout: 4 * time.Second, CheckRedirect: func(req *http.Request, via []*http.Request) error {
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

func (c *client) bytes(ctx context.Context, endpoint, contentType, referer string, body []byte, headers map[string]string) ([]byte, error) {
	u, err := url.Parse(endpoint)
	if err != nil || !allowed(u) {
		return nil, errors.New("invalid lyric endpoint")
	}
	method := http.MethodGet
	if body != nil {
		method = http.MethodPost
	}
	r, err := http.NewRequestWithContext(ctx, method, u.String(), bytes.NewReader(body))
	if err != nil {
		return nil, err
	}
	r.Header.Set("User-Agent", "DotIsle/5.1 (https://github.com/Heiamer/DotIsle)")
	if referer != "" {
		r.Header.Set("Referer", referer)
		if u.Hostname() != "lrclib.net" {
			r.Header.Set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
		}
	}
	if contentType != "" {
		r.Header.Set("Content-Type", contentType)
	}
	for k, v := range headers {
		r.Header.Set(k, v)
	}
	res, err := c.http.Do(r)
	if err != nil {
		if ctx.Err() != nil {
			return nil, ctx.Err()
		}
		return nil, fmt.Errorf("%s unavailable", u.Hostname())
	}
	defer res.Body.Close()
	if res.StatusCode != 200 {
		return nil, fmt.Errorf("%s HTTP %d", u.Hostname(), res.StatusCode)
	}
	b, err := io.ReadAll(io.LimitReader(res.Body, bodyLimit+1))
	if err != nil {
		return nil, err
	}
	if len(b) > bodyLimit || len(bytes.TrimSpace(b)) == 0 {
		return nil, errors.New("empty or oversized lyric response")
	}
	return b, nil
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

func (c *client) json(ctx context.Context, endpoint, typ, ref string, body []byte, headers map[string]string) (any, error) {
	b, err := c.bytes(ctx, endpoint, typ, ref, body, headers)
	if err != nil {
		return nil, err
	}
	return parseJSON(b)
}

const neteaseRef = "https://music.163.com/"
const qqRef = "https://y.qq.com/"
const sodaRef = "https://music.douyin.com/"
const form = "application/x-www-form-urlencoded"

func (c *client) netease(ctx context.Context, path string, data obj) (any, error) {
	b, _ := json.Marshal(data)
	h := md5.Sum([]byte("nobody" + path + "use" + string(b) + "md5forencrypt"))
	d := []byte(path + "-36cd479b6b5-" + string(b) + "-36cd479b6b5-" + hex.EncodeToString(h[:]))
	padding := aes.BlockSize - len(d)%aes.BlockSize
	d = append(d, bytes.Repeat([]byte{byte(padding)}, padding)...)
	cipher, _ := aes.NewCipher([]byte("e82ckenh8dichen8"))
	for offset := 0; offset < len(d); offset += aes.BlockSize {
		cipher.Encrypt(d[offset:], d[offset:])
	}
	v, err := c.json(ctx, "https://interface.music.163.com/e"+path[1:], form, neteaseRef, []byte("params="+strings.ToUpper(hex.EncodeToString(d))), nil)
	if err == nil && text(field(v, "code")) != "200" {
		err = errors.New("Netease rejected request")
	}
	return v, err
}
