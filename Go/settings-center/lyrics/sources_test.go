package lyrics

import (
	"bufio"
	"context"
	"encoding/json"
	"io"
	"net/http"
	"strings"
	"testing"
	"time"
)

type fixtureTransport func(*http.Request) (*http.Response, error)

func (f fixtureTransport) RoundTrip(r *http.Request) (*http.Response, error) { return f(r) }
func response(s string) (*http.Response, error) {
	return &http.Response{StatusCode: 200, Body: io.NopCloser(strings.NewReader(s)), Header: make(http.Header)}, nil
}

func TestSources(t *testing.T) {
	for _, tc := range []struct{ source, search, fetch, id, format string }{
		{"Netease", `{"code":200,"result":{"songs":[{"id":123,"name":"Song","ar":[{"name":"Artist"}],"dt":185000}]}}`, `{"code":200,"lrc":{"lyric":"[00:01.00]A"}}`, "123", "Lrc"},
		{"QQMusic", `{"req":{"data":{"body":{"song":{"list":[{"id":123,"title":"Song","mid":"mid123","interval":185,"singer":[{"name":"Artist"}]}]}}}}}`, `<!--<contentts><![CDATA[CCDD]]></contentts><content type="1"><![CDATA[AABB]]></content>-->`, "123", "Qrc"},
		{"LRCLIB", `[{"id":123,"trackName":"Song","artistName":"Artist","duration":185}]`, `{"syncedLyrics":"[00:01.00]A"}`, "123", "Lrc"},
		{"SodaMusic", `{"result_groups":[{"id":"tracks","data":[{"entity":{"track":{"id":"123","name":"Song","artists":[{"name":"Artist"}],"duration":185000,"preview":{"start":60000,"duration":30000}}}}]}]}`, `{"seo_track":{"track":{"id":"123"}},"lyric":{"type":"krc","content":"[60000,2000]<0,1000,0>A"}}`, "123", "Krc"},
		{"Kugou", `{"candidates":[{"id":123,"song":"Song","singer":"Artist","duration":185000,"accesskey":"token"}]}`, `{"content":"encoded-lyric"}`, "123", "Krc"},
		{"Kuwo", `{'abslist':[{'MUSICRID':'MUSIC_123','SONGNAME':'Song','ARTIST':'Artist','DURATION':'185'}]}`, `{"data":{"lrclist":[{"time":"1.1","lineLyric":"A &amp; B"}]}}`, "123", "Lrc"},
		{"Migu", `{"code":"000000","songResultData":{"resultList":[[{"copyrightId":"123","name":"Song","singerList":[{"singerName":"Artist"}],"duration":185,"lrcUrl":"http://cdn.migumusic.cn/lyrics/123"}]]}}`, `[00:01.00]A`, "123", "Lrc"},
		{"AMLL", `{"status":200,"data":{"items":[{"id":7192686136682066,"musicNames":["Song"],"artistNames":["Artist"]}]}}`, `{"status":200,"data":{"id":7192686136682066,"lyrics":"<tt><body><div><p begin='1s'>A</p></div></body></tt>"}}`, "7192686136682066", "Ttml"},
	} {
		t.Run(tc.source, func(t *testing.T) {
			responses := []string{tc.search, tc.fetch}
			c := &client{&http.Client{Transport: fixtureTransport(func(r *http.Request) (*http.Response, error) {
				if len(responses) == 0 {
					t.Fatalf("unexpected request %s", r.URL.Host)
				}
				if r.URL.Scheme != "https" {
					t.Fatalf("non TLS request: %s", r.URL)
				}
				s := responses[0]
				responses = responses[1:]
				return response(s)
			})}}
			r := request{Source: tc.source, Query: "Song Artist", Title: "Song", Artist: "Artist", DurationMS: 185000, Structured: true}
			h, err := c.search(t.Context(), r)
			if err != nil || len(h) != 1 || h[0].ID != tc.id || h[0].Title != "Song" || len(h[0].Artists) != 1 || h[0].Artists[0] != "Artist" {
				t.Fatalf("search: %+v %v", h, err)
			}
			r.Hit = h[0]
			p, err := c.fetch(t.Context(), r)
			if err != nil || p == nil || p.Format != tc.format || p.Original == "" {
				t.Fatalf("fetch: %+v %v", p, err)
			}
			if tc.source == "QQMusic" && (p.Original != "AABB" || p.Translation != "CCDD") {
				t.Fatal("QRC fields lost")
			}
		})
	}
}

func TestEmptySodaAndUntrustedMigu(t *testing.T) {
	c := &client{&http.Client{Transport: fixtureTransport(func(r *http.Request) (*http.Response, error) {
		if r.URL.Host != "api.qishui.com" {
			t.Fatalf("request to %s", r.URL.Host)
		}
		return response("")
	})}}
	if _, err := c.search(t.Context(), request{Source: "SodaMusic", Query: "Song"}); err == nil {
		t.Fatal("empty body accepted")
	}
	if _, err := c.migu(t.Context(), "https://migu.cn.attacker.invalid/lyrics"); err == nil {
		t.Fatal("untrusted domain accepted")
	}
}

func TestServeMultiplexesAndCancels(t *testing.T) {
	reader, writer := io.Pipe()
	out, send := io.Pipe()
	defer writer.Close()
	defer out.Close()
	started := make(chan struct{})
	c := &client{&http.Client{Transport: fixtureTransport(func(r *http.Request) (*http.Response, error) {
		close(started)
		<-r.Context().Done()
		return nil, r.Context().Err()
	})}}
	done := make(chan error, 1)
	go func() { done <- serve(reader, send, c); send.Close() }()
	enc, scan := json.NewEncoder(writer), bufio.NewScanner(out)
	write := func(r request) {
		r.Version = Protocol
		if err := enc.Encode(r); err != nil {
			t.Fatal(err)
		}
	}
	read := func(id uint64) result {
		if !scan.Scan() {
			t.Fatal("worker closed output")
		}
		var r result
		if err := json.Unmarshal(scan.Bytes(), &r); err != nil || r.ID != id {
			t.Fatalf("response: %+v %v", r, err)
		}
		return r
	}
	write(request{ID: 1, Op: "search", Source: "SodaMusic", Query: "slow", TimeoutMS: 3000})
	<-started
	write(request{ID: 2, Op: "ping", TimeoutMS: 3000})
	if r := read(2); r.Error != "" {
		t.Fatal(r.Error)
	}
	write(request{ID: 1, Op: "cancel"})
	if r := read(1); r.Error != context.Canceled.Error() {
		t.Fatalf("cancel: %+v", r)
	}
	writer.Close()
	select {
	case err := <-done:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(time.Second):
		t.Fatal("worker did not exit")
	}
}
