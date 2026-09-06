//go:build !linux

package cgroups

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

// ReadMemoryStatBreakdown is a non-Linux stub.
func ReadMemoryStatBreakdown(cgroupPath string) (DetailedMemoryStatBreakdown, error) {
	return DetailedMemoryStatBreakdown{}, nil
}
