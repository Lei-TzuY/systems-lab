package builder

import (
	"fmt"
	"os"
	"path"
	"path/filepath"
	"strings"
)

const maxBuildSymlinks = 255

func normalizeContainerPath(workDir, requested string) (string, error) {
	requested = strings.TrimSpace(filepath.ToSlash(requested))
	if requested == "" {
		return "", fmt.Errorf("container path cannot be empty")
	}
	base := filepath.ToSlash(workDir)
	if base == "" || !path.IsAbs(base) {
		base = "/"
	}
	var logical string
	if path.IsAbs(requested) {
		logical = path.Clean(requested)
	} else {
		logical = path.Clean(path.Join(base, requested))
	}
	if logical == "." {
		logical = "/"
	}
	if !path.IsAbs(logical) {
		logical = "/" + logical
	}
	return logical, nil
}

func canonicalBuildRoot(root string) (string, error) {
	abs, err := filepath.Abs(root)
	if err != nil {
		return "", fmt.Errorf("absolute build root: %w", err)
	}
	resolved, err := filepath.EvalSymlinks(abs)
	if err != nil {
		return "", fmt.Errorf("resolve build root: %w", err)
	}
	info, err := os.Stat(resolved)
	if err != nil {
		return "", fmt.Errorf("stat build root: %w", err)
	}
	if !info.IsDir() {
		return "", fmt.Errorf("build root %q is not a directory", root)
	}
	return resolved, nil
}

func splitLogicalComponents(value string) []string {
	value = filepath.ToSlash(value)
	parts := strings.Split(value, "/")
	out := make([]string, 0, len(parts))
	for _, part := range parts {
		if part != "" && part != "." {
			out = append(out, part)
		}
	}
	return out
}

func resolveRootFSPath(root, logical string) (string, error) {
	root, err := canonicalBuildRoot(root)
	if err != nil {
		return "", err
	}
	logical, err = normalizeContainerPath("/", logical)
	if err != nil {
		return "", err
	}

	queue := splitLogicalComponents(logical)
	resolved := make([]string, 0, len(queue))
	symlinks := 0
	for len(queue) > 0 {
		component := queue[0]
		queue = queue[1:]
		switch component {
		case "", ".":
			continue
		case "..":
			if len(resolved) > 0 {
				resolved = resolved[:len(resolved)-1]
			}
			continue
		}

		candidateParts := append(append([]string{}, resolved...), filepath.FromSlash(component))
		candidate := filepath.Join(append([]string{root}, candidateParts...)...)
		info, statErr := os.Lstat(candidate)
		if statErr != nil {
			if os.IsNotExist(statErr) {
				resolved = append(resolved, component)
				continue
			}
			return "", fmt.Errorf("inspect rootfs path %q: %w", candidate, statErr)
		}
		if info.Mode()&os.ModeSymlink == 0 {
			resolved = append(resolved, component)
			continue
		}

		symlinks++
		if symlinks > maxBuildSymlinks {
			return "", fmt.Errorf("too many symlinks while resolving container path %q", logical)
		}
		target, err := os.Readlink(candidate)
		if err != nil {
			return "", fmt.Errorf("read rootfs symlink %q: %w", candidate, err)
		}
		target = filepath.ToSlash(target)
		if path.IsAbs(target) {
			resolved = resolved[:0]
		}
		queue = append(splitLogicalComponents(target), queue...)
	}

	parts := []string{root}
	for _, component := range resolved {
		parts = append(parts, filepath.FromSlash(component))
	}
	result := filepath.Join(parts...)
	rel, err := filepath.Rel(root, result)
	if err != nil {
		return "", fmt.Errorf("relativize resolved rootfs path: %w", err)
	}
	if rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return "", fmt.Errorf("resolved container path escaped build root")
	}
	return result, nil
}

func resolveRootFSLeaf(root, logical string) (string, error) {
	logical, err := normalizeContainerPath("/", logical)
	if err != nil {
		return "", err
	}
	if logical == "/" {
		return "", fmt.Errorf("cannot replace build root")
	}
	parent, err := resolveRootFSPath(root, path.Dir(logical))
	if err != nil {
		return "", err
	}
	return filepath.Join(parent, filepath.FromSlash(path.Base(logical))), nil
}

func mkdirRootFSPath(root, logical string, mode os.FileMode) error {
	target, err := resolveRootFSPath(root, logical)
	if err != nil {
		return err
	}
	if err := os.MkdirAll(target, mode); err != nil {
		return fmt.Errorf("create rootfs path %q: %w", logical, err)
	}
	return nil
}

func resolveBuildContextSource(contextDir, source string) (string, error) {
	root, err := canonicalBuildRoot(contextDir)
	if err != nil {
		return "", err
	}
	source = filepath.FromSlash(strings.TrimSpace(filepath.ToSlash(source)))
	if source == "" {
		return "", fmt.Errorf("COPY source cannot be empty")
	}
	if filepath.IsAbs(source) {
		return "", fmt.Errorf("COPY source %q must be relative to build context", source)
	}
	clean := filepath.Clean(source)
	if clean == ".." || strings.HasPrefix(clean, ".."+string(filepath.Separator)) {
		return "", fmt.Errorf("COPY source %q escapes build context", source)
	}
	candidate := filepath.Join(root, clean)
	rel, err := filepath.Rel(root, candidate)
	if err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return "", fmt.Errorf("COPY source %q escapes build context", source)
	}
	current := root
	for _, component := range strings.Split(rel, string(filepath.Separator)) {
		if component == "" || component == "." {
			continue
		}
		current = filepath.Join(current, component)
		info, err := os.Lstat(current)
		if err != nil {
			return "", fmt.Errorf("inspect COPY source %q: %w", source, err)
		}
		if info.Mode()&os.ModeSymlink != 0 {
			return "", fmt.Errorf("COPY source %q traverses symlink %q", source, current)
		}
	}
	return candidate, nil
}
