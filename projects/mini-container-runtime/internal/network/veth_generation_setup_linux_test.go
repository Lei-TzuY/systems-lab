//go:build linux

package network

import (
	"errors"
	"testing"
)

func TestVethGenerationOwnedSetupUsesExactPersistableName(t *testing.T) {
	owner := "minicontainer:0123456789abcdef0123456789abcdef"
	host := VethHostIfaceOwned(owner)
	var createdName string
	removeCalls := 0
	err := setupVethHostNamedWithOps(host, 71, "172.20.0.1/24", false, vethHostSetupOps{
		createPair: func(name, peer string) error {
			createdName = name
			if peer != vethPeerName {
				t.Fatalf("peer=%q, want %q", peer, vethPeerName)
			}
			return nil
		},
		addAddr: func(name, _ string) error {
			if name != host {
				t.Fatalf("addr name=%q", name)
			}
			return nil
		},
		setLinkUp: func(name string) error {
			if name != host {
				t.Fatalf("up name=%q", name)
			}
			return nil
		},
		movePeer: func(name string, pid int) error {
			if name != vethPeerName || pid != 71 {
				t.Fatalf("move=%q/%d", name, pid)
			}
			return nil
		},
		removeHost: func(int, bool) error {
			removeCalls++
			return nil
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if createdName != host || removeCalls != 0 {
		t.Fatalf("created=%q removeCalls=%d, want %q/0", createdName, removeCalls, host)
	}
}

func TestVethGenerationOwnedPostCreateFailureRollsBackExactName(t *testing.T) {
	owner := "minicontainer:0123456789abcdef0123456789abcdef"
	host := VethHostIfaceOwned(owner)
	cause := errors.New("address failed")
	removeCalls := 0
	err := setupVethHostNamedWithOps(host, 72, "172.20.0.1/24", false, vethHostSetupOps{
		createPair: func(name, peer string) error { return nil },
		addAddr:    func(name, cidr string) error { return cause },
		setLinkUp:  func(string) error { t.Fatal("setLinkUp after addr failure"); return nil },
		movePeer:   func(string, int) error { t.Fatal("movePeer after addr failure"); return nil },
		removeHost: func(pid int, debug bool) error {
			removeCalls++
			if pid != 72 {
				t.Fatalf("rollback pid=%d", pid)
			}
			return nil
		},
	})
	if !errors.Is(err, cause) || removeCalls != 1 {
		t.Fatalf("rollback err=%v removeCalls=%d", err, removeCalls)
	}
}

func TestSetupVethHostGenerationOwnedRejectsMismatchedNameBeforeMutation(t *testing.T) {
	owner := "minicontainer:0123456789abcdef0123456789abcdef"
	other := "minicontainer:fedcba9876543210fedcba9876543210"
	if err := SetupVethHostGenerationOwned(owner, VethHostIfaceOwned(other), 73, "172.20.0.1/24", false); err == nil {
		t.Fatal("mismatched generation veth name accepted")
	}
}
