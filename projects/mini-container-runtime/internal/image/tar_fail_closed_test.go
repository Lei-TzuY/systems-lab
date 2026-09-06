package image

import (
	"archive/tar"
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestApplyTarEntryRejectsUnknownType(t *testing.T) {
	dest := t.TempDir()
	target := filepath.Join(dest, "mystery")
	hdr := &tar.Header{Name: "mystery", Typeflag: 0x7f, Mode: 0o644}

	err := applyTarEntry(target, hdr, bytes.NewReader(nil), dest)
	if err == nil {
		t.Fatal("unknown tar type was accepted")
	}
	if !strings.Contains(err.Error(), "unsupported tar entry") {
		t.Fatalf("unexpected error: %v", err)
	}
	if _, statErr := os.Lstat(target); !os.IsNotExist(statErr) {
		t.Fatalf("unknown tar type mutated filesystem: %v", statErr)
	}
}

func TestApplyTarEntryPropagatesSpecialNodeFailure(t *testing.T) {
	dest := t.TempDir()
	target := filepath.Join(dest, "fifo")
	if err := os.Mkdir(target, 0o755); err != nil {
		t.Fatal(err)
	}
	hdr := &tar.Header{Name: "fifo", Typeflag: tar.TypeFifo, Mode: 0o600}

	err := applyTarEntry(target, hdr, bytes.NewReader(nil), dest)
	if err == nil {
		t.Fatal("special-node creation failure was swallowed")
	}
	if !strings.Contains(err.Error(), "create special tar entry") {
		t.Fatalf("unexpected error: %v", err)
	}
	info, statErr := os.Lstat(target)
	if statErr != nil {
		t.Fatalf("existing directory disappeared: %v", statErr)
	}
	if !info.IsDir() {
		t.Fatalf("existing directory was replaced: mode=%v", info.Mode())
	}
}
