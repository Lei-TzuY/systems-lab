//go:build linux

package container

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"

	volumestore "minicontainer/internal/volume"
)

func setManagedVolumeTestHome(t *testing.T) {
	t.Helper()
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("USERPROFILE", home)
}

func TestResolveVolumeMountSourcePinsManagedData(t *testing.T) {
	setManagedVolumeTestHome(t)
	vol, err := volumestore.CreateVolume("db")
	if err != nil {
		t.Fatal(err)
	}

	source, file, err := resolveVolumeMountSource(vol.MountPath)
	if err != nil {
		t.Fatalf("resolve managed source: %v", err)
	}
	if file == nil {
		t.Fatal("managed source did not return a pinned file")
	}
	defer file.Close()
	if source != "/proc/self/fd/"+fdString(file) {
		t.Fatalf("source=%q does not reference fd %d", source, file.Fd())
	}
	pinned, err := os.Stat(source)
	if err != nil {
		t.Fatalf("stat pinned source: %v", err)
	}
	current, err := os.Stat(vol.MountPath)
	if err != nil {
		t.Fatalf("stat managed data path: %v", err)
	}
	if !os.SameFile(pinned, current) {
		t.Fatal("pinned source is not the managed _data directory")
	}
}

func TestResolveVolumeMountSourceLeavesOrdinaryHostPathUnchanged(t *testing.T) {
	setManagedVolumeTestHome(t)
	host := t.TempDir()
	source, file, err := resolveVolumeMountSource(host)
	if err != nil {
		t.Fatalf("resolve ordinary host source: %v", err)
	}
	if source != host {
		t.Fatalf("ordinary source changed: got %q want %q", source, host)
	}
	if file != nil {
		file.Close()
		t.Fatal("ordinary host source unexpectedly returned managed fd")
	}
}

func TestResolveVolumeMountSourceRejectsManagedRootReplacement(t *testing.T) {
	setManagedVolumeTestHome(t)
	vol, err := volumestore.CreateVolume("db")
	if err != nil {
		t.Fatal(err)
	}
	ownedRoot := volumestore.DefaultVolumeDir() + ".owned"
	if err := os.Rename(volumestore.DefaultVolumeDir(), ownedRoot); err != nil {
		t.Fatal(err)
	}
	outside := t.TempDir()
	sentinel := filepath.Join(outside, "sentinel")
	if err := os.WriteFile(sentinel, []byte("keep"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, volumestore.DefaultVolumeDir()); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}

	if _, file, err := resolveVolumeMountSource(vol.MountPath); err == nil {
		if file != nil {
			file.Close()
		}
		t.Fatal("managed source followed replaced volume root")
	}
	if data, err := os.ReadFile(sentinel); err != nil || string(data) != "keep" {
		t.Fatalf("outside sentinel changed: data=%q err=%v", data, err)
	}
	if data, err := os.ReadFile(filepath.Join(ownedRoot, "db", "volume.json")); err != nil || len(data) == 0 {
		t.Fatalf("original managed volume changed: len=%d err=%v", len(data), err)
	}
}

func fdString(file *os.File) string {
	return fmt.Sprintf("%d", file.Fd())
}
