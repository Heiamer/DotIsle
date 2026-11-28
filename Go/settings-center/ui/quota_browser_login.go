// SPDX-License-Identifier: Apache-2.0
// Browser authorization flows adapted from BrimDeck WorkBuddyUsage.cs and QianwenUsage.cs.
package ui

import (
	"context"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"time"

	"golang.org/x/sys/windows"
)

var errQuotaLogin = errors.New("官网暂未完成授权，请重试。")
var quotaLoginHTTP = &http.Client{Timeout: 14 * time.Second, CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse }}

type quotaSession struct {
	Token   string `json:"token"`
	User    string `json:"user,omitempty"`
	Domain  string `json:"domain,omitempty"`
	Expires string `json:"expires,omitempty"`
}

func (s quotaSession) pack() (string, error) {
	if !quotaLoginField(s.Token, 5800) {
		return "", errQuotaLogin
	}
	for _, part := range []string{s.User, s.Domain} {
		if part != "" && !quotaLoginField(part, 200) {
			return "", errQuotaLogin
		}
	}
	b, err := json.Marshal(s)
	if err != nil {
		return "", errQuotaLogin
	}
	defer clear(b)
	secret := base64.RawURLEncoding.EncodeToString(b)
	if len(secret) > 8192 {
		return "", errQuotaLogin
	}
	return secret, nil
}
func quotaLoginField(s string, limit int) bool {
	if len(s) == 0 || len(s) > limit {
		return false
	}
	for _, c := range s {
		if c <= 32 || c >= 127 {
			return false
		}
	}
	return true
}
func quotaLoginFetch(ctx context.Context, client *http.Client, method, address, body string, headers map[string]string, out any) error {
	var payload io.Reader
	if body != "" {
		payload = strings.NewReader(body)
	}
	req, err := http.NewRequestWithContext(ctx, method, address, payload)
	if err != nil {
		return errQuotaLogin
	}
	req.Header.Set("Accept", "application/json")
	if payload != nil {
		req.Header.Set("Content-Type", "application/json")
	}
	for k, v := range headers {
		if v != "" {
			req.Header.Set(k, v)
		}
	}
	resp, err := client.Do(req)
	// Never propagate URL, OAuth parameters or transport errors into the UI.
	if err != nil {
		return errQuotaLogin
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK && resp.StatusCode != http.StatusBadRequest {
		return errQuotaLogin
	}
	const maxResponse = 256 * 1024
	b, err := io.ReadAll(io.LimitReader(resp.Body, maxResponse+1))
	defer clear(b)
	if err != nil || len(b) > maxResponse || json.Unmarshal(b, out) != nil {
		return errQuotaLogin
	}
	return nil
}
func quotaLoginURL(raw, expectedHost, expectedPath, state string) bool {
	u, err := url.Parse(raw)
	if err != nil || len(raw) > 4096 || u.Scheme != "https" || u.Host != expectedHost || u.User != nil || u.Opaque != "" || u.Fragment != "" {
		return false
	}
	if expectedPath != "" && u.EscapedPath() != expectedPath {
		return false
	}
	if state != "" {
		query, err := url.ParseQuery(u.RawQuery)
		if err != nil || len(query["state"]) != 1 || query.Get("state") != state {
			return false
		}
	}
	return true
}
func quotaOpenLogin(address string) error {
	value, err := windows.UTF16PtrFromString(address)
	if err != nil {
		return errQuotaLogin
	}
	verb, _ := windows.UTF16PtrFromString("open")
	if windows.ShellExecute(0, verb, value, nil, nil, 1) != nil {
		return errors.New("无法打开默认浏览器。")
	}
	return nil
}
func quotaWait(ctx context.Context, wait time.Duration) bool {
	t := time.NewTimer(wait)
	defer t.Stop()
	select {
	case <-t.C:
		return true
	case <-ctx.Done():
		return false
	}
}
func quotaBrowserLogin(ctx context.Context, kind string, open func(string) error, cookieNotice func()) (string, error) {
	var s quotaSession
	var err error
	switch kind {
	case "workbuddy", "workbuddy-ai":
		s, err = workBuddyLogin(ctx, quotaLoginHTTP, kind, open, 2*time.Second)
	case "qianwen", "qwencloud":
		s, err = qianwenLogin(ctx, quotaLoginHTTP, kind, open)
	case "mimo", "doubao", "dola":
		s, err = quotaCookieLogin(ctx, kind, cookieNotice)
	default:
		return "", errQuotaLogin
	}
	if err != nil {
		return "", err
	}
	return s.pack()
}
func workBuddyLogin(ctx context.Context, client *http.Client, kind string, open func(string) error, every time.Duration) (quotaSession, error) {
	var empty quotaSession
	host, platform := "www.workbuddy.cn", "dotisle"
	if kind == "workbuddy-ai" {
		host, platform = "www.workbuddy.ai", "cli"
	}
	base := "https://" + host
	var start struct {
		Code *int
		Data struct{ State, AuthURL string }
	}
	err := quotaLoginFetch(ctx, client, http.MethodPost, base+"/v2/plugin/auth/state?platform="+platform, "{}", map[string]string{"X-No-Authorization": "true", "X-No-User-Id": "true"}, &start)
	if err != nil || start.Code == nil || *start.Code != 0 || !quotaLoginField(start.Data.State, 256) || !quotaLoginURL(start.Data.AuthURL, host, "/login", start.Data.State) {
		return empty, errQuotaLogin
	}
	if err := open(start.Data.AuthURL); err != nil {
		return empty, err
	}
	for fails := 0; quotaWait(ctx, every); {
		var res struct {
			Code *int
			Data struct{ AccessToken, Domain string }
		}
		err := quotaLoginFetch(ctx, client, http.MethodGet, base+"/v2/plugin/auth/token?state="+url.QueryEscape(start.Data.State), "", map[string]string{"X-No-Authorization": "true"}, &res)
		if err != nil {
			fails++
			if fails >= 4 {
				break
			}
			continue
		}
		fails = 0
		if res.Code != nil && *res.Code == 11217 {
			continue
		}
		token, domain := res.Data.AccessToken, res.Data.Domain
		if res.Code == nil || *res.Code != 0 || !quotaLoginField(token, 5800) || domain != "" && !quotaLoginField(domain, 200) {
			return empty, errQuotaLogin
		}
		var accounts struct {
			Code *int
			Data struct {
				Accounts []struct {
					EnterpriseID, UID        json.RawMessage
					IsCurrentOneIDEnterprise bool
				}
			}
		}
		err = quotaLoginFetch(ctx, client, http.MethodGet, base+"/v2/plugin/accounts", "", map[string]string{"Authorization": "Bearer " + token, "X-Domain": domain}, &accounts)
		if err != nil || (accounts.Code != nil && *accounts.Code != 0) {
			return empty, errQuotaLogin
		}
		for _, a := range accounts.Data.Accounts {
			uid := quotaLoginID(a.UID)
			if !a.IsCurrentOneIDEnterprise && quotaLoginID(a.EnterpriseID) == "" && quotaLoginField(uid, 200) {
				return quotaSession{Token: token, User: uid, Domain: domain}, nil
			}
		}
		return empty, errors.New("请使用 WorkBuddy 个人账户授权。")
	}
	return empty, errQuotaLogin
}
func quotaLoginID(raw json.RawMessage) string {
	var s string
	if json.Unmarshal(raw, &s) == nil {
		return s
	}
	var n json.Number
	if json.Unmarshal(raw, &n) == nil {
		return n.String()
	}
	return ""
}
func qianwenLogin(ctx context.Context, client *http.Client, kind string, open func(string) error) (quotaSession, error) {
	var empty quotaSession
	domain := "qianwenai.com"
	if kind == "qwencloud" {
		domain = "qwencloud.com"
	}
	base := "https://t." + domain + "/cli/device/"
	var nonce [32]byte
	var id [16]byte
	if _, err := rand.Read(nonce[:]); err != nil {
		return empty, errQuotaLogin
	}
	if _, err := rand.Read(id[:]); err != nil {
		return empty, errQuotaLogin
	}
	id[6] = id[6]&0x0f | 0x40
	id[8] = id[8]&0x3f | 0x80
	cid := fmt.Sprintf("%s-%s-%s-%s-%s", hex.EncodeToString(id[:4]), hex.EncodeToString(id[4:6]), hex.EncodeToString(id[6:8]), hex.EncodeToString(id[8:10]), hex.EncodeToString(id[10:]))
	verifier := base64.RawURLEncoding.EncodeToString(nonce[:])
	clear(nonce[:])
	challenge := sha256.Sum256([]byte(verifier))
	query := url.Values{"client_id": {cid}, "code_challenge": {base64.RawURLEncoding.EncodeToString(challenge[:])}, "code_challenge_method": {"S256"}}
	var start struct {
		Success bool
		Data    struct {
			Token, VerificationURL string
			Interval, ExpiresIn    int
		}
	}
	err := quotaLoginFetch(ctx, client, http.MethodPost, base+"code?"+query.Encode(), "", nil, &start)
	if err != nil || !start.Success || !quotaLoginField(start.Data.Token, 800) || !quotaLoginURL(start.Data.VerificationURL, "account."+domain, "", "") {
		return empty, errQuotaLogin
	}
	if err := open(start.Data.VerificationURL); err != nil {
		return empty, err
	}
	interval, expires := start.Data.Interval, start.Data.ExpiresIn
	if interval < 1 {
		interval = 2
	}
	if expires < 1 {
		expires = 300
	}
	ctx, cancel := context.WithTimeout(ctx, time.Duration(min(max(expires, 1), 1800))*time.Second)
	defer cancel()
	every := time.Duration(min(interval, 30)) * time.Second
	query = url.Values{"client_id": {cid}, "token": {start.Data.Token}, "code_verifier": {verifier}}
	for failed := 0; quotaWait(ctx, every); {
		var res struct {
			Data struct {
				Status      string
				Credentials struct{ AccessToken, ExpireTime string }
			}
		}
		if quotaLoginFetch(ctx, client, http.MethodPost, base+"token?"+query.Encode(), "", nil, &res) != nil {
			failed++
			if failed >= 4 {
				break
			}
			continue
		}
		failed = 0
		switch strings.ToLower(res.Data.Status) {
		case "", "authorization_pending":
			continue
		case "slow_down":
			every = min(every+5*time.Second, 60*time.Second)
			continue
		case "complete":
			return quotaSession{Token: res.Data.Credentials.AccessToken, Expires: res.Data.Credentials.ExpireTime}, nil
		case "access_denied":
			return empty, errors.New("已取消官网授权。")
		default:
			return empty, errQuotaLogin
		}
	}
	return empty, errQuotaLogin
}
