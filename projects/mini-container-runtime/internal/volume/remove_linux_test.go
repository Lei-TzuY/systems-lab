//go:build linux

package volume

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestRemoveVolumeDoesNotFollowDataSymlink(t *testing.T) {
	setVolumeTestHome(t)
	vol, err := CreateVolume("db")
	if err != nil {
		t.Fatal(err)
	}
	outside := t.TempDir()
	sentinel := filepath.Join(outside, "sentinel")
	if err := os.WriteFile(sentinel, []byte("keep"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, filepath.Join(vol.MountPath, "outside-link")); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	if err := RemoveVolume("db"); err != nil {
		t.Fatalf("RemoveVolume: %v", err)
	}
	if data, err := os.ReadFile(sentinel); err != nil || string(data) != "keep" {
		t.Fatalf("outside target changed: data=%q err=%v", data, err)
	}
	if _, err := os.Lstat(filepath.Join(DefaultVolumeDir(), "db")); !os.IsNotExist(err) {
		t.Fatalf("volume directory remains: %v", err)
	}
}

func TestPinnedRemovalRejectsVolumeRootReplacementAfterValidation(t *testing.T) {
	setVolumeTestHome(t)
	if _, err := CreateVolume("db"); err != nil {
		t.Fatal(err)
	}
	if _, err := readVolume(DefaultVolumeDir(), "db"); err != nil {
		t.Fatalf("pre-race validation: %v", err)
	}
	ownedRoot := DefaultVolumeDir() + ".owned"
	if err := os.Rename(DefaultVolumeDir(), ownedRoot); err != nil {
		t.Fatal(err)
	}
	outside := t.TempDir()
	outsideVolume := filepath.Join(outside, "db")
	if err := os.MkdirAll(filepath.Join(outsideVolume, "_data"), 0o755); err != nil {
		t.Fatal(err)
	}
	sentinel := filepath.Join(outsideVolume, "_data", "sentinel")
	if err := os.WriteFile(sentinel, []byte("keep"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, DefaultVolumeDir()); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}

	err := removeVolumeDir(DefaultVolumeDir(), "db")
	if err == nil {
		t.Fatal("pinned removal followed replaced volume root")
	}
	if data, err := os.ReadFile(sentinel); err != nil || string(data) != "keep" {
		t.Fatalf("replacement target changed: data=%q err=%v", data, err)
	}
	if data, err := os.ReadFile(filepath.Join(ownedRoot, "db", "volume.json")); err != nil || len(data) == 0 {
		t.Fatalf("original validated volume was unexpectedly removed: len=%d err=%v", len(data), err)
	}
}

func TestPruneVolumesReportsCorruptManagedVolume(t *testing.T) {
	setVolumeTestHome(t)
	if _, err := CreateVolume("good"); err != nil {
		t.Fatal(err)
	}
	if _, err := CreateVolume("bad"); err != nil {
		t.Fatal(err)
	}
	badMeta := filepath.Join(DefaultVolumeDir(), "bad", "volume.json")
	if err := os.WriteFile(badMeta, []byte("{"), 0o600); err != nil {
		t.Fatal(err)
	}

	count, err := PruneVolumes()
	if count != 1 {
		t.Fatalf("pruned=%d, want one healthy volume", count)
	}
	if err == nil || !strings.Contains(err.Error(), "bad") {
		t.Fatalf("corrupt volume prune error=%v", err)
	}
	if _, err := os.Lstat(filepath.Join(DefaultVolumeDir(), "good")); !os.IsNotExist(err) {
		t.Fatalf("healthy volume remains after prune: %v", err)
	}
	if _, err := os.Lstat(filepath.Join(DefaultVolumeDir(), "bad")); err != nil {
		t.Fatalf("corrupt volume should remain for explicit repair: %v", err)
	}
}
