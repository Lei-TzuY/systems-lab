//go:build linux

package container

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"minicontainer/internal/state"
)

func TestPersistAppliedCgroupOwnershipPreservesCgroupWhenStopCannotCommit(t *testing.T) {
	dir := t.TempDir()
	st, err := state.Open(dir)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const (
		id    = "ctr-applied-stop-gate"
		pid   = 7373
		start = uint64(8181)
	)
	name := saveOwnershipTestRunning(t, st, id, pid, start)

	// Corrupt the lifecycle record before ownership persistence. The applied
	// cgroup is known-owned in memory, but neither the ownership token nor the
	// stopped transition can be durably established. Cleanup must therefore be
	// deferred rather than making host state contradict the running/corrupt
	// durable lifecycle record.
	statePath := filepath.Join(dir, "containers", id+".json")
	if err := os.WriteFile(statePath, []byte("{"), 0o600); err != nil {
		t.Fatal(err)
	}

	cleanupCalls := 0
	err = persistAppliedCgroupOwnershipWithAbortAndCleanup(
		nil,
		nil,
		st,
		id,
		pid,
		start,
		name,
		false,
		func(_ *exec.Cmd, _ *os.File) (bool, error) { return true, nil },
		func(gotName string, _ bool) error {
			cleanupCalls++
			if gotName != name {
				t.Fatalf("cleanup target=%q, want %q", gotName, name)
			}
			return nil
		},
	)
	if err == nil {
		t.Fatal("corrupt lifecycle state unexpectedly allowed ownership recovery")
	}
	if cleanupCalls != 0 {
		t.Fatalf("known-owned cgroup cleaned %d time(s) before stopped state was durable", cleanupCalls)
	}
	if !isRuntimeControlError(err) {
		t.Fatalf("state/ownership failure not classified runtime-control: %v", err)
	}
	if !strings.Contains(err.Error(), "persist cgroup ownership") || !strings.Contains(err.Error(), "persist stopped state") {
		t.Fatalf("error lacks both durability failures: %v", err)
	}
}
