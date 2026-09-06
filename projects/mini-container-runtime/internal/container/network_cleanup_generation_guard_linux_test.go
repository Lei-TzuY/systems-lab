//go:build linux

package container

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func TestCleanupNetworkOwnershipRejectsCrossRecordGeneration(t *testing.T) {
	root := t.TempDir()
	st, err := state.Open(root)
	if err != nil {
		t.Fatal(err)
	}
	defer st.Close()

	const (
		sourceID    = "ctr-network-source"
		targetID    = "ctr-network-target"
		sourcePID   = 5151
		sourceStart = 6161
		targetPID   = 7171
		targetStart = 8181
	)

	for _, id := range []string{sourceID, targetID} {
		if err := st.Save(&state.Container{
			ID:        id,
			Status:    state.StatusCreated,
			RootFS:    "/tmp/rootfs",
			Command:   []string{"true"},
			CreatedAt: time.Now(),
		}); err != nil {
			t.Fatalf("save %s: %v", id, err)
		}
	}
	if err := st.MarkRunning(sourceID, sourcePID, sourceStart, time.Now()); err != nil {
		t.Fatalf("mark source running: %v", err)
	}
	sourceOwnership := networkOwnershipForGeneration(
		"minicontainer:cross-record-source",
		sourcePID,
		sourceStart,
		"172.20.0.2",
		[]PortMapping{{HostPort: 18080, ContainerPort: 80}},
	)
	if err := st.MarkNetworkOwnedIfIdentity(sourceID, sourceOwnership); err != nil {
		t.Fatalf("persist source ownership: %v", err)
	}

	if err := st.MarkRunning(targetID, targetPID, targetStart, time.Now()); err != nil {
		t.Fatalf("mark target running: %v", err)
	}
	if changed, err := st.MarkStoppedIfIdentity(targetID, targetPID, targetStart, 0, time.Now()); err != nil || !changed {
		t.Fatalf("stop target: changed=%v err=%v", changed, err)
	}

	// Simulate a historical pre-schema sidecar being moved under another storage
	// key. Current sidecars carry container provenance and are rejected at the
	// state read boundary; legacy sidecars remain readable for upgrade recovery,
	// so destructive cleanup must still bind them to the durable stopped process
	// generation rather than trusting the filename alone.
	sourcePath := filepath.Join(root, "containers", sourceID+".network")
	targetPath := filepath.Join(root, "containers", targetID+".network")
	data, err := os.ReadFile(sourcePath)
	if err != nil {
		t.Fatalf("read source ownership sidecar: %v", err)
	}
	var persisted map[string]json.RawMessage
	if err := json.Unmarshal(data, &persisted); err != nil {
		t.Fatalf("decode source ownership sidecar: %v", err)
	}
	delete(persisted, "schema_version")
	delete(persisted, "container_id")
	legacyData, err := json.Marshal(persisted)
	if err != nil {
		t.Fatalf("encode legacy ownership sidecar: %v", err)
	}
	if err := os.WriteFile(targetPath, legacyData, 0o600); err != nil {
		t.Fatalf("inject cross-record ownership sidecar: %v", err)
	}

	ownership, ok, err := st.GetNetworkOwnership(targetID)
	if err != nil || !ok {
		t.Fatalf("read injected legacy ownership: ok=%v err=%v", ok, err)
	}

	portCalls := 0
	vethCalls := 0
	err = cleanupNetworkOwnershipAfterDurableStopWith(
		st,
		targetID,
		ownership,
		false,
		func(string, int, int, string, string, bool) error { portCalls++; return nil },
		func(string, string, bool) error { vethCalls++; return nil },
	)
	if err == nil || !strings.Contains(err.Error(), "ownership belongs to process 5151/6161, stopped generation is 7171/8181") {
		t.Fatalf("cross-generation cleanup error = %v", err)
	}
	if portCalls != 0 || vethCalls != 0 {
		t.Fatalf("destructive cleanup ran for cross-record ownership: port=%d veth=%d", portCalls, vethCalls)
	}
	if got, ok, err := st.GetNetworkOwnership(targetID); err != nil || !ok || got.PID != sourcePID || got.PIDStartTime != sourceStart {
		t.Fatalf("cross-record ownership proof was consumed: got=%+v ok=%v err=%v", got, ok, err)
	}
}
