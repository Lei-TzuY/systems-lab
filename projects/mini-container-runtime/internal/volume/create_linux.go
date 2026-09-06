//go:build linux

package volume

import (
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"golang.org/x/sys/unix"
	"minicontainer/internal/state"
)

const (
	volumeCreateStageStatePinned   = "state-pinned"
	volumeCreateStageRootPinned    = "volume-root-pinned"
	volumeCreateStageVolumePinned  = "volume-pinned"
	volumeCreateStageBeforePublish = "before-metadata-publish"
)

type volumeCreateHook func(stage string) error

func createVolume(name string, createdAt time.Time) (*Volume, error) {
	return createVolumeWithHook(name, createdAt, nil)
}

// createVolumeWithHook creates or reopens a named volume while every mutation
// below the state root is relative to pinned directory descriptors. The hook is
// test-only fault/race injection; production callers pass nil.
func createVolumeWithHook(name string, createdAt time.Time, hook volumeCreateHook) (*Volume, error) {
	statePath := state.DefaultDir()
	stateFD, err := openOrCreateManagedStateDir(statePath)
	if err != nil {
		return nil, fmt.Errorf("pin state directory for volume creation: %w", err)
	}
	defer unix.Close(stateFD)
	if err := runVolumeCreateHook(hook, volumeCreateStageStatePinned); err != nil {
		return nil, err
	}
	if err := requireConfiguredDirIdentity(statePath, stateFD, "state directory"); err != nil {
		return nil, err
	}

	volumesFD, _, err := openOrCreateDirAt(stateFD, "volumes", 0o700, "volume storage directory")
	if err != nil {
		return nil, err
	}
	defer unix.Close(volumesFD)
	if err := runVolumeCreateHook(hook, volumeCreateStageRootPinned); err != nil {
		return nil, err
	}
	if err := requireConfiguredDirIdentity(statePath, stateFD, "state directory"); err != nil {
		return nil, err
	}
	if err := requirePinnedChildIdentity(stateFD, "volumes", volumesFD, "volume storage directory"); err != nil {
		return nil, err
	}

	volumeFD, _, err := openOrCreateDirAt(volumesFD, name, 0o700, fmt.Sprintf("volume %q directory", name))
	if err != nil {
		return nil, err
	}
	defer unix.Close(volumeFD)
	if err := runVolumeCreateHook(hook, volumeCreateStageVolumePinned); err != nil {
		return nil, err
	}
	if err := requireVolumeCreationChain(statePath, stateFD, volumesFD, name, volumeFD); err != nil {
		return nil, err
	}

	dataFD, _, err := openOrCreateDirAt(volumeFD, "_data", 0o755, fmt.Sprintf("volume %q data directory", name))
	if err != nil {
		return nil, err
	}
	defer unix.Close(dataFD)
	if err := runVolumeCreateHook(hook, volumeCreateStageBeforePublish); err != nil {
		return nil, err
	}
	if err := requireVolumeCreationChain(statePath, stateFD, volumesFD, name, volumeFD); err != nil {
		return nil, err
	}
	if err := requirePinnedChildIdentity(volumeFD, "_data", dataFD, fmt.Sprintf("volume %q data directory", name)); err != nil {
		return nil, err
	}

	vol := &Volume{
		Name:      name,
		MountPath: filepath.Join(DefaultVolumeDir(), name, "_data"),
		CreatedAt: createdAt,
	}
	if err := writePinnedVolumeMetadata(volumeFD, name, vol); err != nil {
		return nil, err
	}

	// Metadata publication is useful only if the configured path still names the
	// exact directory generation we modified. Fail closed on pathname replacement.
	if err := requireVolumeCreationChain(statePath, stateFD, volumesFD, name, volumeFD); err != nil {
		return nil, err
	}
	if err := requirePinnedChildIdentity(volumeFD, "_data", dataFD, fmt.Sprintf("volume %q data directory", name)); err != nil {
		return nil, err
	}
	if err := unix.Fsync(volumeFD); err != nil {
		return nil, fmt.Errorf("sync volume %q directory: %w", name, err)
	}
	if err := unix.Fsync(volumesFD); err != nil {
		return nil, fmt.Errorf("sync volume storage directory: %w", err)
	}
	if err := unix.Fsync(stateFD); err != nil {
		return nil, fmt.Errorf("sync state directory after volume creation: %w", err)
	}
	return vol, nil
}

func runVolumeCreateHook(hook volumeCreateHook, stage string) error {
	if hook == nil {
		return nil
	}
	if err := hook(stage); err != nil {
		return fmt.Errorf("volume creation hook at %s: %w", stage, err)
	}
	return nil
}

func openOrCreateDirAt(parentFD int, name string, mode uint32, label string) (int, bool, error) {
	created := false
	if err := unix.Mkdirat(parentFD, name, mode); err != nil {
		if !errors.Is(err, unix.EEXIST) {
			return -1, false, fmt.Errorf("create %s: %w", label, err)
		}
	} else {
		created = true
	}
	fd, err := unix.Openat(parentFD, name, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return -1, created, pinnedDirOpenError(label, err)
	}
	var st unix.Stat_t
	if err := unix.Fstat(fd, &st); err != nil {
		_ = unix.Close(fd)
		return -1, created, fmt.Errorf("inspect pinned %s: %w", label, err)
	}
	if st.Mode&unix.S_IFMT != unix.S_IFDIR {
		_ = unix.Close(fd)
		return -1, created, fmt.Errorf("%s is not a real directory", label)
	}
	if err := unix.Fchmod(fd, mode); err != nil {
		_ = unix.Close(fd)
		return -1, created, fmt.Errorf("secure pinned %s: %w", label, err)
	}
	return fd, created, nil
}

func requireConfiguredDirIdentity(path string, fd int, label string) error {
	var pinned unix.Stat_t
	if err := unix.Fstat(fd, &pinned); err != nil {
		return fmt.Errorf("inspect pinned %s: %w", label, err)
	}
	var current unix.Stat_t
	if err := unix.Lstat(path, &current); err != nil {
		return fmt.Errorf("recheck configured %s: %w", label, err)
	}
	if current.Mode&unix.S_IFMT != unix.S_IFDIR || current.Dev != pinned.Dev || current.Ino != pinned.Ino {
		return fmt.Errorf("configured %s changed filesystem identity", label)
	}
	return nil
}

func requirePinnedChildIdentity(parentFD int, name string, childFD int, label string) error {
	var pinned unix.Stat_t
	if err := unix.Fstat(childFD, &pinned); err != nil {
		return fmt.Errorf("inspect pinned %s: %w", label, err)
	}
	var current unix.Stat_t
	if err := unix.Fstatat(parentFD, name, &current, unix.AT_SYMLINK_NOFOLLOW); err != nil {
		return fmt.Errorf("recheck %s: %w", label, err)
	}
	if current.Mode&unix.S_IFMT != unix.S_IFDIR || current.Dev != pinned.Dev || current.Ino != pinned.Ino {
		return fmt.Errorf("%s changed filesystem identity", label)
	}
	return nil
}

func requireVolumeCreationChain(statePath string, stateFD, volumesFD int, name string, volumeFD int) error {
	if err := requireConfiguredDirIdentity(statePath, stateFD, "state directory"); err != nil {
		return err
	}
	if err := requirePinnedChildIdentity(stateFD, "volumes", volumesFD, "volume storage directory"); err != nil {
		return err
	}
	return requirePinnedChildIdentity(volumesFD, name, volumeFD, fmt.Sprintf("volume %q directory", name))
}

func writePinnedVolumeMetadata(volumeFD int, name string, vol *Volume) error {
	data, err := json.MarshalIndent(vol, "", "  ")
	if err != nil {
		return fmt.Errorf("marshal volume metadata: %w", err)
	}
	if err := validateMetadataLeafAt(volumeFD, name); err != nil {
		return err
	}

	tmpName, tmpFD, err := createPinnedMetadataTemp(volumeFD)
	if err != nil {
		return err
	}
	renamed := false
	defer func() {
		if tmpFD >= 0 {
			_ = unix.Close(tmpFD)
		}
		if !renamed {
			_ = unix.Unlinkat(volumeFD, tmpName, 0)
		}
	}()

	file := os.NewFile(uintptr(tmpFD), tmpName)
	if file == nil {
		return fmt.Errorf("wrap volume metadata temp descriptor")
	}
	tmpFD = -1 // os.File owns the descriptor from here.
	n, err := file.Write(data)
	if err != nil {
		_ = file.Close()
		return fmt.Errorf("write volume metadata: %w", err)
	}
	if n != len(data) {
		_ = file.Close()
		return fmt.Errorf("write volume metadata: short write %d/%d", n, len(data))
	}
	if err := file.Sync(); err != nil {
		_ = file.Close()
		return fmt.Errorf("sync volume metadata: %w", err)
	}
	if err := file.Close(); err != nil {
		return fmt.Errorf("close volume metadata: %w", err)
	}
	if err := validateMetadataLeafAt(volumeFD, name); err != nil {
		return err
	}
	if err := unix.Renameat(volumeFD, tmpName, volumeFD, "volume.json"); err != nil {
		return fmt.Errorf("publish volume metadata: %w", err)
	}
	renamed = true
	if err := unix.Fsync(volumeFD); err != nil {
		return fmt.Errorf("sync volume directory after metadata publish: %w", err)
	}
	return nil
}

func validateMetadataLeafAt(volumeFD int, name string) error {
	var st unix.Stat_t
	err := unix.Fstatat(volumeFD, "volume.json", &st, unix.AT_SYMLINK_NOFOLLOW)
	if errors.Is(err, unix.ENOENT) {
		return nil
	}
	if err != nil {
		return fmt.Errorf("inspect volume %q metadata: %w", name, err)
	}
	if st.Mode&unix.S_IFMT != unix.S_IFREG {
		return fmt.Errorf("volume metadata is not a regular file")
	}
	return nil
}

func createPinnedMetadataTemp(volumeFD int) (string, int, error) {
	for attempt := 0; attempt < 16; attempt++ {
		var suffix [8]byte
		if _, err := rand.Read(suffix[:]); err != nil {
			return "", -1, fmt.Errorf("generate volume metadata temp name: %w", err)
		}
		name := ".volume-" + hex.EncodeToString(suffix[:]) + ".tmp"
		fd, err := unix.Openat(volumeFD, name, unix.O_WRONLY|unix.O_CREAT|unix.O_EXCL|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0o600)
		if errors.Is(err, unix.EEXIST) {
			continue
		}
		if err != nil {
			return "", -1, fmt.Errorf("create pinned volume metadata temp file: %w", err)
		}
		if err := unix.Fchmod(fd, 0o600); err != nil {
			_ = unix.Close(fd)
			_ = unix.Unlinkat(volumeFD, name, 0)
			return "", -1, fmt.Errorf("secure pinned volume metadata temp file: %w", err)
		}
		return name, fd, nil
	}
	return "", -1, fmt.Errorf("allocate unique volume metadata temp file")
}
