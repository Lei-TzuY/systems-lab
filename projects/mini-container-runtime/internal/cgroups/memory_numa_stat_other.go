//go:build !linux

package cgroups

// NUMANodeStat maps NUMA node IDs to byte counts.
type NUMANodeStat map[int]uint64

// MemoryNUMAStat holds parsed per-NUMA-node memory consumption metrics.
type MemoryNUMAStat struct {
	Anon   NUMANodeStat
	File   NUMANodeStat
	Kernel NUMANodeStat
}

// ReadMemoryNUMAStat is a non-Linux stub.
func ReadMemoryNUMAStat(cgroupPath string) (MemoryNUMAStat, error) {
	return MemoryNUMAStat{
		Anon:   make(NUMANodeStat),
		File:   make(NUMANodeStat),
		Kernel: make(NUMANodeStat),
	}, nil
}
