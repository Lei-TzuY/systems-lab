//go:build linux

package network

import (
	"errors"
	"testing"
)

func TestVethHostOwnedCreateFailureDoesNotDeleteForeignInterface(t *testing.T) {
	cause := errors.New("already exists")
	removeCalls := 0
	err := setupVethHostOwnedWithOps(42, "172.20.0.1/24", false, vethHostSetupOps{
		createPair: func(string, string) error { return cause },
		addAddr:    func(string, string) error { t.Fatal("addAddr called after create failure"); return nil },
		setLinkUp:  func(string) error { t.Fatal("setLinkUp called after create failure"); return nil },
		movePeer:   func(string, int) error { t.Fatal("movePeer called after create failure"); return nil },
		removeHost: func(int, bool) error { removeCalls++; return nil },
	})
	if !errors.Is(err, cause) {
		t.Fatalf("create cause not preserved: %v", err)
	}
	if removeCalls != 0 {
		t.Fatalf("foreign veth delete calls=%d, want 0", removeCalls)
	}
}

func TestVethHostOwnedPostCreateFailureRollsBackOwnedInterface(t *testing.T) {
	cause := errors.New("address rejected")
	removeCalls := 0
	err := setupVethHostOwnedWithOps(43, "172.20.0.1/24", false, vethHostSetupOps{
		createPair: func(name, peer string) error {
			if name != VethHostIface(43) || peer != vethPeerName {
				t.Fatalf("unexpected pair %q %q", name, peer)
			}
			return nil
		},
		addAddr: func(name, cidr string) error {
			if name != VethHostIface(43) || cidr != "172.20.0.1/24" {
				t.Fatalf("unexpected addr %q %q", name, cidr)
			}
			return cause
		},
		setLinkUp: func(string) error { t.Fatal("setLinkUp called after address failure"); return nil },
		movePeer:  func(string, int) error { t.Fatal("movePeer called after address failure"); return nil },
		removeHost: func(pid int, debug bool) error {
			removeCalls++
			if pid != 43 {
				t.Fatalf("remove pid=%d", pid)
			}
			return nil
		},
	})
	if !errors.Is(err, cause) {
		t.Fatalf("setup cause not preserved: %v", err)
	}
	if removeCalls != 1 {
		t.Fatalf("owned veth rollback calls=%d, want 1", removeCalls)
	}
}

func TestVethHostOwnedPreservesSetupAndRollbackFailures(t *testing.T) {
	setupCause := errors.New("move rejected")
	cleanupCause := errors.New("delete rejected")
	err := setupVethHostOwnedWithOps(44, "172.20.0.1/24", false, vethHostSetupOps{
		createPair: func(string, string) error { return nil },
		addAddr:    func(string, string) error { return nil },
		setLinkUp:  func(string) error { return nil },
		movePeer:   func(string, int) error { return setupCause },
		removeHost: func(int, bool) error { return cleanupCause },
	})
	if !errors.Is(err, setupCause) || !errors.Is(err, cleanupCause) {
		t.Fatalf("joined setup/rollback failures not preserved: %v", err)
	}
}

func TestVethHostOwnedSuccessTransfersCleanupOwnershipToCaller(t *testing.T) {
	removeCalls := 0
	err := setupVethHostOwnedWithOps(45, "172.20.0.1/24", false, vethHostSetupOps{
		createPair: func(string, string) error { return nil },
		addAddr:    func(string, string) error { return nil },
		setLinkUp:  func(string) error { return nil },
		movePeer:   func(string, int) error { return nil },
		removeHost: func(int, bool) error { removeCalls++; return nil },
	})
	if err != nil {
		t.Fatalf("setup failed: %v", err)
	}
	if removeCalls != 0 {
		t.Fatalf("successful setup cleaned veth early: calls=%d", removeCalls)
	}
}
