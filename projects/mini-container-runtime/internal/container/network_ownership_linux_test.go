//go:build linux

package container

import (
	"errors"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/network"
	"minicontainer/internal/state"
)

func saveNetworkOwnershipContainer(t *testing.T, st *state.Store, id string, pid int, start uint64) state.NetworkOwnership {
	t.Helper()
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
		"minicontainer:test-owner",
		pid,
		start,
		"172.20.0.2",
		[]PortMapping{
			{HostPort: 8080, ContainerPort: 80},
			{HostPort: 5353, ContainerPort: 53, Protocol: "udp"},
		},
	)
	if err := st.MarkNetworkOwnedIfIdentity(id, ownership); err != nil {
		t.Fatal(err)
	}
	if _, err := st.MarkStoppedIfIdentity(id, pid, start, -1, time.Now()); err != nil {
		t.Fatal(err)
	}
	return ownership
}

func TestNetworkOwnershipForGenerationIncludesVethAndNormalizesProtocol(t *testing.T) {
	owner := "minicontainer:test-owner"
	ownership := networkOwnershipForGeneration(
		owner,
		1,
		2,
		"172.20.0.2",
		[]PortMapping{{HostPort: 8080, ContainerPort: 80}, {HostPort: 5353, ContainerPort: 53, Protocol: "udp"}},
	)
	if ownership.VethHost != network.VethHostIfaceOwned(owner) {
		t.Fatalf("veth host=%q, want generation-owned %q", ownership.VethHost, network.VethHostIfaceOwned(owner))
	}
	if len(ownership.Mappings) != 2 || ownership.Mappings[0].Protocol != "tcp" || ownership.Mappings[1].Protocol != "udp" {
		t.Fatalf("unexpected normalized ownership: %+v", ownership)
	}
}

func TestNetworkOwnershipForGenerationSupportsBridgeWithoutPublishedPorts(t *testing.T) {
	owner := "minicontainer:veth-only"
	ownership := networkOwnershipForGeneration(owner, 3, 4, "172.20.0.2", nil)
	if ownership.VethHost == "" || len(ownership.Mappings) != 0 {
		t.Fatalf("unexpected veth-only ownership: %+v", ownership)
	}
}

func TestCleanupNetworkOwnershipAttemptsPortsThenVethAndClearsSidecar(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-network-cleanup"
	ownership := saveNetworkOwnershipContainer(t, st, id, 101, 202)

	var calls []string
	removePort := func(owner string, hostPort, containerPort int, containerIP, protocol string, debug bool) error {
		calls = append(calls, "port "+owner+" "+protocol+" "+containerIP)
		return nil
	}
	removeVeth := func(name, owner string, debug bool) error {
		calls = append(calls, "veth "+name+" "+owner)
		return nil
	}
	if err := cleanupNetworkOwnershipWith(st, id, ownership, false, removePort, removeVeth); err != nil {
		t.Fatalf("cleanup network ownership: %v", err)
	}
	if len(calls) != 3 || !strings.Contains(calls[0], "udp") || !strings.Contains(calls[1], "tcp") || !strings.HasPrefix(calls[2], "veth ") {
		t.Fatalf("cleanup order/calls=%v", calls)
	}
	if _, ok, err := st.GetNetworkOwnership(id); err != nil || ok {
		t.Fatalf("ownership remains after cleanup: ok=%v err=%v", ok, err)
	}
}

func TestCleanupNetworkOwnershipFailurePreservesSidecarAndAttemptsAllResources(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-network-cleanup-fail"
	ownership := saveNetworkOwnershipContainer(t, st, id, 301, 302)
	cause := errors.New("iptables unavailable")
	portCalls := 0
	vethCalls := 0
	removePort := func(owner string, hostPort, containerPort int, containerIP, protocol string, debug bool) error {
		portCalls++
		if protocol == "udp" {
			return cause
		}
		return nil
	}
	removeVeth := func(name, owner string, debug bool) error {
		vethCalls++
		return nil
	}
	err = cleanupNetworkOwnershipWith(st, id, ownership, false, removePort, removeVeth)
	if !errors.Is(err, cause) {
		t.Fatalf("cleanup cause not preserved: %v", err)
	}
	if portCalls != 2 || vethCalls != 1 {
		t.Fatalf("cleanup calls ports=%d veth=%d, want all resources attempted", portCalls, vethCalls)
	}
	got, ok, readErr := st.GetNetworkOwnership(id)
	if readErr != nil || !ok || got.Owner != ownership.Owner || got.VethHost != ownership.VethHost {
		t.Fatalf("cleanup failure lost recovery sidecar: got=%+v ok=%v err=%v", got, ok, readErr)
	}
}

func TestCleanupNetworkOwnershipVethFailurePreservesSidecar(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-veth-cleanup-fail"
	ownership := saveNetworkOwnershipContainer(t, st, id, 401, 402)
	cause := errors.New("netlink delete failed")
	portCalls := 0
	err = cleanupNetworkOwnershipWith(
		st,
		id,
		ownership,
		false,
		func(string, int, int, string, string, bool) error { portCalls++; return nil },
		func(name, owner string, debug bool) error { return cause },
	)
	if !errors.Is(err, cause) || portCalls != len(ownership.Mappings) {
		t.Fatalf("veth cleanup failure err=%v portCalls=%d", err, portCalls)
	}
	if _, ok, readErr := st.GetNetworkOwnership(id); readErr != nil || !ok {
		t.Fatalf("veth cleanup failure lost sidecar: ok=%v err=%v", ok, readErr)
	}
}

func TestCleanupStoppedNetworkIsNoopWithoutOwnership(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := &state.Container{ID: "ctr-network-none", Status: state.StatusStopped, RootFS: "/tmp/rootfs", Command: []string{"true"}, CreatedAt: time.Now()}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}
	if err := CleanupStoppedNetwork(st, c); err != nil {
		t.Fatalf("legacy stopped container cleanup: %v", err)
	}
}
