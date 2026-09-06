//go:build linux

package container

import (
	"errors"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func persistRuntimeGenerationOwnership(t *testing.T, st *state.Store, id string, pid int, start uint64) {
	t.Helper()
	snapshot := &state.Container{
		ID:           id,
		Status:       state.StatusRunning,
		PID:          pid,
		PIDStartTime: start,
		RootFS:       "/tmp/rootfs",
		Command:      []string{"true"},
		CreatedAt:    time.Now(),
	}
	persistOwnedGeneration(t, st, snapshot)
	ownership := networkOwnershipForGeneration(
		"minicontainer:generation-cleanup-test",
		pid,
		start,
		"172.20.0.2",
		[]PortMapping{{HostPort: 18080, ContainerPort: 80}},
	)
	if err := st.MarkNetworkOwnedIfIdentity(id, ownership); err != nil {
		t.Fatal(err)
	}
	if _, err := st.MarkStoppedIfIdentity(id, pid, start, -1, time.Now()); err != nil {
		t.Fatal(err)
	}
}

func TestCleanupRuntimeGenerationResourcesSkipsNewerOwnership(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const id = "ctr-stale-generation-cleanup"
	const newPID = 2222
	const newStart = 20
	persistRuntimeGenerationOwnership(t, st, id, newPID, newStart)

	cgroupCalls := 0
	portCalls := 0
	vethCalls := 0
	dnsCalls := 0
	err = cleanupRuntimeGenerationResourcesWith(
		st,
		id,
		1111,
		10,
		func(string, int, uint64) error { cgroupCalls++; return nil },
		func(string, int, int, string, string, bool) error { portCalls++; return nil },
		func(string, string, bool) error { vethCalls++; return nil },
		func(networkName, gotID string, gotPID int, gotStart uint64) error {
			dnsCalls++
			if networkName != defaultBridgeDNSNetwork || gotID != id || gotPID != 1111 || gotStart != 10 {
				t.Fatalf("wrong stale DNS generation: network=%s id=%s generation=%d/%d", networkName, gotID, gotPID, gotStart)
			}
			return nil
		},
	)
	if err != nil {
		t.Fatalf("stale generation cleanup: %v", err)
	}
	if cgroupCalls != 0 || portCalls != 0 || vethCalls != 0 || dnsCalls != 1 {
		t.Fatalf("stale cleanup calls: cgroup=%d port=%d veth=%d dns=%d", cgroupCalls, portCalls, vethCalls, dnsCalls)
	}
	if ownership, ok, err := st.GetCgroupOwnership(id); err != nil || !ok || ownership.PID != newPID || ownership.PIDStartTime != newStart {
		t.Fatalf("newer cgroup ownership changed: ownership=%+v ok=%v err=%v", ownership, ok, err)
	}
	if ownership, ok, err := st.GetNetworkOwnership(id); err != nil || !ok || ownership.PID != newPID || ownership.PIDStartTime != newStart {
		t.Fatalf("newer network ownership changed: ownership=%+v ok=%v err=%v", ownership, ok, err)
	}
}

func TestCleanupRuntimeGenerationResourcesConsumesMatchingOwnership(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const id = "ctr-matching-generation-cleanup"
	const pid = 3333
	const start = 30
	persistRuntimeGenerationOwnership(t, st, id, pid, start)

	cgroupCalls := 0
	portCalls := 0
	vethCalls := 0
	dnsCalls := 0
	err = cleanupRuntimeGenerationResourcesWith(
		st,
		id,
		pid,
		start,
		func(gotID string, gotPID int, gotStart uint64) error {
			cgroupCalls++
			if gotID != id || gotPID != pid || gotStart != start {
				t.Fatalf("wrong cgroup generation: %s %d/%d", gotID, gotPID, gotStart)
			}
			return nil
		},
		func(string, int, int, string, string, bool) error { portCalls++; return nil },
		func(string, string, bool) error { vethCalls++; return nil },
		func(networkName, gotID string, gotPID int, gotStart uint64) error {
			dnsCalls++
			if networkName != defaultBridgeDNSNetwork || gotID != id || gotPID != pid || gotStart != start {
				t.Fatalf("wrong DNS generation: network=%s id=%s generation=%d/%d", networkName, gotID, gotPID, gotStart)
			}
			return nil
		},
	)
	if err != nil {
		t.Fatalf("matching generation cleanup: %v", err)
	}
	if cgroupCalls != 1 || portCalls != 1 || vethCalls != 1 || dnsCalls != 1 {
		t.Fatalf("matching cleanup calls: cgroup=%d port=%d veth=%d dns=%d", cgroupCalls, portCalls, vethCalls, dnsCalls)
	}
	if _, ok, err := st.GetCgroupOwnership(id); err != nil || ok {
		t.Fatalf("matching cgroup ownership remains: ok=%v err=%v", ok, err)
	}
	if _, ok, err := st.GetNetworkOwnership(id); err != nil || ok {
		t.Fatalf("matching network ownership remains: ok=%v err=%v", ok, err)
	}
}

func TestCleanupRuntimeGenerationResourcesJoinsDNSErrorAfterOtherCleanup(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const id = "ctr-dns-cleanup-failure"
	const pid = 4444
	const start = 40
	persistRuntimeGenerationOwnership(t, st, id, pid, start)

	cgroupCalls := 0
	portCalls := 0
	vethCalls := 0
	dnsSentinel := errors.New("dns write failed")
	err = cleanupRuntimeGenerationResourcesWith(
		st,
		id,
		pid,
		start,
		func(string, int, uint64) error { cgroupCalls++; return nil },
		func(string, int, int, string, string, bool) error { portCalls++; return nil },
		func(string, string, bool) error { vethCalls++; return nil },
		func(string, string, int, uint64) error { return dnsSentinel },
	)
	if err == nil || !errors.Is(err, dnsSentinel) || !strings.Contains(err.Error(), "cleanup bridge DNS registration") {
		t.Fatalf("expected contextual DNS cleanup error, got %v", err)
	}
	if cgroupCalls != 1 || portCalls != 1 || vethCalls != 1 {
		t.Fatalf("DNS failure suppressed other cleanup: cgroup=%d port=%d veth=%d", cgroupCalls, portCalls, vethCalls)
	}
	if _, ok, err := st.GetCgroupOwnership(id); err != nil || ok {
		t.Fatalf("cgroup ownership remains after DNS failure: ok=%v err=%v", ok, err)
	}
	if _, ok, err := st.GetNetworkOwnership(id); err != nil || ok {
		t.Fatalf("network ownership remains after DNS failure: ok=%v err=%v", ok, err)
	}
}
