//go:build linux

package cgroups

import (
	"fmt"
	"os"
	"path/filepath"
)

// ApplyHugeTLBLimit configures Cgroup v2 hugetlb limits (e.g. 2MB, 1GB).
func ApplyHugeTLBLimit(cgroupPath string, pageSize string, limitBytes int64) error {
	if pageSize == "" {
		pageSize = "2MB"
	}

	limitFile := filepath.Join(cgroupPath, fmt.Sprintf("hugetlb.%s.max", pageSize))
	return os.WriteFile(limitFile, []byte(fmt.Sprintf("%d\n", limitBytes)), 0644)
}
