package daemon

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/stats"
	"minicontainer/internal/state"
)

func newTestServer(t *testing.T) *Server {
	t.Helper()
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatalf("state.Open: %v", err)
	}
	return &Server{store: st}
}

func TestHandleStatsEmptyStore(t *testing.T) {
	srv := newTestServer(t)
	req := httptest.NewRequest(http.MethodGet, "/v1/stats?interval=2s", nil)
	rec := httptest.NewRecorder()

	start := time.Now()
	srv.handleStats(rec, req)
	if elapsed := time.Since(start); elapsed >= time.Second {
		t.Fatalf("empty stats request unnecessarily waited for sample interval: %s", elapsed)
	}
	if rec.Code != http.StatusOK {
		t.Fatalf("status = %d, body=%s", rec.Code, rec.Body.String())
	}
	if got := rec.Header().Get("Content-Type"); !strings.HasPrefix(got, "application/json") {
		t.Fatalf("Content-Type = %q", got)
	}

	var got []stats.ContainerStat
	if err := json.Unmarshal(rec.Body.Bytes(), &got); err != nil {
		t.Fatalf("decode stats response: %v", err)
	}
	if got == nil || len(got) != 0 {
		t.Fatalf("expected empty JSON array, got %#v", got)
	}
}

func TestHandleStatsRejectsInvalidIntervals(t *testing.T) {
	srv := newTestServer(t)
	for _, raw := range []string{"nope", "0s", "9ms", "6s", "-1s"} {
		t.Run(raw, func(t *testing.T) {
			req := httptest.NewRequest(http.MethodGet, "/v1/stats?interval="+raw, nil)
			rec := httptest.NewRecorder()
			srv.handleStats(rec, req)
			if rec.Code != http.StatusBadRequest {
				t.Fatalf("status = %d, want %d; body=%s", rec.Code, http.StatusBadRequest, rec.Body.String())
			}
		})
	}
}

func TestHandleStatsRejectsNonGET(t *testing.T) {
	srv := newTestServer(t)
	req := httptest.NewRequest(http.MethodPost, "/v1/stats", nil)
	rec := httptest.NewRecorder()
	srv.handleStats(rec, req)

	if rec.Code != http.StatusMethodNotAllowed {
		t.Fatalf("status = %d, want %d", rec.Code, http.StatusMethodNotAllowed)
	}
	if got := rec.Header().Get("Allow"); got != http.MethodGet {
		t.Fatalf("Allow = %q, want GET", got)
	}
}
