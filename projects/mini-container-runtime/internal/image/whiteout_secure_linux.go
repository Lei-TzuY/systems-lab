//go:build linux

package image

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"

	"golang.org/x/sys/unix"
)

func removeWhiteoutSecure(target, destDir string) error {
	return removeWhiteoutSecureWithHook(target, destDir, nil)
}

func removeWhiteoutSecureWithHook(target, destDir string, beforeRemove func()) error {
	root, err := openExtractionRoot(destDir)
	if err != nil {
		return err
	}
	defer root.Close()

	parent, err := root.openParent(target, "whiteout", false)
	if errors.Is(err, unix.ENOENT) {
		return nil
	}
	if err != nil {
		return err
	}
	defer parent.Close()

	if beforeRemove != nil {
		beforeRemove()
	}
	if err := removeTreeAt(parent.fd, parent.leaf); err != nil {
		return fmt.Errorf("remove whiteout target %s relative to pinned parent: %w", target, err)
	}
	return nil
}

func clearOpaqueWhiteoutSecure(targetDir, destDir string) error {
	return clearOpaqueWhiteoutSecureWithHook(targetDir, destDir, nil)
}

func clearOpaqueWhiteoutSecureWithHook(targetDir, destDir string, beforeRemove func()) error {
	root, err := openExtractionRoot(destDir)
	if err != nil {
		return err
	}
	defer root.Close()

	dirFD, closeDir, err := root.openDirectory(targetDir, "opaque whiteout")
	if errors.Is(err, unix.ENOENT) || errors.Is(err, unix.ENOTDIR) {
		return nil
	}
	if err != nil {
		return err
	}
	defer closeDir()

	if beforeRemove != nil {
		beforeRemove()
	}
	if err := clearDirectoryFD(dirFD); err != nil {
		return fmt.Errorf("clear opaque whiteout directory %s: %w", targetDir, err)
	}
	return nil
}

func (r *extractionRoot) openDirectory(target, role string) (int, func(), error) {
	if r == nil || r.fd < 0 {
		return -1, func() {}, fmt.Errorf("extraction root is closed")
	}
	targetAbs, err := filepath.Abs(target)
	if err != nil {
		return -1, func() {}, fmt.Errorf("resolve %s target: %w", role, err)
	}
	if targetAbs == r.abs {
		fd, err := unix.Dup(r.fd)
		if err != nil {
			return -1, func() {}, fmt.Errorf("duplicate extraction root for %s: %w", role, err)
		}
		unix.CloseOnExec(fd)
		return fd, func() { _ = unix.Close(fd) }, nil
	}

	parent, err := r.openParent(target, role, false)
	if err != nil {
		return -1, func() {}, err
	}
	fd, openErr := unix.Openat(parent.fd, parent.leaf, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	parent.Close()
	if openErr != nil {
		return -1, func() {}, fmt.Errorf("open %s directory %s without symlinks: %w", role, target, openErr)
	}
	return fd, func() { _ = unix.Close(fd) }, nil
}

func clearDirectoryFD(dirFD int) error {
	dupFD, err := unix.Dup(dirFD)
	if err != nil {
		return fmt.Errorf("duplicate directory fd for enumeration: %w", err)
	}
	unix.CloseOnExec(dupFD)
	f := os.NewFile(uintptr(dupFD), "whiteout-directory")
	if f == nil {
		_ = unix.Close(dupFD)
		return fmt.Errorf("wrap directory fd for enumeration")
	}
	names, err := f.Readdirnames(-1)
	_ = f.Close()
	if err != nil {
		return fmt.Errorf("read directory entries: %w", err)
	}
	for _, name := range names {
		if err := removeTreeAt(dirFD, name); err != nil {
			return err
		}
	}
	return nil
}

func removeTreeAt(parentFD int, name string) error {
	if name == "" || name == "." || name == ".." {
		return fmt.Errorf("invalid whiteout leaf %q", name)
	}

	var st unix.Stat_t
	if err := unix.Fstatat(parentFD, name, &st, unix.AT_SYMLINK_NOFOLLOW); err != nil {
		if errors.Is(err, unix.ENOENT) {
			return nil
		}
		return fmt.Errorf("inspect whiteout target %q: %w", name, err)
	}
	if st.Mode&unix.S_IFMT != unix.S_IFDIR {
		if err := unix.Unlinkat(parentFD, name, 0); err != nil && !errors.Is(err, unix.ENOENT) {
			return fmt.Errorf("unlink whiteout target %q: %w", name, err)
		}
		return nil
	}

	fd, err := unix.Openat(parentFD, name, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		if errors.Is(err, unix.ENOENT) {
			return nil
		}
		return fmt.Errorf("open whiteout directory %q without symlinks: %w", name, err)
	}
	var opened unix.Stat_t
	if err := unix.Fstat(fd, &opened); err != nil {
		_ = unix.Close(fd)
		return fmt.Errorf("stat opened whiteout directory %q: %w", name, err)
	}
	if err := clearDirectoryFD(fd); err != nil {
		_ = unix.Close(fd)
		return err
	}

	var current unix.Stat_t
	statErr := unix.Fstatat(parentFD, name, &current, unix.AT_SYMLINK_NOFOLLOW)
	if statErr != nil {
		_ = unix.Close(fd)
		if errors.Is(statErr, unix.ENOENT) {
			return nil
		}
		return fmt.Errorf("reinspect whiteout directory %q: %w", name, statErr)
	}
	if current.Dev != opened.Dev || current.Ino != opened.Ino || current.Mode&unix.S_IFMT != unix.S_IFDIR {
		_ = unix.Close(fd)
		return fmt.Errorf("whiteout directory %q changed identity during removal", name)
	}
	_ = unix.Close(fd)
	if err := unix.Unlinkat(parentFD, name, unix.AT_REMOVEDIR); err != nil && !errors.Is(err, unix.ENOENT) {
		return fmt.Errorf("remove whiteout directory %q: %w", name, err)
	}
	return nil
}
