//go:build !linux

package cgroups

type IOLimits struct {
	ReadBPS   int64
	WriteBPS  int64
	ReadIOPS  int64
	WriteIOPS int64
	Device    string
}

func ApplyIOMax(cgroupPath string, limits IOLimits) error {
	return nil
}
