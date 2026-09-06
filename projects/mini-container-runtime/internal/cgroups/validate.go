package cgroups

import (
	"fmt"
	"math"
)

const (
	maxCgroupNameLen = 255
	cpuPeriodUsec    = 100000
)

// validateCgroupName restricts runtime-managed cgroups to a single safe path
// component. The runtime only creates names such as "minicontainer-1234", so
// accepting path separators, whitespace, or control characters is unnecessary
// and would make filesystem operations vulnerable to path traversal.
func validateCgroupName(name string) error {
	if name == "" {
		return fmt.Errorf("cgroup name must not be empty")
	}
	if len(name) > maxCgroupNameLen {
		return fmt.Errorf("cgroup name exceeds %d bytes", maxCgroupNameLen)
	}

	for i := 0; i < len(name); i++ {
		c := name[i]
		valid := (c >= 'a' && c <= 'z') ||
			(c >= 'A' && c <= 'Z') ||
			(c >= '0' && c <= '9') ||
			c == '-' || c == '_' || c == '.'
		if !valid {
			return fmt.Errorf("cgroup name %q contains invalid character %q", name, c)
		}
	}
	if name == "." || name == ".." {
		return fmt.Errorf("invalid cgroup name %q", name)
	}
	return nil
}

func validateResourceValues(memoryMax, cpuWeight int64, cpus float64, pidsMax int64) error {
	if memoryMax < 0 {
		return fmt.Errorf("memory limit must not be negative: %d", memoryMax)
	}
	if cpuWeight < 0 || cpuWeight > 10000 {
		return fmt.Errorf("CPU weight must be 0 or in range 1..10000: %d", cpuWeight)
	}
	maxCPUs := float64(math.MaxInt64) / cpuPeriodUsec
	if math.IsNaN(cpus) || math.IsInf(cpus, 0) || cpus < 0 || cpus > maxCPUs {
		return fmt.Errorf("CPU quota must be finite and in range 0..%.0f: %v", maxCPUs, cpus)
	}
	if pidsMax < 0 {
		return fmt.Errorf("PID limit must not be negative: %d", pidsMax)
	}
	return nil
}
