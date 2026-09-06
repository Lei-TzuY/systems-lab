package state

import (
	"errors"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestSaveAssignsAndAdvancesRevision(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	c := &Container{ID: "ctr-revision", Status: StatusCreated, CreatedAt: time.Now()}
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}
	if c.Revision != 1 {
		t.Fatalf("first revision=%d, want 1", c.Revision)
	}
	c.Hostname = "updated"
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}
	if c.Revision != 2 {
		t.Fatalf("second revision=%d, want 2", c.Revision)
	}
	got, err := st.Get(c.ID)
	if err != nil {
		t.Fatal(err)
	}
	if got.Revision != 2 || got.Hostname != "updated" {
		t.Fatalf("unexpected persisted state: %+v", got)
	}
}

func TestSaveRejectsStaleSnapshotAcrossStoreInstances(t *testing.T) {
	dir := t.TempDir()
	st1, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	st2, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	seed := &Container{ID: "ctr-stale", Status: StatusCreated, CreatedAt: time.Now()}
	if err := st1.Save(seed); err != nil {
		t.Fatal(err)
	}

	a, err := st1.Get(seed.ID)
	if err != nil {
		t.Fatal(err)
	}
	b, err := st2.Get(seed.ID)
	if err != nil {
		t.Fatal(err)
	}
	a.Hostname = "winner"
	if err := st1.Save(a); err != nil {
		t.Fatal(err)
	}
	b.Hostname = "stale-loser"
	if err := st2.Save(b); !errors.Is(err, ErrRevisionConflict) {
		t.Fatalf("stale Save error=%v, want ErrRevisionConflict", err)
	}

	current, err := st1.Get(seed.ID)
	if err != nil {
		t.Fatal(err)
	}
	if current.Hostname != "winner" || current.Revision != 2 {
		t.Fatalf("stale writer changed current state: %+v", current)
	}
}

func TestStaleSnapshotCannotRecreateDeletedContainer(t *testing.T) {
	dir := t.TempDir()
	st1, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	st2, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	c := &Container{ID: "ctr-deleted-cas", Status: StatusCreated, CreatedAt: time.Now()}
	if err := st1.Save(c); err != nil {
		t.Fatal(err)
	}
	stale, err := st1.Get(c.ID)
	if err != nil {
		t.Fatal(err)
	}
	if err := st2.Delete(c.ID); err != nil {
		t.Fatal(err)
	}
	stale.Hostname = "resurrected"
	if err := st1.Save(stale); !errors.Is(err, ErrRevisionConflict) {
		t.Fatalf("save after delete error=%v, want revision conflict", err)
	}
	if _, err := st2.Get(c.ID); err == nil {
		t.Fatal("stale snapshot recreated deleted container")
	}
}

func TestLifecycleTransitionsMakeCreationSnapshotStale(t *testing.T) {
	dir := t.TempDir()
	creator, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	runtimeStore, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}

	created := &Container{ID: "ctr-run-cas", Status: StatusCreated, CreatedAt: time.Now()}
	if err := creator.Save(created); err != nil {
		t.Fatal(err)
	}
	if created.Revision != 1 {
		t.Fatalf("created revision=%d", created.Revision)
	}
	if err := runtimeStore.MarkRunning(created.ID, 4242, 9999, time.Now()); err != nil {
		t.Fatal(err)
	}
	if changed, err := runtimeStore.MarkStoppedIfIdentity(created.ID, 4242, 9999, 23, time.Now()); err != nil || !changed {
		t.Fatalf("MarkStopped changed=%v err=%v", changed, err)
	}

	// This mirrors cmdRun retaining the creation-time record while container.Run
	// independently owns the authoritative running/stopped lifecycle updates.
	created.Status = StatusStopped
	created.ExitCode = 0
	if err := creator.Save(created); !errors.Is(err, ErrRevisionConflict) {
		t.Fatalf("creation snapshot overwrite error=%v, want revision conflict", err)
	}

	current, err := creator.Get(created.ID)
	if err != nil {
		t.Fatal(err)
	}
	if current.Status != StatusStopped || current.ExitCode != 23 || current.Revision != 3 {
		t.Fatalf("authoritative lifecycle state was overwritten: %+v", current)
	}
}

func TestLegacyRevisionZeroRecordCanBeUpgraded(t *testing.T) {
	dir := t.TempDir()
	st, err := Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	legacyPath := filepath.Join(dir, "containers", "ctr-legacy-rev.json")
	legacy := `{"id":"ctr-legacy-rev","pid":0,"status":"created","rootfs":"/tmp/root","command":["sh"],"hostname":"legacy","created_at":"2026-01-01T00:00:00Z","exit_code":0}`
	if err := os.WriteFile(legacyPath, []byte(legacy), 0o600); err != nil {
		t.Fatal(err)
	}
	c, err := st.Get("ctr-legacy-rev")
	if err != nil {
		t.Fatal(err)
	}
	if c.Revision != 0 {
		t.Fatalf("legacy revision=%d, want 0", c.Revision)
	}
	c.Hostname = "upgraded"
	if err := st.Save(c); err != nil {
		t.Fatal(err)
	}
	if c.Revision != 1 {
		t.Fatalf("upgraded revision=%d, want 1", c.Revision)
	}
}
