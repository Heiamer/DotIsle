// Package lyrics implements the bounded online source worker for the island.
package lyrics

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"io"
	"sync"
	"time"
)

const Protocol = 1

type request struct {
	ID                                      uint64
	Version                                 int
	Op, Source, Query, Title, Artist, Album string
	Structured                              bool
	DurationMS, TimeoutMS                   int64
	Hit                                     hit
}

type hit struct {
	ID, Title, Album string
	Artists          []string
	DurationMS       int64
	Extra            map[string]string
}

type payload struct {
	Format                 string
	Original               string
	Translation            string
	Instrumental           bool
	Fallback, Romanization string
	Document               *tbDocument
}

type result struct {
	ID      uint64
	Error   string   `json:",omitempty"`
	Hits    []hit    `json:",omitempty"`
	Payload *payload `json:",omitempty"`
}

func Serve(input io.Reader, output io.Writer) error {
	return serve(input, output, newClient())
}

func serve(input io.Reader, output io.Writer, client *client) error {
	s := bufio.NewScanner(input)
	s.Buffer(make([]byte, 4096), 128<<10)
	var writes, mu sync.Mutex
	var wg sync.WaitGroup
	active := make(map[uint64]context.CancelFunc)
	root, stop := context.WithCancel(context.Background())
	defer func() { stop(); wg.Wait() }()
	respond := func(r result) {
		writes.Lock()
		defer writes.Unlock()
		if root.Err() == nil && json.NewEncoder(output).Encode(r) != nil {
			stop()
		}
	}
	for s.Scan() {
		var req request
		if err := json.Unmarshal(s.Bytes(), &req); err != nil {
			return err
		}
		if req.Version != Protocol || req.ID == 0 {
			return errors.New("invalid lyric protocol")
		}
		if root.Err() != nil {
			return root.Err()
		}
		mu.Lock()
		if req.Op == "cancel" {
			if cancel := active[req.ID]; cancel != nil {
				cancel()
			}
			mu.Unlock()
			continue
		}
		if active[req.ID] != nil {
			mu.Unlock()
			return errors.New("duplicated lyric request")
		}
		if len(active) >= 32 {
			mu.Unlock()
			respond(result{ID: req.ID, Error: "worker busy"})
			continue
		}
		ctx, cancel := context.WithTimeout(root, time.Duration(max(1, min(30000, req.TimeoutMS)))*time.Millisecond)
		active[req.ID] = cancel
		mu.Unlock()
		wg.Go(func() {
			defer cancel()
			defer func() { mu.Lock(); delete(active, req.ID); mu.Unlock() }()
			r := result{ID: req.ID}
			var err error
			switch req.Op {
			case "taskbar-plan":
				r.Hits = tbVariants(req)
			case "taskbar-search":
				r.Hits, err = client.tbSearch(ctx, req)
			case "taskbar-kugou-candidates":
				r.Hits, err = client.tbKugouCandidates(ctx, req)
			case "taskbar-fetch", "taskbar-qrc":
				r.Payload, err = client.tbFetch(ctx, req)
			case "ping":
			default:
				err = errors.New("unknown lyric operation")
			}
			if err != nil {
				r.Error = err.Error()
			}
			respond(r)
		})
	}
	return s.Err()
}
