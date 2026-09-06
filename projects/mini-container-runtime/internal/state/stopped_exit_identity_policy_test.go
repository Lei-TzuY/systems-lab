package state

import (
	"testing"
	"time"
)

func TestGetStoppedExitIdentityPolicyModernStop(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&Container{ID: "modern-policy", Status: StatusCreated, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkRunning("modern-policy", 4242, 98765, time.Now()); err != nil {
		t.Fatal(err)
	}
	changed, err := st.MarkStoppedIfIdentity("modern-policy", 4242, 98765, 0, time.Now())
	if err != nil || !changed {
		t.Fatalf("MarkStoppedIfIdentity changed=%v err=%v", changed, err)
	}
	stopped, err := st.Get("modern-policy")
	if err != nil {
		t.Fatal(err)
	}

	pid, start, current, ok, required, err := st.GetStoppedExitIdentityPolicy("modern-policy", stopped.Revision)
	if err != nil {
		t.Fatal(err)
	}
	if !current || !ok || !required || pid != 4242 || start != 98765 {
		t.Fatalf("unexpected policy: pid=%d start=%d current=%v ok=%v required=%v", pid, start, current, ok, required)
	}
}

func TestGetStoppedExitIdentityPolicyMissingModernIdentityFailsClosed(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&Container{ID: "missing-policy", Status: StatusCreated, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkRunning("missing-policy", 77, 7700, time.Now()); err != nil {
		t.Fatal(err)
	}
	changed, err := st.MarkStoppedIfIdentity("missing-policy", 77, 7700, 0, time.Now())
	if err != nil || !changed {
		t.Fatalf("MarkStoppedIfIdentity changed=%v err=%v", changed, err)
	}
	stopped, err := st.Get("missing-policy")
	if err != nil {
		t.Fatal(err)
	}

	// Remove only the embedded identity while retaining the in-JSON capability.
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

	pid, start, current, ok, required, err := st.GetStoppedExitIdentityPolicy("missing-policy", stopped.Revision)
	if err != nil {
		t.Fatal(err)
	}
	if pid != 0 || start != 0 || !current || ok || !required {
		t.Fatalf("missing modern identity must fail closed: pid=%d start=%d current=%v ok=%v required=%v", pid, start, current, ok, required)
	}
}

func TestGetStoppedExitIdentityPolicyHistoricalAndStale(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&Container{ID: "legacy-policy", Status: StatusStopped, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	legacy, err := st.Get("legacy-policy")
	if err != nil {
		t.Fatal(err)
	}

	_, _, current, ok, required, err := st.GetStoppedExitIdentityPolicy("legacy-policy", legacy.Revision)
	if err != nil {
		t.Fatal(err)
	}
	if !current || ok || required {
		t.Fatalf("historical state should retain migration compatibility: current=%v ok=%v required=%v", current, ok, required)
	}

	_, _, current, ok, required, err = st.GetStoppedExitIdentityPolicy("legacy-policy", legacy.Revision+1)
	if err != nil {
		t.Fatal(err)
	}
	if current || ok || required {
		t.Fatalf("stale revision acquired policy authority: current=%v ok=%v required=%v", current, ok, required)
	}
}
