package imagestore

import (
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestGetImageHistory(t *testing.T) {
	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	img := &state.Image{
		ID:       "img-hist-1",
		Tag:      "hist:v1",
		Name:     "hist:v1",
		Size:     4096,
		Cmd:      []string{"sh"},
		LoadedAt: time.Now(),
	}
	_ = st.SaveImage(img)

	history, err := GetImageHistory(st, "hist:v1")
	if err != nil {
		t.Fatalf("GetImageHistory error: %v", err)
	}
	if len(history) == 0 {
		t.Fatalf("History is empty")
	}
}
