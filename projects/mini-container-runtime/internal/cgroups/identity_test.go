package cgroups

import (
	"strings"
	"testing"
)

func TestNameForContainerProcessBindsProcessGeneration(t *testing.T) {
	first, err := NameForContainerProcess("0123456789abcdef", 101, 12345)
	if err != nil {
		t.Fatalf("NameForContainerProcess: %v", err)
	}
	newPIDSameTick, err := NameForContainerProcess("0123456789abcdef", 102, 12345)
	if err != nil {
		t.Fatalf("NameForContainerProcess same-tick restart: %v", err)
	}
	reusedPID, err := NameForContainerProcess("0123456789abcdef", 101, 12346)
	if err != nil {
		t.Fatalf("NameForContainerProcess PID reuse: %v", err)
	}
	if first == newPIDSameTick {
		t.Fatalf("same-tick restart produced same cgroup name %q", first)
	}
	if first == reusedPID {
		t.Fatalf("reused PID produced same cgroup name %q", first)
	}
	if first != "minicontainer-0123456789abcdef-101-12345" {
		t.Fatalf("name = %q", first)
	}
}

func TestNameForContainerProcessRejectsMissingIdentity(t *testing.T) {
	for _, tc := range []struct {
		id        string
		pid       int
		startTime uint64
	}{
		{id: "", pid: 1, startTime: 1},
		{id: "ctr", pid: 0, startTime: 1},
		{id: "ctr", pid: -1, startTime: 1},
		{id: "ctr", pid: 1, startTime: 0},
	} {
		if _, err := NameForContainerProcess(tc.id, tc.pid, tc.startTime); err == nil {
			t.Fatalf("NameForContainerProcess(%q, %d, %d) succeeded, want error", tc.id, tc.pid, tc.startTime)
		}
	}
}

func TestNameForContainerProcessRejectsUnsafeOrOversizedID(t *testing.T) {
	for _, id := range []string{"../escape", "bad id", strings.Repeat("a", maxCgroupNameLen)} {
		if _, err := NameForContainerProcess(id, 1, 1); err == nil {
			t.Fatalf("NameForContainerProcess(%q, 1, 1) succeeded, want error", id)
		}
	}
}
