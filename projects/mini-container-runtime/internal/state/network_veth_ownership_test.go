package state

import (
	"strings"
	"testing"
	"time"
)

func testVethOnlyNetworkOwnership(pid int, start uint64) NetworkOwnership {
	return NetworkOwnership{
		Owner:        "minicontainer:veth-only-owner",
		PID:          pid,
		PIDStartTime: start,
		VethHost:     "vhabcdefghijklm",
	}
}

func TestNetworkOwnershipAcceptsVethOnlyResourceAndPersistsAcrossStop(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-veth-only-owner"
	saveCreatedContainer(t, st, id)
	if err := st.MarkRunning(id, 501, 502, time.Now()); err != nil {
		t.Fatal(err)
	}
	ownership := testVethOnlyNetworkOwnership(501, 502)
	if err := st.MarkNetworkOwnedIfIdentity(id, ownership); err != nil {
		t.Fatalf("persist veth-only ownership: %v", err)
	}
	if _, err := st.MarkStoppedIfIdentity(id, 501, 502, -1, time.Now()); err != nil {
		t.Fatal(err)
	}
	got, ok, err := st.GetNetworkOwnership(id)
	if err != nil || !ok || !networkOwnershipEqual(got, ownership) {
		t.Fatalf("veth-only ownership after stop got=%+v ok=%v err=%v", got, ok, err)
	}
	if err := st.MarkRunning(id, 601, 602, time.Now()); err == nil || !strings.Contains(err.Error(), "pending network cleanup") {
		t.Fatalf("restart with pending veth cleanup error=%v", err)
	}
}

func TestNetworkOwnershipRejectsInvalidVethHost(t *testing.T) {
	ownership := testVethOnlyNetworkOwnership(1, 2)
	for _, bad := range []string{"veth-h1", "vhABCDEFGHIJKLM", "vh1234567890123", "vh../../escape.."} {
		ownership.VethHost = bad
		if err := validateNetworkOwnership(ownership); err == nil {
			t.Fatalf("invalid veth host %q accepted", bad)
		}
	}
}

func TestLegacyRulesOnlyNetworkOwnershipRemainsValid(t *testing.T) {
	ownership := NetworkOwnership{
		Owner:        "minicontainer:legacy-rules",
		PID:          7,
		PIDStartTime: 8,
		Mappings: []PortForwardingOwnership{{
			HostPort:      8080,
			ContainerPort: 80,
			ContainerIP:   "172.20.0.2",
			Protocol:      "tcp",
		}},
	}
	if err := validateNetworkOwnership(ownership); err != nil {
		t.Fatalf("legacy rules-only ownership rejected: %v", err)
	}
}
