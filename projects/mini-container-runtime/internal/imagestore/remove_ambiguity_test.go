package imagestore

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"minicontainer/internal/state"
)

func writeRootFSSentinel(t *testing.T, dir, name string) string {
	t.Helper()
	if err := os.MkdirAll(dir, 0o700); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(dir, name)
	if err := os.WriteFile(path, []byte("keep"), 0o600); err != nil {
		t.Fatal(err)
	}
	return path
}

func TestRemoveImageAmbiguousPrefixPreservesMetadataAndRootFS(t *testing.T) {
	store, err := state.Open(filepath.Join(t.TempDir(), "state"))
	if err != nil {
		t.Fatal(err)
	}
	rootA := filepath.Join(t.TempDir(), "root-a")
	rootB := filepath.Join(t.TempDir(), "root-b")
	sentinelA := writeRootFSSentinel(t, rootA, "sentinel")
	sentinelB := writeRootFSSentinel(t, rootB, "sentinel")
	first := &state.Image{ID: "abc111111111", Name: "first:latest", RootFS: rootA}
	second := &state.Image{ID: "abc222222222", Name: "second:latest", RootFS: rootB}
	if err := store.SaveImage(first); err != nil {
		t.Fatal(err)
	}
	if err := store.SaveImage(second); err != nil {
		t.Fatal(err)
	}

	if _, err := RemoveImage(store, "abc", true); err == nil || !strings.Contains(err.Error(), "ambiguous image ID prefix") {
		t.Fatalf("RemoveImage ambiguous prefix error=%v", err)
	}
	for _, path := range []string{sentinelA, sentinelB} {
		if _, err := os.Stat(path); err != nil {
			t.Fatalf("rootfs changed after rejected ambiguous removal: %s: %v", path, err)
		}
	}
	for _, name := range []string{first.Name, second.Name} {
		if _, err := store.GetImage(name); err != nil {
			t.Fatalf("metadata %q changed after rejected removal: %v", name, err)
		}
	}
}

func TestRemoveImageSharedIDRequiresExactTagAndPreservesRootFS(t *testing.T) {
	store, err := state.Open(filepath.Join(t.TempDir(), "state"))
	if err != nil {
		t.Fatal(err)
	}
	root := filepath.Join(t.TempDir(), "root")
	sentinel := writeRootFSSentinel(t, root, "sentinel")
	const id = "abcdef123456"
	for _, name := range []string{"app:v1", "app:latest"} {
		if err := store.SaveImage(&state.Image{ID: id, Name: name, RootFS: root}); err != nil {
			t.Fatal(err)
		}
	}

	if _, err := RemoveImage(store, id, true); err == nil || !strings.Contains(err.Error(), "multiple tags") {
		t.Fatalf("RemoveImage shared ID error=%v", err)
	}
	if _, err := os.Stat(sentinel); err != nil {
		t.Fatalf("shared rootfs changed after rejected ID removal: %v", err)
	}
	for _, name := range []string{"app:v1", "app:latest"} {
		if _, err := store.GetImage(name); err != nil {
			t.Fatalf("alias %q changed after rejected ID removal: %v", name, err)
		}
	}
}
