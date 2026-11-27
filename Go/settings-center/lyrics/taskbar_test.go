package lyrics

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"os"
	"reflect"
	"strings"
	"testing"
)

func TestTaskbarOriginalCppParity(t *testing.T) {
	data, err := os.ReadFile("testdata/taskbar-parity.json")
	if err != nil {
		t.Fatal(err)
	}
	var cases []struct {
		Title, Artist, CandidateTitle, CandidateArtist string
		Duration, CandidateDuration                    int64
		Score                                          int
		Variants                                       []struct{ Title, Artist, Query string }
	}
	if err = json.Unmarshal(data, &cases); err != nil {
		t.Fatal(err)
	}
	for i, c := range cases {
		r := request{Title: c.Title, Artist: c.Artist, DurationMS: c.Duration}
		h := hit{Title: c.CandidateTitle, Artists: []string{c.CandidateArtist}, DurationMS: c.CandidateDuration}
		if got := tbScore(r, h); got != c.Score {
			t.Errorf("case %d %q/%q vs %q/%q score=%d want=%d", i, c.Title, c.Artist, c.CandidateTitle, c.CandidateArtist, got, c.Score)
		}
		variants := tbVariants(r)
		if len(variants) != len(c.Variants) {
			t.Errorf("case %d variant count=%d want=%d: %#v", i, len(variants), len(c.Variants), variants)
			continue
		}
		for j, h := range variants {
			v := c.Variants[j]
			if h.Title != v.Title || strings.Join(h.Artists, "/") != v.Artist || h.Extra["query"] != v.Query {
				t.Errorf("case %d variant %d: %#v want %#v", i, j, h, v)
			}
		}
	}
}

type tbTransport func(*http.Request) (*http.Response, error)

func (f tbTransport) RoundTrip(r *http.Request) (*http.Response, error) { return f(r) }
func tbResponse(s string) *http.Response {
	return &http.Response{StatusCode: 200, Body: io.NopCloser(strings.NewReader(s)), Header: http.Header{}}
}
func TestTaskbarEndpointContractsAndRanking(t *testing.T) {
	c := newClient()
	var urls []string
	c.http.Transport = tbTransport(func(r *http.Request) (*http.Response, error) {
		urls = append(urls, r.URL.String())
		if r.Method != "GET" {
			t.Fatal(r.Method)
		}
		switch r.URL.Hostname() {
		case "c.y.qq.com":
			if r.URL.Path == "/soso/fcgi-bin/client_search_cp" {
				if r.URL.Query().Get("n") != "10" || r.URL.Query().Get("new_json") != "1" {
					t.Fatal(r.URL)
				}
				return tbResponse(`{"data":{"song":{"list":[{"id":1,"mid":"a","name":"Song","singer":[{"name":"A"}],"interval":200},{"id":2,"mid":"b","name":"Song","singer":[{"name":"A"}],"interval":204},{"id":3,"mid":"a","name":"duplicate"},{"id":4,"mid":"wrong","name":"Song (Live)","singer":[{"name":"A"}],"interval":200}]}}}`), nil
			}
			if r.URL.Path == "/qqmusic/fcgi-bin/lyric_download.fcg" {
				if r.URL.Query().Get("lrctype") != "4" || r.Header.Get("Referer") != "https://c.y.qq.com/" {
					t.Fatal(r)
				}
				return tbResponse(`<content><![CDATA[abcd]]></content>`), nil
			}
			if r.URL.Query().Get("songmid") != "a" || r.URL.Query().Get("nobase64") != "0" || r.Header.Get("Referer") != "https://y.qq.com/portal/player.html" {
				t.Fatal(r)
			}
			return tbResponse(`{"retcode":0,"lyric":"WzAwOjAxXXg="}`), nil
		case "mobilecdn.kugou.com":
			if r.URL.Scheme != "http" || r.URL.Query().Get("pagesize") != "10" {
				t.Fatal(r.URL)
			}
			return tbResponse(`{"data":{"info":[{"hash":"abc","songname":"Song","singername":"A","duration":200}]}}`), nil
		case "lyrics.kugou.com":
			if r.URL.Scheme != "http" {
				t.Fatal(r.URL)
			}
			return tbResponse(`{"candidates":[{"id":"1","accesskey":"a"},{"id":"2","accesskey":"b"},{"id":"3","accesskey":"c"},{"id":"4","accesskey":"d"}]}`), nil
		}
		t.Fatalf("unexpected URL %s", r.URL)
		return nil, nil
	})
	r := request{Source: "QQMusic", Title: "Song", Artist: "A", DurationMS: 200000, Hit: tbVariants(request{Title: "Song", Artist: "A"})[0]}
	h, err := c.tbSearch(context.Background(), r)
	if err != nil || len(h) != 2 || h[0].ID != "1" || h[1].ID != "2" {
		t.Fatalf("%+v %v", h, err)
	}
	r.Hit = h[0]
	r.Op = "taskbar-qrc"
	p, err := c.tbFetch(context.Background(), r)
	if err != nil || p.Original != "abcd" {
		t.Fatal(p, err)
	}
	r.Op = "taskbar-fetch"
	p, err = c.tbFetch(context.Background(), r)
	if err != nil || p.Original != "[00:01]x" {
		t.Fatal(p, err)
	}
	r.Source = "Kugou"
	_, err = c.tbSearch(context.Background(), r)
	if err != nil {
		t.Fatal(err)
	}
	candidates, err := c.tbKugouCandidates(context.Background(), r)
	if err != nil || len(candidates) != 3 {
		t.Fatal(candidates, err)
	}
}
func TestTaskbarLyricsfileValidationAndTiming(t *testing.T) {
	doc := tbLyricsfile("version: '1.0'\nlines:\n - start_ms: 1000\n   end_ms: 4000\n   words:\n    - {start_ms: 1000, text: 'hello '}\n    - {start_ms: 2000, text: world}\n")
	if doc == nil || doc.Lines[0].Text != "hello world" || doc.Lines[0].Words[0].Until != 2000 || doc.Lines[0].Words[1].Until != 4000 {
		t.Fatal(doc)
	}
	for _, s := range []string{"version: '1.0'\nversion: '1.0'\nlines: []", "version: '1.0'\nlines: []\n---\nx: y", "version: '2.0'\nlines: []"} {
		if tbLyricsfile(s) != nil {
			t.Fatal("invalid document accepted", s)
		}
	}
	if !reflect.DeepEqual(tbPreview(200000, 60000, 30000, 30000), int64(60000)) || tbPreview(200000, 60000, 30000, 200000) != 0 {
		t.Fatal("preview mismatch")
	}
}
