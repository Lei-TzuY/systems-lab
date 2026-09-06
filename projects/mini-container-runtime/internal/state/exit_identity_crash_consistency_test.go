package state

import (
	"errors"
	"os"
	"testing"
	"time"
)

func TestModernStopDoesNotDependOnLegacyExitSidecar(t *testing.T) {
	const (
		id    = "ctr-identity-failure"
		pid   = 9301
		start = 9401
	)
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&Container{ID: id, Status: StatusCreated, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkRunning(id, pid, start, time.Now()); err != nil {
		t.Fatal(err)
	}

	// A path that would make the historical .exit publication fail must no
	// longer affect a modern stop: the generation key is part of the JSON commit.
	identityPath := exitedIdentityPath(st.ctrDir, id)
	if err := os.Mkdir(identityPath, 0o700); err != nil {
		t.Fatal(err)
	}
	changed, err := st.MarkStoppedIfIdentity(id, pid, start, 0, time.Now())
	if err != nil || !changed {
		t.Fatalf("MarkStoppedIfIdentity changed=%v err=%v", changed, err)
	}
	stopped, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}
	if stopped.Status != StatusStopped {
		t.Fatalf("status=%s, want stopped", stopped.Status)
	}
	required, present, err := st.containerExitIdentityRequirementUnlocked(id)
	if err != nil || !present || !required {
		t.Fatalf("successful stop did not atomically publish capability: present=%v required=%v err=%v", present, required, err)
	}
	embedded, embeddedPresent, err := st.containerEmbeddedExitedIdentityUnlocked(id)
	if err != nil || !embeddedPresent || embedded.PID != pid || embedded.PIDStartTime != start {
		t.Fatalf("embedded identity=%+v present=%v err=%v", embedded, embeddedPresent, err)
	}
	if _, statErr := os.Stat(exitedIdentityRequiredPath(st.ctrDir, id)); !errors.Is(statErr, os.ErrNotExist) {
		t.Fatalf("successful new stop created legacy marker: stat err=%v", statErr)
	}
	gotPID, gotStart, current, ok, err := st.GetExitedIdentityForStoppedRevision(id, stopped.Revision)
	if err != nil || !current || !ok || gotPID != pid || gotStart != start {
		t.Fatalf("stop lost exact identity: pid=%d start=%d current=%v ok=%v err=%v", gotPID, gotStart, current, ok, err)
	}
}

func TestLegacyExitRequiredMarkerRemainsFailClosedAfterUpgrade(t *testing.T) {
	const id = "ctr-upgrade-marker"
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&Container{ID: id, Status: StatusStopped, CreatedAt: time.Now()}); err != nil {
		t.Fatal(err)
	}
	rewriteAsPreSchemaStoppedFixture(t, st, id)
	legacy, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}
	if err := atomicWriteFile(st.ctrDir, exitedIdentityRequiredPath(st.ctrDir, id), []byte("1\n")); err != nil {
		t.Fatal(err)
	}

	_, _, current, ok, required, err := st.GetStoppedExitIdentityPolicy(id, legacy.Revision)
	if err != nil {
		t.Fatal(err)
	}
	if !current || ok || !required {
		t.Fatalf("legacy modern marker lost fail-closed semantics: current=%v ok=%v required=%v", current, ok, required)
	}
}
