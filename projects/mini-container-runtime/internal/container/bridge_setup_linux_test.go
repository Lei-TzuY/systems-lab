//go:build linux

package container

import (
	"errors"
	"fmt"
	"strings"
	"testing"
)

func TestBridgeHostVethFailureDoesNotInferCleanupOwnership(t *testing.T) {
	cause := errors.New("veth setup failed")
	removeCalls := 0
	portCalls := 0
	cleanup, err := setupBridgeHostWithOps(42, "172.20.0.1/24", "172.20.0.2", []PortMapping{{HostPort: 8080, ContainerPort: 80}}, false, bridgeHostOps{
		setupVeth: func(int, string, bool) error { return cause },
		removeVeth: func(pid int, debug bool) error {
			removeCalls++
			return nil
		},
		setupPort: func(int, int, string, string, bool) error { portCalls++; return nil },
		removePort: func(int, int, string, string, bool) error { return nil },
	})
	if cleanup != nil {
		t.Fatal("failed setup returned cleanup ownership")
	}
	if !errors.Is(err, cause) {
		t.Fatalf("setup cause not preserved: %v", err)
	}
	if removeCalls != 0 || portCalls != 0 {
		t.Fatalf("failed veth setup triggered unowned cleanup/setup: removeVeth=%d setupPort=%d", removeCalls, portCalls)
	}
}

func TestBridgeHostPortFailureRollsBackInstalledRulesInReverseAndVeth(t *testing.T) {
	mappings := []PortMapping{
		{HostPort: 8080, ContainerPort: 80, Protocol: "tcp"},
		{HostPort: 5353, ContainerPort: 53, Protocol: "udp"},
		{HostPort: 8443, ContainerPort: 443, Protocol: "tcp"},
	}
	cause := errors.New("third mapping rejected")
	var order []string
	setupIndex := 0
	cleanup, err := setupBridgeHostWithOps(99, "172.20.0.1/24", "172.20.0.2", mappings, false, bridgeHostOps{
		setupVeth: func(int, string, bool) error { order = append(order, "veth+"); return nil },
		removeVeth: func(int, bool) error { order = append(order, "veth-"); return nil },
		setupPort: func(hostPort, containerPort int, containerIP, protocol string, debug bool) error {
			setupIndex++
			order = append(order, fmt.Sprintf("port+%d", hostPort))
			if setupIndex == 3 {
				return cause
			}
			return nil
		},
		removePort: func(hostPort, containerPort int, containerIP, protocol string, debug bool) error {
			order = append(order, fmt.Sprintf("port-%d", hostPort))
			return nil
		},
	})
	if cleanup != nil {
		t.Fatal("failed setup returned cleanup ownership")
	}
	if !errors.Is(err, cause) {
		t.Fatalf("port failure not preserved: %v", err)
	}
	want := []string{"veth+", "port+8080", "port+5353", "port+8443", "port-5353", "port-8080", "veth-"}
	if fmt.Sprint(order) != fmt.Sprint(want) {
		t.Fatalf("rollback order=%v, want %v", order, want)
	}
}

func TestBridgeHostSuccessReturnsCleanupForAllResources(t *testing.T) {
	mappings := []PortMapping{{HostPort: 8080, ContainerPort: 80}, {HostPort: 8443, ContainerPort: 443}}
	var order []string
	cleanup, err := setupBridgeHostWithOps(7, "172.20.0.1/24", "172.20.0.2", mappings, false, bridgeHostOps{
		setupVeth: func(int, string, bool) error { order = append(order, "veth+"); return nil },
		removeVeth: func(int, bool) error { order = append(order, "veth-"); return nil },
		setupPort: func(hostPort, containerPort int, containerIP, protocol string, debug bool) error {
			order = append(order, fmt.Sprintf("port+%d", hostPort))
			return nil
		},
		removePort: func(hostPort, containerPort int, containerIP, protocol string, debug bool) error {
			order = append(order, fmt.Sprintf("port-%d", hostPort))
			return nil
		},
	})
	if err != nil {
		t.Fatalf("setupBridgeHostWithOps: %v", err)
	}
	if cleanup == nil {
		t.Fatal("successful setup missing cleanup")
	}
	if err := cleanup(); err != nil {
		t.Fatalf("cleanup: %v", err)
	}
	want := []string{"veth+", "port+8080", "port+8443", "port-8443", "port-8080", "veth-"}
	if fmt.Sprint(order) != fmt.Sprint(want) {
		t.Fatalf("setup/cleanup order=%v, want %v", order, want)
	}
}

func TestBridgeHostPreservesSetupAndRollbackFailures(t *testing.T) {
	setupCause := errors.New("mapping failed")
	cleanupCause := errors.New("veth delete failed")
	_, err := setupBridgeHostWithOps(12, "172.20.0.1/24", "172.20.0.2", []PortMapping{{HostPort: 80, ContainerPort: 80}}, false, bridgeHostOps{
		setupVeth:  func(int, string, bool) error { return nil },
		removeVeth: func(int, bool) error { return cleanupCause },
		setupPort:  func(int, int, string, string, bool) error { return setupCause },
		removePort: func(int, int, string, string, bool) error { return nil },
	})
	if !errors.Is(err, setupCause) || !errors.Is(err, cleanupCause) {
		t.Fatalf("joined setup/rollback errors not preserved: %v", err)
	}
}

func TestBridgeHostCleanupPreservesPortAndVethFailures(t *testing.T) {
	portCause := errors.New("port delete failed")
	vethCause := errors.New("veth delete failed")
	cleanup, err := setupBridgeHostWithOps(13, "172.20.0.1/24", "172.20.0.2", []PortMapping{{HostPort: 8080, ContainerPort: 80}}, false, bridgeHostOps{
		setupVeth:  func(int, string, bool) error { return nil },
		removeVeth: func(int, bool) error { return vethCause },
		setupPort:  func(int, int, string, string, bool) error { return nil },
		removePort: func(int, int, string, string, bool) error { return portCause },
	})
	if err != nil {
		t.Fatalf("setup failed: %v", err)
	}
	err = cleanup()
	if !errors.Is(err, portCause) || !errors.Is(err, vethCause) {
		t.Fatalf("cleanup failures not preserved: %v", err)
	}
}

func TestBridgeContainerDisabledDoesNotConfigure(t *testing.T) {
	calls := 0
	if err := setupBridgeContainerWith(false, "172.20.0.2/24", "172.20.0.1", false, func(string, string, bool) error {
		calls++
		return errors.New("must not run")
	}); err != nil {
		t.Fatalf("disabled bridge returned error: %v", err)
	}
	if calls != 0 {
		t.Fatalf("disabled bridge setup calls=%d", calls)
	}
}

func TestBridgeContainerFailureIsFatal(t *testing.T) {
	cause := errors.New("route rejected")
	err := setupBridgeContainerWith(true, "172.20.0.2/24", "172.20.0.1", false, func(cidr, gateway string, debug bool) error {
		if cidr != "172.20.0.2/24" || gateway != "172.20.0.1" {
			t.Fatalf("unexpected bridge args %q %q", cidr, gateway)
		}
		return cause
	})
	if !errors.Is(err, cause) || !strings.Contains(err.Error(), "configure container bridge network") {
		t.Fatalf("bridge child failure not preserved: %v", err)
	}
}

func TestBridgeSetupRejectsNilOperations(t *testing.T) {
	_, err := setupBridgeHostWithOps(1, "a", "b", nil, false, bridgeHostOps{})
	if err == nil || !strings.Contains(err.Error(), "operation is nil") {
		t.Fatalf("nil host ops error=%v", err)
	}
	if err := setupBridgeContainerWith(true, "a", "b", false, nil); err == nil {
		t.Fatal("nil container setup unexpectedly accepted")
	}
}
