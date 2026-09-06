//go:build linux

package container

import (
	"errors"
	"testing"
)

func TestManagedBridgeSetupFailureDefersDestructiveRollback(t *testing.T) {
	cause := errors.New("second mapping rejected")
	setupCalls := 0
	removePortCalls := 0
	removeVethCalls := 0

	cleanup, err := setupBridgeHostWithOpsPolicy(
		77,
		"172.20.0.1/24",
		"172.20.0.2",
		[]PortMapping{{HostPort: 8080, ContainerPort: 80}, {HostPort: 8443, ContainerPort: 443}},
		false,
		bridgeHostOps{
			setupVeth: func(int, string, bool) error { return nil },
			removeVeth: func(int, bool) error { removeVethCalls++; return nil },
			setupPort: func(int, int, string, string, bool) error {
				setupCalls++
				if setupCalls == 2 {
					return cause
				}
				return nil
			},
			removePort: func(int, int, string, string, bool) error { removePortCalls++; return nil },
		},
		false,
	)
	if cleanup != nil {
		t.Fatal("failed durable setup returned cleanup ownership")
	}
	if !errors.Is(err, cause) {
		t.Fatalf("setup cause not preserved: %v", err)
	}
	if removePortCalls != 0 || removeVethCalls != 0 {
		t.Fatalf("managed setup rolled back before durable stop: ports=%d veth=%d", removePortCalls, removeVethCalls)
	}
}
