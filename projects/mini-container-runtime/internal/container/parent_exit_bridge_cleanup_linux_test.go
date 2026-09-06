//go:build linux

package container

import (
	"errors"
	"testing"

	"minicontainer/internal/state"
)

func TestCleanupBridgeAfterNormalExitSkipsManagedGeneration(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	calls := 0
	err = cleanupBridgeAfterNormalExit(st, func() error {
		calls++
		return errors.New("must not run before durable finalization")
	})
	if err != nil {
		t.Fatalf("managed cleanup returned error: %v", err)
	}
	if calls != 0 {
		t.Fatalf("managed eager bridge cleanup calls=%d, want 0", calls)
	}
}

func TestCleanupBridgeAfterNormalExitPreservesUnmanagedCleanup(t *testing.T) {
	calls := 0
	if err := cleanupBridgeAfterNormalExit(nil, func() error {
		calls++
		return nil
	}); err != nil {
		t.Fatalf("unmanaged cleanup failed: %v", err)
	}
	if calls != 1 {
		t.Fatalf("unmanaged cleanup calls=%d, want 1", calls)
	}
}

func TestCleanupBridgeAfterNormalExitWrapsUnmanagedFailure(t *testing.T) {
	cleanupErr := errors.New("remove veth")
	err := cleanupBridgeAfterNormalExit(nil, func() error { return cleanupErr })
	if !errors.Is(err, cleanupErr) {
		t.Fatalf("cleanup error lost: %v", err)
	}
	if !isRuntimeControlError(err) {
		t.Fatalf("cleanup failure must remain runtime-control: %v", err)
	}
}
