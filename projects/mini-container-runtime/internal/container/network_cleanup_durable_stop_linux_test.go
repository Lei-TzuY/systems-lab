//go:build linux

package container

import (
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestCleanupNetworkOwnershipWaitsForDurableStoppedState(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const id = "ctr-network-durable-stop"
	const pid = 5151
	const start = 6161
	if err := st.Save(&state.Container{
		ID:        id,
		Status:    state.StatusCreated,
		RootFS:    "/tmp/rootfs",
		Command:   []string{"true"},
		CreatedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkRunning(id, pid, start, time.Now()); err != nil {
		t.Fatal(err)
	}
	ownership := networkOwnershipForGeneration(
		"minicontainer:durable-stop-test",
		pid,
		start,
		"172.20.0.2",
		[]PortMapping{{HostPort: 18080, ContainerPort: 80}},
	)
	if err := st.MarkNetworkOwnedIfIdentity(id, ownership); err != nil {
		t.Fatal(err)
	}

	portCalls := 0
	vethCalls := 0
	removePort := func(string, int, int, string, string, bool) error { portCalls++; return nil }
	removeVeth := func(string, string, bool) error { vethCalls++; return nil }

	if err := cleanupNetworkOwnershipAfterDurableStopWith(st, id, ownership, false, removePort, removeVeth); err != nil {
		t.Fatalf("running cleanup gate: %v", err)
	}
	if portCalls != 0 || vethCalls != 0 {
		t.Fatalf("destructive cleanup ran before durable stop: port=%d veth=%d", portCalls, vethCalls)
	}
	if got, ok, err := st.GetNetworkOwnership(id); err != nil || !ok || got.Owner != ownership.Owner {
		t.Fatalf("running cleanup consumed ownership proof: got=%+v ok=%v err=%v", got, ok, err)
	}

	if changed, err := st.MarkStoppedIfIdentity(id, pid, start, 0, time.Now()); err != nil || !changed {
		t.Fatalf("persist stopped state: changed=%v err=%v", changed, err)
	}
	if err := cleanupNetworkOwnershipAfterDurableStopWith(st, id, ownership, false, removePort, removeVeth); err != nil {
		t.Fatalf("stopped cleanup: %v", err)
	}
	if portCalls != 1 || vethCalls != 1 {
		t.Fatalf("stopped cleanup calls port=%d veth=%d, want 1/1", portCalls, vethCalls)
	}
	if _, ok, err := st.GetNetworkOwnership(id); err != nil || ok {
		t.Fatalf("stopped cleanup left ownership: ok=%v err=%v", ok, err)
	}
}
