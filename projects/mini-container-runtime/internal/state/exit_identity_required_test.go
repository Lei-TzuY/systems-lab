package state

import (
	"testing"
	"time"
)

func TestStoppedRevisionRequiresExitedIdentityAfterModernStop(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&Container{ID: "modern", Status: StatusCreated, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkRunning("modern", 4242, 98765, time.Now()); err != nil {
		t.Fatal(err)
	}
	changed, err := st.MarkStoppedIfIdentity("modern", 4242, 98765, 0, time.Now())
	if err != nil || !changed {
		t.Fatalf("MarkStoppedIfIdentity changed=%v err=%v", changed, err)
	}
	stopped, err := st.Get("modern")
	if err != nil {
		t.Fatal(err)
	}

	// Simulate corruption of a modern record: capability remains durable while
	// its exact embedded generation key is missing.
	st.mu.Lock()
	if err := lockStateFile(st.lockFile); err != nil {
		st.mu.Unlock()
		t.Fatal(err)
	}
	if err := st.writeContainerRevisionWithExitPolicyUnlocked(stopped, stopped.Revision, true, nil); err != nil {
		_ = unlockStateFile(st.lockFile)
		st.mu.Unlock()
		t.Fatal(err)
	}
	_ = unlockStateFile(st.lockFile)
	st.mu.Unlock()

	if _, _, current, ok, err := st.GetExitedIdentityForStoppedRevision("modern", stopped.Revision); err != nil {
		t.Fatal(err)
	} else if !current || ok {
		t.Fatalf("missing identity should remain a current stopped revision: current=%v ok=%v", current, ok)
	}
	current, required, err := st.StoppedRevisionRequiresExitedIdentity("modern", stopped.Revision)
	if err != nil {
		t.Fatal(err)
	}
	if !current || !required {
		t.Fatalf("modern stopped revision must require identity: current=%v required=%v", current, required)
	}
}

func TestHistoricalStoppedRevisionDoesNotRequireExitedIdentity(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&Container{ID: "legacy", Status: StatusStopped, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	legacy, err := st.Get("legacy")
	if err != nil {
		t.Fatal(err)
	}
	current, required, err := st.StoppedRevisionRequiresExitedIdentity("legacy", legacy.Revision)
	if err != nil {
		t.Fatal(err)
	}
	if !current || required {
		t.Fatalf("historical stopped revision should retain migration compatibility: current=%v required=%v", current, required)
	}
}

func TestStoppedRevisionIdentityRequirementIsRevisionScoped(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&Container{ID: "scoped", Status: StatusCreated, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkRunning("scoped", 7, 77, time.Now()); err != nil {
		t.Fatal(err)
	}
	changed, err := st.MarkStoppedIfIdentity("scoped", 7, 77, 0, time.Now())
	if err != nil || !changed {
		t.Fatalf("MarkStoppedIfIdentity changed=%v err=%v", changed, err)
	}
	stopped, err := st.Get("scoped")
	if err != nil {
		t.Fatal(err)
	}
	current, required, err := st.StoppedRevisionRequiresExitedIdentity("scoped", stopped.Revision+1)
	if err != nil {
		t.Fatal(err)
	}
	if current || required {
		t.Fatalf("stale revision unexpectedly acquired policy authority: current=%v required=%v", current, required)
	}
}
