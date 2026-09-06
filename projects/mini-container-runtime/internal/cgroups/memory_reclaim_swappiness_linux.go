//go:build linux

package cgroups

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

// MemoryReclaimOptions holds advanced arguments for Cgroup v2 memory.reclaim.
type MemoryReclaimOptions struct {
	BytesToReclaim int64 // bytes to reclaim
	Swappiness     int   // -1 to ignore/default, 0-200 to enforce swap behavior (Linux 6.8+)
	NumaNode       int   // -1 to ignore, >=0 to specify NUMA node (Linux 6.8+)
}

// ReclaimMemoryWithOptions triggers Cgroup v2 memory.reclaim with optional swappiness and NUMA target.
func ReclaimMemoryWithOptions(cgroupPath string, opts MemoryReclaimOptions) error {
	if cgroupPath == "" {
		return errors.New("cgroup path is empty")
	}

	if opts.Swappiness < -1 || opts.Swappiness > 200 {
		return fmt.Errorf("invalid swappiness %d: must be -1 (default) or 0-200", opts.Swappiness)
	}
	if opts.NumaNode < -1 {
		return fmt.Errorf("invalid numa node %d: must be >= 0 or -1 (default)", opts.NumaNode)
	}

	if opts.BytesToReclaim <= 0 {
		opts.BytesToReclaim = 1048576 // 1MB default
	}

	reclaimFile := filepath.Join(cgroupPath, "memory.reclaim")
	if _, err := os.Stat(reclaimFile); err != nil {
		if os.IsNotExist(err) {
			return fmt.Errorf("%w: %s", ErrMemoryReclaimUnavailable, reclaimFile)
		}
		return fmt.Errorf("stat memory.reclaim: %w", err)
	}

	var parts []string
	parts = append(parts, fmt.Sprintf("%d", opts.BytesToReclaim))

	if opts.Swappiness >= 0 && opts.Swappiness <= 200 {
		parts = append(parts, fmt.Sprintf("swappiness=%d", opts.Swappiness))
	}
	if opts.NumaNode >= 0 {
		parts = append(parts, fmt.Sprintf("node=%d", opts.NumaNode))
	}

	cmd := strings.Join(parts, " ") + "\n"
	if err := os.WriteFile(reclaimFile, []byte(cmd), 0644); err != nil {
		return fmt.Errorf("write memory.reclaim: %w", err)
	}
	return nil
}
