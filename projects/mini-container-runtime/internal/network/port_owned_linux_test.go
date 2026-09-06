//go:build linux

package network

import (
	"errors"
	"os/exec"
	"strings"
	"testing"
)

func TestOwnedPortSetupTagsRulesAndRollbackWithSameOwner(t *testing.T) {
	owner := "minicontainer:test-generation"
	outputCause := errors.New("output rejected")
	var calls []string
	err := setupPortForwardingOwnedWith(owner, 8080, 80, "172.20.0.2", "", false, func(args ...string) ([]byte, error) {
		calls = append(calls, strings.Join(args, " "))
		if len(calls) == 2 {
			return []byte("output"), outputCause
		}
		return nil, nil
	})
	if !errors.Is(err, outputCause) {
		t.Fatalf("setup cause not preserved: %v", err)
	}
	if len(calls) != 3 {
		t.Fatalf("calls=%v, want add/add/rollback", calls)
	}
	for i, call := range calls {
		if !strings.Contains(call, "-m comment --comment "+owner) {
			t.Fatalf("call %d missing owner tag: %s", i, call)
		}
	}
	if !strings.Contains(calls[0], "-A PREROUTING") || !strings.Contains(calls[1], "-A OUTPUT") || !strings.Contains(calls[2], "-D PREROUTING") {
		t.Fatalf("unexpected setup/rollback sequence: %v", calls)
	}
}

func TestOwnedPortCleanupChecksThenDeletesBothTaggedRulesAndJoinsFailures(t *testing.T) {
	owner := "minicontainer:test-generation"
	preCause := errors.New("prerouting delete failed")
	outCause := errors.New("output delete failed")
	var calls []string
	err := removePortForwardingOwnedWith(owner, 8080, 80, "172.20.0.2", "tcp", false, func(args ...string) ([]byte, error) {
		calls = append(calls, strings.Join(args, " "))
		switch len(calls) {
		case 2:
			return []byte("pre"), preCause
		case 4:
			return []byte("out"), outCause
		default:
			return nil, nil
		}
	})
	if len(calls) != 4 {
		t.Fatalf("calls=%v, want check/delete for both tagged rules", calls)
	}
	for i, call := range calls {
		if !strings.Contains(call, "-m comment --comment "+owner) {
			t.Fatalf("cleanup call %d missing owner tag: %s", i, call)
		}
	}
	if !strings.Contains(calls[0], "-C PREROUTING") || !strings.Contains(calls[1], "-D PREROUTING") ||
		!strings.Contains(calls[2], "-C OUTPUT") || !strings.Contains(calls[3], "-D OUTPUT") {
		t.Fatalf("unexpected cleanup sequence: %v", calls)
	}
	if !errors.Is(err, preCause) || !errors.Is(err, outCause) {
		t.Fatalf("cleanup errors not joined: %v", err)
	}
}

func TestOwnedPortCleanupTreatsConfirmedMissingRulesAsSuccess(t *testing.T) {
	owner := "minicontainer:test-generation"
	missingErr := exec.Command("sh", "-c", "exit 1").Run()
	if missingErr == nil {
		t.Fatal("expected exit status 1")
	}
	var calls []string
	err := removePortForwardingOwnedWith(owner, 8080, 80, "172.20.0.2", "tcp", false, func(args ...string) ([]byte, error) {
		calls = append(calls, strings.Join(args, " "))
		return nil, missingErr
	})
	if err != nil {
		t.Fatalf("confirmed missing rules should be idempotent success: %v", err)
	}
	if len(calls) != 2 {
		t.Fatalf("calls=%v, want only two -C checks", calls)
	}
	for _, call := range calls {
		if !strings.Contains(call, " -C ") {
			t.Fatalf("unexpected delete after missing check: %s", call)
		}
	}
}

func TestOwnedPortCleanupDoesNotHideCheckFailures(t *testing.T) {
	owner := "minicontainer:test-generation"
	cause := errors.New("iptables unavailable")
	calls := 0
	err := removePortForwardingOwnedWith(owner, 8080, 80, "172.20.0.2", "tcp", false, func(args ...string) ([]byte, error) {
		calls++
		return []byte("failed"), cause
	})
	if !errors.Is(err, cause) {
		t.Fatalf("check failure not preserved: %v", err)
	}
	if calls != 2 {
		t.Fatalf("calls=%d, want both checks attempted", calls)
	}
}

func TestOwnedPortRulesRejectMissingOwnerBeforeIPTables(t *testing.T) {
	calls := 0
	run := func(args ...string) ([]byte, error) {
		calls++
		return nil, nil
	}
	if err := setupPortForwardingOwnedWith("", 8080, 80, "172.20.0.2", "tcp", false, run); err == nil {
		t.Fatal("empty setup owner unexpectedly accepted")
	}
	if err := removePortForwardingOwnedWith("", 8080, 80, "172.20.0.2", "tcp", false, run); err == nil {
		t.Fatal("empty cleanup owner unexpectedly accepted")
	}
	if calls != 0 {
		t.Fatalf("iptables called %d times for invalid owner", calls)
	}
}

func TestNewPortForwardingOwnerIsGenerationScoped(t *testing.T) {
	first, err := NewPortForwardingOwner()
	if err != nil {
		t.Fatalf("first owner: %v", err)
	}
	second, err := NewPortForwardingOwner()
	if err != nil {
		t.Fatalf("second owner: %v", err)
	}
	if !strings.HasPrefix(first, portForwardingOwnerPrefix) || !strings.HasPrefix(second, portForwardingOwnerPrefix) {
		t.Fatalf("owner prefix missing: %q %q", first, second)
	}
	if first == second {
		t.Fatalf("generation owners unexpectedly identical: %q", first)
	}
}
