// Direct HTTP contracts from H0308/taskbar-lyrics-displayer (Apache-2.0).
// Upstream 3fae9287dfb0636fc19a0df2587a1a59a54cf9bb; orchestration is in Rust.
package lyrics

import (
	"context"
	"encoding/base64"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"sort"
	"strconv"
	"strings"
	"sync"
	"time"
)

var tbLrclib struct {
	sync.Mutex
	retryAt time.Time
}

func (c *client) tbGet(ctx context.Context, endpoint, referer string) (any, error) {
	u, err := url.Parse(endpoint)
	if err != nil || !allowed(u) {
		return nil, errors.New("invalid taskbar lyric endpoint")
	}
	soda := u.Hostname() == "api.qishui.com" || u.Hostname() == "beta-luna.douyin.com"
	lrclib := u.Hostname() == "lrclib.net"
	if lrclib {
		tbLrclib.Lock()
		defer tbLrclib.Unlock()
		if time.Now().Before(tbLrclib.retryAt) {
			return nil, errors.New("LRCLIB retry cooldown")
		}
	}
	for attempt := 0; attempt < 2; attempt++ {
		req, err := http.NewRequestWithContext(ctx, http.MethodGet, endpoint, nil)
		if err != nil {
			return nil, err
		}
		req.Header.Set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36")
		if lrclib {
			req.Header.Set("User-Agent", "DotIsle/5.2.8 (https://github.com/Heiamer/DotIsle)")
		}
		if referer != "" {
			req.Header.Set("Referer", referer)
		}
		res, callErr := c.http.Do(req)
		retry := false
		if callErr == nil {
			data, readErr := io.ReadAll(io.LimitReader(res.Body, (8<<20)+1))
			res.Body.Close()
			if res.StatusCode == 200 && readErr == nil && len(data) <= 8<<20 {
				// QRC is returned as an XML-like document rather than JSON.
				if strings.Contains(u.Path, "lyric_download") {
					return string(data), nil
				}
				return parseJSON(data)
			}
			err = fmt.Errorf("%s HTTP %d", u.Hostname(), res.StatusCode)
			retry = res.StatusCode == 429 || res.StatusCode >= 500 && res.StatusCode < 600 || readErr != nil
			if lrclib && (res.StatusCode == 429 || res.StatusCode == 503) {
				seconds, _ := strconv.ParseInt(res.Header.Get("Retry-After"), 10, 64)
				if date, e := http.ParseTime(res.Header.Get("Retry-After")); e == nil {
					seconds = int64(time.Until(date).Seconds())
				}
				tbLrclib.retryAt = time.Now().Add(time.Duration(max(1, seconds)) * time.Second)
			}
		} else {
			err = callErr
			var ne net.Error
			retry = errors.As(callErr, &ne)
			if ctx.Err() != nil {
				return nil, ctx.Err()
			}
		}
		if !soda || !retry || attempt == 1 {
			return nil, err
		}
		timer := time.NewTimer(250 * time.Millisecond)
		select {
		case <-ctx.Done():
			timer.Stop()
			return nil, ctx.Err()
		case <-timer.C:
		}
	}
	return nil, err
}

func tbPreview(full, start, length, playing int64) int64 {
	abs := func(v int64) int64 {
		if v < 0 {
			return -v
		}
		return v
	}
	if playing <= 0 || start <= 0 || length <= 0 || full <= 0 || start >= full || length > full-start+1000 || abs(full-playing) <= 1000 {
		return 0
	}
	if abs(length-playing) <= 1000 {
		return start
	}
	return 0
}
func (c *client) tbSearch(ctx context.Context, r request) ([]hit, error) {
	keyword := r.Hit.Extra["query"]
	var v any
	var err error
	var out []hit
	switch r.Source {
	case "QQMusic":
		v, err = c.tbGet(ctx, "https://c.y.qq.com/soso/fcgi-bin/client_search_cp?w="+escape(keyword)+"&n=10&p=1&format=json&new_json=1", qqRef)
		for _, s := range array(field(v, "data", "song", "list")) {
			h := hit{ID: text(pick(field(s, "id"), field(s, "songid"))), Title: text(pick(field(s, "name"), field(s, "songname"))), Artists: names(field(s, "singer"), "name"), DurationMS: int64(number(field(s, "interval"))) * 1000, Extra: map[string]string{"mid": text(pick(field(s, "mid"), field(s, "songmid"))), "albumMid": text(pick(field(s, "album", "mid"), field(s, "albummid")))}}
			out = append(out, h)
		}
	case "Kugou":
		v, err = c.tbGet(ctx, "http://mobilecdn.kugou.com/api/v3/search/song?format=json&keyword="+escape(keyword)+"&page=1&pagesize=10&showtype=1", "")
		for _, s := range array(field(v, "data", "info")) {
			if hash := text(field(s, "hash")); hash != "" {
				out = append(out, hit{ID: hash, Title: text(field(s, "songname")), Artists: []string{text(field(s, "singername"))}, DurationMS: int64(number(field(s, "duration"))) * 1000, Extra: map[string]string{}})
				if len(out) >= 10 {
					break
				}
			}
		}
	case "Netease":
		v, err = c.tbGet(ctx, "https://music.163.com/api/cloudsearch/pc?csrf_token=&s="+escape(keyword)+"&type=1&offset=0&limit=5", neteaseRef)
		if err == nil && field(v, "result", "songs") == nil {
			return nil, errors.New("invalid Netease search response")
		}
		for _, s := range array(field(v, "result", "songs")) {
			id, title := text(field(s, "id")), text(field(s, "name"))
			if id == "" || title == "" {
				continue
			}
			duration := int64(number(field(s, "dt")))
			if duration <= 0 {
				duration = int64(number(field(s, "duration")))
			}
			out = append(out, hit{ID: id, Title: title, Artists: names(field(s, "ar"), "name"), DurationMS: duration, Extra: map[string]string{}})
			if len(out) >= 5 {
				break
			}
		}
	case "SodaMusic":
		v, err = c.tbGet(ctx, "https://api.qishui.com/luna/search/track?q="+escape(keyword)+"&cursor=0&count=20&aid=386088", sodaRef)
		if err == nil && field(v, "result_groups") == nil {
			return nil, errors.New("invalid Soda search response")
		}
		for _, g := range array(field(v, "result_groups")) {
			if text(field(g, "id")) != "tracks" {
				continue
			}
			for _, item := range array(field(g, "data")) {
				s := field(item, "entity", "track")
				id := text(field(s, "id"))
				if !numericID(id) {
					continue
				}
				full, start, length := int64(number(field(s, "duration"))), int64(number(field(s, "preview", "start"))), int64(number(field(s, "preview", "duration")))
				offset := tbPreview(full, start, length, r.DurationMS)
				duration := full
				if offset > 0 {
					duration = length
				}
				out = append(out, hit{ID: id, Title: text(field(s, "name")), Artists: names(field(s, "artists"), "name"), DurationMS: duration, Extra: map[string]string{"previewOffsetMs": strconv.FormatInt(offset, 10), "fullDurationMs": strconv.FormatInt(full, 10)}})
			}
		}
	default:
		return nil, errors.New("invalid taskbar source")
	}
	if err != nil {
		return nil, err
	}
	seen := map[string]bool{}
	var ranked []hit
	for _, h := range out {
		id := h.ID
		if r.Source == "QQMusic" {
			if h.Extra["mid"] != "" {
				id = "mid:" + h.Extra["mid"]
			} else if id != "" {
				id = "id:" + id
			}
		}
		if id == "" || ((r.Source == "Kugou" || r.Source == "QQMusic") && seen[id]) {
			continue
		}
		seen[id] = true
		score := tbScore(r, h)
		if score >= 80 {
			h.Extra["score"] = strconv.Itoa(score)
			ranked = append(ranked, h)
		}
	}
	sort.SliceStable(ranked, func(i, j int) bool {
		a, _ := strconv.Atoi(ranked[i].Extra["score"])
		b, _ := strconv.Atoi(ranked[j].Extra["score"])
		return a > b
	})
	return ranked, nil
}
func (c *client) tbKugouCandidates(ctx context.Context, r request) ([]hit, error) {
	v, err := c.tbGet(ctx, "http://lyrics.kugou.com/search?ver=1&man=yes&client=pc&hash="+escape(r.Hit.ID)+"&duration="+strconv.FormatInt(r.DurationMS, 10), "")
	if err != nil {
		return nil, err
	}
	var out []hit
	for _, s := range array(field(v, "candidates")) {
		id, key := text(field(s, "id")), text(field(s, "accesskey"))
		if id != "" && key != "" {
			out = append(out, hit{ID: id, Extra: map[string]string{"key": key}})
		}
		if len(out) >= 3 {
			break
		}
	}
	return out, nil
}
func (c *client) tbFetch(ctx context.Context, r request) (*payload, error) {
	h := r.Hit
	id := escape(h.ID)
	p := &payload{Format: "Lrc"}
	var v any
	var err error
	switch r.Source {
	case "Kugou":
		v, err = c.tbGet(ctx, "http://lyrics.kugou.com/download?ver=1&client=pc&id="+id+"&accesskey="+escape(h.Extra["key"])+"&fmt=krc&charset=utf8", "")
		p.Format = "Krc"
		p.Original = text(field(v, "content"))
	case "QQMusic":
		if r.Op == "taskbar-qrc" {
			if h.ID == "" {
				return nil, nil
			}
			v, err = c.tbGet(ctx, "https://c.y.qq.com/qqmusic/fcgi-bin/lyric_download.fcg?musicid="+id+"&version=15&miniversion=82&lrctype=4", "https://c.y.qq.com/")
			s, _ := v.(string)
			p.Format = "Qrc"
			p.Original = qqContent(s, "content")
		} else {
			if h.Extra["mid"] == "" {
				return nil, nil
			}
			v, err = c.tbGet(ctx, "https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg?songmid="+escape(h.Extra["mid"])+"&g_tk=5381&format=json&nobase64=0", "https://y.qq.com/portal/player.html")
			if err == nil && text(field(v, "retcode")) != "0" {
				return nil, errors.New("QQ LRC rejected")
			}
			p.Original = decode64(field(v, "lyric"))
			p.Translation = decode64(field(v, "trans"))
			p.Romanization = decode64(field(v, "roma"))
		}
	case "Netease":
		v, err = c.tbGet(ctx, "https://music.163.com/api/song/lyric?id="+id+"&lv=1&kv=1&tv=1&yv=1&ytv=1&rv=1&yrv=1", neteaseRef)
		p.Original = text(field(v, "yrc", "lyric"))
		p.Fallback = text(field(v, "lrc", "lyric"))
		if p.Original != "" {
			p.Format = "Yrc"
		} else {
			p.Original = p.Fallback
		}
		p.Translation = text(pick(field(v, "ytlrc", "lyric"), field(v, "tlyric", "lyric")))
		p.Romanization = text(pick(field(v, "yromalrc", "lyric"), field(v, "romalrc", "lyric")))
	case "SodaMusic":
		v, err = c.tbGet(ctx, "https://beta-luna.douyin.com/luna/h5/seo_track?track_id="+id+"&device_platform=web", sodaRef)
		if err == nil && text(field(v, "seo_track", "track", "id")) != h.ID {
			return nil, errors.New("Soda track ID mismatch")
		}
		switch text(field(v, "lyric", "type")) {
		case "krc":
			p.Format = "Krc"
		case "lrc":
		default:
			return nil, err
		}
		p.Original = text(field(v, "lyric", "content"))
		p.Translation = text(field(v, "lyric", "translations", "cn"))
	case "LRCLIB":
		// Each attempt is parsed by Rust before requesting the simplified variant.
		if r.Title == "" || r.Artist == "" || r.DurationMS < 1000 || r.DurationMS > 3600000 {
			return nil, nil
		}
		title, artist, album := r.Title, r.Artist, r.Album
		if r.Structured {
			title = tbUTF8(tbSimple(tbWide(title)))
			artist = tbUTF8(tbSimple(tbWide(artist)))
			album = tbUTF8(tbSimple(tbWide(album)))
			if title == r.Title && artist == r.Artist && album == r.Album {
				return nil, nil
			}
		}
		endpoint := "https://lrclib.net/api/get?track_name=" + escape(title) + "&artist_name=" + escape(artist) + "&duration=" + strconv.FormatInt((r.DurationMS+500)/1000, 10)
		if album != "" {
			endpoint += "&album_name=" + escape(album)
		}
		v, err = c.tbGet(ctx, endpoint, "")
		if field(v, "instrumental") == true {
			return nil, nil
		}
		p.Original = text(field(v, "syncedLyrics"))
		if field(v, "hasWordSync") == true {
			p.Document = tbLyricsfile(text(field(v, "lyricsfile")))
		}
	default:
		return nil, errors.New("invalid taskbar source")
	}
	if err != nil {
		return nil, err
	}
	return p, nil
}

func decode64(v any) string {
	b, err := base64.StdEncoding.DecodeString(text(v))
	if err != nil {
		return ""
	}
	return string(b)
}

// The response is an XML-like comment whose unrelated tags may be malformed.
func qqContent(s, name string) string {
	start := "<" + name
	for {
		_, rest, ok := strings.Cut(s, start)
		if !ok || rest == "" {
			return ""
		}
		s = rest
		if !strings.ContainsRune("> \t\r\n", rune(rest[0])) {
			continue
		}
		_, rest, ok = strings.Cut(rest, ">")
		if !ok {
			return ""
		}
		b, _, ok := strings.Cut(rest, "</"+name+">")
		if !ok {
			return ""
		}
		return strings.TrimSpace(strings.TrimSuffix(strings.TrimPrefix(strings.TrimSpace(b), "<![CDATA["), "]]>"))
	}
}
