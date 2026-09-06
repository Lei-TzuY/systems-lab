package imagestore

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestRemoveImageIfMatchRejectsReplacedTagGeneration(t *testing.T) {
	base := t.TempDir()
	st, err := state.Open(filepath.Join(base, "state"))
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	oldRoot := filepath.Join(base, "old-root")
	newRoot := filepath.Join(base, "new-root")
	for _, root := range []string{oldRoot, newRoot} {
		if err := os.MkdirAll(root, 0o700); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(filepath.Join(root, "sentinel"), []byte(root), 0o600); err != nil {
			t.Fatal(err)
		}
	}

	oldImg := &state.Image{
		ID:         "old-prune-generation",
		Name:       "app:latest",
		Repository: "app",
		Tag:        "latest",
		RootFS:     oldRoot,
		LoadedAt:   time.Now(),
	}
	if err := st.PublishImage(oldImg); err != nil {
		t.Fatal(err)
	}
	expected, err := st.GetImage(oldImg.Name)
	if err != nil {
		t.Fatal(err)
	}

	newImg := &state.Image{
		ID:         "new-prune-generation",
		Name:       oldImg.Name,
		Repository: "app",
		Tag:        "latest",
		RootFS:     newRoot,
		LoadedAt:   time.Now(),
	}
	if err := st.PublishImage(newImg); err != nil {
		t.Fatal(err)
	}

	if _, err := RemoveImageIfMatch(st, oldImg.Name, expected, true); err == nil || !strings.Contains(err.Error(), "changed after prune snapshot") {
		t.Fatalf("stale snapshot removal error=%v", err)
	}
	current, err := st.GetImage(oldImg.Name)
	if err != nil {
		t.Fatalf("replacement metadata disappeared: %v", err)
	}
	if current.ID != newImg.ID || filepath.Clean(current.RootFS) != filepath.Clean(newRoot) {
		t.Fatalf("current image=%+v, want replacement %+v", current, newImg)
	}
	if _, err := os.Stat(filepath.Join(newRoot, "sentinel")); err != nil {
		t.Fatalf("replacement payload was removed by stale prune: %v", err)
	}
}
