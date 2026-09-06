//go:build linux

package container

import (
	"fmt"

	"golang.org/x/sys/unix"
)

type unshareCall func(flags int) error

type execCgroupAdmissionFunc func() error

func prepareExecThread() error {
	return prepareExecThreadWithAdmission(unix.Unshare, attachExecInitToPersistedCgroup)
}

func prepareExecThreadWith(unshare unshareCall) error {
	return prepareExecThreadWithAdmission(unshare, func() error { return nil })
}

func prepareExecThreadWithAdmission(unshare unshareCall, admit execCgroupAdmissionFunc) error {
	if unshare == nil {
		return fmt.Errorf("exec thread unshare function is nil")
	}
	if admit == nil {
		return fmt.Errorf("exec cgroup admission function is nil")
	}
	if err := unshare(unix.CLONE_FS); err != nil {
		return fmt.Errorf("unshare CLONE_FS before namespace entry: %w", err)
	}
	// Join the exact managed cgroup while the helper still has host cgroupfs
	// access. The subsequently spawned payload inherits this resource domain.
	if err := admit(); err != nil {
		return fmt.Errorf("admit exec-init to container cgroup: %w", err)
	}
	// ExecInit has consumed every runtime-owned handoff value before reaching
	// this point. Drop the entire reserved control namespace now so future
	// MINICONTAINER_* bootstrap keys cannot silently cross into exec payloads.
	// Explicit workload environment is not sourced from this ambient namespace.
	if err := clearRuntimeControlEnvironment(); err != nil {
		return fmt.Errorf("isolate exec runtime control environment: %w", err)
	}
	return nil
}
