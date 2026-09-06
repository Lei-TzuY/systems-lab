//go:build !linux

package cgroups

// MiscEventMax represents a max capacity limit failure event for a misc hardware resource.
type MiscEventMax struct {
	ResourceName string
	MaxFails     uint64
}

// ReadMiscEventsMax is a non-Linux stub.
func ReadMiscEventsMax(cgroupPath string) ([]MiscEventMax, error) {
	return nil, nil
}
