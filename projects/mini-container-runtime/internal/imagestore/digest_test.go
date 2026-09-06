package imagestore

import (
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestSearchImageByDigest(t *testing.T) {
	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	img := &state.Image{
		ID:       "a1b2c3d4e5f6",
		Tag:      "digesttest:v1",
		Name:     "digesttest:v1",
		LoadedAt: time.Now(),
	}
	_ = st.SaveImage(img)

	matches, err := SearchImageByDigest(st, "a1b2c3")
	if err != nil {
		t.Fatalf("SearchImageByDigest error: %v", err)
	}
	if len(matches) != 1 {
		t.Fatalf("Matches count = %d, want 1", len(matches))
	}
}
