//go:build !linux

package cgroups

func ReadPSIStats(cgroupPath, resource string) (*PSIStats, error) {
	if err := validatePSIResource(resource); err != nil {
		return nil, err
	}
	return &PSIStats{}, nil
}

func ReadPSI(cgroupPath, resource string) (*PSIValues, error) {
	if err := validatePSIResource(resource); err != nil {
		return nil, err
	}
	return &PSIValues{}, nil
}
