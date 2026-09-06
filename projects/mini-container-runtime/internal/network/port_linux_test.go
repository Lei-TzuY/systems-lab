//go:build linux

package network

import (
	"errors"
	"fmt"
	"strings"
	"testing"
)

func TestSetupPortForwardingSuccessInstallsBothRules(t *testing.T) {
	var calls [][]string
	run := func(args ...string) ([]byte, error) {
		calls = append(calls, append([]string(nil), args...))
		return nil, nil
	}
	if err := setupPortForwardingWith(8080, 80, "172.20.0.2", "", false, run); err != nil {
		t.Fatalf("setupPortForwardingWith: %v", err)
	}
	if len(calls) != 2 {
		t.Fatalf("calls=%d, want 2", len(calls))
	}
	if got := strings.Join(calls[0], " "); !strings.Contains(got, "-A PREROUTING -p tcp --dport 8080") {
		t.Fatalf("unexpected PREROUTING rule: %s", got)
	}
	if got := strings.Join(calls[1], " "); !strings.Contains(got, "-A OUTPUT -p tcp") || !strings.Contains(got, "--to-destination 172.20.0.2:80") {
		t.Fatalf("unexpected OUTPUT rule: %s", got)
	}
}

func TestSetupPortForwardingPreroutingFailureStopsImmediately(t *testing.T) {
	cause := errors.New("prerouting rejected")
	calls := 0
	err := setupPortForwardingWith(8080, 80, "172.20.0.2", "tcp", false, func(args ...string) ([]byte, error) {
		calls++
		return []byte("boom"), cause
	})
	if !errors.Is(err, cause) {
		t.Fatalf("cause not preserved: %v", err)
	}
	if calls != 1 {
		t.Fatalf("calls=%d, want 1", calls)
	}
}

func TestSetupPortForwardingOutputFailureRollsBackPrerouting(t *testing.T) {
	cause := errors.New("output rejected")
	var calls []string
	err := setupPortForwardingWith(8080, 80, "172.20.0.2", "udp", false, func(args ...string) ([]byte, error) {
		calls = append(calls, strings.Join(args, " "))
		switch len(calls) {
		case 1:
			return nil, nil
		case 2:
			return []byte("output failed"), cause
		case 3:
			return nil, nil
		default:
			return nil, fmt.Errorf("unexpected extra call")
		}
	})
	if !errors.Is(err, cause) {
		t.Fatalf("output cause not preserved: %v", err)
	}
	if len(calls) != 3 {
		t.Fatalf("calls=%v", calls)
	}
	if !strings.Contains(calls[2], "-D PREROUTING -p udp --dport 8080") {
		t.Fatalf("rollback rule=%s", calls[2])
	}
}

func TestSetupPortForwardingPreservesRollbackFailure(t *testing.T) {
	setupCause := errors.New("output rejected")
	rollbackCause := errors.New("delete rejected")
	calls := 0
	err := setupPortForwardingWith(8080, 80, "172.20.0.2", "tcp", false, func(args ...string) ([]byte, error) {
		calls++
		switch calls {
		case 1:
			return nil, nil
		case 2:
			return nil, setupCause
		case 3:
			return nil, rollbackCause
		default:
			return nil, nil
		}
	})
	if !errors.Is(err, setupCause) || !errors.Is(err, rollbackCause) {
		t.Fatalf("joined failures not preserved: %v", err)
	}
}

func TestSetupPortForwardingRejectsNilRunner(t *testing.T) {
	if err := setupPortForwardingWith(1, 1, "127.0.0.1", "tcp", false, nil); err == nil {
		t.Fatal("nil runner unexpectedly accepted")
	}
}
