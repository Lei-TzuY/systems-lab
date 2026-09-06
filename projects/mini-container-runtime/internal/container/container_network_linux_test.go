//go:build linux

package container

import (
	"errors"
	"fmt"
	"strings"
	"testing"
)

func TestContainerNetworkRequiresLoopbackWithoutBridge(t *testing.T) {
	loopbackCalls := 0
	bridgeCalls := 0
	err := setupContainerNetworkWith(false, "172.20.0.2/24", "172.20.0.1", false,
		func(bool) error { loopbackCalls++; return nil },
		func(string, string, bool) error { bridgeCalls++; return nil },
	)
	if err != nil {
		t.Fatalf("setupContainerNetworkWith: %v", err)
	}
	if loopbackCalls != 1 || bridgeCalls != 0 {
		t.Fatalf("loopbackCalls=%d bridgeCalls=%d, want 1,0", loopbackCalls, bridgeCalls)
	}
}

func TestContainerNetworkLoopbackFailureIsFatalAndSkipsBridge(t *testing.T) {
	cause := errors.New("cannot raise lo")
	bridgeCalls := 0
	err := setupContainerNetworkWith(true, "172.20.0.2/24", "172.20.0.1", false,
		func(bool) error { return cause },
		func(string, string, bool) error { bridgeCalls++; return nil },
	)
	if !errors.Is(err, cause) || !strings.Contains(err.Error(), "configure container loopback") {
		t.Fatalf("loopback failure not preserved: %v", err)
	}
	if bridgeCalls != 0 {
		t.Fatalf("bridge setup ran %d time(s) after loopback failure", bridgeCalls)
	}
}

func TestContainerNetworkConfiguresLoopbackBeforeBridge(t *testing.T) {
	var order []string
	err := setupContainerNetworkWith(true, "172.20.0.2/24", "172.20.0.1", false,
		func(bool) error { order = append(order, "loopback"); return nil },
		func(cidr, gateway string, debug bool) error {
			if cidr != "172.20.0.2/24" || gateway != "172.20.0.1" {
				t.Fatalf("unexpected bridge args %q %q", cidr, gateway)
			}
			order = append(order, "bridge")
			return nil
		},
	)
	if err != nil {
		t.Fatalf("setupContainerNetworkWith: %v", err)
	}
	if got, want := fmt.Sprint(order), "[loopback bridge]"; got != want {
		t.Fatalf("setup order=%s, want %s", got, want)
	}
}

func TestContainerNetworkRejectsNilLoopbackOperation(t *testing.T) {
	err := setupContainerNetworkWith(false, "", "", false, nil, nil)
	if err == nil || !strings.Contains(err.Error(), "loopback network operation is nil") {
		t.Fatalf("nil loopback error=%v", err)
	}
}
