//go:build linux

package image

import (
	"archive/tar"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"golang.org/x/sys/unix"
)

func TestTarUnixModePreservesSpecialBits(t *testing.T) {
	mode := os.FileMode(0o755) | os.ModeSetuid | os.ModeSetgid | os.ModeSticky
	got := tarUnixMode(mode)
	want := uint32(0o755 | unix.S_ISUID | unix.S_ISGID | unix.S_ISVTX)
	if got != want {
		t.Fatalf("tarUnixMode=%#o want %#o", got, want)
	}
}

func TestWriteRegularSecureRestoresSetuidBit(t *testing.T) {
	root := t.TempDir()
	target := filepath.Join(root, "tool")
	hdr := &tar.Header{Name: "tool", Mode: 0o4755, Uid: os.Geteuid(), Gid: os.Getegid()}
	if err := writeRegularSecure(target, root, hdr, strings.NewReader("payload")); err != nil {
		t.Fatal(err)
	}
	var st unix.Stat_t
	if err := unix.Stat(target, &st); err != nil {
		t.Fatal(err)
	}
	if got := st.Mode & 0o7777; got != 0o4755 {
		t.Fatalf("regular mode=%#o want %#o", got, uint32(0o4755))
	}
}

func TestFinalizeDirectoryMetadataRestoresStickySetgidBits(t *testing.T) {
	root := t.TempDir()
	target := filepath.Join(root, "shared")
	if err := os.Mkdir(target, 0o755); err != nil {
		t.Fatal(err)
	}
	mode := os.FileMode(0o775) | os.ModeSetgid | os.ModeSticky
	if err := finalizeDirectoryMetadata(root, []directoryMetadata{{
		target: target,
		mode: mode,
		uid: os.Geteuid(),
		gid: os.Getegid(),
	}}); err != nil {
		t.Fatal(err)
	}
	var st unix.Stat_t
	if err := unix.Stat(target, &st); err != nil {
		t.Fatal(err)
	}
	want := uint32(0o775 | unix.S_ISGID | unix.S_ISVTX)
	if got := st.Mode & 0o7777; got != want {
		t.Fatalf("directory mode=%#o want %#o", got, want)
	}
}

func TestSpecialModeDevicePreservesStickyBit(t *testing.T) {
	hdr := &tar.Header{Name: "fifo", Typeflag: tar.TypeFifo, Mode: 0o1777}
	mode, _, err := specialModeDevice(hdr)
	if err != nil {
		t.Fatal(err)
	}
	want := uint32(unix.S_IFIFO | unix.S_ISVTX | 0o777)
	if mode != want {
		t.Fatalf("fifo mode=%#o want %#o", mode, want)
	}
}
