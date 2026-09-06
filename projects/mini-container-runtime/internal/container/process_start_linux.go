//go:build linux

package container

import (
	"fmt"
	"os"
	"os/exec"
	"strconv"
)

const pinnedRootFSEnvKey = "MINICONTAINER_ROOTFS_FD"

// startContainerProcess is the last parent-side admission gate before the
// kernel creates a child process. Managed runs revalidate the filesystem object
// pinned at CLI admission here so setup performed earlier in the attempt cannot
// widen the RootFS pathname TOCTOU window all the way to exec.Cmd.Start.
//
// For managed runs, the admitted RootFS is also opened as a directory and
// inherited by the child. The child receives /proc/self/fd/N as its rootfs
// argument, so pathname replacement after process creation cannot redirect
// child-side rootfs setup to a different filesystem object. The inherited FD is
// identified to the runtime bootstrap so it can be sealed close-on-exec before
// the final payload exec rather than leaking a host-side directory capability.
func startContainerProcess(cfg Config, cmd *exec.Cmd) error {
	if err := validateSecurityProcessPolicy(cfg); err != nil {
		return &runtimeSetupError{err: fmt.Errorf("validate security process policy: %w", err)}
	}
	if err := validateAdmittedRootFSIdentity(cfg); err != nil {
		return err
	}
	if cfg.RootFSIdentity == nil {
		return cmd.Start()
	}
	if cmd == nil {
		return &runtimeSetupError{err: fmt.Errorf("start container process: command is nil")}
	}

	rootfsHandle, err := os.Open(cfg.RootFS)
	if err != nil {
		return &runtimeSetupError{err: fmt.Errorf("pin admitted rootfs %q: %w", cfg.RootFS, err)}
	}
	defer rootfsHandle.Close()

	pinned, err := rootfsHandle.Stat()
	if err != nil {
		return &runtimeSetupError{err: fmt.Errorf("stat pinned admitted rootfs %q: %w", cfg.RootFS, err)}
	}
	if !pinned.IsDir() {
		return &runtimeSetupError{err: fmt.Errorf("pin admitted rootfs %q: no longer a directory", cfg.RootFS)}
	}
	if !os.SameFile(cfg.RootFSIdentity, pinned) {
		return &runtimeSetupError{err: fmt.Errorf("pin admitted rootfs %q: filesystem identity changed before process creation", cfg.RootFS)}
	}

	rootArgIndex := len(cmd.Args) - len(cfg.Command) - 1
	if rootArgIndex < 1 || rootArgIndex >= len(cmd.Args) || cmd.Args[rootArgIndex] != cfg.RootFS {
		return &runtimeSetupError{err: fmt.Errorf("pin admitted rootfs %q: could not locate child rootfs argument", cfg.RootFS)}
	}

	childFD := 3 + len(cmd.ExtraFiles)
	cmd.ExtraFiles = append(cmd.ExtraFiles, rootfsHandle)
	cmd.Args[rootArgIndex] = fmt.Sprintf("/proc/self/fd/%d", childFD)
	if cmd.Env == nil {
		cmd.Env = os.Environ()
	}
	cmd.Env = append(cmd.Env, pinnedRootFSEnvKey+"="+strconv.Itoa(childFD))
	return cmd.Start()
}
