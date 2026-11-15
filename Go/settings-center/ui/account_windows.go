package ui

import (
	"os"
	"os/user"
	"path/filepath"
	"strings"
	"sync"

	"github.com/egoist/mygo/ui"
	"golang.org/x/sys/windows/registry"
)

type localAccount struct {
	Name   string
	Avatar *ui.Bitmap
}

var accountOnce sync.Once
var account localAccount

// Read the signed-in Windows profile locally; no account data leaves the process.
func currentAccount() localAccount {
	accountOnce.Do(func() {
		account.Name = os.Getenv("USERNAME")
		if profile, err := user.Current(); err == nil {
			if profile.Name != "" {
				account.Name = profile.Name
			} else {
				account.Name = profile.Username
			}
			if i := strings.LastIndex(account.Name, `\`); i >= 0 {
				account.Name = account.Name[i+1:]
			}
			key, err := registry.OpenKey(registry.LOCAL_MACHINE, `SOFTWARE\Microsoft\Windows\CurrentVersion\AccountPicture\Users\`+profile.Uid, registry.QUERY_VALUE)
			if err == nil {
				defer key.Close()
				for _, size := range []string{"Image192", "Image240", "Image448", "Image96", "Image64", "Image32"} {
					path, _, err := key.GetStringValue(size)
					if err != nil || path == "" {
						continue
					}
					path, err = registry.ExpandString(path)
					if err != nil {
						continue
					}
					if info, err := os.Stat(path); err != nil || info.Size() > 8<<20 {
						continue
					}
					if raw, err := os.ReadFile(path); err == nil {
						if image, err := ui.DecodeBitmap(raw); err == nil {
							account.Avatar = image
							break
						}
					}
				}
			}
		}
		if account.Avatar == nil {
			if raw, err := os.ReadFile(filepath.Join(os.Getenv("ProgramData"), "Microsoft", "User Account Pictures", "user.png")); err == nil {
				account.Avatar, _ = ui.DecodeBitmap(raw)
			}
		}
		if account.Name == "" {
			account.Name = "本机用户"
		}
	})
	return account
}
