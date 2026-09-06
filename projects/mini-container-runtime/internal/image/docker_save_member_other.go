//go:build !linux

package image

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

// openDockerSaveMember is the portable fallback for platforms without Linux
// dirfd pinning. It rejects every symlink component before opening the final
// regular file. Linux uses the descriptor-bound implementation instead.
func openDockerSaveMember(root, member string) (*os.File, error) {
	memberPath, err := safePath(root, member)
	if err != nil {
		return nil, err
	}
	rootAbs, err := filepath.Abs(root)
	if err != nil {
		return nil, fmt.Errorf("resolve docker-save root %q: %w", root, err)
	}
	memberAbs, err := filepath.Abs(memberPath)
	if err != nil {
		return nil, fmt.Errorf("resolve docker-save member %q: %w", member, err)
	}
	rel, err := filepath.Rel(filepath.Clean(rootAbs), filepath.Clean(memberAbs))
	if err != nil || rel == "." || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return nil, fmt.Errorf("docker-save member %q escapes extraction root", member)
	}

	current := filepath.Clean(rootAbs)
	parts := strings.Split(rel, string(filepath.Separator))
	for i, part := range parts {
		if part == "" || part == "." || part == ".." {
			return nil, fmt.Errorf("invalid docker-save member component %q in %q", part, member)
		}
		current = filepath.Join(current, part)
		info, err := os.Lstat(current)
		if err != nil {
			return nil, fmt.Errorf("inspect docker-save member component %q: %w", current, err)
		}
		if info.Mode()&os.ModeSymlink != 0 {
			return nil, fmt.Errorf("docker-save member %q traverses symlink %q", member, current)
		}
		if i < len(parts)-1 && !info.IsDir() {
			return nil, fmt.Errorf("docker-save member component %q is not a directory", current)
		}
		if i == len(parts)-1 && !info.Mode().IsRegular() {
			return nil, fmt.Errorf("docker-save member %q is not a regular file", member)
		}
	}
	return os.Open(memberAbs)
}
