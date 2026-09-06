package image

import (
	"archive/tar"
	"os"
	"path/filepath"
	"testing"
)

func TestUnpackReplacesPreexistingHardlinkWithoutTruncatingExternalInode(t *testing.T) {
	base := t.TempDir()
	dest := filepath.Join(base, "rootfs")
	if err := os.Mkdir(dest, 0o700); err != nil {
		t.Fatal(err)
	}

	outside := filepath.Join(base, "outside-sentinel")
	if err := os.WriteFile(outside, []byte("sentinel"), 0o600); err != nil {
		t.Fatal(err)
	}
	victim := filepath.Join(dest, "victim")
	if err := os.Link(outside, victim); err != nil {
		t.Fatal(err)
	}

	tarPath := filepath.Join(base, "layer.tar")
	f, err := os.Create(tarPath)
	if err != nil {
		t.Fatal(err)
	}
	tw := tar.NewWriter(f)
	payload := []byte("container-data")
	if err := tw.WriteHeader(&tar.Header{Name: "victim", Mode: 0o644, Size: int64(len(payload)), Typeflag: tar.TypeReg}); err != nil {
		t.Fatal(err)
	}
	if _, err := tw.Write(payload); err != nil {
		t.Fatal(err)
	}
	if err := tw.Close(); err != nil {
		t.Fatal(err)
	}
	if err := f.Close(); err != nil {
		t.Fatal(err)
	}

	if err := Unpack(tarPath, dest); err != nil {
		t.Fatalf("Unpack: %v", err)
	}

	outsideData, err := os.ReadFile(outside)
	if err != nil {
		t.Fatal(err)
	}
	if string(outsideData) != "sentinel" {
		t.Fatalf("external hardlink inode was modified: %q", outsideData)
	}

	insideData, err := os.ReadFile(victim)
	if err != nil {
		t.Fatal(err)
	}
	if string(insideData) != string(payload) {
		t.Fatalf("extracted victim=%q, want %q", insideData, payload)
	}

	outsideInfo, err := os.Stat(outside)
	if err != nil {
		t.Fatal(err)
	}
	insideInfo, err := os.Stat(victim)
	if err != nil {
		t.Fatal(err)
	}
	if os.SameFile(outsideInfo, insideInfo) {
		t.Fatal("extracted regular file still aliases external inode")
	}
}

func TestWriteRegularUsesExclusiveCreation(t *testing.T) {
	target := filepath.Join(t.TempDir(), "target")
	if err := os.WriteFile(target, []byte("existing"), 0o600); err != nil {
		t.Fatal(err)
	}

	hdr := &tar.Header{Name: "target", Mode: 0o644, Size: 0, Typeflag: tar.TypeReg}
	if err := writeRegular(target, hdr, nil); err == nil {
		t.Fatal("writeRegular replaced an existing pathname instead of requiring an absent leaf")
	}
	data, err := os.ReadFile(target)
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != "existing" {
		t.Fatalf("exclusive-create failure modified existing file: %q", data)
	}
}
