package system

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestSystemPruneAllRemovesDanglingManagedImage(t *testing.T) {
	base := t.TempDir()
	home := filepath.Join(base, "home")
	if err := os.MkdirAll(home, 0o700); err != nil {
		t.Fatal(err)
	}
	t.Setenv("HOME", home)

	stateRoot := filepath.Join(base, "store")
	st, err := state.Open(stateRoot)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const id = "dangling-system-prune"
	rootFS := filepath.Join(stateRoot, "images", id, "rootfs")
	if err := os.MkdirAll(rootFS, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(rootFS, "payload"), []byte("old generation"), 0o600); err != nil {
		t.Fatal(err)
	}
	dangling := &state.Image{
		ID:       id,
		Name:     "",
		Tag:      "<none>",
		RootFS:   rootFS,
		Size:     14,
		LoadedAt: time.Now(),
	}
	if err := st.SaveImage(dangling); err != nil {
		t.Fatal(err)
	}

	res, err := SystemPrune(st, true)
	if err != nil {
		t.Fatalf("SystemPrune dangling image: %v", err)
	}
	if res.ImagesReclaimed != 1 {
		t.Fatalf("images reclaimed=%d, want 1", res.ImagesReclaimed)
	}
	if _, err := st.GetImage(id); err == nil {
		t.Fatal("dangling image metadata still resolves after system prune")
	}
	if _, err := os.Lstat(rootFS); !os.IsNotExist(err) {
		t.Fatalf("dangling managed payload still exists after system prune: %v", err)
	}
}

func TestSystemPruneReportsVolumePruneFailure(t *testing.T) {
	base := t.TempDir()
	home := filepath.Join(base, "home")
	if err := os.MkdirAll(home, 0o700); err != nil {
		t.Fatal(err)
	}
	t.Setenv("HOME", home)

	st, err := state.Open(filepath.Join(base, "store"))
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	// A valid-name managed volume with its data directory but missing metadata
	// is corrupt state. PruneVolumes reports it; SystemPrune must not hide it.
	dataDir := filepath.Join(home, ".minicontainer", "volumes", "broken", "_data")
	if err := os.MkdirAll(dataDir, 0o755); err != nil {
		t.Fatal(err)
	}

	res, err := SystemPrune(st, false)
	if err == nil {
		t.Fatal("SystemPrune unexpectedly reported success for corrupt volume state")
	}
	if res == nil {
		t.Fatal("SystemPrune returned nil partial progress result")
	}
	if res.VolumesReclaimed != 0 {
		t.Fatalf("volumes reclaimed=%d, want 0", res.VolumesReclaimed)
	}
	if !strings.Contains(err.Error(), "prune volumes during system prune") || !strings.Contains(err.Error(), "broken") {
		t.Fatalf("volume prune error=%v", err)
	}
}
