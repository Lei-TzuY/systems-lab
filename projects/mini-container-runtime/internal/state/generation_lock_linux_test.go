//go:build linux

package state

import (
	"testing"
	"time"
)

func TestWithRunningGenerationLockedSerializesLifecycleMutationAcrossStores(t *testing.T) {
	dir := t.TempDir()
	st1, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	defer st1.Close()
	st2, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	defer st2.Close()

	c := &Container{
		ID:           "generation-lock-cross-store",
		Status:       StatusRunning,
		PID:          2468,
		PIDStartTime: 13579,
		CreatedAt:    time.Now(),
	}
	if err := st1.Save(c); err != nil {
		t.Fatal(err)
	}

	firstEntered := make(chan struct{})
	releaseFirst := make(chan struct{})
	firstDone := make(chan error, 1)
	go func() {
		firstDone <- st1.WithRunningGenerationLocked(c.ID, c.PID, c.PIDStartTime, func() error {
			close(firstEntered)
			<-releaseFirst
			return nil
		})
	}()
	select {
	case <-firstEntered:
	case <-time.After(2 * time.Second):
		t.Fatal("generation callback did not acquire state lock")
	}

	type mutationResult struct {
		changed bool
		err     error
	}
	mutationStarted := make(chan struct{})
	mutationDone := make(chan mutationResult, 1)
	go func() {
		close(mutationStarted)
		changed, err := st2.MarkStoppedIfIdentity(c.ID, c.PID, c.PIDStartTime, -1, time.Now())
		mutationDone <- mutationResult{changed: changed, err: err}
	}()
	<-mutationStarted

	select {
	case result := <-mutationDone:
		t.Fatalf("lifecycle mutation escaped generation lock: changed=%v err=%v", result.changed, result.err)
	case <-time.After(75 * time.Millisecond):
	}

	close(releaseFirst)
	if err := <-firstDone; err != nil {
		t.Fatalf("generation operation: %v", err)
	}

	select {
	case result := <-mutationDone:
		if result.err != nil {
			t.Fatalf("lifecycle mutation after release: %v", result.err)
		}
		if !result.changed {
			t.Fatal("lifecycle mutation did not update the expected generation after lock release")
		}
	case <-time.After(2 * time.Second):
		t.Fatal("lifecycle mutation remained blocked after generation lock release")
	}
}
