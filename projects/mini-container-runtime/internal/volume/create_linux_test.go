//go:build linux

package volume

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestPinnedCreateRejectsStateRootReplacement(t *testing.T) {
	setVolumeTestHome(t)
	outside := t.TempDir()
	sentinel := filepath.Join(outside, "sentinel")
	if err := os.WriteFile(sentinel, []byte("keep"), 0o600); err != nil {
		t.Fatal(err)
	}
	ownedState := statePathForVolumeTest() + ".owned"

	_, err := createVolumeWithHook("db", time.Unix(10, 0), func(stage string) error {
		if stage != volumeCreateStageStatePinned {
			return nil
		}
		if err := os.Rename(statePathForVolumeTest(), ownedState); err != nil {
			return err
		}
		return os.Symlink(outside, statePathForVolumeTest())
	})
	if err == nil || !strings.Contains(err.Error(), "changed filesystem identity") {
		t.Fatalf("state replacement create error=%v", err)
	}
	if data, err := os.ReadFile(sentinel); err != nil || string(data) != "keep" {
		t.Fatalf("outside sentinel changed: data=%q err=%v", data, err)
	}
	if _, err := os.Lstat(filepath.Join(outside, "volumes")); !os.IsNotExist(err) {
		t.Fatalf("creation escaped through replaced state root: %v", err)
	}
	if _, err := os.Lstat(filepath.Join(ownedState, "volumes")); !os.IsNotExist(err) {
		t.Fatalf("creation continued in detached original state root: %v", err)
	}
}

func TestPinnedCreateRejectsVolumeRootReplacementBeforeVolumeMutation(t *testing.T) {
	setVolumeTestHome(t)
	outside := t.TempDir()
	sentinel := filepath.Join(outside, "sentinel")
	if err := os.WriteFile(sentinel, []byte("keep"), 0o600); err != nil {
		t.Fatal(err)
	}
	ownedRoot := DefaultVolumeDir() + ".owned"

	_, err := createVolumeWithHook("db", time.Unix(20, 0), func(stage string) error {
		if stage != volumeCreateStageRootPinned {
			return nil
		}
		if err := os.Rename(DefaultVolumeDir(), ownedRoot); err != nil {
			return err
		}
		return os.Symlink(outside, DefaultVolumeDir())
	})
	if err == nil || !strings.Contains(err.Error(), "changed filesystem identity") {
		t.Fatalf("volume-root replacement create error=%v", err)
	}
	if data, err := os.ReadFile(sentinel); err != nil || string(data) != "keep" {
		t.Fatalf("outside sentinel changed: data=%q err=%v", data, err)
	}
	if _, err := os.Lstat(filepath.Join(outside, "db")); !os.IsNotExist(err) {
		t.Fatalf("creation escaped through replaced volume root: %v", err)
	}
	if _, err := os.Lstat(filepath.Join(ownedRoot, "db")); !os.IsNotExist(err) {
		t.Fatalf("creation continued in detached original volume root: %v", err)
	}
}

func TestPinnedCreateRejectsVolumeReplacementBeforeMetadataPublish(t *testing.T) {
	setVolumeTestHome(t)
	vol, err := CreateVolume("db")
	if err != nil {
		t.Fatal(err)
	}
	sentinel := filepath.Join(vol.MountPath, "sentinel")
	if err := os.WriteFile(sentinel, []byte("preserve-data"), 0o600); err != nil {
		t.Fatal(err)
	}
	metaPath := filepath.Join(DefaultVolumeDir(), "db", "volume.json")
	originalMeta, err := os.ReadFile(metaPath)
	if err != nil {
		t.Fatal(err)
	}

	ownedVolume := filepath.Join(DefaultVolumeDir(), "db.owned")
	outside := t.TempDir()
	outsideSentinel := filepath.Join(outside, "outside-sentinel")
	if err := os.WriteFile(outsideSentinel, []byte("outside"), 0o600); err != nil {
		t.Fatal(err)
	}

	_, err = createVolumeWithHook("db", time.Unix(30, 0), func(stage string) error {
		if stage != volumeCreateStageBeforePublish {
			return nil
		}
		if err := os.Rename(filepath.Join(DefaultVolumeDir(), "db"), ownedVolume); err != nil {
			return err
		}
		return os.Symlink(outside, filepath.Join(DefaultVolumeDir(), "db"))
	})
	if err == nil || !strings.Contains(err.Error(), "changed filesystem identity") {
		t.Fatalf("volume replacement create error=%v", err)
	}
	if _, err := os.Lstat(filepath.Join(outside, "volume.json")); !os.IsNotExist(err) {
		t.Fatalf("metadata escaped through replacement volume: %v", err)
	}
	if _, err := os.Lstat(filepath.Join(outside, "_data")); !os.IsNotExist(err) {
		t.Fatalf("data directory escaped through replacement volume: %v", err)
	}
	if data, err := os.ReadFile(outsideSentinel); err != nil || string(data) != "outside" {
		t.Fatalf("outside sentinel changed: data=%q err=%v", data, err)
	}
	movedMeta, err := os.ReadFile(filepath.Join(ownedVolume, "volume.json"))
	if err != nil {
		t.Fatalf("read original metadata after race: %v", err)
	}
	if string(movedMeta) != string(originalMeta) {
		t.Fatalf("original metadata changed after rejected recreate:\nold=%s\nnew=%s", originalMeta, movedMeta)
	}
	if data, err := os.ReadFile(filepath.Join(ownedVolume, "_data", "sentinel")); err != nil || string(data) != "preserve-data" {
		t.Fatalf("original volume data changed: data=%q err=%v", data, err)
	}
}

func statePathForVolumeTest() string {
	return filepath.Dir(DefaultVolumeDir())
}
