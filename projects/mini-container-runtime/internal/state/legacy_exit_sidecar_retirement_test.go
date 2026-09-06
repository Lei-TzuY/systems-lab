package state

import (
	"os"
	"testing"
	"time"
)

func TestStoppedGenerationMigrationRetiresLegacySidecars(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	if err := st.Save(&Container{ID: "retire-migrate", Status: StatusStopped, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	rewriteAsPreSchemaStoppedFixture(t, st, "retire-migrate")
	container, err := st.Get("retire-migrate")
	if err != nil {
		t.Fatal(err)
	}

	st.mu.Lock()
	if err := lockStateFile(st.lockFile); err != nil {
		st.mu.Unlock()
		t.Fatal(err)
	}
	if err := st.writeExitedIdentityUnlocked("retire-migrate", 7331, 444); err != nil {
		_ = unlockStateFile(st.lockFile)
		st.mu.Unlock()
		t.Fatal(err)
	}
	if err := atomicWriteFile(st.ctrDir, exitedIdentityRequiredPath(st.ctrDir, "retire-migrate"), []byte("1\n")); err != nil {
		_ = unlockStateFile(st.lockFile)
		st.mu.Unlock()
		t.Fatal(err)
	}
	_ = unlockStateFile(st.lockFile)
	st.mu.Unlock()

	pid, start, current, ok, required, err := st.GetStoppedExitIdentityPolicy("retire-migrate", container.Revision)
	if err != nil {
		t.Fatal(err)
	}
	if !current || !ok || !required || pid != 7331 || start != 444 {
		t.Fatalf("unexpected migrated policy: pid=%d start=%d current=%v ok=%v required=%v", pid, start, current, ok, required)
	}
	for _, path := range []string{
		exitedIdentityPath(st.ctrDir, "retire-migrate"),
		exitedIdentityRequiredPath(st.ctrDir, "retire-migrate"),
	} {
		if _, err := os.Lstat(path); !os.IsNotExist(err) {
			t.Fatalf("legacy sidecar survived successful migration: path=%s err=%v", path, err)
		}
	}
}

func TestVersionedStoppedGenerationRetriesInterruptedLegacySidecarRetirement(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	if err := st.Save(&Container{ID: "retire-retry", Status: StatusStopped, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	container, err := st.Get("retire-retry")
	if err != nil {
		t.Fatal(err)
	}
	identity := exitedIdentity{PID: 8442, PIDStartTime: 555}

	st.mu.Lock()
	if err := lockStateFile(st.lockFile); err != nil {
		st.mu.Unlock()
		t.Fatal(err)
	}
	if err := st.writeContainerRevisionWithExitPolicyUnlocked(container, container.Revision, true, &identity); err != nil {
		_ = unlockStateFile(st.lockFile)
		st.mu.Unlock()
		t.Fatal(err)
	}
	// Model a crash immediately after the authoritative lifecycle JSON commit:
	// obsolete sidecars are still present, including a conflicting identity.
	if err := st.writeExitedIdentityUnlocked("retire-retry", 9999, 666); err != nil {
		_ = unlockStateFile(st.lockFile)
		st.mu.Unlock()
		t.Fatal(err)
	}
	if err := atomicWriteFile(st.ctrDir, exitedIdentityRequiredPath(st.ctrDir, "retire-retry"), []byte("1\n")); err != nil {
		_ = unlockStateFile(st.lockFile)
		st.mu.Unlock()
		t.Fatal(err)
	}
	_ = unlockStateFile(st.lockFile)
	st.mu.Unlock()

	pid, start, current, ok, required, err := st.GetStoppedExitIdentityPolicy("retire-retry", container.Revision)
	if err != nil {
		t.Fatal(err)
	}
	if !current || !ok || !required || pid != identity.PID || start != identity.PIDStartTime {
		t.Fatalf("legacy debris overrode authoritative identity: pid=%d start=%d current=%v ok=%v required=%v", pid, start, current, ok, required)
	}
	for _, path := range []string{
		exitedIdentityPath(st.ctrDir, "retire-retry"),
		exitedIdentityRequiredPath(st.ctrDir, "retire-retry"),
	} {
		if _, err := os.Lstat(path); !os.IsNotExist(err) {
			t.Fatalf("interrupted retirement was not retried: path=%s err=%v", path, err)
		}
	}
}

func TestStaleStoppedRevisionDoesNotRetireSidecars(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	if err := st.Save(&Container{ID: "retire-stale", Status: StatusStopped, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	container, err := st.Get("retire-stale")
	if err != nil {
		t.Fatal(err)
	}
	identity := exitedIdentity{PID: 9553, PIDStartTime: 777}

	st.mu.Lock()
	if err := lockStateFile(st.lockFile); err != nil {
		st.mu.Unlock()
		t.Fatal(err)
	}
	if err := st.writeContainerRevisionWithExitPolicyUnlocked(container, container.Revision, true, &identity); err != nil {
		_ = unlockStateFile(st.lockFile)
		st.mu.Unlock()
		t.Fatal(err)
	}
	if err := st.writeExitedIdentityUnlocked("retire-stale", 1111, 888); err != nil {
		_ = unlockStateFile(st.lockFile)
		st.mu.Unlock()
		t.Fatal(err)
	}
	_ = unlockStateFile(st.lockFile)
	st.mu.Unlock()

	_, _, current, _, _, err := st.GetStoppedExitIdentityPolicy("retire-stale", container.Revision+1)
	if err != nil {
		t.Fatal(err)
	}
	if current {
		t.Fatal("stale revision unexpectedly became current")
	}
	if _, err := os.Lstat(exitedIdentityPath(st.ctrDir, "retire-stale")); err != nil {
		t.Fatalf("stale caller mutated newer-generation debris: %v", err)
	}
}
