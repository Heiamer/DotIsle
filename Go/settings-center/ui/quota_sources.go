package ui

import (
	"context"
	"crypto/rand"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"
	"unsafe"

	"dotisle/settings/island"
	"github.com/egoist/mygo/ui"
	"golang.org/x/sys/windows"
)

type quotaSourcePanel struct {
	cancel                                                             context.CancelFunc
	open, busy                                                         bool
	page                                                               int
	selected, mode, title, name, site, accessKey, secret, code, status string
}

func storeQuotaKey(dir, id, provider, site, token string) error {
	if dir == "" || len(id) != 32 || len(token) < 1 || len(token) > 8192 {
		return errors.New("额度凭据无效")
	}
	if _, err := hex.DecodeString(id); err != nil {
		return errors.New("额度来源 ID 无效")
	}
	for _, c := range token {
		if c <= 0x20 || c >= 0x7f {
			return errors.New("密钥含有无效字符")
		}
	}
	raw, err := json.Marshal(map[string]string{"provider": provider, "site": site, "token": token})
	if err != nil {
		return err
	}
	defer clear(raw)
	in := windows.DataBlob{Data: &raw[0], Size: uint32(len(raw))}
	var enc windows.DataBlob
	if err := windows.CryptProtectData(&in, nil, nil, 0, nil, 1, &enc); err != nil {
		return err
	}
	defer windows.LocalFree(windows.Handle(uintptr(unsafe.Pointer(enc.Data))))
	dir = filepath.Join(dir, "ai-quota")
	if err := os.MkdirAll(dir, 0700); err != nil {
		return err
	}
	f, err := os.CreateTemp(dir, "key-*.tmp")
	if err != nil {
		return err
	}
	defer os.Remove(f.Name())
	_, wr := f.Write(unsafe.Slice(enc.Data, int(enc.Size)))
	closeErr := f.Close()
	if err := errors.Join(wr, closeErr); err != nil {
		return err
	}
	return os.Rename(f.Name(), filepath.Join(dir, id+".dpapi"))
}

func (a *App) addQuotaSource() {
	v := &a.quotaSources
	browser := v.mode == "browser"
	script := v.mode == "custom"
	if v.busy || v.selected == "" || (!browser && !script && v.mode != "profile" && strings.TrimSpace(v.secret) == "") {
		return
	}
	var bytes [16]byte
	if _, err := rand.Read(bytes[:]); err != nil {
		v.status = "无法创建额度来源。"
		a.Refresh()
		return
	}
	id, kind, site, title, secret := hex.EncodeToString(bytes[:]), v.selected, strings.TrimSpace(v.site), strings.TrimSpace(v.name), strings.TrimSpace(v.secret)
	if v.mode != "relay" && !script {
		site = ""
	}
	if v.mode == "relay" && site == "" {
		v.status = "请填写中转站地址。"
		a.Refresh()
		return
	}
	if script {
		var err error
		secret, err = packQuotaScript(v.code, site, secret)
		if err != nil {
			v.status = err.Error()
			a.Refresh()
			return
		}
	}
	if v.mode == "cloud-ak" {
		access := strings.TrimSpace(v.accessKey)
		if access == "" || strings.Contains(secret, ":") || strings.Contains(access, ":") {
			v.status = "请分别填写云访问密钥 ID 和密钥。"
			a.Refresh()
			return
		}
		secret = access + ":" + secret
	}
	data := a.Data
	v.busy = true
	v.secret = ""
	v.accessKey = ""
	v.status = "正在保存来源…"
	var ctx context.Context
	var cancel context.CancelFunc
	if browser {
		ctx, cancel = context.WithTimeout(context.Background(), 10*time.Minute)
		v.cancel = cancel
		v.status = "正在准备官网授权…"
	}
	a.startAsync(func() {
		status := "额度来源已添加。"
		ok := false
		if browser {
			defer cancel()
			var err error
			secret, err = quotaBrowserLogin(ctx, kind, func(address string) error {
				if ctx.Err() != nil {
					return ctx.Err()
				}
				if err := quotaOpenLogin(address); err != nil {
					return err
				}
				a.applyAsync(func() {
					if ctx.Err() == nil {
						v.status = "请在默认浏览器完成授权，授权结果会自动返回。"
					}
				})
				return nil
			}, func() {
				a.applyAsync(func() {
					if ctx.Err() == nil {
						v.status = "请在独立浏览器窗口登录；不会读取日常浏览器的登录。"
					}
				})
			})
			if ctx.Err() != nil {
				status = "官网授权已取消或超时。"
			} else if err != nil {
				status = err.Error()
			}
			// Disable cancellation once committing the source and DPAPI file.
			commitStatus := "正在保存来源…"
			if status != "额度来源已添加。" {
				commitStatus = status
			}
			a.applyAsync(func() { v.cancel = nil; v.status = commitStatus })
		}
		if status == "额度来源已添加。" && secret != "" {
			if err := storeQuotaKey(data, id, kind, site, secret); err != nil {
				status = "无法加密并保存凭据，请检查密钥格式与数据目录。"
			}
		}
		if status == "额度来源已添加。" {
			reply, err := island.Request("aiQuotaSourceAdd", map[string]any{"id": id, "provider": kind, "name": title, "site": site})
			if err != nil {
				status = "控制中心没有收到保存确认，请刷新后检查来源。"
			} else if reply["success"] != true {
				status = stateString(reply, "额度来源保存失败。", "message")
				if secret != "" {
					_ = os.Remove(filepath.Join(data, "ai-quota", id+".dpapi"))
				}
			} else {
				ok = true
			}
		}
		a.applyAsync(func() {
			v.busy = false
			v.cancel = nil
			v.status = status
			if ok {
				v.selected = ""
				v.name = ""
				v.site = ""
				v.code = ""
				a.pushToast(status)
			}
			a.Refresh()
		})
	})
}

func (a *App) quotaSourcesDialog(c *ui.Context) {
	v, p := &a.quotaSources, &a.Palette
	s := &p.Scheme
	state, _ := a.State["aiQuota"].(map[string]any)
	catalog, _ := state["catalog"].([]any)
	profiles, _ := state["profiles"].([]any)
	open := true
	const w float32 = 660
	h := float32(514)
	if v.mode == "custom" && v.selected != "" {
		h = 600
	}
	ui.DialogBase(c, &open, func(backdrop, panel ui.Element) {
		backdrop.Background(AlphaCol(0, .42))
		panel.Width(w).Height(h)
		overlayStyle(&panel, p)
		panel.Children(func() {
			Label(c, "AI 额度来源", 24, 22, 21, s.OnSurface, true, w-110)
			IconButton(c, p, "关闭额度来源", w-67, 14, 44, 44, "close", false, !v.busy, func() { open = false })
			if v.selected != "" {
				Button(c, p, "返回来源列表", 24, 72, 116, 34, "返回列表", "", false, !v.busy, func() {
					v.selected = ""
					v.secret = ""
					v.accessKey = ""
					v.code = ""
					v.status = ""
					a.Refresh()
				})
				Label(c, v.title, 24, 124, 17, s.OnSurface, true, w-48)
				if v.mode == "custom" {
					Label(c, "只运行你信任的脚本；脚本只看指定接口的 JSON，不读取密钥。", 24, 153, 12, s.OnSurfaceVariant, false, w-48)
					const firstFieldY float32 = 175
					const fieldStep = dialogFieldHeight + 12
					const scriptLabelY = firstFieldY + 2*fieldStep + dialogFieldHeight + 12
					a.dialogFieldInput(c, p, 24, firstFieldY, w-48, "显示名（可选）", &v.name, 50, false, v.busy)
					a.dialogFieldInput(c, p, 24, firstFieldY+fieldStep, w-48, "指定 HTTPS JSON 地址（可选）", &v.site, 512, false, v.busy)
					a.dialogFieldInput(c, p, 24, firstFieldY+2*fieldStep, w-48, "仅发往该地址的 Bearer 密钥（可选）", &v.secret, 1024, true, v.busy)
					Label(c, "JavaScript · fetchUsage(ctx)", 24, scriptLabelY, 13, s.OnSurface, true, w-48)
					field := ui.TextArea(c, &v.code).Absolute().Left(24).Top(scriptLabelY + 22).Width(w - 48).Height(88).Label("自定义额度脚本").Placeholder("async function fetchUsage(ctx) { … }")
					fieldStyle(&field, p)
					field.Disabled(v.busy)
					Label(c, v.status, 24, h-52, 12, s.OnSurfaceVariant, false, w-198)
					Button(c, p, "保存并运行额度脚本", w-160, h-58, 136, 36, "保存并运行", "", false, !v.busy, a.addQuotaSource)
					return
				}
				if v.mode == "profile" {
					Label(c, "添加后读取 "+v.title+" 的本机登录或已保存的凭据。", 24, 157, 12, s.OnSurfaceVariant, false, w-48)
				} else if v.mode == "browser" {
					Label(c, "在官网授权后读取额度。凭据由当前 Windows 用户加密保存。", 24, 157, 12, s.OnSurfaceVariant, false, w-48)
				} else if v.mode == "cloud-ak" {
					Label(c, "填写有权查询套餐与余额的云访问密钥，不是模型 API Key。", 24, 157, 12, s.OnSurfaceVariant, false, w-48)
				} else {
					Label(c, "密钥由 Windows 用户级加密保存，不通过设置管道传输。", 24, 157, 12, s.OnSurfaceVariant, false, w-48)
				}
				a.dialogField(c, p, 24, 186, w-48, "显示名（可选）", &v.name, 50)
				const secondFieldY = 186 + dialogFieldHeight + 12
				top := secondFieldY
				if v.mode == "relay" {
					a.dialogField(c, p, 24, secondFieldY, w-48, "API 站点 · 使用 HTTPS，本机可用 HTTP", &v.site, 512)
					top = secondFieldY + dialogFieldHeight + 12
				}
				if v.mode != "profile" && v.mode != "browser" {
					label := "额度 API 密钥"
					if v.mode == "cloud-ak" {
						a.dialogField(c, p, 24, secondFieldY, w-48, "云 Access Key ID", &v.accessKey, 256)
						label = "云 Secret Access Key（加密保存）"
						top = secondFieldY + dialogFieldHeight + 12
					}
					field := ui.TextInput(c, &v.secret).Absolute().Left(24).Top(top).Width(w - 48).Height(40).Label(label).Placeholder(label).Password()
					fieldStyle(&field, p)
					field.Disabled(v.busy)
				}
				Label(c, v.status, 24, 414, 12, s.OnSurfaceVariant, false, w-48)
				if v.cancel != nil {
					Button(c, p, "取消官网授权", w-160, h-58, 136, 36, "取消授权", "", false, true, func() {
						if v.cancel != nil {
							v.cancel()
						}
					})
				} else {
					label := "添加来源"
					if v.mode == "browser" {
						label = "前往官网授权"
					}
					Button(c, p, "添加此额度来源", w-160, h-58, 136, 36, label, "", false, !v.busy, a.addQuotaSource)
				}
				return
			}
			if v.status != "" {
				Label(c, v.status, 24, 61, 12, s.OnSurfaceVariant, false, w-48)
			} else {
				Label(c, "全部来源目录 · 选择渠道配置；只读取已启用并授权的来源。", 24, 61, 12, s.OnSurfaceVariant, false, w-48)
			}
			type item struct{ id, title, kind string }
			rows := make([]item, 0, len(profiles)+len(catalog))
			names := make(map[string]string, len(catalog))
			for _, raw := range catalog {
				row, _ := raw.(map[string]any)
				names[stateString(row, "", "id")] = stateString(row, "", "name")
			}
			for _, raw := range profiles {
				row, _ := raw.(map[string]any)
				key := stateString(row, "", "provider")
				rows = append(rows, item{stateString(row, "", "id"), stateString(row, names[key], "name"), "saved"})
			}
			for _, raw := range catalog {
				row, _ := raw.(map[string]any)
				rows = append(rows, item{stateString(row, "", "id"), stateString(row, "额度来源", "name"), stateString(row, "pending", "mode")})
			}
			const perPage = 7
			lastPage := max(len(rows)-1, 0) / perPage
			page := min(max(v.page, 0), lastPage)
			for i, entry := range rows[min(page*perPage, len(rows)):min((page+1)*perPage, len(rows))] {
				y := float32(102 + i*49)
				Label(c, entry.title, 24, y+9, 14, s.OnSurface, true, w-194)
				switch entry.kind {
				case "saved":
					Button(c, p, "移除已配置来源 "+entry.title, w-136, y, 112, 35, "移除来源", "", false, a.Connected, func() { a.command("aiQuotaSourceRemove", map[string]any{"id": entry.id}) })
				case "local":
					Label(c, "内置本机读取", w-144, y+9, 12, s.OnSurfaceVariant, false, 120)
				case "pending":
					Label(c, "待原生转译", w-144, y+9, 12, s.OnSurfaceVariant, false, 120)
				default:
					Button(c, p, "配置 "+entry.title, w-136, y, 112, 35, "配置", "", false, a.Connected, func() {
						v.selected = entry.id
						v.mode = entry.kind
						v.title = entry.title
						v.name = ""
						v.site = ""
						v.accessKey = ""
						v.secret = ""
						v.code = ""
						v.status = ""
						a.Refresh()
					})
				}
			}
			Label(c, fmt.Sprintf("第 %d / %d 页", page+1, lastPage+1), 24, h-46, 12, s.OnSurfaceVariant, false, 160)
			IconButton(c, p, "上一页额度来源", w-110, h-58, 36, 36, "arrow_back", false, page > 0, func() { v.page = page - 1; a.Refresh() })
			IconButton(c, p, "下一页额度来源", w-64, h-58, 36, 36, "arrow_forward", false, page < lastPage, func() { v.page = page + 1; a.Refresh() })
		})
	})
	if !open && !v.busy {
		*v = quotaSourcePanel{}
	}
}

func packQuotaScript(code, site, bearer string) (string, error) {
	if strings.TrimSpace(code) == "" || len(code) > 4096 || strings.ContainsRune(code, 0) {
		return "", errors.New("请输入 1–4096 字节的 JavaScript。")
	}
	if bearer != "" && (site == "" || !quotaLoginField(bearer, 1024)) {
		return "", errors.New("填写密钥时必须指定 HTTPS 地址。")
	}
	raw, err := json.Marshal(map[string]string{"code": code, "bearer": bearer})
	if err != nil {
		return "", errors.New("无法保存脚本。")
	}
	defer clear(raw)
	packed := base64.RawURLEncoding.EncodeToString(raw)
	if len(packed) > 8192 {
		return "", errors.New("脚本或密钥过长，请精简后重试。")
	}
	return packed, nil
}
