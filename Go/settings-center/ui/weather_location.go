package ui

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"strings"
	"time"
	"unicode"
)

// Use the existing weather provider's approximate IP city only at the user's
// request. The endpoint receives no account identifier or precise coordinates.
func locateWeatherCity(ctx context.Context, client *http.Client, endpoint string) (string, string, error) {
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, endpoint, nil)
	if err != nil {
		return "", "", err
	}
	request.Header.Set("Accept", "application/json")
	response, err := client.Do(request)
	if err != nil {
		return "", "", err
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		return "", "", errors.New("定位服务暂不可用")
	}
	type name struct {
		Value string `json:"value"`
	}
	var result struct {
		Areas []struct {
			City    []name `json:"areaName"`
			Region  []name `json:"region"`
			Country []name `json:"country"`
		} `json:"nearest_area"`
	}
	if err := json.NewDecoder(io.LimitReader(response.Body, 512*1024)).Decode(&result); err != nil {
		return "", "", err
	}
	read := func(names []name) string {
		if len(names) == 0 {
			return ""
		}
		v := strings.TrimSpace(names[0].Value)
		if len([]rune(v)) > 64 || strings.ContainsFunc(v, unicode.IsControl) {
			return ""
		}
		return v
	}
	if len(result.Areas) == 0 {
		return "", "", errors.New("未返回城市")
	}
	area := result.Areas[0]
	city := read(area.City)
	if city == "" {
		return "", "", errors.New("未返回城市")
	}
	parts := []string{city}
	for _, part := range []string{read(area.Region), read(area.Country)} {
		if part != "" && !strings.EqualFold(part, city) {
			parts = append(parts, part)
		}
	}
	return city, strings.Join(parts, " · "), nil
}

func (a *App) requestWeatherLocation(draft *cityPromptState) {
	if draft == nil || draft.locating || draft.saving {
		return
	}
	draft.locating, draft.error, draft.location = true, "", ""
	before := draft.text
	a.startAsync(func() {
		ctx, cancel := context.WithTimeout(context.Background(), 8*time.Second)
		defer cancel()
		client := &http.Client{CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse }}
		city, label, err := locateWeatherCity(ctx, client, "https://wttr.in/?format=j1")
		a.applyAsync(func() {
			if a.cityPrompt != draft {
				return
			}
			draft.locating = false
			if draft.saving || draft.text != before {
				a.Refresh()
				return
			}
			if err != nil {
				draft.error = "暂时无法定位。请重试或手动填写城市。"
			} else {
				draft.text, draft.location = city, label
			}
			a.Refresh()
		})
	})
}
