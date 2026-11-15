package prefs

import (
	"encoding/json"
	"os"
)

// CenterLayout belongs only to the Go control center, outside schema 3.
type CenterLayout struct {
	Sidebar string `json:"sidebar"`
}

func LoadCenterLayout(path string) (CenterLayout, error) {
	p := CenterLayout{Sidebar: "auto"}
	raw, err := os.ReadFile(path)
	if os.IsNotExist(err) {
		return p, nil
	}
	if err != nil {
		return p, err
	}
	if len(raw) > 4096 {
		return p, ErrPrefs("布局配置过大，已保留原文件")
	}
	if err := json.Unmarshal(raw, &p); err != nil {
		return CenterLayout{Sidebar: "auto"}, err
	}
	if p.Sidebar != "expanded" && p.Sidebar != "collapsed" {
		p.Sidebar = "auto"
	}
	return p, nil
}

func SaveCenterLayout(path string, p CenterLayout) error { return atomicSave(path, p) }
