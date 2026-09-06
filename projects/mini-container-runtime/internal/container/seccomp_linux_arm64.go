//go:build linux && arm64

package container

// AUDIT_ARCH_AARCH64 from linux/audit.h.
const auditArch uint32 = 0xc00000b7

// blockedSyscalls for AArch64 (arm64), using the generic syscall table.
var blockedSyscalls = []uint32{
	104, 294, 117, 142, 116, 105, 273, 106,
	170, 112, 404, 40, 39, 41, 224, 225, 89,
	217, 218, 219, 280, 241, 270, 271, 264, 262, 282, 97,
}
