//go:build linux

package cgroups

import (
	"bufio"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// NUMANodeStat maps NUMA node IDs (e.g. 0, 1) to byte counts.
type NUMANodeStat map[int]uint64

// MemoryNUMAStat holds parsed per-NUMA-node memory consumption metrics.
type MemoryNUMAStat struct {
	Anon NUMANodeStat
	File NUMANodeStat
	Kernel NUMANodeStat
}

// ReadMemoryNUMAStat parses the memory.numa_stat interface file.
func ReadMemoryNUMAStat(cgroupPath string) (MemoryNUMAStat, error) {
	stat := MemoryNUMAStat{
		Anon:   make(NUMANodeStat),
		File:   make(NUMANodeStat),
		Kernel: make(NUMANodeStat),
	}

	data, err := os.Open(filepath.Join(cgroupPath, "memory.numa_stat"))
	if err != nil {
		if os.IsNotExist(err) {
			return stat, nil
		}
		return stat, err
	}
	defer data.Close()

	scanner := bufio.NewScanner(data)
	for scanner.Scan() {
		fields := strings.Fields(scanner.Text())
		if len(fields) < 2 {
			continue
		}

		key := fields[0]
		var target NUMANodeStat
		switch key {
		case "anon":
			target = stat.Anon
		case "file":
			target = stat.File
		case "kernel_stack", "pagetables", "percpu", "sock", "vmalloc", "shmem":
			target = stat.Kernel
		default:
			continue
		}

		for _, nodeField := range fields[1:] {
			parts := strings.SplitN(nodeField, "=", 2)
			if len(parts) != 2 || !strings.HasPrefix(parts[0], "N") {
				continue
			}
			nodeID, err := strconv.Atoi(strings.TrimPrefix(parts[0], "N"))
			if err != nil {
				continue
			}
			bytesVal, err := strconv.ParseUint(parts[1], 10, 64)
			if err != nil {
				continue
			}
			target[nodeID] += bytesVal
		}
	}

	return stat, nil
}
