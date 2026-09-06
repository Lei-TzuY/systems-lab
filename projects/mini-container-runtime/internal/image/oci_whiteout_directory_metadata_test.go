package image

import (
	"archive/tar"
	"bytes"
	"os"
	"path/filepath"
	"testing"
)

func TestApplyLayerReaderWhiteoutCancelsDeferredDirectoryMetadata(t *testing.T) {
	var layer bytes.Buffer
	tw := tar.NewWriter(&layer)
	writeLayerHeader(t, tw, &tar.Header{Name: "gone/", Typeflag: tar.TypeDir, Mode: 0o500})
	writeLayerHeader(t, tw, &tar.Header{Name: "gone/child", Typeflag: tar.TypeReg, Mode: 0o644, Size: 1}, []byte("x"))
	writeLayerHeader(t, tw, &tar.Header{Name: ".wh.gone", Typeflag: tar.TypeReg, Mode: 0o000})
	if err := tw.Close(); err != nil {
		t.Fatalf("close tar: %v", err)
	}

	dest := t.TempDir()
	if err := applyLayerReader(bytes.NewReader(layer.Bytes()), dest); err != nil {
		t.Fatalf("apply layer with later whiteout: %v", err)
	}
	if _, err := os.Lstat(filepath.Join(dest, "gone")); !os.IsNotExist(err) {
		t.Fatalf("whiteouted directory survived: err=%v", err)
	}
}

func TestApplyLayerReaderOpaqueWhiteoutCancelsOnlyDescendantDirectoryMetadata(t *testing.T) {
	var layer bytes.Buffer
	tw := tar.NewWriter(&layer)
	writeLayerHeader(t, tw, &tar.Header{Name: "keep/", Typeflag: tar.TypeDir, Mode: 0o755})
	writeLayerHeader(t, tw, &tar.Header{Name: "keep/old/", Typeflag: tar.TypeDir, Mode: 0o500})
	writeLayerHeader(t, tw, &tar.Header{Name: "keep/old/child", Typeflag: tar.TypeReg, Mode: 0o644, Size: 1}, []byte("x"))
	writeLayerHeader(t, tw, &tar.Header{Name: "keep/.wh..wh..opq", Typeflag: tar.TypeReg, Mode: 0o000})
	writeLayerHeader(t, tw, &tar.Header{Name: "keep/new", Typeflag: tar.TypeReg, Mode: 0o644, Size: 1}, []byte("y"))
	if err := tw.Close(); err != nil {
		t.Fatalf("close tar: %v", err)
	}

	dest := t.TempDir()
	if err := applyLayerReader(bytes.NewReader(layer.Bytes()), dest); err != nil {
		t.Fatalf("apply layer with opaque whiteout: %v", err)
	}
	if _, err := os.Stat(filepath.Join(dest, "keep", "new")); err != nil {
		t.Fatalf("new entry missing after opaque whiteout: %v", err)
	}
	if _, err := os.Lstat(filepath.Join(dest, "keep", "old")); !os.IsNotExist(err) {
		t.Fatalf("opaque-whiteouted descendant survived: err=%v", err)
	}
	info, err := os.Stat(filepath.Join(dest, "keep"))
	if err != nil {
		t.Fatalf("stat preserved opaque directory: %v", err)
	}
	if got := info.Mode().Perm(); got != 0o755 {
		t.Fatalf("preserved directory mode = %04o, want 0755", got)
	}
}

func writeLayerHeader(t *testing.T, tw *tar.Writer, hdr *tar.Header, body ...[]byte) {
	t.Helper()
	if err := tw.WriteHeader(hdr); err != nil {
		t.Fatalf("write tar header %q: %v", hdr.Name, err)
	}
	if len(body) > 0 {
		if _, err := tw.Write(body[0]); err != nil {
			t.Fatalf("write tar body %q: %v", hdr.Name, err)
		}
	}
}
