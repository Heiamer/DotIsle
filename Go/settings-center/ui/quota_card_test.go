package ui

import (
	"image/png"
	"os"
	"path/filepath"
	"slices"
	"testing"

	"dotisle/settings/prefs"
	"github.com/egoist/mygo/ui"
)

func TestQuotaCardInOverviewAndSources(t *testing.T) {
	RegisterFonts()
	a := newTestApp(t)
	a.State["pendingPage"] = ""
	a.State["aiQuota"] = map[string]any{
		"nativeSources": true,
		"settings":      map[string]any{"enabled": true, "fiveHour": float64(20), "weekly": float64(10)},
		"readings": []any{
			map[string]any{"id": "fixture-commandcode", "name": "Command Code", "status": "已连接", "fiveHour": float64(24), "weekly": float64(61)},
			map[string]any{"id": "fixture-dsh", "name": "DeepSeek Harness", "status": "已连接", "metrics": []any{map[string]any{"label": "账户余额", "value": "¥8.42"}}},
		},
		"catalog": []any{
			map[string]any{"id": "codex", "name": "Codex", "mode": "local"},
			map[string]any{"id": "commandcode", "name": "Command Code", "mode": "profile"},
			map[string]any{"id": "kimi-platform", "name": "Kimi 开放平台", "mode": "key"},
			map[string]any{"id": "dsh", "name": "DeepSeek Harness", "mode": "profile"},
			map[string]any{"id": "newapi", "name": "New API 中转站", "mode": "relay"},
			map[string]any{"id": "custom", "name": "自定义 JavaScript", "mode": "custom"},
			map[string]any{"id": "doubao", "name": "豆包", "mode": "browser"},
		},
	}
	a.overview.loaded = true
	a.overview.config = prefs.Overview{View: "cards", Cards: []string{"music"}}
	a.addQuotaOverviewCard()
	a.addQuotaOverviewCard()
	saved, err := prefs.LoadOverview(filepath.Join(a.Data, "center-overview.json"))
	if err != nil || !slices.Equal(saved.Cards, []string{"music", "aiQuota"}) {
		t.Fatalf("overview quota card not saved: %+v, %v", saved, err)
	}
	tt := ui.NewTester(a.View, 1200, 800)
	tt.Frame()
	if _, ok := tt.Find("打开 AI 额度插件设置"); !ok {
		t.Fatal("missing quota card on overview")
	}
	save := func(name string) {
		dest := os.Getenv("DOTISLE_QUOTA_TEST_CAPTURE")
		if dest == "" {
			return
		}
		if err := os.MkdirAll(dest, 0700); err != nil {
			t.Fatal(err)
		}
		f, err := os.Create(filepath.Join(dest, name+".png"))
		if err != nil {
			t.Fatal(err)
		}
		defer f.Close()
		if err := png.Encode(f, tt.Image()); err != nil {
			t.Fatal(err)
		}
	}
	save("overview-quota")
	if err := tt.Click("概览：下一个额度来源"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if a.quotaFields.row != 1 {
		t.Fatal("different source is not selected")
	}
	save("overview-quota-balance")
	a.quotaSources.open = true
	tt.Frame()
	save("quota-source-dialog")
	if err := tt.Click("配置 New API 中转站"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if a.quotaSources.selected != "newapi" || a.quotaSources.mode != "relay" {
		t.Fatal("relay configuration did not open")
	}
	if _, ok := tt.Find("额度 API 密钥"); !ok {
		t.Fatal("missing protected credential input")
	}
	save("quota-relay-setup")
	if err := tt.Click("返回来源列表"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if err := tt.Click("配置 Command Code"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if _, ok := tt.Find("额度 API 密钥"); ok || a.quotaSources.mode != "profile" {
		t.Fatal("on-device accounts must not ask for an API key")
	}
	if err := tt.Click("返回来源列表"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if err := tt.Click("配置 Kimi 开放平台"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if _, ok := tt.Find("额度 API 密钥"); !ok || a.quotaSources.mode != "key" {
		t.Fatal("manual accounts must ask for an API key")
	}
	if err := tt.Click("返回来源列表"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if err := tt.Click("配置 豆包"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if _, ok := tt.Find("额度 API 密钥"); ok || a.quotaSources.mode != "browser" {
		t.Fatal("Doubao uses official sign-in, not an API key")
	}
	if _, ok := tt.Find("添加此额度来源"); !ok {
		t.Fatal("missing browser authorization action")
	}
	save("quota-doubao-setup")
	if err := tt.Click("返回来源列表"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if err := tt.Click("配置 自定义 JavaScript"); err != nil {
		t.Fatal(err)
	}
	tt.Frame()
	if _, ok := tt.Find("自定义额度脚本"); !ok {
		t.Fatal("missing script input")
	}
	if _, ok := tt.Find("保存并运行额度脚本"); !ok {
		t.Fatal("missing explicit script execution action")
	}
	save("quota-custom-setup")
}

func TestQuotaBalanceCurrencySavesIndependentlyOfQuotaThresholds(t *testing.T) {
	RegisterFonts()
	a := newTestApp(t)
	a.Connected = true
	settings := map[string]any{"enabled": true, "fiveHour": float64(26), "weekly": float64(14)}
	a.State["aiQuota"] = map[string]any{"nativeSources": true, "settings": settings}
	var writes []string
	a.onCommand = func(name string, value any) {
		v, ok := value.(map[string]any)
		if name != "aiQuotaConfigure" || !ok || v["enabled"] != true || v["fiveHour"] != 26 || v["weekly"] != 14 {
			t.Fatalf("changing currency must leave the other settings untouched: %s %+v", name, value)
		}
		unit := v["relayCurrency"].(string)
		writes = append(writes, unit)
		settings["relayCurrency"] = unit
	}
	tt := ui.NewTester(func(c *ui.Context) { a.quotaPluginRow(c, 26, 30, 620) }, 672, 520)
	tt.Frame()
	// Clicking the current choice is a no-op; switching also works in both directions.
	for _, option := range []string{"美元余额提醒", "人民币余额提醒", "人民币余额提醒", "美元余额提醒", "人民币余额提醒", "积分余额提醒", "积分余额提醒"} {
		if err := tt.Click(option); err != nil {
			t.Fatal(err)
		}
		tt.Frame()
	}
	if !slices.Equal(writes, []string{"CNY", "USD", "CNY", "POINTS"}) {
		t.Fatalf("expected both currencies to be selectable, got %v", writes)
	}
	if dest := os.Getenv("DOTISLE_QUOTA_TEST_CAPTURE"); dest != "" {
		if err := os.MkdirAll(dest, 0700); err != nil {
			t.Fatal(err)
		}
		f, err := os.Create(filepath.Join(dest, "quota-currency-settings.png"))
		if err != nil {
			t.Fatal(err)
		}
		defer f.Close()
		if err := png.Encode(f, tt.Image()); err != nil {
			t.Fatal(err)
		}
	}
}

func TestQuotaRemainingDoesNotConflateUnknownAndZero(t *testing.T) {
	row := map[string]any{"fiveHour": float64(0), "weekly": nil}
	if quotaFormat(row, "fiveHour") != "0%" || quotaFormat(row, "weekly") != "—" {
		t.Fatal("unknown is not zero")
	}
}

func TestZCodeQuotaDisplayUsesWindowsBeforeToken(t *testing.T) {
	token := map[string]any{"value": "剩余 0 / 2000000 TOKEN", "tokenQuota": map[string]any{"remaining": 0.0, "total": 2000000.0}}
	mcp := map[string]any{"value": "剩余 5 次"}
	for _, tc := range []struct {
		name       string
		hour, week any
		windows    bool
	}{
		{"no time windows", nil, nil, false},
		{"only five hour at zero", 0.0, nil, true},
		{"only week at zero", nil, 0.0, true},
		{"both windows", 27.0, 10.0, true},
	} {
		t.Run(tc.name, func(t *testing.T) {
			row := map[string]any{"provider": "zcode", "fiveHour": tc.hour, "weekly": tc.week, "metrics": []any{token, mcp}}
			if quotaHasWindows(row) != tc.windows {
				t.Fatal("wrong window availability")
			}
			want := "剩余 0 / 2000000 TOKEN"
			if tc.windows {
				want = ""
			}
			if shown := quotaOverviewToken(row); shown != want {
				t.Fatalf("overview token = %q, want %q", shown, want)
			}
			detail := quotaDetailMetrics(row)
			if len(detail) == 0 || quotaTokenMetric(detail[0]) == tc.windows {
				t.Fatalf("wrong token visibility in detail: %v", detail)
			}
		})
	}
	if quotaOverviewToken(map[string]any{"provider": "zcode", "metrics": []any{mcp}}) != "" ||
		quotaOverviewToken(map[string]any{"provider": "sub2api", "metrics": []any{token}}) != "" {
		t.Fatal("non-ZCode-token data must not appear as ZCode TOKEN")
	}
}

func TestZCodeQuotaOverviewRendersOnlySelectedUnit(t *testing.T) {
	RegisterFonts()
	a := newTestApp(t)
	a.State["pendingPage"] = ""
	row := map[string]any{
		"provider": "zcode", "name": "ZCode", "status": "已连接", "fiveHour": 0.0,
		"metrics": []any{map[string]any{"value": "剩余 0 / 2000000 TOKEN", "tokenQuota": map[string]any{"remaining": 0.0, "total": 2000000.0}}},
	}
	a.State["aiQuota"] = map[string]any{"readings": []any{row}}
	a.overview.loaded = true
	a.overview.config = prefs.Overview{View: "cards", Cards: []string{"aiQuota"}}
	tt := ui.NewTester(a.View, 1200, 800)
	check := func(want, reject, name string) {
		t.Helper()
		tt.Frame()
		if _, ok := tt.Find(want); !ok || tt.HasText(reject) {
			t.Fatalf("want %q without %q in the rendered card: %v", want, reject, tt.Texts())
		}
		if dest := os.Getenv("DOTISLE_QUOTA_TEST_CAPTURE"); dest != "" {
			if err := os.MkdirAll(dest, 0700); err != nil {
				t.Fatal(err)
			}
			f, err := os.Create(filepath.Join(dest, name+".png"))
			if err != nil {
				t.Fatal(err)
			}
			defer f.Close()
			if err := png.Encode(f, tt.Image()); err != nil {
				t.Fatal(err)
			}
		}
	}
	check("0%", "TOKEN", "zcode-overview-window")
	delete(row, "fiveHour")
	check("剩余 0 / 2000000 TOKEN", "5 小时剩余", "zcode-overview-token")
	delete(row, "metrics")
	check("—", "TOKEN", "zcode-overview-unknown")
}
