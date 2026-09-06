//go:build linux

package cgroups

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
)

// ReadPSIStats reads and parses one cgroup v2 pressure file while preserving
// both the "some" and optional "full" scopes.
func ReadPSIStats(cgroupPath, resource string) (*PSIStats, error) {
	if err := validatePSIResource(resource); err != nil {
		return nil, err
	}

	psiFile := filepath.Join(cgroupPath, resource+".pressure")
	content, err := os.ReadFile(psiFile)
	if err != nil {
		return nil, fmt.Errorf("read %s.pressure: %w", resource, err)
	}

	stats, err := parsePSI(content)
	if err != nil {
		return nil, fmt.Errorf("parse %s.pressure: %w", resource, err)
	}
	return stats, nil
}

// ReadPSI is the legacy convenience API that returns the "some" scope.
// New callers that need full-stall information should use ReadPSIStats.
func ReadPSI(cgroupPath, resource string) (*PSIValues, error) {
	stats, err := ReadPSIStats(cgroupPath, resource)
	if err != nil {
		return nil, err
	}
	values := stats.Some
	return &values, nil
}

func readPressureStallTotal(cgroupPath, resource string) (uint64, error) {
	stats, err := ReadPSIStats(cgroupPath, resource)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return 0, nil
		}
		return 0, err
	}
	return stats.Some.Total, nil
}
