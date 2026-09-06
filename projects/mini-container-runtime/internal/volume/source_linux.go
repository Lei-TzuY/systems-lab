//go:build linux

package volume

import (
	"fmt"
	"os"

	"golang.org/x/sys/unix"
	"minicontainer/internal/state"
)

// OpenPinnedData reopens one named volume's _data directory from the managed
// state root without following symlinks. It validates the pinned volume metadata
// and then proves every configured name still refers to the exact directory
// generation that was opened before returning the source descriptor.
func OpenPinnedData(name string) (*os.File, error) {
	if err := ValidateVolumeName(name); err != nil {
		return nil, err
	}
	statePath := state.DefaultDir()
	stateFD, err := openAbsoluteDirNoSymlinks(statePath)
	if err != nil {
		return nil, fmt.Errorf("pin state directory for volume source: %w", err)
	}
	defer unix.Close(stateFD)

	volumesFD, err := unix.Openat(stateFD, "volumes", unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return nil, pinnedDirOpenError("volume storage directory", err)
	}
	defer unix.Close(volumesFD)

	volumeFD, err := unix.Openat(volumesFD, name, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return nil, pinnedDirOpenError(fmt.Sprintf("volume %q directory", name), err)
	}
	defer unix.Close(volumeFD)

	dataFD, err := unix.Openat(volumeFD, "_data", unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return nil, pinnedDirOpenError(fmt.Sprintf("volume %q data directory", name), err)
	}

	cleanupData := true
	defer func() {
		if cleanupData {
			_ = unix.Close(dataFD)
		}
	}()

	if err := validatePinnedVolume(volumeFD, DefaultVolumeDir(), name); err != nil {
		return nil, err
	}
	if err := requireConfiguredDirIdentity(statePath, stateFD, "state directory"); err != nil {
		return nil, err
	}
	if err := requirePinnedChildIdentity(stateFD, "volumes", volumesFD, "volume storage directory"); err != nil {
		return nil, err
	}
	if err := requirePinnedChildIdentity(volumesFD, name, volumeFD, fmt.Sprintf("volume %q directory", name)); err != nil {
		return nil, err
	}
	if err := requirePinnedChildIdentity(volumeFD, "_data", dataFD, fmt.Sprintf("volume %q data directory", name)); err != nil {
		return nil, err
	}

	file := os.NewFile(uintptr(dataFD), "managed-volume-"+name)
	if file == nil {
		return nil, fmt.Errorf("wrap volume %q data descriptor", name)
	}
	cleanupData = false
	return file, nil
}
