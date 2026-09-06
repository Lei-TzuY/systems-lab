package container

import (
	"fmt"

	"minicontainer/internal/cgroups"
	"minicontainer/internal/state"
)

func openRunningCgroup(st *state.Store, containerID string) (*state.Container, *ProcessHandle, string, error) {
	c, handle, err := openRunningProcess(st, containerID)
	if err != nil {
		return nil, nil, "", err
	}

	name, err := cgroups.NameForContainerProcess(c.ID, c.PID, c.PIDStartTime)
	if err != nil {
		handle.Close()
		return nil, nil, "", fmt.Errorf("derive cgroup identity: %w", err)
	}
	return c, handle, name, nil
}

func controlRunningCgroup(
	st *state.Store,
	containerID, operation string,
	control func(string) error,
) (*state.Container, error) {
	if control == nil {
		return nil, fmt.Errorf("%s container cgroup control is nil", operation)
	}

	c, handle, cgroupName, err := openRunningCgroup(st, containerID)
	if err != nil {
		return nil, err
	}
	defer handle.Close()

	if err := requireCgroupControlGenerationAlive(handle, c, operation, "before"); err != nil {
		return nil, err
	}
	if err := control(cgroupName); err != nil {
		return nil, fmt.Errorf("%s container cgroup: %w", operation, err)
	}
	if err := requireCgroupControlGenerationAlive(handle, c, operation, "after"); err != nil {
		return nil, err
	}
	return c, nil
}

func requireCgroupControlGenerationAlive(
	handle *ProcessHandle,
	c *state.Container,
	operation, phase string,
) error {
	exited, err := handle.WaitExit(0)
	if err != nil {
		return fmt.Errorf("verify container process generation %s %s cgroup control: %w", phase, operation, err)
	}
	if exited {
		return fmt.Errorf(
			"container process generation %d/%d exited %s %s cgroup control: %w",
			c.PID,
			c.PIDStartTime,
			phase,
			operation,
			ErrProcessNotFound,
		)
	}
	return nil
}

// FreezeContainer pauses the exact running process generation stored for a
// container. The generation-derived cgroup name prevents a concurrent restart
// or PID reuse from redirecting the freeze operation to another process.
func FreezeContainer(st *state.Store, containerID string) error {
	_, err := FreezeContainerResolved(st, containerID)
	return err
}

// FreezeContainerResolved pauses the exact persisted running generation and
// returns the canonical snapshot that remained alive across the kernel cgroup
// control operation.
func FreezeContainerResolved(st *state.Store, containerID string) (*state.Container, error) {
	return controlRunningCgroup(st, containerID, "freeze", cgroups.Freeze)
}

// ThawContainer resumes the exact running process generation stored for a
// container.
func ThawContainer(st *state.Store, containerID string) error {
	_, err := ThawContainerResolved(st, containerID)
	return err
}

// ThawContainerResolved resumes the exact persisted running generation and
// returns the canonical snapshot that remained alive across the kernel cgroup
// control operation.
func ThawContainerResolved(st *state.Store, containerID string) (*state.Container, error) {
	return controlRunningCgroup(st, containerID, "unfreeze", cgroups.Unfreeze)
}

// UpdateContainerResources applies resource changes only to the cgroup for the
// currently persisted process generation.
func UpdateContainerResources(st *state.Store, containerID string, cfg cgroups.UpdateConfig, debug bool) error {
	_, err := UpdateContainerResourcesResolved(st, containerID, cfg, debug)
	return err
}

// UpdateContainerResourcesResolved applies resource changes only while the
// exact persisted process generation remains alive and returns that canonical
// snapshot on success. The state generation lock serializes the multi-file
// cgroup transaction against concurrent updates and lifecycle transitions.
func UpdateContainerResourcesResolved(
	st *state.Store,
	containerID string,
	cfg cgroups.UpdateConfig,
	debug bool,
) (*state.Container, error) {
	c, handle, cgroupName, err := openRunningCgroup(st, containerID)
	if err != nil {
		return nil, err
	}
	defer handle.Close()

	err = st.WithRunningGenerationLocked(c.ID, c.PID, c.PIDStartTime, func() error {
		if err := requireCgroupControlGenerationAlive(handle, c, "update", "before"); err != nil {
			return err
		}
		if err := cgroups.UpdateLimits(cgroupName, cfg, debug); err != nil {
			return fmt.Errorf("update container cgroup: %w", err)
		}
		return requireCgroupControlGenerationAlive(handle, c, "update", "after")
	})
	if err != nil {
		return nil, err
	}
	return c, nil
}
