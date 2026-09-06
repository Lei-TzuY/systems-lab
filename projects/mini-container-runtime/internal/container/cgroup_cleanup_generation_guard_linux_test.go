//go:build linux

package container

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/cgroups"
	"minicontainer/internal/state"
)

func TestCleanupStoppedCgroupRejectsStaleSameContainerGeneration(t *testing.T) {
	root := t.TempDir()
	st, err := state.Open(root)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const (
		id           = "ctr-cgroup-generation-guard"
		stalePID     = 5151
		staleStart   = 6161
		currentPID   = 7171
		currentStart = 8181
	)

	if err := st.Save(&state.Container{
		ID:        id,
		Status:    state.StatusCreated,
		RootFS:    "/tmp/rootfs",
		Command:   []string{"true"},
		CreatedAt: time.Now(),
	}); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkRunning(id, currentPID, currentStart, time.Now()); err != nil {
		t.Fatal(err)
	}
	if changed, err := st.MarkStoppedIfIdentity(id, currentPID, currentStart, 0, time.Now()); err != nil || !changed {
		t.Fatalf("stop current generation: changed=%v err=%v", changed, err)
	}
	stopped, err := st.Get(id)
	if err != nil {
		t.Fatal(err)
	}

	staleName, err := cgroups.NameForContainerProcess(id, stalePID, staleStart)
	if err != nil {
		t.Fatal(err)
	}
	ownershipJSON := fmt.Sprintf("{\"name\":%q,\"pid\":%d,\"pid_start_time\":%d}", staleName, stalePID, staleStart)
	ownershipPath := filepath.Join(root, "containers", id+".cgroup")
	if err := os.WriteFile(ownershipPath, []byte(ownershipJSON), 0o600); err != nil {
		t.Fatalf("inject stale ownership: %v", err)
	}

	cleanupCalls := 0
	err = cleanupStoppedCgroupWithCleanup(st, stopped, func(string, int, uint64) error {
		cleanupCalls++
		return nil
	})
	if err == nil || !strings.Contains(err.Error(), "ownership belongs to process 5151/6161, stopped generation is 7171/8181") {
		t.Fatalf("stale-generation cleanup error = %v", err)
	}
	if cleanupCalls != 0 {
		t.Fatalf("destructive cgroup cleanup ran %d time(s)", cleanupCalls)
	}
	ownership, ok, err := st.GetCgroupOwnership(id)
	if err != nil || !ok {
		t.Fatalf("stale ownership proof was consumed: ok=%v err=%v", ok, err)
	}
	if ownership.PID != stalePID || ownership.PIDStartTime != staleStart {
		t.Fatalf("ownership changed unexpectedly: %+v", ownership)
	}
}
