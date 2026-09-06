//go:build linux

package cgroups

import (
	"bufio"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// DetailedMemoryStatBreakdown holds comprehensive parsed memory stats from memory.stat.
type DetailedMemoryStatBreakdown struct {
	Anon              uint64
	File              uint64
	KernelStack       uint64
	Pagetables        uint64
	SecPagetables     uint64
	PerCPU            uint64
	Sock              uint64
	VMAlloc           uint64
	Shmem             uint64
	Zswap             uint64
	Zswapped          uint64
	FileMapped        uint64
	FileDirty         uint64
	FileWriteback     uint64
	Slab              uint64
	SlabReclaimable   uint64
	SlabUnreclaimable uint64
	PGFault           uint64
	PGMajFault        uint64
	KernelTotal       uint64
	UserTotal         uint64
	SlabReclaimRatio  float64
}

// ReadMemoryStatBreakdown parses the memory.stat cgroup interface file.
func ReadMemoryStatBreakdown(cgroupPath string) (DetailedMemoryStatBreakdown, error) {
	var b DetailedMemoryStatBreakdown
	file, err := os.Open(filepath.Join(cgroupPath, "memory.stat"))
	if err != nil {
		if os.IsNotExist(err) {
			return b, nil
		}
		return b, err
	}
	defer file.Close()

	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		fields := strings.Fields(scanner.Text())
		if len(fields) != 2 {
			continue
		}

		val, _ := strconv.ParseUint(fields[1], 10, 64)
		switch fields[0] {
		case "anon":
			b.Anon = val
		case "file":
			b.File = val
		case "kernel_stack":
			b.KernelStack = val
		case "pagetables":
			b.Pagetables = val
		case "sec_pagetables":
			b.SecPagetables = val
		case "percpu":
			b.PerCPU = val
		case "sock":
			b.Sock = val
		case "vmalloc":
			b.VMAlloc = val
		case "shmem":
			b.Shmem = val
		case "zswap":
			b.Zswap = val
		case "zswapped":
			b.Zswapped = val
		case "file_mapped":
			b.FileMapped = val
		case "file_dirty":
			b.FileDirty = val
		case "file_writeback":
			b.FileWriteback = val
		case "slab":
			b.Slab = val
		case "slab_reclaimable":
			b.SlabReclaimable = val
		case "slab_unreclaimable":
			b.SlabUnreclaimable = val
		case "pgfault":
			b.PGFault = val
		case "pgmajfault":
			b.PGMajFault = val
		}
	}

	b.UserTotal = b.Anon + b.File + b.Shmem
	b.KernelTotal = b.KernelStack + b.Pagetables + b.PerCPU + b.Sock + b.VMAlloc + b.Slab
	if b.Slab > 0 {
		b.SlabReclaimRatio = float64(b.SlabReclaimable) / float64(b.Slab)
	}

	return b, nil
}
