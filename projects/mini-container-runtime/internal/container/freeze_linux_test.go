//go:build linux

package container

import (
	"errors"
	"os"
	"testing"
	"time"

	"minicontainer/internal/cgroups"
	"minicontainer/internal/state"
)

func TestCgroupControlsRejectProcessIdentityMismatch(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open state store: %v", err)
	}
	startTime, err := ProcessStartTime(os.Getpid())
	if err != nil {
		t.Fatalf("ProcessStartTime: %v", err)
	}
	wrongStartTime := startTime + 1
	if wrongStartTime == 0 {
		wrongStartTime = startTime - 1
	}
	c := &state.Container{
		ID:           "ctr-cgroup-reuse",
		Status:       state.StatusRunning,
		PID:          os.Getpid(),
		PIDStartTime: wrongStartTime,
		CreatedAt:    time.Now(),
	}
	if err := st.Save(c); err != nil {
		t.Fatalf("Save: %v", err)
	}

	for name, fn := range map[string]func() error{
		"freeze": func() error { return FreezeContainer(st, c.ID) },
		"thaw":   func() error { return ThawContainer(st, c.ID) },
		"update": func() error { return UpdateContainerResources(st, c.ID, cgroups.UpdateConfig{}, false) },
	} {
		t.Run(name, func(t *testing.T) {
			if err := fn(); !errors.Is(err, ErrProcessIdentityMismatch) {
				t.Fatalf("error = %v, want ErrProcessIdentityMismatch", err)
			}
		})
	}
}
