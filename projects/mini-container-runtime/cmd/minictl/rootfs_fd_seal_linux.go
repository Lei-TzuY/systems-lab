//go:build linux

package main

import (
	"fmt"
	"os"
	"strconv"
	"syscall"
)

const pinnedRootFSEnvKey = "MINICONTAINER_ROOTFS_FD"

func init() {
	if os.Getenv("MINICONTAINER_INIT") != "1" {
		return
	}
	if err := sealInheritedFDsForPayload(); err != nil {
		fmt.Fprintf(os.Stderr, "container init: seal inherited fds: %v\n", err)
		os.Exit(1)
	}
	rawFD, ok := os.LookupEnv(pinnedRootFSEnvKey)
	if !ok {
		return
	}
	if err := sealPinnedRootFSFDForPayload(rawFD); err != nil {
		fmt.Fprintf(os.Stderr, "container init: seal pinned rootfs fd: %v\n", err)
		os.Exit(1)
	}
	if err := os.Unsetenv(pinnedRootFSEnvKey); err != nil {
		fmt.Fprintf(os.Stderr, "container init: clear pinned rootfs fd environment: %v\n", err)
		os.Exit(1)
	}
}

func sealPinnedRootFSFDForPayload(rawFD string) error {
	fd, err := strconv.Atoi(rawFD)
	if err != nil || fd < 3 {
		return fmt.Errorf("invalid inherited fd %q", rawFD)
	}

	var st syscall.Stat_t
	if err := syscall.Fstat(fd, &st); err != nil {
		return fmt.Errorf("fstat inherited fd %d: %w", fd, err)
	}
	if st.Mode&syscall.S_IFMT != syscall.S_IFDIR {
		return fmt.Errorf("inherited fd %d is not a directory", fd)
	}

	// The runtime init still needs the descriptor while it performs mounts and
	// pivot_root through /proc/self/fd/N. CloseOnExec preserves that setup-time
	// capability but guarantees the final payload exec cannot inherit it.
	syscall.CloseOnExec(fd)
	return nil
}
