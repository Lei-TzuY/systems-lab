//go:build linux

package image

import (
	"archive/tar"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestUnpackFinalizesRestrictiveDirectoryMetadataAfterChildren(t *testing.T) {
	tmp := t.TempDir()
	archive := filepath.Join(tmp, "rootfs.tar")
	f, err := os.Create(archive)
	if err != nil {
		t.Fatal(err)
	}
	tw := tar.NewWriter(f)
	parentTime := time.Unix(1_700_000_000, 0)
	childTime := time.Unix(1_700_000_100, 0)
	entries := []struct {
		hdr  *tar.Header
		body string
	}{
		{hdr: &tar.Header{Name: "locked/", Typeflag: tar.TypeDir, Mode: 0o500, ModTime: parentTime}},
		{hdr: &tar.Header{Name: "locked/nested/", Typeflag: tar.TypeDir, Mode: 0o555, ModTime: childTime}},
		{hdr: &tar.Header{Name: "locked/nested/payload", Typeflag: tar.TypeReg, Mode: 0o600, Size: 7, ModTime: childTime}, body: "payload"},
	}
	for _, entry := range entries {
		if err := tw.WriteHeader(entry.hdr); err != nil {
			t.Fatal(err)
		}
		if entry.body != "" {
			if _, err := tw.Write([]byte(entry.body)); err != nil {
				t.Fatal(err)
			}
		}
	}
	if err := tw.Close(); err != nil {
		t.Fatal(err)
	}
	if err := f.Close(); err != nil {
		t.Fatal(err)
	}

	dest := filepath.Join(tmp, "rootfs")
	if err := Unpack(archive, dest); err != nil {
		t.Fatalf("unpack restrictive directory archive: %v", err)
	}
	locked := filepath.Join(dest, "locked")
	nested := filepath.Join(locked, "nested")
	defer func() {
		// Restore owner-write after assertions so testing.TempDir can remove the
		// intentionally restrictive extracted tree during test cleanup.
		_ = os.Chmod(locked, 0o700)
		_ = os.Chmod(nested, 0o700)
	}()

	payload, err := os.ReadFile(filepath.Join(nested, "payload"))
	if err != nil {
		t.Fatal(err)
	}
	if string(payload) != "payload" {
		t.Fatalf("payload=%q", payload)
	}
	for _, tc := range []struct {
		path string
		mode os.FileMode
		mt   time.Time
	}{
		{locked, 0o500, parentTime},
		{nested, 0o555, childTime},
	} {
		info, err := os.Stat(tc.path)
		if err != nil {
			t.Fatal(err)
		}
		if got := info.Mode().Perm(); got != tc.mode {
			t.Fatalf("mode %s=%#o, want %#o", tc.path, got, tc.mode)
		}
		if !info.ModTime().Equal(tc.mt) {
			t.Fatalf("mtime %s=%s, want %s", tc.path, info.ModTime(), tc.mt)
		}
	}
}
