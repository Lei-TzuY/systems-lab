package health

import (
	"net/http"
	"net/http/httptest"
	"testing"
	"time"
)

func TestProbes(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))
	defer ts.Close()

	if !ProbeHTTP(ts.URL, 2*time.Second) {
		t.Fatalf("ProbeHTTP failed on live test server")
	}

	if ProbeTCP("127.0.0.1:59999", 100*time.Millisecond) {
		t.Fatalf("ProbeTCP on closed port should return false")
	}
}
