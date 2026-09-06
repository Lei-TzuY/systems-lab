package container

import (
	"errors"
	"reflect"
	"testing"

	"minicontainer/internal/state"
)

func TestCleanupStoppedGenerationExternalResourcesWithdrawsDNSBeforeNetwork(t *testing.T) {
	var order []string
	const id = "ctr-dns-first"
	const pid = 4242
	const start = uint64(99)

	err := cleanupStoppedGenerationExternalResourcesWith(
		nil,
		id,
		pid,
		start,
		func(st *state.Store, gotID string, gotPID int, gotStart uint64, debug bool) error {
			order = append(order, "network")
			if st != nil || gotID != id || gotPID != pid || gotStart != start || debug {
				t.Fatalf("wrong network cleanup args: st=%v id=%s generation=%d/%d debug=%v", st, gotID, gotPID, gotStart, debug)
			}
			return nil
		},
		func(networkName, gotID string, gotPID int, gotStart uint64) error {
			order = append(order, "dns")
			if networkName != defaultBridgeDNSNetwork || gotID != id || gotPID != pid || gotStart != start {
				t.Fatalf("wrong DNS cleanup args: network=%s id=%s generation=%d/%d", networkName, gotID, gotPID, gotStart)
			}
			return nil
		},
	)
	if err != nil {
		t.Fatalf("cleanup external resources: %v", err)
	}
	if want := []string{"dns", "network"}; !reflect.DeepEqual(order, want) {
		t.Fatalf("cleanup order=%v want=%v", order, want)
	}
}

func TestCleanupStoppedGenerationExternalResourcesStillRunsNetworkAfterDNSError(t *testing.T) {
	var order []string
	dnsSentinel := errors.New("dns registry unavailable")

	err := cleanupStoppedGenerationExternalResourcesWith(
		nil,
		"ctr-dns-error",
		5151,
		111,
		func(*state.Store, string, int, uint64, bool) error {
			order = append(order, "network")
			return nil
		},
		func(string, string, int, uint64) error {
			order = append(order, "dns")
			return dnsSentinel
		},
	)
	if err == nil || !errors.Is(err, dnsSentinel) {
		t.Fatalf("expected DNS cleanup error, got %v", err)
	}
	if want := []string{"dns", "network"}; !reflect.DeepEqual(order, want) {
		t.Fatalf("cleanup order after DNS failure=%v want=%v", order, want)
	}
}
