//go:build !linux

// internal/rootfs/rootfs_other.go
// Non-Linux build stub for rootfs isolation.

package rootfs

import "fmt"

func Isolate(newRoot string, debug bool) error {
	return fmt.Errorf("filesystem isolation (pivot_root/chroot) requires Linux")
}
