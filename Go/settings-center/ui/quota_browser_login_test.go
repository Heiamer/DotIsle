package ui

import "testing"

func TestQuotaLoginRejectsMismatchedEditionAndState(t *testing.T) {
	if !quotaLoginURL("https://www.workbuddy.cn/login?state=paired", "www.workbuddy.cn", "/login", "paired") {
		t.Fatal("valid link rejected")
	}
	for _, u := range []string{
		"https://www.workbuddy.ai/login?state=paired", "https://www.workbuddy.cn.evil.test/login?state=paired", "https://www.workbuddy.cn/login?state=unrelated",
		"https://www.workbuddy.cn/login?state=paired&state=unrelated", "https://foo@www.workbuddy.cn/login?state=paired",
	} {
		if quotaLoginURL(u, "www.workbuddy.cn", "/login", "paired") {
			t.Fatalf("accepted unsafe URL: %s", u)
		}
	}
}

func TestQuotaCookieSessionOnlyRetainsSelectedEdition(t *testing.T) {
	rows := []quotaLoginCookie{{"sessionid", "bad-sibling", "www.dola.com"}, {"sessionid", "root", ".doubao.com"},
		{"sessionid", "site-value", "www.doubao.com"}, {"unrelated", "account-value", "login.doubao.com"},
		{"passport_csrf_token", "csrf-fixture", ".doubao.com"}, {"bad", "payload;other=x", ".doubao.com"}}
	cookie, csrf := quotaCookieHeader(quotaCookieProvider("doubao").endpoint, rows)
	if cookie != "passport_csrf_token=csrf-fixture;sessionid=site-value" || csrf != "csrf-fixture" {
		t.Fatalf("cookie selection mismatch: %q", cookie)
	}
	var err error
	if _, err = packQuotaScript("function fetchUsage(){return []}", "", "not-allowed"); err == nil {
		t.Fatal("must not accept a key without an approved endpoint")
	}
}
