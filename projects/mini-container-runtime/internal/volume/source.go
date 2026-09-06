package volume

import (
	"path/filepath"
	"strings"
)

// ManagedNameFromDataPath recovers named-volume provenance from the exact
// managed data-path layout emitted by ResolveVolumePath. It is intentionally
// lexical: callers must still reopen and validate the named volume through a
// pinned descriptor before trusting the path as a mount source.
func ManagedNameFromDataPath(path string) (string, bool) {
	if path == "" || !filepath.IsAbs(path) {
		return "", false
	}
	root := filepath.Clean(DefaultVolumeDir())
	clean := filepath.Clean(path)
	rel, err := filepath.Rel(root, clean)
	if err != nil || rel == "." || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return "", false
	}
	parts := strings.Split(rel, string(filepath.Separator))
	if len(parts) != 2 || parts[1] != "_data" {
		return "", false
	}
	if err := ValidateVolumeName(parts[0]); err != nil {
		return "", false
	}
	return parts[0], true
}
