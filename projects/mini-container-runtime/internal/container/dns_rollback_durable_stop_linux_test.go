//go:build linux

package container

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func saveDNSRollbackTestContainer(t *testing.T, st *state.Store, id string) {
	t.Helper()
	if err := st.Save(&state.Container{
		ID:        id,
		Status:    state.StatusCreated,
		RootFS:    "/tmp/rootfs",
		Command:   []string{"true"},
		CreatedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}
}

func TestNetworkAdmissionRollbackPreservesRunningGeneration(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const id = "ctr-dns-rollback-running"
	saveDNSRollbackTestContainer(t, st, id)
	if err := st.MarkRunning(id, 4242, 5151, time.Now()); err != nil {
		t.Fatal(err)
	}

	calls := 0
	if err := rollbackNetworkAdmissionAfterRun(st, id, func() error {
		calls++
		return nil
	}); err != nil {
		t.Fatalf("running rollback gate: %v", err)
	}
	if calls != 0 {
		t.Fatalf("DNS admission consumed before durable stop: calls=%d", calls)
	}
}

func TestNetworkAdmissionRollbackRunsOnlyBeforeGenerationAdmission(t *testing.T) {
	for _, tc := range []struct {
		name      string
		status    state.Status
		wantCalls int
	}{
		{name: "created", status: state.StatusCreated, wantCalls: 1},
		{name: "stopped", status: state.StatusStopped, wantCalls: 0},
	} {
		t.Run(tc.name, func(t *testing.T) {
			st, err := state.Open(t.TempDir())
			if err != nil {
				t.Fatal(err)
			}
			defer st.Close()

			id := "ctr-dns-rollback-" + tc.name
			saveDNSRollbackTestContainer(t, st, id)
			if tc.status == state.StatusStopped {
				if err := st.MarkRunning(id, 6262, 7171, time.Now()); err != nil {
					t.Fatal(err)
				}
				changed, err := st.MarkStoppedIfIdentity(id, 6262, 7171, 0, time.Now())
				if err != nil || !changed {
					t.Fatalf("mark stopped: changed=%v err=%v", changed, err)
				}
			}

			calls := 0
			if err := rollbackNetworkAdmissionAfterRun(st, id, func() error {
				calls++
				return nil
			}); err != nil {
				t.Fatalf("rollback gate: %v", err)
			}
			if calls != tc.wantCalls {
				t.Fatalf("rollback calls=%d, want %d", calls, tc.wantCalls)
			}
		})
	}
}

func TestNetworkAdmissionRollbackPreservesStoppedRegistrationForConcurrentRestart(t *testing.T) {
	st, err := state.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const id = "ctr-dns-rollback-stopped-restart"
	saveDNSRollbackTestContainer(t, st, id)
	if err := st.MarkRunning(id, 8080, 9090, time.Now()); err != nil {
		t.Fatal(err)
	}
	changed, err := st.MarkStoppedIfIdentity(id, 8080, 9090, 1, time.Now())
	if err != nil || !changed {
		t.Fatalf("mark stopped: changed=%v err=%v", changed, err)
	}

	// A newer attempt can re-register DNS before it has spawned/MarkRunning'd a
	// new child. Registrar-owned registration is idempotent within this process,
	// so the old attempt's rollback cannot distinguish and must not unregister it.
	calls := 0
	if err := rollbackNetworkAdmissionAfterRun(st, id, func() error {
		calls++
		return nil
	}); err != nil {
		t.Fatalf("stopped rollback gate: %v", err)
	}
	if calls != 0 {
		t.Fatalf("stale stopped rollback consumed next-attempt registration: calls=%d", calls)
	}
}

func TestNetworkAdmissionRollbackPreservesRegistrationOnStateReadFailure(t *testing.T) {
	dir := t.TempDir()
	st, err := state.Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const id = "ctr-dns-rollback-corrupt"
	saveDNSRollbackTestContainer(t, st, id)
	statePath := filepath.Join(dir, "containers", id+".json")
	if err := os.WriteFile(statePath, []byte("{broken-json"), 0o600); err != nil {
		t.Fatal(err)
	}

	calls := 0
	err = rollbackNetworkAdmissionAfterRun(st, id, func() error {
		calls++
		return nil
	})
	if err == nil {
		t.Fatal("state read failure unexpectedly allowed DNS rollback")
	}
	if calls != 0 {
		t.Fatalf("DNS admission consumed after state read failure: calls=%d", calls)
	}
}
