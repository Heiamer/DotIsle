//go:build windows

package mailbox

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"unsafe"

	"golang.org/x/sys/windows"
)

const noUI = 1                // CRYPTPROTECT_UI_FORBIDDEN
func path(data string) string { return filepath.Join(data, "mail-account.dpapi") }
func crypt(raw []byte, encrypt bool) ([]byte, error) {
	if len(raw) == 0 || len(raw) > 65536 {
		return nil, errors.New("邮件账户配置无效")
	}
	in := windows.DataBlob{Size: uint32(len(raw)), Data: &raw[0]}
	var out windows.DataBlob
	var err error
	if encrypt {
		err = windows.CryptProtectData(&in, nil, nil, 0, nil, noUI, &out)
	} else {
		err = windows.CryptUnprotectData(&in, nil, nil, 0, nil, noUI, &out)
	}
	if err != nil {
		return nil, err
	}
	defer windows.LocalFree(windows.Handle(uintptr(unsafe.Pointer(out.Data))))
	data := unsafe.Slice(out.Data, int(out.Size))
	defer clear(data)
	return append([]byte(nil), data...), nil
}
func Load(data string) (Account, error) {
	b, err := os.ReadFile(path(data))
	if err != nil {
		return Account{}, err
	}
	raw, err := crypt(b, false)
	if err != nil {
		return Account{}, err
	}
	defer clear(raw)
	var a Account
	if err = json.Unmarshal(raw, &a); err != nil {
		return Account{}, err
	}
	return Validate(a)
}
func Save(data string, a Account) error {
	raw, err := json.Marshal(a)
	if err != nil {
		return err
	}
	defer clear(raw)
	b, err := crypt(raw, true)
	if err != nil {
		return err
	}
	if err := os.MkdirAll(data, 0700); err != nil {
		return err
	}
	f, err := os.CreateTemp(data, "mail-account-*.tmp")
	if err != nil {
		return err
	}
	defer os.Remove(f.Name())
	_, writeErr := f.Write(b)
	closeErr := f.Close()
	if err := errors.Join(writeErr, closeErr); err != nil {
		return err
	}
	return os.Rename(f.Name(), path(data))
}
func Forget(data string) error {
	err := os.Remove(path(data))
	if os.IsNotExist(err) {
		return nil
	}
	return err
}
