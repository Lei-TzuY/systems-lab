// internal/image/export.go
//
// Container & Image Export (`minictl export` / `minictl commit`)
// ─────────────────────────────────────────────────────────────
// `docker export` creates a tar stream of a container's filesystem.
// `docker commit` creates a new image from a container's current changes.
//
// This module provides functions to package any directory tree (such as a
// container's rootfs or overlay upper layer) into a `.tar` or `.tar.gz` file,
// preserving permissions, file modes, and symlinks.

package image

import (
	"archive/tar"
	"compress/gzip"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

// ExportDir packs rootDir into tarPath (.tar or .tar.gz).
//
// The archive is built in a temporary file beside tarPath and atomically
// published only after the tar/gzip writers, file sync, and file close all
// succeed. This keeps an existing destination intact on export failure and
// prevents callers from mistaking a partial archive for a successful export.
func ExportDir(rootDir, tarPath string) (retErr error) {
	sourceRoot, err := resolveExportSource(rootDir)
	if err != nil {
		return err
	}
	destinationPath, err := resolveExportDestination(tarPath)
	if err != nil {
		return err
	}

	out, err := os.CreateTemp(filepath.Dir(destinationPath), "."+filepath.Base(destinationPath)+".tmp-")
	if err != nil {
		return fmt.Errorf("create temporary archive for %q: %w", destinationPath, err)
	}
	tempPath := out.Name()
	closed := false
	published := false
	defer func() {
		if !closed {
			if err := out.Close(); err != nil {
				retErr = errors.Join(retErr, fmt.Errorf("close temporary archive %q: %w", tempPath, err))
			}
		}
		if !published {
			if err := os.Remove(tempPath); err != nil && !errors.Is(err, os.ErrNotExist) {
				retErr = errors.Join(retErr, fmt.Errorf("remove temporary archive %q: %w", tempPath, err))
			}
		}
	}()

	var writer io.Writer = out
	var gz *gzip.Writer
	if isGzipArchive(destinationPath) {
		gz = gzip.NewWriter(out)
		writer = gz
	}
	tw := tar.NewWriter(writer)

	var writeErr error
	if err := walkExportSource(sourceRoot, destinationPath, tempPath, tw); err != nil {
		writeErr = errors.Join(writeErr, err)
	}
	if err := tw.Close(); err != nil {
		writeErr = errors.Join(writeErr, fmt.Errorf("finalize tar stream: %w", err))
	}
	if gz != nil {
		if err := gz.Close(); err != nil {
			writeErr = errors.Join(writeErr, fmt.Errorf("finalize gzip stream: %w", err))
		}
	}
	if writeErr == nil {
		if err := out.Sync(); err != nil {
			writeErr = errors.Join(writeErr, fmt.Errorf("sync archive contents: %w", err))
		}
	}
	if err := out.Close(); err != nil {
		writeErr = errors.Join(writeErr, fmt.Errorf("close archive contents: %w", err))
	}
	closed = true
	if writeErr != nil {
		return fmt.Errorf("export directory %q: %w", sourceRoot, writeErr)
	}

	if err := os.Rename(tempPath, destinationPath); err != nil {
		return fmt.Errorf("publish archive %q: %w", destinationPath, err)
	}
	published = true
	return nil
}

func resolveExportSource(rootDir string) (string, error) {
	absolute, err := filepath.Abs(filepath.Clean(rootDir))
	if err != nil {
		return "", fmt.Errorf("resolve source directory %q: %w", rootDir, err)
	}
	resolved, err := filepath.EvalSymlinks(absolute)
	if err != nil {
		return "", fmt.Errorf("resolve source directory %q: %w", rootDir, err)
	}
	info, err := os.Stat(resolved)
	if err != nil {
		return "", fmt.Errorf("stat source directory %q: %w", rootDir, err)
	}
	if !info.IsDir() {
		return "", fmt.Errorf("source path %q is not a directory", rootDir)
	}
	return filepath.Clean(resolved), nil
}

func resolveExportDestination(tarPath string) (string, error) {
	if tarPath == "" {
		return "", fmt.Errorf("archive path is empty")
	}
	absolute, err := filepath.Abs(filepath.Clean(tarPath))
	if err != nil {
		return "", fmt.Errorf("resolve archive path %q: %w", tarPath, err)
	}
	base := filepath.Base(absolute)
	if base == "." || base == string(filepath.Separator) || base == "" {
		return "", fmt.Errorf("archive path %q does not name a file", tarPath)
	}
	parent, err := filepath.EvalSymlinks(filepath.Dir(absolute))
	if err != nil {
		return "", fmt.Errorf("resolve archive directory for %q: %w", tarPath, err)
	}
	info, err := os.Stat(parent)
	if err != nil {
		return "", fmt.Errorf("stat archive directory for %q: %w", tarPath, err)
	}
	if !info.IsDir() {
		return "", fmt.Errorf("archive parent for %q is not a directory", tarPath)
	}
	return filepath.Join(parent, base), nil
}

func isGzipArchive(path string) bool {
	lower := strings.ToLower(path)
	return strings.HasSuffix(lower, ".gz") || strings.HasSuffix(lower, ".tgz")
}

func walkExportSource(rootDir, destinationPath, tempPath string, tw *tar.Writer) error {
	return filepath.Walk(rootDir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return fmt.Errorf("walk %q: %w", path, err)
		}

		cleanPath := filepath.Clean(path)
		if cleanPath == destinationPath || cleanPath == tempPath {
			if info.IsDir() {
				return filepath.SkipDir
			}
			return nil
		}

		rel, err := filepath.Rel(rootDir, path)
		if err != nil {
			return fmt.Errorf("relative archive path for %q: %w", path, err)
		}
		if rel == "." {
			return nil
		}

		// Convert Windows backslashes to Unix slashes for tar compatibility.
		tarName := filepath.ToSlash(rel)
		if info.IsDir() {
			tarName += "/"
		}

		var linkTarget string
		if info.Mode()&os.ModeSymlink != 0 {
			linkTarget, err = os.Readlink(path)
			if err != nil {
				return fmt.Errorf("readlink %s: %w", path, err)
			}
		}

		hdr, err := tar.FileInfoHeader(info, linkTarget)
		if err != nil {
			return fmt.Errorf("header %s: %w", path, err)
		}
		hdr.Name = tarName

		if err := tw.WriteHeader(hdr); err != nil {
			return fmt.Errorf("write header %s: %w", tarName, err)
		}
		if !info.Mode().IsRegular() {
			return nil
		}

		f, err := os.Open(path)
		if err != nil {
			return fmt.Errorf("open %s: %w", path, err)
		}
		openedInfo, statErr := f.Stat()
		if statErr != nil {
			_ = f.Close()
			return fmt.Errorf("stat opened source %s: %w", path, statErr)
		}
		if !openedInfo.Mode().IsRegular() || !os.SameFile(info, openedInfo) {
			_ = f.Close()
			return fmt.Errorf("source file %q changed identity during export", path)
		}

		_, copyErr := io.Copy(tw, f)
		closeErr := f.Close()
		if copyErr != nil || closeErr != nil {
			var fileErr error
			if copyErr != nil {
				fileErr = errors.Join(fileErr, fmt.Errorf("copy %s: %w", path, copyErr))
			}
			if closeErr != nil {
				fileErr = errors.Join(fileErr, fmt.Errorf("close %s: %w", path, closeErr))
			}
			return fileErr
		}
		return nil
	})
}
