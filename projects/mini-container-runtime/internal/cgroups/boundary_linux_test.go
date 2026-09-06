//go:build linux

package cgroups

import (
	"math"
	"testing"
)

func TestApplyRejectsDangerousInputsBeforeFilesystemAccess(t *testing.T) {
	tests := []struct {
		name string
		pid  int
		cfg  Config
	}{
		{"zero PID", 0, Config{Name: "minicontainer-1"}},
		{"negative PID", -1, Config{Name: "minicontainer-1"}},
		{"path traversal name", 1234, Config{Name: "../escape"}},
		{"negative memory", 1234, Config{Name: "minicontainer-1", MemoryMax: -1}},
		{"weight too high", 1234, Config{Name: "minicontainer-1", CPUWeight: 10001}},
		{"NaN CPUs", 1234, Config{Name: "minicontainer-1", CPUs: math.NaN()}},
		{"negative pids", 1234, Config{Name: "minicontainer-1", PidsMax: -1}},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			if err := Apply(tc.pid, tc.cfg, false); err == nil {
				t.Fatal("expected validation error")
			}
		})
	}
}

func TestUpdateLimitsRejectsDangerousInputsBeforeFilesystemAccess(t *testing.T) {
	if err := UpdateLimits("../escape", UpdateConfig{}, false); err == nil {
		t.Fatal("expected invalid cgroup name error")
	}
	if err := UpdateLimits("minicontainer-1", UpdateConfig{CPUs: math.Inf(1)}, false); err == nil {
		t.Fatal("expected invalid CPU quota error")
	}
	if err := UpdateLimits("minicontainer-1", UpdateConfig{CPUWeight: 10001}, false); err == nil {
		t.Fatal("expected invalid CPU weight error")
	}
}

func TestFreezerRejectsDangerousNamesBeforeFilesystemAccess(t *testing.T) {
	for _, name := range []string{"../escape", "nested/name", "has space"} {
		if err := Freeze(name); err == nil {
			t.Fatalf("Freeze accepted invalid name %q", name)
		}
		if err := Unfreeze(name); err == nil {
			t.Fatalf("Unfreeze accepted invalid name %q", name)
		}
		if _, err := IsFrozen(name); err == nil {
			t.Fatalf("IsFrozen accepted invalid name %q", name)
		}
	}
}
