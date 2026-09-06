//go:build linux && !amd64 && !arm64

package container

// Unknown audit architecture: applySeccomp rejects enabling the filter rather
// than installing an allow-all or ABI-ambiguous policy.
const auditArch uint32 = 0

var blockedSyscalls = []uint32{}
