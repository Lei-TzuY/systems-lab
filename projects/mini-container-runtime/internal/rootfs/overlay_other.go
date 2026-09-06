//go:build !linux

// internal/rootfs/overlay_other.go — build stub for non-Linux targets.
// OverlayFS is a Linux kernel feature; this file satisfies the compiler
// on Windows and macOS without using any Linux-specific types.

package rootfs

import "fmt"

// OverlayDirs is a placeholder on non-Linux builds.
type OverlayDirs struct {
	Lower  []string
	Upper  string
	Work   string
	Merged string
}

func PrepareOverlay(_, _ string) (*OverlayDirs, error) {
	return nil, fmt.Errorf("overlayfs requires Linux")
}

func PrepareOverlayMultiLayer(_ []string, _ string) (*OverlayDirs, error) {
	return nil, fmt.Errorf("overlayfs requires Linux")
}

func IsMounted() bool { return false }
