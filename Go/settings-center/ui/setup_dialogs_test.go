package ui

import (
	"errors"
	"image/png"
	"os"
	"path/filepath"
	"testing"

	"github.com/egoist/mygo/ui"
)

func TestHydrationAmountsAndRecordAcknowledgement(t *testing.T) {
	for _, pair := range [][2]string{{"99", "10"}, {"1600", "1601"}, {"2000", "0"}, {"10000", "3001"}, {"", "250"}, {"1200.5", "250"}} {
		if _, _, ok := hydrationAmounts(pair[0], pair[1]); ok {
			t.Fatalf("accepted %v", pair)
		}
	}
	if goal, each, ok := hydrationAmounts(" 1600 ", "175"); !ok || goal != 1600 || each != 175 {
		t.Fatal("rejected a valid plan")
	}
	a := newTestApp(t)
	a.applyCommandReply("hydrationRecord", nil, errors.New("offline"))
	if a.State["hydrationToday"] != nil {
		t.Fatal("failed save changed intake")
	}
	a.applyCommandReply("hydrationRecord", map[string]any{"success": true, "hydrationToday": map[string]any{"drankMl": float64(175)}}, nil)
	if a.State["hydrationToday"].(map[string]any)["drankMl"] != float64(175) {
		t.Fatal("record acknowledgement not reflected")
	}
}

func TestSetupDialogsKeepFieldsAndActionsSeparated(t *testing.T) {
	out := os.Getenv("DOTISLE_DIALOG_CAPTURE")
	RegisterFonts()
	for _, dark := range []bool{false, true} {
		for _, kind := range []string{"mail", "city", "city-result", "city-error"} {
			t.Run(kind+map[bool]string{true: "-dark", false: "-light"}[dark], func(t *testing.T) {
				a := cityTestApp(t)
				a.Prefs.Dark = dark
				a.settings()["SettingsTheme"] = map[bool]string{true: "Dark", false: "Light"}[dark]
				if kind == "mail" {
					a.mailOpen = true
				} else {
					a.maybePromptCity()
					if kind == "city-result" {
						a.cityPrompt.text = "上海"
						a.cityPrompt.location = "上海"
					}
					if kind == "city-error" {
						a.cityPrompt.error = "暂时无法定位，请手动填写城市。"
					}
				}
				for _, size := range [][2]int{{900, 640}, {1200, 800}} {
					tt := ui.NewTester(a.View, size[0], size[1])
					tt.Frame()
					if kind == "mail" {
						a, okA := tt.Find("邮箱地址")
						b, okB := tt.Find("IMAP 域名 · QQ / 163 / 126 可留空")
						c, okC := tt.Find("IMAP 授权码")
						if !okA || !okB || !okC || b.Y-a.Y < 78 || c.Y-b.Y < 78 {
							t.Fatalf("fields not separated: %v %v %v (%v %v %v)", a, b, c, okA, okB, okC)
						}
					} else {
						field, okField := tt.Find("城市 · 例如 上海 / Beijing")
						loc, okLoc := tt.Find("定位当前城市")
						save, okSave := tt.Find("保存城市")
						if !okField || !okLoc || !okSave || loc.Y-field.Y < 58 || save.Y-loc.Y < 112 {
							t.Fatalf("actions too close: %v %v %v (%v %v %v)", field, loc, save, okField, okLoc, okSave)
						}
					}
					if out != "" && size[0] == 1200 {
						if err := os.MkdirAll(out, 0755); err != nil {
							t.Fatal(err)
						}
						f, err := os.Create(filepath.Join(out, t.Name()[len("TestSetupDialogsKeepFieldsAndActionsSeparated/"):]+".png"))
						if err != nil {
							t.Fatal(err)
						}
						defer f.Close()
						if err := png.Encode(f, tt.Image()); err != nil {
							t.Fatal(err)
						}
					}
				}
			})
		}
	}
}
