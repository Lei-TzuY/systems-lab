//go:build linux

package imagestore

import (
	"fmt"
	"os"
	"strings"

	"golang.org/x/sys/unix"
)

type linuxManagedImageRootFSRemoval struct {
	imageFD  int
	rootFSFD int
	rootStat unix.Stat_t
	absent   bool
	removed  bool
}

func pinManagedImageRootFS(imagesPath, imageID string) (managedImageRootFSRemoval, error) {
	// imagesPath is supplied by ImageStorageLease and names an independently
	// pinned Store image-directory descriptor (normally /proc/self/fd/<n>).
	imagesFD, err := unix.Open(imagesPath, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC, 0)
	if err != nil {
		return nil, fmt.Errorf("open leased image storage for removal: %w", err)
	}
	defer unix.Close(imagesFD)

	imageFD, err := unix.Openat(imagesFD, imageID, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		if err == unix.ENOENT {
			return &linuxManagedImageRootFSRemoval{imageFD: -1, rootFSFD: -1, absent: true}, nil
		}
		return nil, fmt.Errorf("pin managed image directory %q: %w", imageID, err)
	}

	var observed unix.Stat_t
	if err := unix.Fstatat(imageFD, "rootfs", &observed, unix.AT_SYMLINK_NOFOLLOW); err != nil {
		_ = unix.Close(imageFD)
		if err == unix.ENOENT {
			return &linuxManagedImageRootFSRemoval{imageFD: -1, rootFSFD: -1, absent: true}, nil
		}
		return nil, fmt.Errorf("inspect managed image rootfs for %q: %w", imageID, err)
	}
	if observed.Mode&unix.S_IFMT != unix.S_IFDIR {
		_ = unix.Close(imageFD)
		return nil, fmt.Errorf("managed image rootfs for %q must be a real directory", imageID)
	}

	rootFSFD, err := unix.Openat(imageFD, "rootfs", unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		_ = unix.Close(imageFD)
		return nil, fmt.Errorf("pin managed image rootfs for %q: %w", imageID, err)
	}
	var pinned unix.Stat_t
	if err := unix.Fstat(rootFSFD, &pinned); err != nil {
		_ = unix.Close(rootFSFD)
		_ = unix.Close(imageFD)
		return nil, fmt.Errorf("inspect pinned managed image rootfs for %q: %w", imageID, err)
	}
	if pinned.Mode&unix.S_IFMT != unix.S_IFDIR || pinned.Dev != observed.Dev || pinned.Ino != observed.Ino {
		_ = unix.Close(rootFSFD)
		_ = unix.Close(imageFD)
		return nil, fmt.Errorf("managed image rootfs for %q changed identity while pinning", imageID)
	}

	return &linuxManagedImageRootFSRemoval{
		imageFD:  imageFD,
		rootFSFD: rootFSFD,
		rootStat: pinned,
	}, nil
}

func (r *linuxManagedImageRootFSRemoval) Remove() error {
	if r == nil || r.absent || r.removed {
		return nil
	}
	if r.imageFD < 0 || r.rootFSFD < 0 {
		return fmt.Errorf("managed image rootfs removal is closed")
	}
	if err := removePinnedImageDirContents(r.rootFSFD); err != nil {
		return fmt.Errorf("remove managed image rootfs contents: %w", err)
	}

	var current unix.Stat_t
	if err := unix.Fstatat(r.imageFD, "rootfs", &current, unix.AT_SYMLINK_NOFOLLOW); err != nil {
		return fmt.Errorf("recheck managed image rootfs before unlink: %w", err)
	}
	if current.Mode&unix.S_IFMT != unix.S_IFDIR || current.Dev != r.rootStat.Dev || current.Ino != r.rootStat.Ino {
		return fmt.Errorf("managed image rootfs changed filesystem identity during removal")
	}

	if err := unix.Close(r.rootFSFD); err != nil {
		return fmt.Errorf("close managed image rootfs before unlink: %w", err)
	}
	r.rootFSFD = -1
	if err := unix.Unlinkat(r.imageFD, "rootfs", unix.AT_REMOVEDIR); err != nil {
		return fmt.Errorf("unlink managed image rootfs directory: %w", err)
	}
	r.removed = true
	return nil
}

func (r *linuxManagedImageRootFSRemoval) Close() error {
	if r == nil {
		return nil
	}
	var first error
	if r.rootFSFD >= 0 {
		if err := unix.Close(r.rootFSFD); err != nil && first == nil {
			first = fmt.Errorf("close pinned managed rootfs: %w", err)
		}
		r.rootFSFD = -1
	}
	if r.imageFD >= 0 {
		if err := unix.Close(r.imageFD); err != nil && first == nil {
			first = fmt.Errorf("close pinned managed image directory: %w", err)
		}
		r.imageFD = -1
	}
	return first
}

func removePinnedImageDirContents(dirFD int) error {
	dupFD, err := unix.Dup(dirFD)
	if err != nil {
		return fmt.Errorf("duplicate directory descriptor for enumeration: %w", err)
	}
	file := os.NewFile(uintptr(dupFD), "image-rootfs-remove-dir")
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
		if err := removePinnedImageDirContents(childFD); err != nil {
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
