package imagestore

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"minicontainer/internal/state"
)

func TestRemoveManagedImageClearsCleanupOwnershipOnSuccess(t *testing.T) {
	base := t.TempDir()
	st, err := state.Open(base)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	id := "successful-cleanup-id"
	rootFS := filepath.Join(base, "images", id, "rootfs")
	if err := os.MkdirAll(rootFS, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(rootFS, "payload"), []byte("payload\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	img := &state.Image{ID: id, Name: "successful:old", Tag: "<none>", RootFS: rootFS}
	if err := st.SaveImage(img); err != nil {
		t.Fatal(err)
	}

	if _, err := RemoveImage(st, img.Name, true); err != nil {
		t.Fatalf("RemoveImage: %v", err)
	}
	if _, err := os.Lstat(rootFS); !os.IsNotExist(err) {
		t.Fatalf("managed rootfs still exists after removal: %v", err)
	}
	cleanups, err := st.ListImageCleanups()
	if err != nil {
		t.Fatal(err)
	}
	if len(cleanups) != 0 {
		t.Fatalf("successful removal left cleanup ownership: %+v", cleanups)
	}
}

func TestRecoverPendingManagedImageCleanupAfterStoreReopen(t *testing.T) {
	base := t.TempDir()
	st, err := state.Open(base)
	if err != nil {
		t.Fatal(err)
	}

	id := "recover-cleanup-id"
	rootFS := filepath.Join(base, "images", id, "rootfs")
	if err := os.MkdirAll(rootFS, 0o755); err != nil {
		t.Fatal(err)
	}
	sentinel := filepath.Join(rootFS, "sentinel")
	if err := os.WriteFile(sentinel, []byte("pending\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	img := &state.Image{ID: id, Name: "recover:old", Tag: "<none>", RootFS: rootFS}
	if err := st.SaveImage(img); err != nil {
		t.Fatal(err)
	}
	expected, err := st.GetImage(img.Name)
	if err != nil {
		t.Fatal(err)
	}
	cleanup := state.ImageCleanup{ID: expected.ID, RootFS: expected.RootFS}
	if _, armed, err := st.DeleteImageIfMatchWithCleanup(img.Name, expected, cleanup); err != nil || !armed {
		t.Fatalf("arm cleanup: armed=%v err=%v", armed, err)
	}
	if _, err := os.Stat(sentinel); err != nil {
		t.Fatalf("payload disappeared before recovery: %v", err)
	}
	if err := st.Close(); err != nil {
		t.Fatal(err)
	}

	reopened, err := state.Open(base)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	if err := recoverPendingManagedImageCleanups(reopened); err != nil {
		t.Fatalf("recoverPendingManagedImageCleanups: %v", err)
	}
	if _, err := os.Lstat(rootFS); !os.IsNotExist(err) {
		t.Fatalf("pending rootfs still exists after recovery: %v", err)
	}
	cleanups, err := reopened.ListImageCleanups()
	if err != nil {
		t.Fatal(err)
	}
	if len(cleanups) != 0 {
		t.Fatalf("cleanup ownership remained after recovery: %+v", cleanups)
	}
}

func TestRecoverPendingManagedImageCleanupPreservesReferencedPayload(t *testing.T) {
	base := t.TempDir()
	st, err := state.Open(base)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	id := "referenced-cleanup-id"
	rootFS := filepath.Join(base, "images", id, "rootfs")
	if err := os.MkdirAll(rootFS, 0o755); err != nil {
		t.Fatal(err)
	}
	sentinel := filepath.Join(rootFS, "sentinel")
	if err := os.WriteFile(sentinel, []byte("keep\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	img := &state.Image{ID: id, Name: "referenced:latest", Tag: "latest", RootFS: rootFS}
	if err := st.SaveImage(img); err != nil {
		t.Fatal(err)
	}

	// Simulate the crash window after the cleanup sidecar became durable but
	// before metadata deletion. State readers intentionally ignore this non-JSON
	// sidecar, while recovery must see the still-live metadata reference.
	cleanup := state.ImageCleanup{ID: id, RootFS: rootFS}
	data, err := json.Marshal(cleanup)
	if err != nil {
		t.Fatal(err)
	}
	sum := sha256.Sum256([]byte(id + "\x00" + filepath.Clean(rootFS)))
	cleanupName := "cleanup-" + hex.EncodeToString(sum[:]) + ".image-cleanup"
	if err := os.WriteFile(filepath.Join(base, "images", cleanupName), data, 0o600); err != nil {
		t.Fatal(err)
	}

	if err := recoverPendingManagedImageCleanups(st); err != nil {
		t.Fatalf("recoverPendingManagedImageCleanups: %v", err)
	}
	if data, err := os.ReadFile(sentinel); err != nil || string(data) != "keep\n" {
		t.Fatalf("referenced payload changed during recovery: data=%q err=%v", data, err)
	}
	if _, err := st.GetImage(img.Name); err != nil {
		t.Fatalf("referenced metadata disappeared during recovery: %v", err)
	}
	cleanups, err := st.ListImageCleanups()
	if err != nil {
		t.Fatal(err)
	}
	if len(cleanups) != 0 {
		t.Fatalf("stale cleanup ownership remained after preserving reference: %+v", cleanups)
	}
}
