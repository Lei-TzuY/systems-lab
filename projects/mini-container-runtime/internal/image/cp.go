// internal/image/cp.go
//
// Container File Transfer (`minictl cp`)
// ─────────────────────────────────────────
// Copies files and directories bidirectionally between host filesystem and
// container RootFS (`minictl cp hostPath id:containerPath` / `minictl cp id:containerPath hostPath`).

package image

import (
	"fmt"
	"io"
	"os"
	pathpkg "path"
	"path/filepath"
	"strings"
)

// CopyPath copies a file, directory, or symbolic link without dereferencing
// symbolic links. Existing symlink components in source or destination paths
// are rejected so a container-controlled link cannot redirect host-side reads
// or writes outside the path selected by the caller.
func CopyPath(src, dst string) error {
	if err := rejectSymlinkComponents(filepath.Dir(src)); err != nil {
		return fmt.Errorf("unsafe source path %q: %w", src, err)
	}
	info, err := os.Lstat(src)
	if err != nil {
		return fmt.Errorf("lstat source %q: %w", src, err)
	}

	if info.Mode()&os.ModeSymlink != 0 {
		return copySymlink(src, dst)
	}
	if info.IsDir() {
		return copyDir(src, dst)
	}
	if !info.Mode().IsRegular() {
		return fmt.Errorf("unsupported source file type %q", src)
	}
	return copyRegularFile(src, dst, info)
}

func copyRegularFile(src, dst string, expected os.FileInfo) error {
	resolvedDst, err := resolveDestination(src, dst)
	if err != nil {
		return err
	}
	if err := ensureDirNoSymlink(filepath.Dir(resolvedDst), 0o755); err != nil {
		return err
	}
	if info, err := os.Lstat(resolvedDst); err == nil {
		if info.Mode()&os.ModeSymlink != 0 {
			return fmt.Errorf("destination %q is a symbolic link", resolvedDst)
		}
		if info.IsDir() {
			return fmt.Errorf("destination %q unexpectedly resolved to a directory", resolvedDst)
		}
	} else if !os.IsNotExist(err) {
		return fmt.Errorf("inspect destination %q: %w", resolvedDst, err)
	}

	srcFile, err := os.Open(src)
	if err != nil {
		return err
	}
	defer srcFile.Close()
	openedInfo, err := srcFile.Stat()
	if err != nil {
		return fmt.Errorf("stat opened source %q: %w", src, err)
	}
	if !openedInfo.Mode().IsRegular() || !os.SameFile(expected, openedInfo) {
		return fmt.Errorf("source %q changed identity while opening", src)
	}

	parent := filepath.Dir(resolvedDst)
	tmp, err := os.CreateTemp(parent, ".minictl-cp-*.tmp")
	if err != nil {
		return fmt.Errorf("create destination temp file: %w", err)
	}
	tmpName := tmp.Name()
	keep := false
	defer func() {
		_ = tmp.Close()
		if !keep {
			_ = os.Remove(tmpName)
		}
	}()
	if err := tmp.Chmod(expected.Mode().Perm()); err != nil {
		return fmt.Errorf("set destination mode: %w", err)
	}
	if _, err := io.Copy(tmp, srcFile); err != nil {
		return err
	}
	if err := tmp.Sync(); err != nil {
		return fmt.Errorf("sync copied file: %w", err)
	}
	if err := tmp.Close(); err != nil {
		return fmt.Errorf("close copied file: %w", err)
	}
	if err := rejectSymlinkComponents(parent); err != nil {
		return fmt.Errorf("destination parent changed while copying: %w", err)
	}
	if info, err := os.Lstat(resolvedDst); err == nil && info.Mode()&os.ModeSymlink != 0 {
		return fmt.Errorf("destination %q became a symbolic link while copying", resolvedDst)
	} else if err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("reinspect destination %q: %w", resolvedDst, err)
	}
	if err := os.Rename(tmpName, resolvedDst); err != nil {
		return fmt.Errorf("publish copied file: %w", err)
	}
	keep = true
	return nil
}

func copySymlink(src, dst string) error {
	resolvedDst, err := resolveDestination(src, dst)
	if err != nil {
		return err
	}
	if err := ensureDirNoSymlink(filepath.Dir(resolvedDst), 0o755); err != nil {
		return err
	}
	if info, err := os.Lstat(resolvedDst); err == nil {
		if info.Mode()&os.ModeSymlink != 0 {
			return fmt.Errorf("destination %q is already a symbolic link", resolvedDst)
		}
		return fmt.Errorf("destination %q already exists", resolvedDst)
	} else if !os.IsNotExist(err) {
		return fmt.Errorf("inspect destination %q: %w", resolvedDst, err)
	}

	target, err := os.Readlink(src)
	if err != nil {
		return fmt.Errorf("read source symlink %q: %w", src, err)
	}
	if err := rejectSymlinkComponents(filepath.Dir(resolvedDst)); err != nil {
		return fmt.Errorf("destination parent changed while copying symlink: %w", err)
	}
	if err := os.Symlink(target, resolvedDst); err != nil {
		return fmt.Errorf("create destination symlink: %w", err)
	}
	return nil
}

func copyDir(src, dst string) error {
	if info, err := os.Lstat(dst); err == nil {
		if info.Mode()&os.ModeSymlink != 0 {
			return fmt.Errorf("destination %q is a symbolic link", dst)
		}
		if !info.IsDir() {
			return fmt.Errorf("destination %q is not a directory", dst)
		}
	} else if !os.IsNotExist(err) {
		return fmt.Errorf("inspect destination %q: %w", dst, err)
	}

	return filepath.Walk(src, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}

		rel, err := filepath.Rel(src, path)
		if err != nil {
			return err
		}
		target := filepath.Join(dst, rel)

		if info.Mode()&os.ModeSymlink != 0 {
			return copySymlink(path, target)
		}
		if info.IsDir() {
			return ensureDirNoSymlink(target, info.Mode().Perm())
		}
		if !info.Mode().IsRegular() {
			return fmt.Errorf("unsupported source file type %q", path)
		}
		return copyRegularFile(path, target, info)
	})
}

func resolveDestination(src, dst string) (string, error) {
	if err := rejectSymlinkComponents(filepath.Dir(dst)); err != nil {
		return "", fmt.Errorf("unsafe destination path %q: %w", dst, err)
	}
	if info, err := os.Lstat(dst); err == nil {
		if info.Mode()&os.ModeSymlink != 0 {
			return "", fmt.Errorf("destination %q is a symbolic link", dst)
		}
		if info.IsDir() {
			dst = filepath.Join(dst, filepath.Base(src))
		}
	} else if !os.IsNotExist(err) {
		return "", fmt.Errorf("inspect destination %q: %w", dst, err)
	}
	return dst, nil
}

func rejectSymlinkComponents(path string) error {
	clean, err := filepath.Abs(path)
	if err != nil {
		return err
	}
	volume := filepath.VolumeName(clean)
	rest := strings.TrimPrefix(clean, volume)
	parts := strings.FieldsFunc(rest, func(r rune) bool { return r == '/' || r == '\\' })
	current := volume + string(os.PathSeparator)
	if volume == "" {
		current = string(os.PathSeparator)
	}
	for _, part := range parts {
		current = filepath.Join(current, part)
		info, err := os.Lstat(current)
		if err != nil {
			if os.IsNotExist(err) {
				return nil
			}
			return err
		}
		if info.Mode()&os.ModeSymlink != 0 {
			return fmt.Errorf("path component %q is a symbolic link", current)
		}
		if !info.IsDir() {
			return fmt.Errorf("path component %q is not a directory", current)
		}
	}
	return nil
}

func ensureDirNoSymlink(path string, mode os.FileMode) error {
	clean, err := filepath.Abs(path)
	if err != nil {
		return err
	}
	volume := filepath.VolumeName(clean)
	rest := strings.TrimPrefix(clean, volume)
	parts := strings.FieldsFunc(rest, func(r rune) bool { return r == '/' || r == '\\' })
	current := volume + string(os.PathSeparator)
	if volume == "" {
		current = string(os.PathSeparator)
	}
	for i, part := range parts {
		current = filepath.Join(current, part)
		info, err := os.Lstat(current)
		if err == nil {
			if info.Mode()&os.ModeSymlink != 0 {
				return fmt.Errorf("path component %q is a symbolic link", current)
			}
			if !info.IsDir() {
				return fmt.Errorf("path component %q is not a directory", current)
			}
			continue
		}
		if !os.IsNotExist(err) {
			return err
		}
		perm := os.FileMode(0o755)
		if i == len(parts)-1 && mode.Perm() != 0 {
			perm = mode.Perm()
		}
		if err := os.Mkdir(current, perm); err != nil && !os.IsExist(err) {
			return err
		}
		info, err = os.Lstat(current)
		if err != nil {
			return err
		}
		if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
			return fmt.Errorf("created path component %q is not a real directory", current)
		}
	}
	return nil
}

func canonicalContainerCopyPath(raw string) string {
	raw = strings.ReplaceAll(raw, "\\", "/")
	raw = strings.TrimPrefix(raw, "/")
	return pathpkg.Clean("/" + raw)
}

func looksLikeWindowsDrivePath(arg string, colon int) bool {
	if colon != 1 || len(arg) < 3 {
		return false
	}
	c := arg[0]
	if !((c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z')) {
		return false
	}
	return arg[2] == '/' || arg[2] == '\\'
}

// ParseCopyTarget splits "id:/path" into ("id", "/path") or returns ("", arg) if plain path.
// Container paths are normalized relative to the container root so lexical `..`
// components cannot escape when the CLI joins them beneath RootFS.
func ParseCopyTarget(arg string) (string, string) {
	if idx := strings.Index(arg, ":"); idx != -1 && !looksLikeWindowsDrivePath(arg, idx) && !strings.Contains(arg[:idx], "/") && !strings.Contains(arg[:idx], "\\") {
		return arg[:idx], canonicalContainerCopyPath(arg[idx+1:])
	}
	return "", arg
}
