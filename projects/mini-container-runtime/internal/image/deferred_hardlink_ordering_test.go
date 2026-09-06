package image

import (
	"archive/tar"
	"os"
	"path/filepath"
	"testing"
)

func TestUnpackLaterRegularAncestorCancelsDeferredDescendant(t *testing.T) {
	base := t.TempDir()
	tarPath := filepath.Join(base, "regular-ancestor.tar")
	archive := writeTarEntries(t, []tarTestEntry{
		{hdr: tar.Header{Name: "a/b", Linkname: "source", Typeflag: tar.TypeLink, Mode: 0o644}},
		{hdr: tar.Header{Name: "a", Typeflag: tar.TypeReg, Mode: 0o644}, data: []byte("new ancestor")},
		{hdr: tar.Header{Name: "source", Typeflag: tar.TypeReg, Mode: 0o644}, data: []byte("source")},
	})
	if err := os.WriteFile(tarPath, archive, 0o600); err != nil {
		t.Fatal(err)
	}
	dest := filepath.Join(base, "rootfs")
	if err := Unpack(tarPath, dest); err != nil {
		t.Fatalf("Unpack regular ancestor replacement: %v", err)
	}
	data, err := os.ReadFile(filepath.Join(dest, "a"))
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != "new ancestor" {
		t.Fatalf("a=%q, want later regular ancestor", data)
	}
}

func TestUnpackLaterSymlinkAncestorCancelsDeferredDescendant(t *testing.T) {
	base := t.TempDir()
	tarPath := filepath.Join(base, "symlink-ancestor.tar")
	archive := writeTarEntries(t, []tarTestEntry{
		{hdr: tar.Header{Name: "a/b", Linkname: "source", Typeflag: tar.TypeLink, Mode: 0o644}},
		{hdr: tar.Header{Name: "a", Linkname: "elsewhere", Typeflag: tar.TypeSymlink, Mode: 0o777}},
		{hdr: tar.Header{Name: "source", Typeflag: tar.TypeReg, Mode: 0o644}, data: []byte("source")},
	})
	if err := os.WriteFile(tarPath, archive, 0o600); err != nil {
		t.Fatal(err)
	}
	dest := filepath.Join(base, "rootfs")
	if err := Unpack(tarPath, dest); err != nil {
		t.Fatalf("Unpack symlink ancestor replacement: %v", err)
	}
	link, err := os.Readlink(filepath.Join(dest, "a"))
	if err != nil {
		t.Fatal(err)
	}
	if link != "elsewhere" {
		t.Fatalf("a -> %q, want elsewhere", link)
	}
}

func TestUnpackLaterDirectoryAncestorPreservesDeferredDescendant(t *testing.T) {
	base := t.TempDir()
	tarPath := filepath.Join(base, "directory-ancestor.tar")
	archive := writeTarEntries(t, []tarTestEntry{
		{hdr: tar.Header{Name: "a/b", Linkname: "source", Typeflag: tar.TypeLink, Mode: 0o644}},
		{hdr: tar.Header{Name: "a", Typeflag: tar.TypeDir, Mode: 0o755}},
		{hdr: tar.Header{Name: "source", Typeflag: tar.TypeReg, Mode: 0o644}, data: []byte("source")},
	})
	if err := os.WriteFile(tarPath, archive, 0o600); err != nil {
		t.Fatal(err)
	}
	dest := filepath.Join(base, "rootfs")
	if err := Unpack(tarPath, dest); err != nil {
		t.Fatalf("Unpack directory ancestor: %v", err)
	}
	aliasInfo, err := os.Stat(filepath.Join(dest, "a", "b"))
	if err != nil {
		t.Fatal(err)
	}
	sourceInfo, err := os.Stat(filepath.Join(dest, "source"))
	if err != nil {
		t.Fatal(err)
	}
	if !os.SameFile(aliasInfo, sourceInfo) {
		t.Fatal("later directory ancestor incorrectly cancelled deferred descendant")
	}
}

func TestUnpackLaterDeferredHardlinkAncestorCancelsOlderDescendant(t *testing.T) {
	base := t.TempDir()
	tarPath := filepath.Join(base, "hardlink-ancestor.tar")
	archive := writeTarEntries(t, []tarTestEntry{
		{hdr: tar.Header{Name: "a/b", Linkname: "old-source", Typeflag: tar.TypeLink, Mode: 0o644}},
		{hdr: tar.Header{Name: "a", Linkname: "new-source", Typeflag: tar.TypeLink, Mode: 0o644}},
		{hdr: tar.Header{Name: "old-source", Typeflag: tar.TypeReg, Mode: 0o644}, data: []byte("old")},
		{hdr: tar.Header{Name: "new-source", Typeflag: tar.TypeReg, Mode: 0o644}, data: []byte("new")},
	})
	if err := os.WriteFile(tarPath, archive, 0o600); err != nil {
		t.Fatal(err)
	}
	dest := filepath.Join(base, "rootfs")
	if err := Unpack(tarPath, dest); err != nil {
		t.Fatalf("Unpack deferred hardlink ancestor replacement: %v", err)
	}
	aInfo, err := os.Stat(filepath.Join(dest, "a"))
	if err != nil {
		t.Fatal(err)
	}
	newInfo, err := os.Stat(filepath.Join(dest, "new-source"))
	if err != nil {
		t.Fatal(err)
	}
	if !os.SameFile(aInfo, newInfo) {
		t.Fatal("later hardlink ancestor did not resolve to its own source")
	}
}
