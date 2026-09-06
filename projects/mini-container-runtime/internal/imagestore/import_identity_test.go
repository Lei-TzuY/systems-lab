package imagestore

import (
	"crypto/sha256"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"minicontainer/internal/image"
	"minicontainer/internal/state"
)

func TestRawRootFSContentIDDistinguishesSharedTwelveHexPrefix(t *testing.T) {
	first := make([]byte, sha256.Size)
	second := make([]byte, sha256.Size)
	second[len(second)-1] = 1

	firstID, err := rawRootFSContentID(first)
	if err != nil {
		t.Fatal(err)
	}
	secondID, err := rawRootFSContentID(second)
	if err != nil {
		t.Fatal(err)
	}
	if firstID[:12] != secondID[:12] {
		t.Fatalf("test setup prefixes differ: %q vs %q", firstID[:12], secondID[:12])
	}
	if firstID == secondID {
		t.Fatalf("full SHA-256 identities aliased: %q", firstID)
	}
	if len(firstID) != sha256.Size*2 || len(secondID) != sha256.Size*2 {
		t.Fatalf("identity lengths = %d/%d, want %d", len(firstID), len(secondID), sha256.Size*2)
	}
}

func TestRawRootFSContentIDRejectsTruncatedDigest(t *testing.T) {
	if _, err := rawRootFSContentID(make([]byte, sha256.Size-1)); err == nil || !strings.Contains(err.Error(), "length") {
		t.Fatalf("truncated digest error = %v", err)
	}
}

func TestImportRawRootFSDoesNotReuseLegacyShortPrefixDirectory(t *testing.T) {
	base := t.TempDir()
	st, err := state.Open(base)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	srcDir := filepath.Join(base, "src")
	if err := os.MkdirAll(srcDir, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(srcDir, "payload.txt"), []byte("new payload\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	tarPath := filepath.Join(base, "rootfs.tar.gz")
	if err := image.ExportDir(srcDir, tarPath); err != nil {
		t.Fatal(err)
	}
	archive, err := os.ReadFile(tarPath)
	if err != nil {
		t.Fatal(err)
	}
	digest := sha256.Sum256(archive)
	fullID, err := rawRootFSContentID(digest[:])
	if err != nil {
		t.Fatal(err)
	}
	legacyID := fullID[:12]
	legacyRootFS := filepath.Join(base, "images", legacyID, "rootfs")
	if err := os.MkdirAll(legacyRootFS, 0o755); err != nil {
		t.Fatal(err)
	}
	legacySentinel := filepath.Join(legacyRootFS, "legacy-sentinel.txt")
	if err := os.WriteFile(legacySentinel, []byte("legacy must remain untouched\n"), 0o600); err != nil {
		t.Fatal(err)
	}

	rec, err := ImportRawRootFS(st, tarPath, "full:latest")
	if err != nil {
		t.Fatalf("ImportRawRootFS: %v", err)
	}
	if rec.ID != fullID {
		t.Fatalf("image ID = %q, want %q", rec.ID, fullID)
	}
	if got := filepath.Base(filepath.Dir(rec.RootFS)); got != fullID {
		t.Fatalf("published payload directory = %q, want full digest %q", got, fullID)
	}
	if _, err := os.Stat(filepath.Join(rec.RootFS, "payload.txt")); err != nil {
		t.Fatalf("new full-digest payload missing: %v", err)
	}
	data, err := os.ReadFile(legacySentinel)
	if err != nil {
		t.Fatalf("legacy sentinel missing: %v", err)
	}
	if string(data) != "legacy must remain untouched\n" {
		t.Fatalf("legacy sentinel changed: %q", data)
	}
}

func TestImportRawRootFSRejectsUnprovenFullDigestDirectory(t *testing.T) {
	base := t.TempDir()
	st, err := state.Open(base)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	srcDir := filepath.Join(base, "src")
	if err := os.MkdirAll(srcDir, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(srcDir, "payload.txt"), []byte("payload\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	tarPath := filepath.Join(base, "rootfs.tar.gz")
	if err := image.ExportDir(srcDir, tarPath); err != nil {
		t.Fatal(err)
	}
	archive, err := os.ReadFile(tarPath)
	if err != nil {
		t.Fatal(err)
	}
	digest := sha256.Sum256(archive)
	fullID, err := rawRootFSContentID(digest[:])
	if err != nil {
		t.Fatal(err)
	}

	foreignDir := filepath.Join(base, "images", fullID)
	foreignRootFS := filepath.Join(foreignDir, "rootfs")
	if err := os.MkdirAll(foreignRootFS, 0o755); err != nil {
		t.Fatal(err)
	}
	foreignSentinel := filepath.Join(foreignRootFS, "foreign.txt")
	if err := os.WriteFile(foreignSentinel, []byte("foreign payload\n"), 0o600); err != nil {
		t.Fatal(err)
	}

	if _, err := ImportRawRootFS(st, tarPath, "must-not-publish:latest"); err == nil || !strings.Contains(err.Error(), "no committed metadata ownership proof") {
		t.Fatalf("unproven full-digest directory error = %v", err)
	}
	data, err := os.ReadFile(foreignSentinel)
	if err != nil {
		t.Fatalf("foreign sentinel missing: %v", err)
	}
	if string(data) != "foreign payload\n" {
		t.Fatalf("foreign sentinel changed: %q", data)
	}
	if _, err := st.GetImage("must-not-publish:latest"); err == nil {
		t.Fatal("unproven payload still produced image metadata")
	}
}
