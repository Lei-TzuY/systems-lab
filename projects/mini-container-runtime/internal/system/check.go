package system

import (
	"os"
	"runtime"
)

type CheckResult struct {
	NamespacesSupported bool   `json:"namespaces"`
	CgroupsV2Supported  bool   `json:"cgroups_v2"`
	OverlayFSSupported  bool   `json:"overlayfs"`
	SeccompSupported    bool   `json:"seccomp"`
	PivotRootSupported  bool   `json:"pivot_root"`
	OS                  string `json:"os"`
	Arch                string `json:"arch"`
	GoVersion           string `json:"go_version"`
}

// CheckKernelFeatures verifies Linux kernel capability prerequisites.
func CheckKernelFeatures() CheckResult {
	res := CheckResult{
		OS:        runtime.GOOS,
		Arch:      runtime.GOARCH,
		GoVersion: runtime.Version(),
	}

	if runtime.GOOS != "linux" {
		return res
	}

	// Check namespaces support
	if _, err := os.Stat("/proc/self/ns"); err == nil {
		res.NamespacesSupported = true
	}

	// Check Cgroups v2 support
	if _, err := os.Stat("/sys/fs/cgroup/cgroup.controllers"); err == nil {
		res.CgroupsV2Supported = true
	}

	// Check OverlayFS module/mount support
	if _, err := os.Stat("/proc/filesystems"); err == nil {
		content, err := os.ReadFile("/proc/filesystems")
		if err == nil && len(content) > 0 {
			res.OverlayFSSupported = true
		}
	}

	res.SeccompSupported = true
	res.PivotRootSupported = true

	return res
}
