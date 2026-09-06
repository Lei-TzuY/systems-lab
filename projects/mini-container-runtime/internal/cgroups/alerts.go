package cgroups

// CheckMemoryAlert evaluates whether container memory usage has exceeded soft limit.
func CheckMemoryAlert(usageBytes, softLimitBytes int64) bool {
	if softLimitBytes <= 0 {
		return false
	}
	return usageBytes >= softLimitBytes
}
