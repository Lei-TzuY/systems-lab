package state

import (
	"strings"
	"testing"
	"time"
)

func TestMarkStoppedIfCreatedRecordsStartupFailure(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := &Container{ID: "startup-failure", Status: StatusCreated, CreatedAt: time.Unix(10, 0)}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}
	before, err := st.Get(c.ID)
	if err != nil {
		t.Fatal(err)
	}
	finishedAt := time.Unix(20, 0)

	changed, err := st.MarkStoppedIfCreated(c.ID, 1, finishedAt)
	if err != nil {
		t.Fatalf("mark stopped if created: %v", err)
	}
	if !changed {
		t.Fatal("created startup failure was not recorded")
	}

	got, err := st.Get(c.ID)
	if err != nil {
		t.Fatal(err)
	}
	if got.Status != StatusStopped || got.ExitCode != 1 {
		t.Fatalf("state=%+v, want stopped exit code 1", got)
	}
	if got.PID != 0 || got.PIDStartTime != 0 || got.StartedAt != nil {
		t.Fatalf("startup failure gained process identity: %+v", got)
	}
	if got.FinishedAt == nil || !got.FinishedAt.Equal(finishedAt) {
		t.Fatalf("finished_at=%v, want %v", got.FinishedAt, finishedAt)
	}
	if got.Revision <= before.Revision {
		t.Fatalf("revision=%d, want > %d", got.Revision, before.Revision)
	}
}

func TestMarkStoppedIfCreatedDoesNotOverwriteRunningGeneration(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := &Container{ID: "running-wins", Status: StatusCreated}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkRunning(c.ID, 4242, 88, time.Unix(30, 0)); err != nil {
		t.Fatal(err)
	}

	changed, err := st.MarkStoppedIfCreated(c.ID, 1, time.Unix(31, 0))
	if err != nil {
		t.Fatalf("mark stopped if created: %v", err)
	}
	if changed {
		t.Fatal("startup fallback overwrote a running generation")
	}
	got, err := st.Get(c.ID)
	if err != nil {
		t.Fatal(err)
	}
	if got.Status != StatusRunning || got.PID != 4242 || got.PIDStartTime != 88 {
		t.Fatalf("running state changed: %+v", got)
	}
}

func TestMarkStoppedIfCreatedDoesNotOverwriteAuthoritativeExit(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := &Container{ID: "authoritative-exit", Status: StatusCreated}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkRunning(c.ID, 5151, 99, time.Unix(40, 0)); err != nil {
		t.Fatal(err)
	}
	authoritativeFinished := time.Unix(41, 0)
	if changed, err := st.MarkStoppedIfIdentity(c.ID, 5151, 99, 17, authoritativeFinished); err != nil || !changed {
		t.Fatalf("authoritative stop: changed=%v err=%v", changed, err)
	}

	changed, err := st.MarkStoppedIfCreated(c.ID, 1, time.Unix(42, 0))
	if err != nil {
		t.Fatalf("mark stopped if created: %v", err)
	}
	if changed {
		t.Fatal("startup fallback overwrote authoritative stopped state")
	}
	got, err := st.Get(c.ID)
	if err != nil {
		t.Fatal(err)
	}
	if got.Status != StatusStopped || got.ExitCode != 17 || got.FinishedAt == nil || !got.FinishedAt.Equal(authoritativeFinished) {
		t.Fatalf("authoritative exit changed: %+v", got)
	}
}

func TestMarkStoppedIfCreatedRejectsCreatedProcessIdentity(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := &Container{
		ID:           "malformed-created",
		Status:       StatusCreated,
		PID:          6161,
		PIDStartTime: 101,
	}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}

	changed, err := st.MarkStoppedIfCreated(c.ID, 1, time.Unix(50, 0))
	if err == nil || !strings.Contains(err.Error(), "process identity") {
		t.Fatalf("error=%v, want process identity refusal", err)
	}
	if changed {
		t.Fatal("malformed created record was modified")
	}
	got, getErr := st.Get(c.ID)
	if getErr != nil {
		t.Fatal(getErr)
	}
	if got.Status != StatusCreated || got.PID != 6161 || got.PIDStartTime != 101 {
		t.Fatalf("malformed created state changed: %+v", got)
	}
}

func TestMarkStoppedIfCreatedRejectsPendingCgroupOwnership(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := &Container{ID: "created-owned", Status: StatusCreated}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}
	ownership := CgroupOwnership{
		Name:         "minicontainer-created-owned-7171-111",
		PID:          7171,
		PIDStartTime: 111,
	}
	st.mu.Lock()
	err = st.writeCgroupOwnershipUnlocked(c.ID, ownership)
	st.mu.Unlock()
	if err != nil {
		t.Fatalf("write adversarial ownership: %v", err)
	}

	changed, err := st.MarkStoppedIfCreated(c.ID, 1, time.Unix(60, 0))
	if err == nil || !strings.Contains(err.Error(), "cgroup ownership") {
		t.Fatalf("error=%v, want cgroup ownership refusal", err)
	}
	if changed {
		t.Fatal("created record with ownership proof was modified")
	}
	got, getErr := st.Get(c.ID)
	if getErr != nil {
		t.Fatal(getErr)
	}
	if got.Status != StatusCreated {
		t.Fatalf("created state changed: %+v", got)
	}
	persisted, ok, getErr := st.GetCgroupOwnership(c.ID)
	if getErr != nil {
		t.Fatal(getErr)
	}
	if !ok || persisted != ownership {
		t.Fatalf("ownership changed: ok=%v ownership=%+v", ok, persisted)
	}
}

func TestMarkStoppedIfCreatedRejectsPendingNetworkOwnership(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := &Container{ID: "created-network-owned", Status: StatusCreated}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}
	ownership := NetworkOwnership{
		Owner:        "minicontainer:created-network-owned-7272-112",
		PID:          7272,
		PIDStartTime: 112,
		VethHost:     "vh2345672345672",
	}
	st.mu.Lock()
	err = st.writeNetworkOwnershipUnlocked(c.ID, ownership)
	st.mu.Unlock()
	if err != nil {
		t.Fatalf("write adversarial network ownership: %v", err)
	}

	changed, err := st.MarkStoppedIfCreated(c.ID, 1, time.Unix(65, 0))
	if err == nil || !strings.Contains(err.Error(), "network ownership") {
		t.Fatalf("error=%v, want network ownership refusal", err)
	}
	if changed {
		t.Fatal("created record with network ownership proof was modified")
	}
	got, getErr := st.Get(c.ID)
	if getErr != nil {
		t.Fatal(getErr)
	}
	if got.Status != StatusCreated {
		t.Fatalf("created state changed: %+v", got)
	}
	persisted, ok, getErr := st.GetNetworkOwnership(c.ID)
	if getErr != nil {
		t.Fatal(getErr)
	}
	if !ok || !networkOwnershipEqual(persisted, ownership) {
		t.Fatalf("network ownership changed: ok=%v ownership=%+v", ok, persisted)
	}
}

func TestMarkStoppedIfCreatedRejectsExitedGenerationProof(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := &Container{ID: "created-exited-proof", Status: StatusCreated}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}
	st.mu.Lock()
	err = st.writeExitedIdentityUnlocked(c.ID, 8181, 121)
	st.mu.Unlock()
	if err != nil {
		t.Fatalf("write adversarial exited identity: %v", err)
	}

	changed, err := st.MarkStoppedIfCreated(c.ID, 1, time.Unix(70, 0))
	if err == nil || !strings.Contains(err.Error(), "exited-generation identity") {
		t.Fatalf("error=%v, want exited-generation refusal", err)
	}
	if changed {
		t.Fatal("created record with exited-generation proof was modified")
	}
	got, getErr := st.Get(c.ID)
	if getErr != nil {
		t.Fatal(getErr)
	}
	if got.Status != StatusCreated {
		t.Fatalf("created state changed: %+v", got)
	}
	exited, ok, getErr := st.readExitedIdentityUnlocked(c.ID)
	if getErr != nil {
		t.Fatal(getErr)
	}
	if !ok || exited.PID != 8181 || exited.PIDStartTime != 121 {
		t.Fatalf("exited identity changed: ok=%v identity=%+v", ok, exited)
	}
}
