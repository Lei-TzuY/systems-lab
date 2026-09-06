//go:build linux

package cgroups

import (
	"fmt"
	"os"
	"path/filepath"
)

// ApplyMiscLimit writes misc.max limits in Cgroup v2.
func ApplyMiscLimit(cgroupPath string, resource string, limit int64) error {
	if resource == "" {
		return nil
	}

	miscFile := filepath.Join(cgroupPath, "misc.max")
	rule := fmt.Sprintf("%s %d\n", resource, limit)
	return os.WriteFile(miscFile, []byte(rule), 0644)
}
