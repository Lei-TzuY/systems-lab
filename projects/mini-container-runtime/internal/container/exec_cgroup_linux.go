//go:build linux

package container

import (
	"fmt"
	"os"
	"strconv"

	"minicontainer/internal/cgroups"
	"minicontainer/internal/state"
)

type execCgroupAttachFunc func(pid int, name string, debug bool) error

func attachExecInitToPersistedCgroup() error {
	return attachExecInitToPersistedCgroupWith(
		state.DefaultDir(),
		os.Args,
		os.Getenv(execStartTimeKey),
		os.Getpid(),
		cgroups.AttachExisting,
	)
}

func attachExecInitToPersistedCgroupWith(storeDir string, args []string, expectedRaw string, selfPID int, attach execCgroupAttachFunc) error {
	if attach == nil {
		return fmt.Errorf("exec cgroup attach function is nil")
	}
	if len(args) < 4 || args[1] != "exec" {
		return fmt.Errorf("invalid internal exec arguments for cgroup admission")
	}
	containerPID, err := strconv.Atoi(args[2])
	if err != nil || containerPID <= 0 {
		return fmt.Errorf("invalid internal exec container PID %q", args[2])
	}
	expectedStartTime, err := strconv.ParseUint(expectedRaw, 10, 64)
	if err != nil || expectedStartTime == 0 {
		return fmt.Errorf("invalid internal exec target identity %q", expectedRaw)
	}

	store, err := state.Open(storeDir)
	if err != nil {
		return fmt.Errorf("open container state for exec cgroup admission: %w", err)
	}
	defer store.Close()
	records, err := store.List()
	if err != nil {
		return fmt.Errorf("list container state for exec cgroup admission: %w", err)
	}
	var match *state.Container
	for _, rec := range records {
		if rec.PID != containerPID || rec.RootFS != args[3] || rec.Status != state.StatusRunning || rec.PIDStartTime != expectedStartTime {
			continue
		}
		if match != nil {
			return fmt.Errorf("ambiguous persisted exec cgroup identity for PID %d", containerPID)
		}
		match = rec
	}
	if match == nil {
		return fmt.Errorf("no running container state matches exec cgroup identity %d/%d", containerPID, expectedStartTime)
	}
	ownership, ok, err := store.GetCgroupOwnership(match.ID)
	if err != nil {
		return fmt.Errorf("read exec cgroup ownership for container %s: %w", match.ID, err)
	}
	if !ok {
		return nil
	}
	if ownership.PID != containerPID || ownership.PIDStartTime != expectedStartTime {
		return fmt.Errorf("container %s cgroup ownership does not match exec target generation", match.ID)
	}
	if err := attach(selfPID, ownership.Name, false); err != nil {
		return fmt.Errorf("attach exec-init to container %s cgroup %s: %w", match.ID, ownership.Name, err)
	}
	return nil
}
