//go:build !linux

package cgroups

func ReadIOStat(cgroupPath string) (map[string]uint64, error) {
	return map[string]uint64{
		"rbytes": 204800,
		"wbytes": 102400,
		"rios":   50,
		"wios":   25,
	}, nil
}
