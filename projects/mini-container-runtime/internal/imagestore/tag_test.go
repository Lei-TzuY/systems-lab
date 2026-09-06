package imagestore

import (
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestCreateTagAlias(t *testing.T) {
	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}

	src := &state.Image{
		ID:       "img-tg-1",
		Tag:      "app:v1",
		Name:     "app:v1",
		RootFS:   tmpDir,
		LoadedAt: time.Now(),
	}
	_ = st.SaveImage(src)

	newImg, err := CreateTagAlias(st, "app:v1", "app:latest")
	if err != nil {
		t.Fatalf("CreateTagAlias error: %v", err)
	}
	if newImg.Tag != "app:latest" {
		t.Fatalf("New tag = %s, want app:latest", newImg.Tag)
	}
}
