package state

import (
	"testing"
	"time"
)

func TestStoreCloseIsIdempotent(t *testing.T) {
	s, err := Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open: %v", err)
	}

	if err := s.Close(); err != nil {
		t.Fatalf("first Close: %v", err)
	}
	if err := s.Close(); err != nil {
		t.Fatalf("second Close: %v", err)
	}
	if s.lockFile != nil {
		t.Fatal("Close retained state lock handle")
	}
	if s.storagePins != nil {
		t.Fatal("Close retained pinned directory handles")
	}
}

func TestNilStoreCloseIsSafe(t *testing.T) {
	var s *Store
	if err := s.Close(); err != nil {
		t.Fatalf("nil Close: %v", err)
	}
}

func TestStoreCloseWaitsForInFlightStateOperation(t *testing.T) {
	s, err := Open(t.TempDir())
	if err != nil {
		t.Fatalf("Open: %v", err)
	}

	// State operations hold mu while using the lock/pinned directory handles.
	// Holding it here models an operation at the exact lifecycle boundary and
	// verifies Close cannot invalidate those handles underneath the operation.
	s.mu.Lock()
	closed := make(chan error, 1)
	go func() { closed <- s.Close() }()

	select {
	case err := <-closed:
		s.mu.Unlock()
		t.Fatalf("Close raced past in-flight operation: %v", err)
	case <-time.After(20 * time.Millisecond):
	}

	s.mu.Unlock()
	select {
	case err := <-closed:
		if err != nil {
			t.Fatalf("Close after operation: %v", err)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("Close did not finish after operation released Store mutex")
	}
}
