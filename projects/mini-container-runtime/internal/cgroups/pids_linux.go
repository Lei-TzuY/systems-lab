//go:build linux

package cgroups

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// ApplyPIDsLimit sets pids.max limit in Cgroup v2.
func ApplyPIDsLimit(cgroupPath string, maxPIDs int64) error {
	pidsFile := filepath.Join(cgroupPath, "pids.max")
	val := "max"
	if maxPIDs > 0 {
		val = fmt.Sprintf("%d", maxPIDs)
	}
	return os.WriteFile(pidsFile, []byte(val+"\n"), 0644)
}

// ReadPIDsCurrent reads current process count in Cgroup v2.
func ReadPIDsCurrent(cgroupPath string) (int64, error) {
	curFile := filepath.Join(cgroupPath, "pids.current")
	content, err := os.ReadFile(curFile)
	if err != nil {
		return 0, fmt.Errorf("read pids.current: %w", err)
	}
	val, err := strconv.ParseInt(strings.TrimSpace(string(content)), 10, 64)
	if err != nil {
		return 0, fmt.Errorf("parse pids.current: %w", err)
	}
	return val, nil
}
