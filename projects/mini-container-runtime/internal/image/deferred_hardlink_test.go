package image

import (
	"archive/tar"
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

type tarTestEntry struct {
	hdr  tar.Header
	data []byte
}

func writeTarEntries(t *testing.T, entries []tarTestEntry) []byte {
	t.Helper()
	var buf bytes.Buffer
	tw := tar.NewWriter(&buf)
	for i := range entries {
		hdr := entries[i].hdr
		hdr.Size = int64(len(entries[i].data))
		if err := tw.WriteHeader(&hdr); err != nil {
			t.Fatalf("write header %q: %v", hdr.Name, err)
		}
		if len(entries[i].data) > 0 {
			if _, err := tw.Write(entries[i].data); err != nil {
				t.Fatalf("write payload %q: %v", hdr.Name, err)
			}
		}
	}
	if err := tw.Close(); err != nil {
		t.Fatal(err)
	}
	return buf.Bytes()
}

func TestUnpackResolvesForwardHardlinkAfterSourceAppears(t *testing.T) {
	base := t.TempDir()
	tarPath := filepath.Join(base, "forward.tar")
	archive := writeTarEntries(t, []tarTestEntry{
		{hdr: tar.Header{Name: "alias", Linkname: "source", Typeflag: tar.TypeLink, Mode: 0o644}},
		{hdr: tar.Header{Name: "source", Typeflag: tar.TypeReg, Mode: 0o644}, data: []byte("payload")},
	})
	if err := os.WriteFile(tarPath, archive, 0o600); err != nil {
		t.Fatal(err)
	}
	dest := filepath.Join(base, "rootfs")
	if err := Unpack(tarPath, dest); err != nil {
		t.Fatalf("Unpack forward hardlink: %v", err)
	}
	aliasInfo, err := os.Stat(filepath.Join(dest, "alias"))
	if err != nil {
		t.Fatal(err)
	}
	sourceInfo, err := os.Stat(filepath.Join(dest, "source"))
	if err != nil {
		t.Fatal(err)
	}
	if !os.SameFile(aliasInfo, sourceInfo) {
		t.Fatal("forward hardlink did not resolve to source inode")
	}
}

func TestUnpackResolvesForwardHardlinkChain(t *testing.T) {
	base := t.TempDir()
	tarPath := filepath.Join(base, "chain.tar")
	archive := writeTarEntries(t, []tarTestEntry{
		{hdr: tar.Header{Name: "a", Linkname: "b", Typeflag: tar.TypeLink, Mode: 0o644}},
		{hdr: tar.Header{Name: "b", Linkname: "c", Typeflag: tar.TypeLink, Mode: 0o644}},
		{hdr: tar.Header{Name: "c", Typeflag: tar.TypeReg, Mode: 0o644}, data: []byte("chain")},
	})
	if err := os.WriteFile(tarPath, archive, 0o600); err != nil {
		t.Fatal(err)
	}
	dest := filepath.Join(base, "rootfs")
	if err := Unpack(tarPath, dest); err != nil {
		t.Fatalf("Unpack hardlink chain: %v", err)
	}
	cInfo, err := os.Stat(filepath.Join(dest, "c"))
	if err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{"a", "b"} {
		info, err := os.Stat(filepath.Join(dest, name))
		if err != nil {
			t.Fatal(err)
		}
		if !os.SameFile(info, cInfo) {
			t.Fatalf("%s does not share final source inode", name)
		}
	}
}

func TestUnpackLaterDestinationEntryCancelsDeferredHardlink(t *testing.T) {
	base := t.TempDir()
	tarPath := filepath.Join(base, "overwrite.tar")
	archive := writeTarEntries(t, []tarTestEntry{
		{hdr: tar.Header{Name: "victim", Linkname: "source", Typeflag: tar.TypeLink, Mode: 0o644}},
		{hdr: tar.Header{Name: "victim", Typeflag: tar.TypeReg, Mode: 0o644}, data: []byte("newer")},
		{hdr: tar.Header{Name: "source", Typeflag: tar.TypeReg, Mode: 0o644}, data: []byte("source")},
	})
	if err := os.WriteFile(tarPath, archive, 0o600); err != nil {
		t.Fatal(err)
	}
	dest := filepath.Join(base, "rootfs")
	if err := Unpack(tarPath, dest); err != nil {
		t.Fatalf("Unpack overwrite: %v", err)
	}
	data, err := os.ReadFile(filepath.Join(dest, "victim"))
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != "newer" {
		t.Fatalf("victim=%q, want later regular entry", data)
	}
	victimInfo, _ := os.Stat(filepath.Join(dest, "victim"))
	sourceInfo, _ := os.Stat(filepath.Join(dest, "source"))
	if os.SameFile(victimInfo, sourceInfo) {
		t.Fatal("cancelled deferred hardlink was resurrected after later overwrite")
	}
}

func TestApplyLayerWhiteoutCancelsDeferredHardlinkDestination(t *testing.T) {
	dest := t.TempDir()
	archive := writeTarEntries(t, []tarTestEntry{
		{hdr: tar.Header{Name: "victim", Linkname: "source", Typeflag: tar.TypeLink, Mode: 0o644}},
		{hdr: tar.Header{Name: ".wh.victim", Typeflag: tar.TypeReg, Mode: 0o000}},
		{hdr: tar.Header{Name: "source", Typeflag: tar.TypeReg, Mode: 0o644}, data: []byte("source")},
	})
	if err := applyLayerReader(bytes.NewReader(archive), dest); err != nil {
		t.Fatalf("applyLayerReader: %v", err)
	}
	if _, err := os.Lstat(filepath.Join(dest, "victim")); !os.IsNotExist(err) {
		t.Fatalf("whiteouted deferred hardlink was recreated: err=%v", err)
	}
}

func TestUnpackRejectsNeverResolvedHardlink(t *testing.T) {
	base := t.TempDir()
	tarPath := filepath.Join(base, "missing.tar")
	archive := writeTarEntries(t, []tarTestEntry{
		{hdr: tar.Header{Name: "alias", Linkname: "missing", Typeflag: tar.TypeLink, Mode: 0o644}},
	})
	if err := os.WriteFile(tarPath, archive, 0o600); err != nil {
		t.Fatal(err)
	}
	err := Unpack(tarPath, filepath.Join(base, "rootfs"))
	if err == nil || !strings.Contains(err.Error(), "source never appeared") {
		t.Fatalf("unresolved hardlink error=%v", err)
	}
}
