package state

import (
	"encoding/json"
	"os"
	"strings"
	"testing"
	"time"
)

func TestCgroupOwnershipPersistsVersionedStorageIdentity(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-cgroup-provenance"
	saveCreatedContainer(t, st, id)
	if err := st.MarkRunning(id, 101, 202, time.Now()); err != nil {
		t.Fatal(err)
	}
	name := "minicontainer-ctr-cgroup-provenance-101-202"
	if err := st.MarkCgroupOwnedIfIdentity(id, 101, 202, name); err != nil {
		t.Fatal(err)
	}

	data, err := os.ReadFile(cgroupOwnershipPath(st.ctrDir, id))
	if err != nil {
		t.Fatal(err)
	}
	var persisted persistedCgroupOwnership
	if err := json.Unmarshal(data, &persisted); err != nil {
		t.Fatal(err)
	}
	if persisted.SchemaVersion != cgroupOwnershipSchemaVersion {
		t.Fatalf("schema_version=%d want=%d", persisted.SchemaVersion, cgroupOwnershipSchemaVersion)
	}
	if persisted.ContainerID != id {
		t.Fatalf("container_id=%q want=%q", persisted.ContainerID, id)
	}
	if persisted.Name != name || persisted.PID != 101 || persisted.PIDStartTime != 202 {
		t.Fatalf("persisted ownership=%+v", persisted)
	}
}

func TestCgroupOwnershipRejectsMovedVersionedSidecarAtReadBoundary(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const (
		sourceID = "ctr-cgroup-source"
		targetID = "ctr-cgroup-target"
	)
	saveCreatedContainer(t, st, sourceID)
	saveCreatedContainer(t, st, targetID)
	if err := st.MarkRunning(sourceID, 301, 302, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkCgroupOwnedIfIdentity(sourceID, 301, 302, "minicontainer-ctr-cgroup-source-301-302"); err != nil {
		t.Fatal(err)
	}

	data, err := os.ReadFile(cgroupOwnershipPath(st.ctrDir, sourceID))
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(cgroupOwnershipPath(st.ctrDir, targetID), data, 0o600); err != nil {
		t.Fatal(err)
	}

	_, ok, err := st.GetCgroupOwnership(targetID)
	if err == nil || !strings.Contains(err.Error(), "storage key") || !strings.Contains(err.Error(), sourceID) {
		t.Fatalf("moved sidecar error=%v", err)
	}
	if ok {
		t.Fatal("moved sidecar reported as valid ownership")
	}
}

func TestCgroupOwnershipRejectsFutureAndPartialProvenance(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-cgroup-schema-guard"
	saveCreatedContainer(t, st, id)
	path := cgroupOwnershipPath(st.ctrDir, id)

	future := `{"schema_version":2,"container_id":"ctr-cgroup-schema-guard","name":"minicontainer-safe","pid":1,"pid_start_time":2}`
	future = strings.ReplaceAll(future, `\"`, `"`)
	if err := os.WriteFile(path, []byte(future), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, ok, err := st.GetCgroupOwnership(id); err == nil || ok || !strings.Contains(err.Error(), "unsupported persisted cgroup ownership schema_version 2") {
		t.Fatalf("future schema ok=%v err=%v", ok, err)
	}

	partial := `{"schema_version":1,"name":"minicontainer-safe","pid":1,"pid_start_time":2}`
	partial = strings.ReplaceAll(partial, `\"`, `"`)
	if err := os.WriteFile(path, []byte(partial), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, ok, err := st.GetCgroupOwnership(id); err == nil || ok || !strings.Contains(err.Error(), "must both be present") {
		t.Fatalf("partial provenance ok=%v err=%v", ok, err)
	}
}

func TestCgroupOwnershipKeepsPreSchemaCompatibility(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-cgroup-legacy"
	saveCreatedContainer(t, st, id)
	legacy := `{"name":"minicontainer-ctr-cgroup-legacy-11-22","pid":11,"pid_start_time":22}`
	legacy = strings.ReplaceAll(legacy, `\"`, `"`)
	if err := os.WriteFile(cgroupOwnershipPath(st.ctrDir, id), []byte(legacy), 0o600); err != nil {
		t.Fatal(err)
	}
	got, ok, err := st.GetCgroupOwnership(id)
	if err != nil || !ok {
		t.Fatalf("legacy ownership ok=%v err=%v", ok, err)
	}
	want := CgroupOwnership{Name: "minicontainer-ctr-cgroup-legacy-11-22", PID: 11, PIDStartTime: 22}
	if got != want {
		t.Fatalf("legacy ownership=%+v want=%+v", got, want)
	}
}
