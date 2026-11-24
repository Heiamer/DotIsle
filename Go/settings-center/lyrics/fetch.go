package lyrics

import (
	"context"
	"encoding/base64"
	"errors"
	"fmt"
	"html"
	"net/url"
	"strings"
)

func (c *client) fetch(ctx context.Context, r request) (*payload, error) {
	h := r.Hit
	id := escape(h.ID)
	p := &payload{Format: "Lrc"}
	get := func(u string) (any, error) { return c.json(ctx, u, "", "", nil, nil) }
	switch r.Source {
	case "AMLL":
		return c.fetchAMLL(ctx, h)
	case "Netease":
		if !numericID(h.ID) {
			return nil, errors.New("invalid Netease ID")
		}
		v, err := c.netease(ctx, "/api/song/lyric/v1", obj{"id": h.ID, "cp": false, "tv": 0, "lv": 0, "rv": 0, "kv": 0, "yv": 0, "ytv": 0, "yrv": 0})
		if err != nil || (text(field(v, "yrc", "lyric")) == "" && text(field(v, "lrc", "lyric")) == "") {
			v, err = get(neteaseRef + "api/song/lyric?id=" + id + "&lv=-1&kv=-1&tv=-1")
		}
		if err != nil {
			return nil, err
		}
		if text(field(v, "code")) != "200" {
			return nil, errors.New("Netease lyric rejected")
		}
		p.Original = text(field(v, "yrc", "lyric"))
		p.Translation = text(pick(field(v, "ytlrc", "lyric"), field(v, "tlyric", "lyric")))
		if p.Original != "" {
			p.Format = "Yrc"
		} else {
			p.Original = text(field(v, "lrc", "lyric"))
		}
	case "QQMusic":
		if r.Op != "fetch-lrc" && numericID(h.ID) {
			b, _ := c.bytes(ctx, "https://c.y.qq.com/qqmusic/fcgi-bin/lyric_download.fcg", form, qqRef, []byte("version=15&miniversion=82&lrctype=4&musicid="+id), nil)
			if q := qqContent(string(b), "content"); q != "" {
				return &payload{Format: "Qrc", Original: q, Translation: qqContent(string(b), "contentts")}, nil
			}
		}
		mid := h.Extra["mid"]
		if mid == "" {
			return nil, nil
		}
		v, err := c.json(ctx, "https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg?songmid="+escape(mid)+"&format=json&g_tk=5381&loginUin=0&hostUin=0&inCharset=utf8&outCharset=utf8&platform=yqq&needNewCode=0", "", qqRef, nil, nil)
		if err != nil {
			return nil, err
		}
		p.Original, p.Translation = decode64(field(v, "lyric")), decode64(field(v, "trans"))
	case "Kugou":
		key := h.Extra["key"]
		if key == "" || !numericID(h.ID) {
			return nil, errors.New("invalid Kugou lyric identity")
		}
		v, err := get("https://lyrics.kugou.com/download?ver=1&client=pc&id=" + id + "&accesskey=" + escape(key) + "&fmt=krc&charset=utf8")
		if err != nil {
			return nil, err
		}
		p.Format = "Krc"
		p.Original = text(field(v, "content"))
	case "LRCLIB":
		if !numericID(h.ID) {
			return nil, errors.New("invalid LRCLIB ID")
		}
		v, err := get("https://lrclib.net/api/get/" + id)
		if err != nil {
			return nil, err
		}
		p.Original = text(field(v, "syncedLyrics"))
		if p.Original == "" {
			p.Format = "Text"
			p.Original = text(field(v, "plainLyrics"))
		}
		p.Instrumental, _ = field(v, "instrumental").(bool)
	case "SodaMusic":
		if !numericID(h.ID) {
			return nil, errors.New("invalid Soda Music ID")
		}
		v, err := c.json(ctx, "https://beta-luna.douyin.com/luna/h5/seo_track?track_id="+id+"&device_platform=web", "", sodaRef, nil, nil)
		if err != nil {
			return nil, err
		}
		if text(field(v, "seo_track", "track", "id")) != h.ID {
			return nil, errors.New("Soda Music track ID mismatch")
		}
		switch text(field(v, "lyric", "type")) {
		case "krc":
			p.Format = "Krc"
		case "lrc":
		default:
			return nil, nil
		}
		p.Original = text(field(v, "lyric", "content"))
		p.Translation = text(field(v, "lyric", "translations", "cn"))
	case "Kuwo":
		if !numericID(h.ID) {
			return nil, errors.New("invalid Kuwo ID")
		}
		v, err := get("https://m.kuwo.cn/newh5/singles/songinfoandlrc?musicId=" + id)
		if err != nil {
			return nil, err
		}
		var lines strings.Builder
		for _, x := range array(field(v, "data", "lrclist")) {
			if text(field(x, "time")) == "" {
				continue
			}
			ms := int64(number(field(x, "time"))*1000 + 0.5)
			if ms > 86400000 {
				continue
			}
			fmt.Fprintf(&lines, "[%02d:%02d.%03d]%s\n", ms/60000, ms/1000%60, ms%1000, html.UnescapeString(text(field(x, "lineLyric"))))
		}
		p.Original = lines.String()
	case "Migu":
		var err error
		p.Original, err = c.migu(ctx, h.Extra["lyric"])
		if err != nil {
			return nil, err
		}
		p.Translation, _ = c.migu(ctx, h.Extra["translation"])
	default:
		return nil, errors.New("unknown lyric provider")
	}
	if !p.Instrumental && strings.TrimSpace(p.Original) == "" {
		return nil, nil
	}
	return p, nil
}

func (c *client) migu(ctx context.Context, endpoint string) (string, error) {
	if endpoint == "" {
		return "", nil
	}
	u, err := url.Parse(endpoint)
	if err != nil || !miguHost(strings.ToLower(u.Hostname())) {
		return "", errors.New("invalid Migu lyric URL")
	}
	if u.Scheme == "http" {
		u.Scheme = "https"
	}
	b, err := c.bytes(ctx, u.String(), "", "https://app.c.nf.migu.cn/", nil, nil)
	return string(b), err
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
