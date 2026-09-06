package registry

import (
	"archive/tar"
	"compress/gzip"
	"crypto/sha256"
	"encoding/hex"
	"io"
	"os"
	"path/filepath"
	"testing"
)

func readLayerHeaders(t *testing.T, archive string) map[string]*tar.Header {
	t.Helper()
	f, err := os.Open(archive)
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	gz, err := gzip.NewReader(f)
	if err != nil {
		t.Fatal(err)
	}
	defer gz.Close()
	tr := tar.NewReader(gz)
	headers := map[string]*tar.Header{}
	for {
		h, err := tr.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			t.Fatalf("read tar: %v", err)
		}
		copyHeader := *h
		headers[h.Name] = &copyHeader
	}
	return headers
}

func TestBuildOCILayerPublishesValidDigestSizeAndSymlinkTarget(t *testing.T) {
	base := t.TempDir()
	rootfs := filepath.Join(base, "rootfs")
	if err := os.MkdirAll(filepath.Join(rootfs, "etc"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(rootfs, "etc", "app.conf"), []byte("mode=prod\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("etc/app.conf", filepath.Join(rootfs, "config-link")); err != nil {
		t.Fatal(err)
	}

	archive := filepath.Join(base, "layer.tar.gz")
	digest, size, err := BuildOCILayer(rootfs, archive)
	if err != nil {
		t.Fatalf("BuildOCILayer: %v", err)
	}
	data, err := os.ReadFile(archive)
	if err != nil {
		t.Fatal(err)
	}
	if int64(len(data)) != size {
		t.Fatalf("size=%d file=%d", size, len(data))
	}
	sum := sha256.Sum256(data)
	wantDigest := "sha256:" + hex.EncodeToString(sum[:])
	if digest != wantDigest {
		t.Fatalf("digest=%q want=%q", digest, wantDigest)
	}

	headers := readLayerHeaders(t, archive)
	link := headers["config-link"]
	if link == nil {
		t.Fatal("config-link missing from layer")
	}
	if link.Typeflag != tar.TypeSymlink || link.Linkname != "etc/app.conf" {
		t.Fatalf("symlink header type=%d linkname=%q", link.Typeflag, link.Linkname)
	}
}

func TestBuildOCILayerReplacesDestinationSymlinkWithoutTouchingTarget(t *testing.T) {
	base := t.TempDir()
	rootfs := filepath.Join(base, "rootfs")
	if err := os.Mkdir(rootfs, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(rootfs, "file"), []byte("layer"), 0o644); err != nil {
		t.Fatal(err)
	}
	outside := filepath.Join(base, "outside")
	if err := os.WriteFile(outside, []byte("sentinel"), 0o600); err != nil {
		t.Fatal(err)
	}
	archive := filepath.Join(base, "layer.tar.gz")
	if err := os.Symlink(outside, archive); err != nil {
		t.Fatal(err)
	}

	if _, _, err := BuildOCILayer(rootfs, archive); err != nil {
		t.Fatalf("BuildOCILayer: %v", err)
	}
	outsideData, err := os.ReadFile(outside)
	if err != nil {
		t.Fatal(err)
	}
	if string(outsideData) != "sentinel" {
		t.Fatalf("symlink target overwritten: %q", outsideData)
	}
	info, err := os.Lstat(archive)
	if err != nil {
		t.Fatal(err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.Mode().IsRegular() {
		t.Fatalf("published archive mode=%v", info.Mode())
	}
	_ = readLayerHeaders(t, archive)
}

func TestAtomicPublishFileReplacesSymlinkWithoutTouchingTarget(t *testing.T) {
	base := t.TempDir()
	outside := filepath.Join(base, "outside.json")
	if err := os.WriteFile(outside, []byte("sentinel"), 0o600); err != nil {
		t.Fatal(err)
	}
	dest := filepath.Join(base, "manifest.json")
	if err := os.Symlink(outside, dest); err != nil {
		t.Fatal(err)
	}
	if err := atomicPublishFile(dest, []byte(`{"ok":true}`), 0o644); err != nil {
		t.Fatalf("atomicPublishFile: %v", err)
	}
	outsideData, err := os.ReadFile(outside)
	if err != nil {
		t.Fatal(err)
	}
	if string(outsideData) != "sentinel" {
		t.Fatalf("manifest symlink target overwritten: %q", outsideData)
	}
	published, err := os.ReadFile(dest)
	if err != nil {
		t.Fatal(err)
	}
	if string(published) != `{"ok":true}` {
		t.Fatalf("published manifest=%q", published)
	}
}

func TestBuildOCILayerRejectsNonDirectoryRootfsWithoutPublishing(t *testing.T) {
	base := t.TempDir()
	rootfs := filepath.Join(base, "rootfs-file")
	if err := os.WriteFile(rootfs, []byte("not a rootfs"), 0o600); err != nil {
		t.Fatal(err)
	}
	dest := filepath.Join(base, "layer.tar.gz")
	if _, _, err := BuildOCILayer(rootfs, dest); err == nil {
		t.Fatal("expected non-directory rootfs to fail")
	}
	if _, err := os.Stat(dest); !os.IsNotExist(err) {
		t.Fatalf("failed build published output: %v", err)
	}
}
