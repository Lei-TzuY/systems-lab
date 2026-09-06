//go:build linux

package builder

import (
	"fmt"
	"io"
	"os"
	"path"
	"path/filepath"
	"sort"
	"strings"

	"golang.org/x/sys/unix"
)

type buildSourceOpenHook func(parentFD int, name string, final bool) error

func copyRegularFile(src, dstRoot, dstLogical string, _ os.FileMode) error {
	in, err := openPinnedBuildSource(src, nil)
	if err != nil {
		return err
	}
	defer in.Close()

	info, err := in.Stat()
	if err != nil {
		return fmt.Errorf("inspect pinned build source %q: %w", src, err)
	}
	if !info.Mode().IsRegular() {
		return fmt.Errorf("source %q is not a regular file", src)
	}
	return copyOpenedBuildRegularFile(in, dstRoot, dstLogical, info.Mode())
}

func copyTree(src, dstRoot, dstLogical string, allowSymlinks bool) error {
	in, err := openPinnedBuildSource(src, nil)
	if err != nil {
		return err
	}
	defer in.Close()
	return copyPinnedBuildNode(in, src, dstRoot, dstLogical, allowSymlinks)
}

func openPinnedBuildSource(source string, hook buildSourceOpenHook) (*os.File, error) {
	abs, err := filepath.Abs(source)
	if err != nil {
		return nil, fmt.Errorf("resolve build source %q: %w", source, err)
	}
	abs = filepath.Clean(abs)
	if abs == string(filepath.Separator) {
		fd, err := unix.Open(abs, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
		if err != nil {
			return nil, fmt.Errorf("open build source root %q: %w", abs, err)
		}
		return os.NewFile(uintptr(fd), abs), nil
	}

	parts := strings.Split(strings.TrimPrefix(abs, string(filepath.Separator)), string(filepath.Separator))
	parentFD, err := unix.Open(string(filepath.Separator), unix.O_PATH|unix.O_DIRECTORY|unix.O_CLOEXEC, 0)
	if err != nil {
		return nil, fmt.Errorf("pin filesystem root for build source %q: %w", abs, err)
	}
	defer func() {
		if parentFD >= 0 {
			_ = unix.Close(parentFD)
		}
	}()

	for i, part := range parts {
		if part == "" || part == "." || part == ".." {
			return nil, fmt.Errorf("invalid build source component %q in %q", part, abs)
		}
		final := i == len(parts)-1
		if hook != nil {
			if err := hook(parentFD, part, final); err != nil {
				return nil, err
			}
		}

		flags := unix.O_PATH | unix.O_CLOEXEC | unix.O_NOFOLLOW
		if !final {
			flags |= unix.O_DIRECTORY
		}
		fd, err := unix.Openat(parentFD, part, flags, 0)
		if err != nil {
			return nil, fmt.Errorf("pin build source component %q without following symlinks: %w", part, err)
		}
		if !final {
			_ = unix.Close(parentFD)
			parentFD = fd
			continue
		}

		var st unix.Stat_t
		if err := unix.Fstat(fd, &st); err != nil {
			_ = unix.Close(fd)
			return nil, fmt.Errorf("inspect pinned build source %q: %w", abs, err)
		}
		typeBits := st.Mode & unix.S_IFMT
		if typeBits != unix.S_IFDIR && typeBits != unix.S_IFREG {
			_ = unix.Close(fd)
			if typeBits == unix.S_IFLNK {
				return nil, fmt.Errorf("build source root %q must not be a symlink", source)
			}
			return nil, fmt.Errorf("unsupported special build source %q", source)
		}
		readFD, err := reopenPinnedBuildFD(fd, typeBits == unix.S_IFDIR)
		_ = unix.Close(fd)
		if err != nil {
			return nil, fmt.Errorf("open pinned build source %q for reading: %w", abs, err)
		}
		return os.NewFile(uintptr(readFD), abs), nil
	}
	return nil, fmt.Errorf("empty build source path %q", source)
}

func reopenPinnedBuildFD(pathFD int, directory bool) (int, error) {
	flags := unix.O_RDONLY | unix.O_CLOEXEC | unix.O_NONBLOCK
	if directory {
		flags |= unix.O_DIRECTORY
	}
	return unix.Open(fmt.Sprintf("/proc/self/fd/%d", pathFD), flags, 0)
}

func copyPinnedBuildNode(in *os.File, sourceLabel, dstRoot, dstLogical string, allowSymlinks bool) error {
	info, err := in.Stat()
	if err != nil {
		return fmt.Errorf("inspect pinned build source %q: %w", sourceLabel, err)
	}
	if info.Mode().IsRegular() {
		return copyOpenedBuildRegularFile(in, dstRoot, dstLogical, info.Mode())
	}
	if !info.IsDir() {
		return fmt.Errorf("unsupported special file %q in build source", sourceLabel)
	}
	if err := mkdirRootFSPath(dstRoot, dstLogical, info.Mode().Perm()); err != nil {
		return err
	}

	entries, err := in.ReadDir(-1)
	if err != nil {
		return fmt.Errorf("read pinned build source directory %q: %w", sourceLabel, err)
	}
	sort.Slice(entries, func(i, j int) bool { return entries[i].Name() < entries[j].Name() })
	parentFD := int(in.Fd())
	for _, entry := range entries {
		name := entry.Name()
		if name == "" || name == "." || name == ".." || strings.ContainsRune(name, filepath.Separator) {
			return fmt.Errorf("invalid build source directory entry %q", name)
		}
		var observed unix.Stat_t
		if err := unix.Fstatat(parentFD, name, &observed, unix.AT_SYMLINK_NOFOLLOW); err != nil {
			return fmt.Errorf("inspect build source child %q: %w", filepath.Join(sourceLabel, name), err)
		}
		pathFD, err := pinObservedBuildChild(parentFD, name, &observed)
		if err != nil {
			return fmt.Errorf("pin build source child %q: %w", filepath.Join(sourceLabel, name), err)
		}

		childLogical := path.Join(dstLogical, filepath.ToSlash(name))
		typeBits := observed.Mode & unix.S_IFMT
		switch typeBits {
		case unix.S_IFLNK:
			if !allowSymlinks {
				_ = unix.Close(pathFD)
				return fmt.Errorf("COPY source tree contains symlink %q", filepath.Join(sourceLabel, name))
			}
			target, readErr := readPinnedBuildSymlink(pathFD)
			_ = unix.Close(pathFD)
			if readErr != nil {
				return fmt.Errorf("read pinned build symlink %q: %w", filepath.Join(sourceLabel, name), readErr)
			}
			if err := copyPinnedBuildSymlink(target, dstRoot, childLogical); err != nil {
				return err
			}
		case unix.S_IFDIR, unix.S_IFREG:
			readFD, openErr := reopenPinnedBuildFD(pathFD, typeBits == unix.S_IFDIR)
			_ = unix.Close(pathFD)
			if openErr != nil {
				return fmt.Errorf("open pinned build source child %q: %w", filepath.Join(sourceLabel, name), openErr)
			}
			child := os.NewFile(uintptr(readFD), filepath.Join(sourceLabel, name))
			copyErr := copyPinnedBuildNode(child, filepath.Join(sourceLabel, name), dstRoot, childLogical, allowSymlinks)
			closeErr := child.Close()
			if copyErr != nil {
				return copyErr
			}
			if closeErr != nil {
				return fmt.Errorf("close pinned build source child %q: %w", filepath.Join(sourceLabel, name), closeErr)
			}
		default:
			_ = unix.Close(pathFD)
			return fmt.Errorf("unsupported special file %q in build source", filepath.Join(sourceLabel, name))
		}
	}
	return nil
}

func pinObservedBuildChild(parentFD int, name string, observed *unix.Stat_t) (int, error) {
	fd, err := unix.Openat(parentFD, name, unix.O_PATH|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return -1, err
	}
	var pinned unix.Stat_t
	if err := unix.Fstat(fd, &pinned); err != nil {
		_ = unix.Close(fd)
		return -1, err
	}
	if observed.Dev != pinned.Dev || observed.Ino != pinned.Ino || observed.Mode&unix.S_IFMT != pinned.Mode&unix.S_IFMT {
		_ = unix.Close(fd)
		return -1, fmt.Errorf("build source child changed generation before it could be pinned")
	}
	return fd, nil
}

func readPinnedBuildSymlink(pathFD int) (string, error) {
	for size := 256; size <= 1<<20; size *= 2 {
		buf := make([]byte, size)
		n, err := unix.Readlinkat(pathFD, "", buf)
		if err != nil {
			return "", err
		}
		if n < len(buf) {
			return string(buf[:n]), nil
		}
	}
	return "", fmt.Errorf("symlink target exceeds supported size")
}

func copyPinnedBuildSymlink(target, dstRoot, dstLogical string) error {
	if err := mkdirRootFSPath(dstRoot, path.Dir(dstLogical), 0o755); err != nil {
		return err
	}
	dst, err := resolveRootFSLeaf(dstRoot, dstLogical)
	if err != nil {
		return err
	}
	if err := os.Remove(dst); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("replace destination symlink %q: %w", dstLogical, err)
	}
	if err := os.Symlink(target, dst); err != nil {
		return fmt.Errorf("copy pinned build symlink to %q: %w", dstLogical, err)
	}
	return nil
}

func copyOpenedBuildRegularFile(in *os.File, dstRoot, dstLogical string, mode os.FileMode) error {
	if err := mkdirRootFSPath(dstRoot, path.Dir(dstLogical), 0o755); err != nil {
		return err
	}
	dst, err := resolveRootFSPath(dstRoot, dstLogical)
	if err != nil {
		return err
	}
	out, err := os.OpenFile(dst, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, mode.Perm())
	if err != nil {
		return err
	}
	_, copyErr := io.Copy(out, in)
	closeErr := out.Close()
	if copyErr != nil {
		return copyErr
	}
	return closeErr
}
