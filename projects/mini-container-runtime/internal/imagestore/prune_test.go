package imagestore

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestPruneOrphanLayersRemovesUnreferencedManagedPayload(t *testing.T) {
	base := t.TempDir()
	st, err := state.Open(base)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}
	defer st.Close()

	id := "orphan-1"
	rootFS := filepath.Join(base, "images", id, "rootfs")
	if err := os.MkdirAll(rootFS, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(rootFS, "payload"), []byte("payload\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	dangling := &state.Image{
		ID:       id,
		Tag:      "<none>",
		Name:     "orphan-1",
		RootFS:   rootFS,
		Size:     2048,
		LoadedAt: time.Now(),
	}
	if err := st.SaveImage(dangling); err != nil {
		t.Fatal(err)
	}

	count, bytes, err := PruneOrphanLayers(st)
	if err != nil {
		t.Fatalf("PruneOrphanLayers error: %v", err)
	}
	if count != 1 || bytes != 2048 {
		t.Fatalf("Prune result count=%d, bytes=%d, want 1, 2048", count, bytes)
	}
	if _, err := os.Lstat(rootFS); !os.IsNotExist(err) {
		t.Fatalf("dangling managed rootfs still exists after prune: %v", err)
	}
	if _, err := st.GetImage(dangling.Name); err == nil {
		t.Fatal("dangling metadata still exists after prune")
	}
}

func TestPruneOrphanLayersPreservesPayloadReferencedByLiveAlias(t *testing.T) {
	base := t.TempDir()
	st, err := state.Open(base)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	id := "shared-prune-id"
	rootFS := filepath.Join(base, "images", id, "rootfs")
	if err := os.MkdirAll(rootFS, 0o755); err != nil {
		t.Fatal(err)
	}
	sentinel := filepath.Join(rootFS, "sentinel")
	if err := os.WriteFile(sentinel, []byte("keep\n"), 0o600); err != nil {
		t.Fatal(err)
	}

	dangling := &state.Image{ID: id, Name: "shared:old", Tag: "<none>", RootFS: rootFS, Size: 4096, LoadedAt: time.Now()}
	live := &state.Image{ID: id, Name: "shared:latest", Repository: "shared", Tag: "latest", RootFS: rootFS, Size: 4096, LoadedAt: time.Now()}
	if err := st.SaveImage(dangling); err != nil {
		t.Fatal(err)
	}
	if err := st.SaveImage(live); err != nil {
		t.Fatal(err)
	}

	count, bytes, err := PruneOrphanLayers(st)
	if err != nil {
		t.Fatalf("PruneOrphanLayers error: %v", err)
	}
	if count != 1 || bytes != 0 {
		t.Fatalf("Prune result count=%d, bytes=%d, want 1, 0 while payload remains referenced", count, bytes)
	}
	if data, err := os.ReadFile(sentinel); err != nil || string(data) != "keep\n" {
		t.Fatalf("shared payload changed: data=%q err=%v", data, err)
	}
	if _, err := st.GetImage(dangling.Name); err == nil {
		t.Fatal("dangling alias still exists after prune")
	}
	if _, err := st.GetImage(live.Name); err != nil {
		t.Fatalf("live alias disappeared after prune: %v", err)
	}
}

func TestPruneOrphanLayersPropagatesDestructiveRemovalFailure(t *testing.T) {
	base := t.TempDir()
	st, err := state.Open(base)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	id := "malformed-managed-prune"
	malformedRootFS := filepath.Join(base, "images", id, "not-rootfs")
	if err := os.MkdirAll(malformedRootFS, 0o755); err != nil {
		t.Fatal(err)
	}
	sentinel := filepath.Join(malformedRootFS, "sentinel")
	if err := os.WriteFile(sentinel, []byte("keep\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	img := &state.Image{ID: id, Name: "malformed:old", Tag: "<none>", RootFS: malformedRootFS, Size: 8192, LoadedAt: time.Now()}
	if err := st.SaveImage(img); err != nil {
		t.Fatal(err)
	}

	count, bytes, err := PruneOrphanLayers(st)
	if err == nil || !strings.Contains(err.Error(), "prune dangling image") || !strings.Contains(err.Error(), "does not match expected") {
		t.Fatalf("PruneOrphanLayers error=%v, want managed ownership failure", err)
	}
	if count != 0 || bytes != 0 {
		t.Fatalf("partial prune result count=%d bytes=%d, want 0,0", count, bytes)
	}
	if _, err := st.GetImage(img.Name); err != nil {
		t.Fatalf("metadata disappeared after failed prune: %v", err)
	}
	if data, err := os.ReadFile(sentinel); err != nil || string(data) != "keep\n" {
		t.Fatalf("payload changed after failed prune: data=%q err=%v", data, err)
	}
}
