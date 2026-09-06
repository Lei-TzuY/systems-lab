//go:build linux

package cgroups

import (
	"os"
	"path/filepath"
)

// ReadMemoryPSI reads Cgroup v2 memory.pressure metrics.
func ReadMemoryPSI(cgroupPath string) (string, error) {
	psiFile := filepath.Join(cgroupPath, "memory.pressure")
	content, err := os.ReadFile(psiFile)
	if err != nil {
		return "", err
	}
	return string(content), nil
}
