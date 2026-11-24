package lyrics

import (
	"context"
	"errors"
	"net/url"
)

const amllBase = "https://api.amll.dev/v1/lyrics/"

func stringsFrom(v any) []string {
	a := array(v)
	out := make([]string, 0, len(a))
	for _, v := range a {
		if s := text(v); s != "" {
			out = append(out, s)
		}
	}
	return out
}

func (c *client) searchAMLL(ctx context.Context, r request) ([]hit, error) {
	q := url.Values{"pageSize": {"16"}}
	if r.Structured {
		q.Set("musicName", r.Title)
		// A precise song ID is checked locally against the returned metadata.
		// The API uses substring AND when given multiple structured fields.
		if r.Artist != "" {
			q.Set("artistName", r.Artist)
		}
	} else {
		q.Set("musicName", r.Query)
	}
	v, err := c.json(ctx, amllBase+"search?"+q.Encode(), "", "", nil, nil)
	if err != nil {
		return nil, err
	}
	if text(field(v, "status")) != "200" {
		return nil, errors.New("AMLL search rejected")
	}
	var out []hit
	for _, song := range array(field(v, "data", "items")) {
		id := text(field(song, "id"))
		artists, titles, albums := stringsFrom(field(song, "artistNames")), stringsFrom(field(song, "musicNames")), stringsFrom(field(song, "albumNames"))
		if !numericID(id) || len(artists) == 0 || len(titles) == 0 {
			continue
		}
		album := ""
		if len(albums) > 0 {
			album = albums[0]
		}
		for _, title := range titles[:min(3, len(titles))] {
			if len(out) >= 48 {
				return out, nil
			}
			out = append(out, hit{ID: id, Title: title, Artists: artists, Album: album})
		}
	}
	return out, nil
}

func (c *client) fetchAMLL(ctx context.Context, h hit) (*payload, error) {
	if !numericID(h.ID) {
		return nil, errors.New("invalid AMLL lyric ID")
	}
	v, err := c.json(ctx, amllBase+"get?id="+h.ID, "", "", nil, nil)
	if err != nil {
		return nil, err
	}
	if text(field(v, "status")) != "200" || text(field(v, "data", "id")) != h.ID {
		return nil, errors.New("AMLL response identity mismatch")
	}
	if tt := text(field(v, "data", "lyrics")); tt != "" {
		return &payload{Format: "Ttml", Original: tt}, nil
	}
	return nil, nil
}
