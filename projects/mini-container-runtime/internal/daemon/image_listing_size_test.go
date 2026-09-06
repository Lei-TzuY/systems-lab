package daemon

import (
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestHandleListImagesSurfacesFallbackSizeFailure(t *testing.T) {
	base := t.TempDir()
	st, err := state.Open(filepath.Join(base, "store"))
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	missing := filepath.Join(base, "missing-rootfs")
	if err := st.SaveImage(&state.Image{
		ID:       "daemon-size-error",
		Name:     "broken:latest",
		Tag:      "latest",
		RootFS:   missing,
		Size:     0,
		LoadedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}

	srv := &Server{store: st}
	recorder := httptest.NewRecorder()
	request := httptest.NewRequest(http.MethodGet, "/v1/images/json", nil)
	srv.handleListImages(recorder, request)

	if recorder.Code != http.StatusInternalServerError {
		t.Fatalf("status=%d body=%s, want 500", recorder.Code, recorder.Body.String())
	}
	body := recorder.Body.String()
	if !strings.Contains(body, "broken:latest") || !strings.Contains(body, "missing-rootfs") {
		t.Fatalf("size failure response=%s", body)
	}
}

func TestHandleListImagesMeasuresValidZeroSizeImage(t *testing.T) {
	base := t.TempDir()
	st, err := state.Open(filepath.Join(base, "store"))
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	rootFS := filepath.Join(base, "rootfs")
	if err := os.MkdirAll(rootFS, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(rootFS, "payload"), []byte("payload"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := st.SaveImage(&state.Image{
		ID:       "daemon-size-ok",
		Name:     "ok:latest",
		Tag:      "latest",
		RootFS:   rootFS,
		Size:     0,
		LoadedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}

	srv := &Server{store: st}
	recorder := httptest.NewRecorder()
	request := httptest.NewRequest(http.MethodGet, "/v1/images/json", nil)
	srv.handleListImages(recorder, request)

	if recorder.Code != http.StatusOK {
		t.Fatalf("status=%d body=%s, want 200", recorder.Code, recorder.Body.String())
	}
	if !strings.Contains(recorder.Body.String(), `"size":7`) {
		t.Fatalf("measured image response=%s", recorder.Body.String())
	}
}
