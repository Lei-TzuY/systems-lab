package image

import (
	"archive/tar"
	"bytes"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestApplyLayerReaderFinalizesRestrictiveDirectoryMetadata(t *testing.T) {
	var layer bytes.Buffer
	tw := tar.NewWriter(&layer)
	mtime := time.Unix(1_700_000_123, 0)

	if err := tw.WriteHeader(&tar.Header{
		Name:     "locked/",
		Typeflag: tar.TypeDir,
		Mode:     0o500,
		Uid:      os.Getuid(),
		Gid:      os.Getgid(),
		ModTime:  mtime,
	}); err != nil {
		t.Fatal(err)
	}
	payload := []byte("child survives restrictive parent")
	if err := tw.WriteHeader(&tar.Header{
		Name:     "locked/payload",
		Typeflag: tar.TypeReg,
		Mode:     0o644,
		Uid:      os.Getuid(),
		Gid:      os.Getgid(),
		ModTime:  mtime.Add(time.Second),
		Size:     int64(len(payload)),
	}); err != nil {
		t.Fatal(err)
	}
	if _, err := tw.Write(payload); err != nil {
		t.Fatal(err)
	}
	if err := tw.Close(); err != nil {
		t.Fatal(err)
	}

	dest := t.TempDir()
	dir := filepath.Join(dest, "locked")
	defer func() { _ = os.Chmod(dir, 0o700) }()
	if err := applyLayerReader(bytes.NewReader(layer.Bytes()), dest); err != nil {
		t.Fatalf("apply layer: %v", err)
	}

	info, err := os.Stat(dir)
	if err != nil {
		t.Fatal(err)
	}
	if got := info.Mode().Perm(); got != 0o500 {
		t.Fatalf("directory mode = %#o, want %#o; temporary extraction permissions leaked into final rootfs", got, os.FileMode(0o500))
	}
	if got := info.ModTime().UnixNano(); got != mtime.UnixNano() {
		t.Fatalf("directory mtime = %d, want %d", got, mtime.UnixNano())
	}
	if got, err := os.ReadFile(filepath.Join(dir, "payload")); err != nil || string(got) != string(payload) {
		t.Fatalf("child payload = %q, err=%v", got, err)
	}
}
