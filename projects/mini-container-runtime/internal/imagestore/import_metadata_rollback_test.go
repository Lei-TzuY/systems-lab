package imagestore

import (
	"crypto/sha256"
	"encoding/hex"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"minicontainer/internal/image"
	"minicontainer/internal/state"
)

func prepareRawImportArchive(t *testing.T, base, payload string) (string, string) {
	t.Helper()
	srcDir := filepath.Join(base, "src-"+strings.ReplaceAll(payload, "/", "_"))
	if err := os.MkdirAll(srcDir, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(srcDir, "payload.txt"), []byte(payload), 0o644); err != nil {
		t.Fatal(err)
	}
	tarPath := filepath.Join(base, "rootfs-"+strings.ReplaceAll(payload, "/", "_")+".tar.gz")
	if err := image.ExportDir(srcDir, tarPath); err != nil {
		t.Fatal(err)
	}
	archive, err := os.ReadFile(tarPath)
	if err != nil {
		t.Fatal(err)
	}
	digest := sha256.Sum256(archive)
	contentID, err := rawRootFSContentID(digest[:])
	if err != nil {
		t.Fatal(err)
	}
	return tarPath, contentID
}

func blockCurrentImageMetadataPath(t *testing.T, base, key string) string {
	t.Helper()
	digest := sha256.Sum256([]byte(key))
	path := filepath.Join(base, "images", "img-"+hex.EncodeToString(digest[:])+".json")
	if err := os.Mkdir(path, 0o700); err != nil {
		t.Fatal(err)
	}
	return path
}

func TestImportRawRootFSMetadataFailureRollsBackOwnedPayload(t *testing.T) {
	base := t.TempDir()
	st, err := state.Open(base)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	tarPath, contentID := prepareRawImportArchive(t, base, "owned rollback payload\n")
	const tag = "rollback-owned:latest"
	blocker := blockCurrentImageMetadataPath(t, base, tag)

	if _, err := ImportRawRootFS(st, tarPath, tag); err == nil || !strings.Contains(err.Error(), "save image record") {
		t.Fatalf("metadata failure error = %v", err)
	}
	payloadDir := filepath.Join(base, "images", contentID)
	if _, err := os.Lstat(payloadDir); !os.IsNotExist(err) {
		t.Fatalf("metadata failure left newly owned payload %q: %v", payloadDir, err)
	}
	if info, err := os.Lstat(blocker); err != nil || !info.IsDir() {
		t.Fatalf("metadata blocker unexpectedly changed: info=%v err=%v", info, err)
	}
	if _, err := st.GetImage(tag); err == nil {
		t.Fatal("failed metadata commit unexpectedly produced image record")
	}
}

func TestImportRawRootFSMetadataFailurePreservesReusedPayload(t *testing.T) {
	base := t.TempDir()
	st, err := state.Open(base)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	tarPath, contentID := prepareRawImportArchive(t, base, "shared payload\n")
	first, err := ImportRawRootFS(st, tarPath, "first-owner:latest")
	if err != nil {
		t.Fatalf("first import: %v", err)
	}
	if first.ID != contentID {
		t.Fatalf("first ID=%q want %q", first.ID, contentID)
	}
	const secondTag = "second-fails:latest"
	blockCurrentImageMetadataPath(t, base, secondTag)

	if _, err := ImportRawRootFS(st, tarPath, secondTag); err == nil || !strings.Contains(err.Error(), "save image record") {
		t.Fatalf("second metadata failure error = %v", err)
	}
	if _, err := os.Stat(filepath.Join(first.RootFS, "payload.txt")); err != nil {
		t.Fatalf("failed second tag deleted shared payload: %v", err)
	}
	got, err := st.GetImage("first-owner:latest")
	if err != nil {
		t.Fatalf("first metadata lost after second failure: %v", err)
	}
	if got.ID != contentID || filepath.Clean(got.RootFS) != filepath.Clean(first.RootFS) {
		t.Fatalf("first metadata changed: %+v", got)
	}
	if _, err := st.GetImage(secondTag); err == nil {
		t.Fatal("failed second metadata commit unexpectedly produced image record")
	}
}
