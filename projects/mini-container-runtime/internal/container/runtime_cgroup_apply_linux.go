//go:build linux

package container

import (
	"fmt"

	"minicontainer/internal/cgroups"
	"minicontainer/internal/state"
)

type cgroupApplyFunc func(pid int, cfg cgroups.Config, debug bool) error

// applyCgroupWithDurableOwnership closes the crash window between host cgroup
// mutation and persistence of the generation-scoped cleanup token. Managed
// runtimes durably reserve the exact cgroup name before Apply is allowed to
// mutate /sys/fs/cgroup. A failed or partial Apply intentionally leaves that
// token in place so stopped-generation recovery can remove any debris.
func applyCgroupWithDurableOwnership(
	st *state.Store,
	containerID string,
	pid int,
	pidStartTime uint64,
	cfg cgroups.Config,
	debug bool,
	apply cgroupApplyFunc,
) (bool, error) {
	if apply == nil {
		return false, &runtimeSetupError{err: fmt.Errorf("cgroup apply operation is nil")}
	}
	if st != nil {
		if err := st.MarkCgroupOwnedIfIdentity(containerID, pid, pidStartTime, cfg.Name); err != nil {
			return false, &runtimeStateError{err: fmt.Errorf("persist cgroup ownership before apply for container %s: %w", containerID, err)}
		}
	}
	if err := apply(pid, cfg, debug); err != nil {
		return false, err
	}
	return true, nil
}
