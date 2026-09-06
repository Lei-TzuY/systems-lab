//go:build linux

package imagestore

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"minicontainer/internal/image"
	"minicontainer/internal/state"
)

func makeImportArchive(t *testing.T) string {
	t.Helper()
	src := t.TempDir()
	if err := os.WriteFile(filepath.Join(src, "payload.txt"), []byte("payload\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	archive := filepath.Join(t.TempDir(), "rootfs.tar.gz")
	if err := image.ExportDir(src, archive); err != nil {
		t.Fatal(err)
	}
	return archive
}

func TestImportRawRootFSRejectsReplacedStateRootGeneration(t *testing.T) {
	parent := t.TempDir()
	root := filepath.Join(parent, "state")
	st, err := state.Open(root)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()
	archive := makeImportArchive(t)

	originalRoot := filepath.Join(parent, "state-original")
	if err := os.Rename(root, originalRoot); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(root, "containers"), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(root, "images"), 0o700); err != nil {
		t.Fatal(err)
	}

	if _, err := ImportRawRootFS(st, archive, "replacement-root:latest"); err == nil || !strings.Contains(err.Error(), "changed generation") {
		t.Fatalf("ImportRawRootFS replacement-root error=%v", err)
	}
	replacementEntries, err := os.ReadDir(filepath.Join(root, "images"))
	if err != nil {
		t.Fatal(err)
	}
	if len(replacementEntries) != 0 {
		t.Fatalf("import mutated replacement image directory: %v", replacementEntries)
	}
	images, err := st.ListImages()
	if err != nil {
		t.Fatal(err)
	}
	if len(images) != 0 {
		t.Fatalf("failed boundary import persisted pinned metadata: %+v", images)
	}
}

func TestImportRawRootFSRejectsReplacedImageDirectoryGeneration(t *testing.T) {
	root := filepath.Join(t.TempDir(), "state")
	st, err := state.Open(root)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()
	archive := makeImportArchive(t)

	imagesPath := filepath.Join(root, "images")
	originalImages := filepath.Join(root, "images-original")
	if err := os.Rename(imagesPath, originalImages); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(imagesPath, 0o700); err != nil {
		t.Fatal(err)
	}

	if _, err := ImportRawRootFS(st, archive, "replacement-images:latest"); err == nil || !strings.Contains(err.Error(), "image directory changed generation") {
		t.Fatalf("ImportRawRootFS replacement-images error=%v", err)
	}
	replacementEntries, err := os.ReadDir(imagesPath)
	if err != nil {
		t.Fatal(err)
	}
	if len(replacementEntries) != 0 {
		t.Fatalf("import mutated replacement image directory: %v", replacementEntries)
	}
}

func TestImportRawRootFSPersistsDurableConfiguredRootFSPath(t *testing.T) {
	root := filepath.Join(t.TempDir(), "state")
	st, err := state.Open(root)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	rec, err := ImportRawRootFS(st, makeImportArchive(t), "durable-path:latest")
	if err != nil {
		t.Fatal(err)
	}
	wantPrefix := filepath.Join(root, "images") + string(filepath.Separator)
	if !strings.HasPrefix(rec.RootFS, wantPrefix) {
		t.Fatalf("RootFS=%q, want durable configured prefix %q", rec.RootFS, wantPrefix)
	}
	if strings.Contains(rec.RootFS, "/proc/self/fd/") {
		t.Fatalf("RootFS persisted process-local fd path: %q", rec.RootFS)
	}
	if _, err := os.Stat(filepath.Join(rec.RootFS, "payload.txt")); err != nil {
		t.Fatalf("durable RootFS path does not resolve published payload: %v", err)
	}
}
