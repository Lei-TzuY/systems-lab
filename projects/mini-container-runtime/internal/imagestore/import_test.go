package imagestore

import (
	"crypto/sha256"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"minicontainer/internal/image"
	"minicontainer/internal/state"
)

func TestImportRawRootFS(t *testing.T) {
	tmpDir := t.TempDir()
	st, err := state.Open(tmpDir)
	if err != nil {
		t.Fatalf("Open state store error: %v", err)
	}
	defer st.Close()

	srcDir := filepath.Join(tmpDir, "src")
	if err := os.MkdirAll(srcDir, 0755); err != nil {
		t.Fatalf("MkdirAll srcDir error: %v", err)
	}
	if err := os.WriteFile(filepath.Join(srcDir, "test.txt"), []byte("sample rootfs content"), 0644); err != nil {
		t.Fatalf("WriteFile error: %v", err)
	}

	tarPath := filepath.Join(tmpDir, "test.tar.gz")
	if err := image.ExportDir(srcDir, tarPath); err != nil {
		t.Fatalf("ExportDir error: %v", err)
	}
	archiveBytes, err := os.ReadFile(tarPath)
	if err != nil {
		t.Fatalf("ReadFile archive: %v", err)
	}
	archiveSum := sha256.Sum256(archiveBytes)
	expectedID, err := rawRootFSContentID(archiveSum[:])
	if err != nil {
		t.Fatalf("derive expected image ID: %v", err)
	}

	rec, err := ImportRawRootFS(st, tarPath, "imported:latest")
	if err != nil {
		t.Fatalf("ImportRawRootFS error: %v", err)
	}

	if rec.Tag != "imported:latest" {
		t.Fatalf("Image tag = %s, want imported:latest", rec.Tag)
	}
	if rec.ID != expectedID || len(rec.ID) != sha256.Size*2 {
		t.Fatalf("Image ID = %q, want full SHA-256 %q", rec.ID, expectedID)
	}
	if got := filepath.Base(filepath.Dir(rec.RootFS)); got != expectedID {
		t.Fatalf("RootFS parent = %q, want full content ID %q", got, expectedID)
	}
	if _, err := os.Stat(filepath.Join(rec.RootFS, "test.txt")); err != nil {
		t.Fatalf("published rootfs missing content: %v", err)
	}
}

func TestImportRawRootFSFailureDoesNotPublishPartialImage(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	bad := []byte("not a gzip stream")
	tarPath := filepath.Join(t.TempDir(), "broken.tar.gz")
	if err := os.WriteFile(tarPath, bad, 0644); err != nil {
		t.Fatal(err)
	}

	if _, err := ImportRawRootFS(st, tarPath, "broken:latest"); err == nil {
		t.Fatal("malformed archive import unexpectedly succeeded")
	}

	digest := sha256.Sum256(bad)
	sum, err := rawRootFSContentID(digest[:])
	if err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(filepath.Join(st.Dir(), "images", sum)); !os.IsNotExist(err) {
		t.Fatalf("failed import published image directory: err=%v", err)
	}
	staging, err := filepath.Glob(filepath.Join(st.Dir(), "images", ".import-"+sum+"-*"))
	if err != nil {
		t.Fatal(err)
	}
	if len(staging) != 0 {
		t.Fatalf("failed import left staging directories: %v", staging)
	}
}

func TestImportRawRootFSJoinsUnpackAndCleanupFailures(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	bad := []byte("not a gzip stream")
	tarPath := filepath.Join(t.TempDir(), "broken-cleanup.tar.gz")
	if err := os.WriteFile(tarPath, bad, 0644); err != nil {
		t.Fatal(err)
	}

	cleanupErr := errors.New("cleanup denied")
	cleanupCalls := 0
	var cleanupPath string
	_, err = importRawRootFSWithCleanup(st, tarPath, "broken-cleanup:latest", func(path string) error {
		cleanupCalls++
		cleanupPath = path
		return cleanupErr
	})
	if err == nil {
		t.Fatal("import with failed staging cleanup unexpectedly succeeded")
	}
	if !errors.Is(err, cleanupErr) {
		t.Fatalf("error=%v, want cleanup cause", err)
	}
	if !strings.Contains(err.Error(), "unpack rootfs") || !strings.Contains(err.Error(), "remove temporary image directory") {
		t.Fatalf("error=%v, want both unpack and cleanup context", err)
	}
	if cleanupCalls != 1 {
		t.Fatalf("cleanup calls=%d, want 1", cleanupCalls)
	}
	if !strings.Contains(filepath.Base(cleanupPath), ".import-") {
		t.Fatalf("cleanup path=%q, want private import staging directory", cleanupPath)
	}
}

func TestImportRawRootFSDuplicateCleanupFailureBlocksMetadata(t *testing.T) {
	base := t.TempDir()
	st, err := state.Open(base)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	srcDir := filepath.Join(base, "src")
	if err := os.MkdirAll(srcDir, 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(srcDir, "payload.txt"), []byte("payload\n"), 0644); err != nil {
		t.Fatal(err)
	}
	tarPath := filepath.Join(base, "rootfs.tar.gz")
	if err := image.ExportDir(srcDir, tarPath); err != nil {
		t.Fatal(err)
	}
	if _, err := ImportRawRootFS(st, tarPath, "first:latest"); err != nil {
		t.Fatalf("first import: %v", err)
	}

	cleanupErr := errors.New("duplicate staging cleanup denied")
	cleanupCalls := 0
	_, err = importRawRootFSWithCleanup(st, tarPath, "second:latest", func(string) error {
		cleanupCalls++
		return cleanupErr
	})
	if err == nil {
		t.Fatal("duplicate import with failed cleanup unexpectedly succeeded")
	}
	if !errors.Is(err, cleanupErr) || !strings.Contains(err.Error(), "discard duplicate import staging") {
		t.Fatalf("error=%v, want duplicate cleanup failure", err)
	}
	if cleanupCalls != 2 {
		// The immediate cleanup fails and the deferred owner makes one final retry.
		t.Fatalf("cleanup calls=%d, want 2", cleanupCalls)
	}
	if _, lookupErr := st.GetImage("second:latest"); lookupErr == nil {
		t.Fatal("duplicate cleanup failure still persisted second image metadata")
	}
}
