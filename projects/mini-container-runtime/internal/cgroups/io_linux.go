//go:build linux

package cgroups

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

type IOLimits struct {
	ReadBPS  int64
	WriteBPS int64
	ReadIOPS int64
	WriteIOPS int64
	Device   string // e.g. "8:0"
}

// ApplyIOMax writes io.max limits for Cgroup v2.
func ApplyIOMax(cgroupPath string, limits IOLimits) error {
	if limits.Device == "" {
		limits.Device = "8:0" // Default major:minor for primary block device
	}

	ioFile := filepath.Join(cgroupPath, "io.max")
	var parts []string
	parts = append(parts, limits.Device)

	if limits.ReadBPS > 0 {
		parts = append(parts, fmt.Sprintf("rbps=%d", limits.ReadBPS))
	}
	if limits.WriteBPS > 0 {
		parts = append(parts, fmt.Sprintf("wbps=%d", limits.WriteBPS))
	}
	if limits.ReadIOPS > 0 {
		parts = append(parts, fmt.Sprintf("riops=%d", limits.ReadIOPS))
	}
	if limits.WriteIOPS > 0 {
		parts = append(parts, fmt.Sprintf("wiops=%d", limits.WriteIOPS))
	}

	if len(parts) <= 1 {
		return nil
	}

	rule := strings.Join(parts, " ")
	return os.WriteFile(ioFile, []byte(rule+"\n"), 0644)
}
