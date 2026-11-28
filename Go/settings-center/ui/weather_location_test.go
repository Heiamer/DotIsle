package ui

import (
	"context"
	"fmt"
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestLocateWeatherCityValidatesExplicitCityResponse(t *testing.T) {
	for _, tc := range []struct {
		raw, city, label string
		ok               bool
	}{
		{`{"nearest_area":[{"areaName":[{"value":" Shanghai "}],"region":[{"value":"Shanghai"}],"country":[{"value":"China"}]}]}`, "Shanghai", "Shanghai · China", true},
		{`{"nearest_area":[]}`, "", "", false},
		{`{"nearest_area":[{"areaName":[{"value":"a\nb"}]}]}`, "", "", false},
	} {
		server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { fmt.Fprint(w, tc.raw) }))
		city, label, err := locateWeatherCity(context.Background(), server.Client(), server.URL)
		server.Close()
		if (err == nil) != tc.ok || city != tc.city || label != tc.label {
			t.Fatalf("got %q/%q: %v; expected %+v", city, label, err, tc)
		}
	}
}

func TestQuotaPercentInputIsStrict(t *testing.T) {
	for _, v := range []string{"", "101", "-1", "20.5", "20%", "²"} {
		if _, ok := quotaPercent(v); ok {
			t.Errorf("accepted %q", v)
		}
	}
	for _, v := range []string{"0", "20", "100"} {
		if _, ok := quotaPercent(v); !ok {
			t.Errorf("rejected %q", v)
		}
	}
}
