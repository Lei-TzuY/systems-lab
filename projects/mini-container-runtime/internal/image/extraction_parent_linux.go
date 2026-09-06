//go:build linux

package image

import (
	"errors"
	"fmt"
	"path/filepath"
	"strings"

	"golang.org/x/sys/unix"
)

type extractionRoot struct {
	fd  int
	abs string
}

func openExtractionRoot(destDir string) (*extractionRoot, error) {
	destAbs, err := filepath.Abs(destDir)
	if err != nil {
		return nil, fmt.Errorf("resolve extraction root: %w", err)
	}
	fd, err := unix.Open(destAbs, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
	if err != nil {
		return nil, fmt.Errorf("open extraction root %s: %w", destAbs, err)
	}
	return &extractionRoot{fd: fd, abs: destAbs}, nil
}

func (r *extractionRoot) Close() {
	if r != nil && r.fd >= 0 {
		_ = unix.Close(r.fd)
		r.fd = -1
	}
}

type extractionParent struct {
	fd    int
	leaf  string
	owned bool
}

func (p *extractionParent) Close() {
	if p != nil && p.owned && p.fd >= 0 {
		_ = unix.Close(p.fd)
		p.fd = -1
	}
}

func (r *extractionRoot) openParent(target, role string, create bool) (*extractionParent, error) {
	if r == nil || r.fd < 0 {
		return nil, fmt.Errorf("extraction root is closed")
	}
	targetAbs, err := filepath.Abs(target)
	if err != nil {
		return nil, fmt.Errorf("resolve %s target: %w", role, err)
	}
	rel, err := filepath.Rel(r.abs, targetAbs)
	if err != nil || rel == "." || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return nil, fmt.Errorf("path traversal detected: %q escapes %q", target, r.abs)
	}

	parts := strings.Split(rel, string(filepath.Separator))
	if len(parts) == 0 {
		return nil, fmt.Errorf("empty %s extraction path", role)
	}

	parentFD := r.fd
	owned := false
	closeOwned := func() {
		if owned {
			_ = unix.Close(parentFD)
		}
	}

	for _, part := range parts[:len(parts)-1] {
		if part == "" || part == "." || part == ".." {
			closeOwned()
			return nil, fmt.Errorf("invalid %s extraction path component %q", role, part)
		}
		fd, openErr := unix.Openat(parentFD, part, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
		if create && errors.Is(openErr, unix.ENOENT) {
			if mkdirErr := unix.Mkdirat(parentFD, part, 0o755); mkdirErr != nil && !errors.Is(mkdirErr, unix.EEXIST) {
				closeOwned()
				return nil, fmt.Errorf("mkdir extraction parent %q: %w", part, mkdirErr)
			}
			fd, openErr = unix.Openat(parentFD, part, unix.O_RDONLY|unix.O_DIRECTORY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
		}
		if openErr != nil {
			closeOwned()
			return nil, fmt.Errorf("open extraction parent %q without symlinks: %w", part, openErr)
		}
		if owned {
			_ = unix.Close(parentFD)
		}
		parentFD = fd
		owned = true
	}

	leaf := parts[len(parts)-1]
	if leaf == "" || leaf == "." || leaf == ".." {
		closeOwned()
		return nil, fmt.Errorf("invalid %s extraction leaf %q", role, leaf)
	}
	return &extractionParent{fd: parentFD, leaf: leaf, owned: owned}, nil
}
