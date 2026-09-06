package container

import (
	"errors"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/cgroups"
	"minicontainer/internal/state"
)

func TestCgroupControlsRejectStoppedContainer(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open state store: %v", err)
	}
	c := &state.Container{
		ID:        "ctr-frz-1",
		Status:    state.StatusStopped,
		PID:       9999,
		CreatedAt: time.Now(),
	}
	if err := st.Save(c); err != nil {
		t.Fatalf("Save: %v", err)
	}

	for name, fn := range map[string]func() error{
		"freeze":   func() error { return FreezeContainer(st, c.ID) },
		"thaw":     func() error { return ThawContainer(st, c.ID) },
		"update":   func() error { return UpdateContainerResources(st, c.ID, cgroups.UpdateConfig{}, false) },
	} {
		t.Run(name, func(t *testing.T) {
			err := fn()
			if err == nil || !strings.Contains(err.Error(), "not running") {
				t.Fatalf("error = %v, want not-running rejection", err)
			}
		})
	}
}

func TestCgroupControlsRejectMissingProcessIdentity(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open state store: %v", err)
	}
	c := &state.Container{
		ID:        "ctr-frz-2",
		Status:    state.StatusRunning,
		PID:       9999,
		CreatedAt: time.Now(),
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
			if err := fn(); !errors.Is(err, ErrProcessIdentityUnavailable) {
				t.Fatalf("error = %v, want ErrProcessIdentityUnavailable", err)
			}
		})
	}
}
