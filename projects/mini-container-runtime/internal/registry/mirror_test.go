package registry

import (
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestStartMirrorServer(t *testing.T) {
	srv, err := StartMirrorServer(5000, t.TempDir())
	if err != nil {
		t.Fatalf("StartMirrorServer error: %v", err)
	}

	req := httptest.NewRequest(http.MethodGet, "/v2/", nil)
	rec := httptest.NewRecorder()

	srv.Handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("Registry mirror status = %d, want %d", rec.Code, http.StatusOK)
	}
}
