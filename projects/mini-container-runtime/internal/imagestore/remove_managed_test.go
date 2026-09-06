package imagestore

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"minicontainer/internal/state"
)

func TestRemoveImageRejectsInconsistentAliasesBeforeMetadataDelete(t *testing.T) {
	root := t.TempDir()
	st, err := state.Open(root)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const id = "fedcba654321"
	firstRoot := filepath.Join(t.TempDir(), "root-one")
	secondRoot := filepath.Join(t.TempDir(), "root-two")
	if err := os.MkdirAll(firstRoot, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(secondRoot, 0o755); err != nil {
		t.Fatal(err)
	}
	first := &state.Image{ID: id, Name: "app:v1", RootFS: firstRoot}
	second := &state.Image{ID: id, Name: "app:latest", RootFS: secondRoot}
	if err := st.SaveImage(first); err != nil {
		t.Fatal(err)
	}
	data, err := json.MarshalIndent(second, "", "  ")
	if err != nil {
		t.Fatal(err)
	}
	// Bypass SaveImage intentionally to model pre-hardening/corrupt metadata.
	if err := os.WriteFile(filepath.Join(root, "images", "app_latest.json"), data, 0o600); err != nil {
		t.Fatal(err)
	}

	if _, err := RemoveImage(st, first.Name, true); err == nil || !strings.Contains(err.Error(), "inconsistent image aliases") {
		t.Fatalf("inconsistent alias removal error=%v", err)
	}
	if _, err := st.GetImage(first.Name); err != nil {
		t.Fatalf("first metadata deleted despite failed preflight: %v", err)
	}
	if _, err := st.GetImage(second.Name); err != nil {
		t.Fatalf("second metadata deleted despite failed preflight: %v", err)
	}
}

func TestRemoveImageRejectsMalformedPathInsideManagedStorage(t *testing.T) {
	root := t.TempDir()
	st, err := state.Open(root)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const id = "abcdef123456"
	malformed := filepath.Join(root, "images", "different-id", "rootfs")
	if err := os.MkdirAll(malformed, 0o755); err != nil {
		t.Fatal(err)
	}
	sentinel := filepath.Join(malformed, "keep.txt")
	if err := os.WriteFile(sentinel, []byte("keep\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	img := &state.Image{ID: id, Name: "malformed:latest", RootFS: malformed}
	if err := st.SaveImage(img); err != nil {
		t.Fatal(err)
	}

	if _, err := RemoveImage(st, img.Name, true); err == nil || !strings.Contains(err.Error(), "does not match expected") {
		t.Fatalf("malformed managed path removal error=%v", err)
	}
	if _, err := st.GetImage(img.Name); err != nil {
		t.Fatalf("metadata deleted despite malformed managed path: %v", err)
	}
	if data, err := os.ReadFile(sentinel); err != nil || string(data) != "keep\n" {
		t.Fatalf("malformed managed payload changed: data=%q err=%v", data, err)
	}
}
