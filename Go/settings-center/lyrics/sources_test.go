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

func TestServeMultiplexesAndCancels(t *testing.T) {
	reader, writer := io.Pipe()
	out, send := io.Pipe()
	defer writer.Close()
	defer out.Close()
	started := make(chan struct{})
	c := &client{http: &http.Client{Transport: fixtureTransport(func(r *http.Request) (*http.Response, error) {
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
	write(request{ID: 1, Op: "taskbar-search", Source: "SodaMusic", Hit: hit{Title: "slow", Extra: map[string]string{"query": "slow"}}, TimeoutMS: 3000})
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
