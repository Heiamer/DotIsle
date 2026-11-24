package lyrics

import (
	"context"
	"crypto/md5"
	"encoding/hex"
	"encoding/json"
	"errors"
	"html"
	"net/url"
	"strconv"
	"strings"
	"time"
)

func (c *client) search(ctx context.Context, r request) ([]hit, error) {
	if strings.TrimSpace(r.Query) == "" {
		return nil, nil
	}
	q := escape(r.Query)
	var out []hit
	add := func(id, title, album any, artists []string, ms float64, extra map[string]string) {
		if len(out) >= 48 || text(id) == "" || strings.TrimSpace(text(title)) == "" {
			return
		}
		if artists == nil {
			artists = []string{}
		}
		out = append(out, hit{ID: text(id), Title: text(title), Album: text(album), Artists: artists, DurationMS: int64(ms + 0.5), Extra: extra})
	}
	get := func(u string) (any, error) { return c.json(ctx, u, "", "", nil, nil) }
	switch r.Source {
	case "AMLL":
		return c.searchAMLL(ctx, r)
	case "Netease":
		v, err := c.netease(ctx, "/api/cloudsearch/pc", obj{"s": r.Query, "type": 1, "limit": 24, "offset": 0, "total": true})
		if err != nil {
			v, err = c.json(ctx, neteaseRef+"api/search/get/web", form, neteaseRef, []byte("s="+q+"&type=1&limit=24&offset=0"), nil)
		}
		if err != nil {
			return nil, err
		}
		if text(field(v, "code")) != "200" {
			return nil, errors.New("Netease search rejected")
		}
		for _, s := range array(field(v, "result", "songs")) {
			aliases := append(stringSlice(field(s, "alia")), stringSlice(field(s, "tns"))...)
			var extra map[string]string
			if len(aliases) > 0 {
				extra = map[string]string{"alias": strings.Join(aliases, " / ")}
			}
			add(field(s, "id"), field(s, "name"), field(pick(field(s, "al"), field(s, "album")), "name"), names(pick(field(s, "ar"), field(s, "artists")), "name"), number(pick(field(s, "dt"), field(s, "duration"))), extra)
		}
	case "QQMusic":
		b, _ := json.Marshal(obj{"req": obj{"module": "music.search.SearchCgiService", "method": "DoSearchForQQMusicDesktop", "param": obj{"query": r.Query, "num_per_page": 24, "page_num": 1, "search_type": 0}}})
		v, err := c.json(ctx, "https://u.y.qq.com/cgi-bin/musicu.fcg", "application/json", qqRef, b, nil)
		for _, s := range array(field(v, "req", "data", "body", "song", "list")) {
			extra := map[string]string{"mid": text(field(s, "mid"))}
			if sub := strings.TrimSpace(text(field(s, "subtitle"))); sub != "" {
				extra["alias"] = sub
			}
			add(field(s, "id"), field(s, "title"), field(s, "album", "name"), names(field(s, "singer"), "name"), number(field(s, "interval"))*1000, extra)
		}
		if len(out) == 0 {
			v, fallback := c.json(ctx, "https://c.y.qq.com/soso/fcgi-bin/client_search_cp?format=json&p=1&n=24&w="+q, "", qqRef, nil, nil)
			if fallback != nil {
				if err != nil {
					return nil, err
				}
				return nil, fallback
			}
			for _, s := range array(field(v, "data", "song", "list")) {
				extra := map[string]string{"mid": text(field(s, "songmid"))}
				if sub := strings.TrimSpace(text(field(s, "songname_extra"))); sub != "" {
					extra["alias"] = sub
				}
				add(field(s, "songid"), field(s, "songname"), field(s, "albumname"), names(field(s, "singer"), "name"), number(field(s, "interval"))*1000, extra)
			}
		}
	case "LRCLIB":
		endpoint := "https://lrclib.net/api/"
		cleanTitle := cleanSearchTitle(r.Title)
		if r.Structured && r.Artist != "" && r.Album != "" && r.DurationMS > 0 {
			params := url.Values{"track_name": {cleanTitle}, "artist_name": {r.Artist}, "album_name": {r.Album}, "duration": {strconv.FormatInt((r.DurationMS+500)/1000, 10)}}
			if s, err := get(endpoint + "get-cached?" + params.Encode()); err == nil {
				addLRCLIB(add, s)
			}
		}
		query := "q=" + q
		if r.Structured {
			query = "track_name=" + escape(cleanTitle)
			if r.Artist != "" {
				query += "&artist_name=" + escape(r.Artist)
			}
		}
		v, err := get(endpoint + "search?" + query)
		if err != nil && len(out) == 0 {
			return nil, err
		}
		for _, s := range array(v) {
			addLRCLIB(add, s)
		}
		// 若结构化查询条数不足，补充纯关键词 q= 放宽召回
		if r.Structured && len(out) == 0 && strings.TrimSpace(r.Query) != "" {
			if v2, err2 := get(endpoint + "search?q=" + q); err2 == nil {
				for _, s := range array(v2) {
					addLRCLIB(add, s)
				}
			}
		}
	case "SodaMusic":
		v, err := c.json(ctx, "https://api.qishui.com/luna/search/track?q="+q+"&cursor=0&count=20&aid=386088", "", sodaRef, nil, nil)
		if err != nil {
			return nil, err
		}
		groups := array(field(v, "result_groups"))
		if groups == nil {
			return nil, errors.New("Soda search response missing result groups")
		}
		for _, g := range groups {
			if text(field(g, "id")) != "tracks" {
				continue
			}
			for _, row := range array(field(g, "data")) {
				s := field(row, "entity", "track")
				id := text(field(s, "id"))
				if !numericID(id) {
					continue
				}
				add(id, field(s, "name"), field(s, "album", "name"), names(field(s, "artists"), "name"), number(field(s, "duration")), map[string]string{"previewStartMs": text(field(s, "preview", "start")), "previewDurationMs": text(field(s, "preview", "duration"))})
			}
		}
	case "Kugou":
		endpoint := "https://lyrics.kugou.com/search?ver=1&man=yes&client=pc"
		collect := func(v any) {
			for _, s := range array(field(v, "candidates")) {
				if k := text(field(s, "accesskey")); k != "" {
					add(field(s, "id"), field(s, "song"), "", []string{text(field(s, "singer"))}, number(field(s, "duration")), map[string]string{"key": k})
				}
			}
		}
		v, firstErr := get(endpoint + "&keyword=" + q + "&duration=" + strconv.FormatInt(r.DurationMS, 10))
		collect(v)
		if len(out) == 0 {
			v, err := get("https://mobilecdn.kugou.com/api/v3/search/song?format=json&keyword=" + q + "&page=1&pagesize=4")
			if err != nil {
				return nil, err
			}
			for _, s := range array(field(v, "data", "info")) {
				if k := text(field(s, "hash")); k != "" {
					v, err := get(endpoint + "&hash=" + escape(k))
					if err != nil {
						firstErr = err
					}
					collect(v)
				}
			}
			if len(out) == 0 && firstErr != nil {
				return nil, firstErr
			}
		}
	case "Kuwo":
		b, err := c.bytes(ctx, "https://search.kuwo.cn/r.s?all="+q+"&pn=0&rn=24&ft=music&encoding=utf8&rformat=json&client=kt&vipver=1&show_copyright_off=1", "", "https://www.kuwo.cn/", nil, nil)
		if err != nil {
			return nil, err
		}
		v, err := kuwoJSON(b)
		if err != nil {
			return nil, err
		}
		for _, s := range array(field(v, "abslist")) {
			t := html.UnescapeString(text(field(s, "SONGNAME")))
			if a, b, ok := strings.Cut(t, "-《"); ok && strings.Contains(b, "曲") && (strings.Contains(b, "插曲") || strings.Contains(b, "主题曲") || strings.Contains(b, "片尾曲") || strings.Contains(b, "片头曲")) {
				t = a
			}
			add(strings.TrimPrefix(text(field(s, "MUSICRID")), "MUSIC_"), t, html.UnescapeString(text(field(s, "ALBUM"))), []string{html.UnescapeString(text(field(s, "ARTIST")))}, number(field(s, "DURATION"))*1000, nil)
		}
	case "Migu":
		now := strconv.FormatInt(time.Now().UnixMilli(), 10)
		const device = "963B7AA0D21511ED807EE5846EC87D20"
		sign := md5.Sum([]byte(r.Query + "6cdc72a439cef99a3418d2a78aa28c73yyapp2d16148780a1dcc7408e06336b98cfd50" + device + now))
		v, err := c.json(ctx, "https://jadeite.migu.cn/music_search/v3/search/searchAll?isCorrect=0&isCopyright=1&searchSwitch=%7B%22song%22%3A1%7D&pageSize=24&text="+q+"&pageNo=1&sort=0&sid=USS", "", "", nil, map[string]string{"uiVersion": "A_music_3.6.1", "deviceId": device, "timestamp": now, "sign": hex.EncodeToString(sign[:]), "channel": "0146921"})
		if err != nil {
			return nil, err
		}
		if text(field(v, "code")) != "000000" {
			return nil, errors.New("Migu search rejected")
		}
		for _, g := range array(field(v, "songResultData", "resultList")) {
			songs := array(g)
			if songs == nil {
				songs = []any{g}
			}
			for _, s := range songs {
				add(field(s, "copyrightId"), field(s, "name"), field(s, "album"), names(field(s, "singerList"), "name", "singerName"), number(field(s, "duration"))*1000, map[string]string{"lyric": text(field(s, "lrcUrl")), "translation": text(field(s, "trcUrl"))})
			}
		}
	default:
		return nil, errors.New("unknown lyric provider")
	}
	return out, nil
}

func addLRCLIB(add func(any, any, any, []string, float64, map[string]string), s any) {
	add(field(s, "id"), field(s, "trackName"), field(s, "albumName"), []string{text(field(s, "artistName"))}, number(field(s, "duration"))*1000, nil)
}

func stringSlice(v any) []string {
	arr := array(v)
	if len(arr) == 0 {
		return nil
	}
	var out []string
	for _, item := range arr {
		if s := strings.TrimSpace(text(item)); s != "" {
			out = append(out, s)
		}
	}
	return out
}

// cleanSearchTitle 去除尾部的 (feat. ...)/[Remaster] 等噪音以便提升 LRCLIB 精确命中率。
func cleanSearchTitle(title string) string {
	s := strings.TrimSpace(title)
	for _, pair := range [][2]string{{"(", ")"}, {"（", "）"}, {"[", "]"}, {"【", "】"}} {
		if idx := strings.LastIndex(s, pair[0]); idx > 0 && strings.HasSuffix(s, pair[1]) {
			tail := strings.ToLower(strings.TrimSpace(s[idx+len(pair[0]) : len(s)-len(pair[1])]))
			if strings.HasPrefix(tail, "feat") || strings.HasPrefix(tail, "ft.") ||
				strings.Contains(tail, "remaster") || strings.Contains(tail, "official") ||
				strings.Contains(tail, "主题曲") || strings.Contains(tail, "片尾曲") || strings.Contains(tail, "插曲") {
				s = strings.TrimSpace(s[:idx])
			}
		}
	}
	return s
}

// Kuwo's legacy endpoint returns quoted JS strings, not necessarily strict JSON.
func kuwoJSON(b []byte) (any, error) {
	if json.Valid(b) {
		return parseJSON(b)
	}
	var out strings.Builder
	for i := 0; i < len(b); i++ {
		if b[i] != '\'' && b[i] != '"' {
			out.WriteByte(b[i])
			continue
		}
		quote := b[i]
		out.WriteByte('"')
		closed := false
		for i++; i < len(b); i++ {
			x := b[i]
			if x == quote {
				closed = true
				break
			}
			if x == '\\' {
				i++
				if i == len(b) {
					break
				}
				x = b[i]
				if x != '\'' {
					out.WriteByte('\\')
				}
			} else if x == '"' {
				out.WriteByte('\\')
			}
			out.WriteByte(x)
		}
		if !closed {
			return nil, errors.New("unclosed Kuwo string")
		}
		out.WriteByte('"')
	}
	return parseJSON([]byte(out.String()))
}
