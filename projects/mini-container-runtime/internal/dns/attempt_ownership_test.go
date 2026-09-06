package dns

import (
	"errors"
	"testing"
)

func resetAttemptOwnersForTest() {
	attemptOwnershipMu.Lock()
	defer attemptOwnershipMu.Unlock()
	attemptOwners = make(map[attemptOwnershipKey]string)
}

func TestAttemptRollbackCannotConsumeNewerSameRegistrarAttempt(t *testing.T) {
	resetAttemptOwnersForTest()
	key := attemptOwnershipKey{networkName: "default", containerID: "ctr-a"}
	registers := 0
	unregisters := 0
	register := func() error { registers++; return nil }
	unregister := func() error { unregisters++; return nil }

	rollbackA, err := beginHostRegistrationAttemptWith(key, "attempt-a", register, unregister)
	if err != nil {
		t.Fatalf("begin attempt A: %v", err)
	}
	rollbackB, err := beginHostRegistrationAttemptWith(key, "attempt-b", register, unregister)
	if err != nil {
		t.Fatalf("begin attempt B: %v", err)
	}
	if registers != 2 {
		t.Fatalf("registrations = %d, want 2", registers)
	}
	if err := rollbackA(); err != nil {
		t.Fatalf("stale rollback A: %v", err)
	}
	if unregisters != 0 {
		t.Fatalf("stale rollback consumed newer registration: unregisters=%d", unregisters)
	}
	if err := rollbackB(); err != nil {
		t.Fatalf("current rollback B: %v", err)
	}
	if unregisters != 1 {
		t.Fatalf("current rollback did not consume registration exactly once: %d", unregisters)
	}
}

func TestAttemptRollbackFailureRetainsExactOwnershipForRetry(t *testing.T) {
	resetAttemptOwnersForTest()
	key := attemptOwnershipKey{networkName: "default", containerID: "ctr-a"}
	wantErr := errors.New("unregister failed")
	calls := 0
	rollback, err := beginHostRegistrationAttemptWith(key, "attempt-a", func() error { return nil }, func() error {
		calls++
		if calls == 1 {
			return wantErr
		}
		return nil
	})
	if err != nil {
		t.Fatalf("begin attempt: %v", err)
	}
	if err := rollback(); !errors.Is(err, wantErr) {
		t.Fatalf("first rollback error = %v, want %v", err, wantErr)
	}
	if err := rollback(); err != nil {
		t.Fatalf("retry rollback: %v", err)
	}
	if calls != 2 {
		t.Fatalf("unregister calls = %d, want 2", calls)
	}
}

func TestAttemptRegistrationFailureDoesNotPublishOwnership(t *testing.T) {
	resetAttemptOwnersForTest()
	key := attemptOwnershipKey{networkName: "default", containerID: "ctr-a"}
	wantErr := errors.New("register failed")
	rollback, err := beginHostRegistrationAttemptWith(key, "attempt-a", func() error { return wantErr }, func() error {
		t.Fatal("unregister called after failed registration")
		return nil
	})
	if !errors.Is(err, wantErr) || rollback != nil {
		t.Fatalf("failed registration returned rollback=%v err=%v", rollback != nil, err)
	}
	attemptOwnershipMu.Lock()
	defer attemptOwnershipMu.Unlock()
	if _, ok := attemptOwners[key]; ok {
		t.Fatal("failed registration published attempt ownership")
	}
}
