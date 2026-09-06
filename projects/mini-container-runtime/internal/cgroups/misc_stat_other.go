//go:build !linux

package cgroups

// MiscResourceUsage represents a single hardware/misc resource allocation.
type MiscResourceUsage struct {
	ResourceName string
	Usage        uint64
}

// ReadMiscCurrent is a non-Linux stub.
func ReadMiscCurrent(cgroupPath string) ([]MiscResourceUsage, error) {
	return nil, nil
}
