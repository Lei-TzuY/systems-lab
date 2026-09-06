//go:build linux

package volume

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"

	"golang.org/x/sys/unix"
	"minicontainer/internal/state"
)

const maxVolumeMetadataBytes = 1 << 20

// removeVolumeDir removes one validated managed volume while every destructive
// pathname lookup is anchored to already-open directory descriptors. Renaming
// or replacing ~/.minicontainer/volumes after validation therefore cannot
// redirect recursive deletion through a new pathname generation.
func removeVolumeDir(root, name string) error {
	if filepath.Clean(root) != filepath.Clean(DefaultVolumeDir()) {
		return fmt.Errorf("volume removal root %q is not the managed volume root %q", root, DefaultVolumeDir())
	}

	stateFD, err := openAbsoluteDirNoSymlinks(state.DefaultDir())
	if err != nil {
		return fmt.Errorf("pin state directory for volume removal: %w", err)
	}
	defer unix.Close(stateFD)

	volumesFD, err := unix.Openat(stateFD, "volumes", unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return fmt.Errorf("pin volume storage directory: %w", err)
	}
	defer unix.Close(volumesFD)

	volumeFD, err := unix.Openat(volumesFD, name, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return fmt.Errorf("pin volume %q directory: %w", name, err)
	}
	defer unix.Close(volumeFD)

	var owned unix.Stat_t
	if err := unix.Fstat(volumeFD, &owned); err != nil {
		return fmt.Errorf("inspect pinned volume %q: %w", name, err)
	}
	if owned.Mode&unix.S_IFMT != unix.S_IFDIR {
		return fmt.Errorf("volume %q is not a directory", name)
	}

	if err := validatePinnedVolume(volumeFD, root, name); err != nil {
		return err
	}
	if err := removePinnedDirContents(volumeFD); err != nil {
		return fmt.Errorf("remove contents of volume %q: %w", name, err)
	}

	// Refuse to unlink whatever currently occupies name unless it is still the
	// exact directory generation whose contents we just removed.
	var current unix.Stat_t
	if err := unix.Fstatat(volumesFD, name, &current, unix.AT_SYMLINK_NOFOLLOW); err != nil {
		return fmt.Errorf("recheck volume %q before unlink: %w", name, err)
	}
	if current.Mode&unix.S_IFMT != unix.S_IFDIR || current.Dev != owned.Dev || current.Ino != owned.Ino {
		return fmt.Errorf("volume %q changed filesystem identity during removal", name)
	}
	if err := unix.Unlinkat(volumesFD, name, unix.AT_REMOVEDIR); err != nil {
		return fmt.Errorf("unlink volume %q directory: %w", name, err)
	}
	return nil
}

func openAbsoluteDirNoSymlinks(path string) (int, error) {
	clean := filepath.Clean(path)
	if !filepath.IsAbs(clean) {
		return -1, fmt.Errorf("path %q is not absolute", path)
	}
	fd, err := unix.Open("/", unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC, 0)
	if err != nil {
		return -1, err
	}
	if clean == string(filepath.Separator) {
		return fd, nil
	}

	for _, part := range strings.Split(strings.TrimPrefix(clean, string(filepath.Separator)), string(filepath.Separator)) {
		if part == "" || part == "." || part == ".." {
			_ = unix.Close(fd)
			return -1, fmt.Errorf("invalid directory component %q in %q", part, path)
		}
		next, err := unix.Openat(fd, part, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
		_ = unix.Close(fd)
		if err != nil {
			return -1, fmt.Errorf("open directory component %q: %w", part, err)
		}
		fd = next
	}
	return fd, nil
}

func validatePinnedVolume(volumeFD int, root, name string) error {
	dataFD, err := unix.Openat(volumeFD, "_data", unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return fmt.Errorf("validate volume %q data directory: %w", name, err)
	}
	var dataStat unix.Stat_t
	statErr := unix.Fstat(dataFD, &dataStat)
	_ = unix.Close(dataFD)
	if statErr != nil {
		return fmt.Errorf("inspect volume %q data directory: %w", name, statErr)
	}
	if dataStat.Mode&unix.S_IFMT != unix.S_IFDIR {
		return fmt.Errorf("volume %q data path is not a directory", name)
	}

	metaFD, err := unix.Openat(volumeFD, "volume.json", unix.O_RDONLY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return fmt.Errorf("open volume %q metadata: %w", name, err)
	}
	var metaStat unix.Stat_t
	if err := unix.Fstat(metaFD, &metaStat); err != nil {
		_ = unix.Close(metaFD)
		return fmt.Errorf("inspect volume %q metadata: %w", name, err)
	}
	if metaStat.Mode&unix.S_IFMT != unix.S_IFREG {
		_ = unix.Close(metaFD)
		return fmt.Errorf("volume %q metadata is not a regular file", name)
	}
	metaFile := os.NewFile(uintptr(metaFD), "volume.json")
	if metaFile == nil {
		_ = unix.Close(metaFD)
		return fmt.Errorf("wrap volume %q metadata descriptor", name)
	}
	data, err := io.ReadAll(io.LimitReader(metaFile, maxVolumeMetadataBytes+1))
	closeErr := metaFile.Close()
	if err != nil {
		return fmt.Errorf("read volume %q metadata: %w", name, err)
	}
	if closeErr != nil {
		return fmt.Errorf("close volume %q metadata: %w", name, closeErr)
	}
	if len(data) > maxVolumeMetadataBytes {
		return fmt.Errorf("volume %q metadata exceeds %d bytes", name, maxVolumeMetadataBytes)
	}
	var vol Volume
	if err := json.Unmarshal(data, &vol); err != nil {
		return fmt.Errorf("decode volume %q metadata: %w", name, err)
	}
	if vol.Name != name {
		return fmt.Errorf("volume metadata name %q does not match directory %q", vol.Name, name)
	}
	expectedData := filepath.Join(root, name, "_data")
	if filepath.Clean(vol.MountPath) != filepath.Clean(expectedData) {
		return fmt.Errorf("volume %q metadata mount path %q does not match managed data path %q", name, vol.MountPath, expectedData)
	}
	return nil
}

func removePinnedDirContents(dirFD int) error {
	dupFD, err := unix.Dup(dirFD)
	if err != nil {
		return fmt.Errorf("duplicate directory descriptor for enumeration: %w", err)
	}
	file := os.NewFile(uintptr(dupFD), "volume-remove-dir")
	if file == nil {
		_ = unix.Close(dupFD)
		return fmt.Errorf("wrap directory descriptor for enumeration")
	}
	names, readErr := file.Readdirnames(-1)
	closeErr := file.Close()
	if readErr != nil {
		return fmt.Errorf("enumerate directory: %w", readErr)
	}
	if closeErr != nil {
		return fmt.Errorf("close directory enumeration descriptor: %w", closeErr)
	}

	for _, name := range names {
		if name == "" || name == "." || name == ".." || strings.ContainsRune(name, '/') {
			return fmt.Errorf("invalid directory entry %q", name)
		}
		var observed unix.Stat_t
		if err := unix.Fstatat(dirFD, name, &observed, unix.AT_SYMLINK_NOFOLLOW); err != nil {
			return fmt.Errorf("inspect child %q: %w", name, err)
		}
		if observed.Mode&unix.S_IFMT != unix.S_IFDIR {
			if err := unix.Unlinkat(dirFD, name, 0); err != nil {
				return fmt.Errorf("unlink child %q: %w", name, err)
			}
			continue
		}

		childFD, err := unix.Openat(dirFD, name, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
		if err != nil {
			return fmt.Errorf("pin child directory %q: %w", name, err)
		}
		var pinned unix.Stat_t
		if err := unix.Fstat(childFD, &pinned); err != nil {
			_ = unix.Close(childFD)
			return fmt.Errorf("inspect pinned child directory %q: %w", name, err)
		}
		if pinned.Dev != observed.Dev || pinned.Ino != observed.Ino {
			_ = unix.Close(childFD)
			return fmt.Errorf("child directory %q changed identity while pinning", name)
		}
		if err := removePinnedDirContents(childFD); err != nil {
			_ = unix.Close(childFD)
			return err
		}
		if err := unix.Fstatat(dirFD, name, &observed, unix.AT_SYMLINK_NOFOLLOW); err != nil {
			_ = unix.Close(childFD)
			return fmt.Errorf("recheck child directory %q: %w", name, err)
		}
		if observed.Mode&unix.S_IFMT != unix.S_IFDIR || observed.Dev != pinned.Dev || observed.Ino != pinned.Ino {
			_ = unix.Close(childFD)
			return fmt.Errorf("child directory %q changed identity during removal", name)
		}
		if err := unix.Close(childFD); err != nil {
			return fmt.Errorf("close child directory %q: %w", name, err)
		}
		if err := unix.Unlinkat(dirFD, name, unix.AT_REMOVEDIR); err != nil {
			return fmt.Errorf("unlink child directory %q: %w", name, err)
		}
	}
	return nil
}
