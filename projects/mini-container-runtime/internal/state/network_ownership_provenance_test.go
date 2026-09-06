package state

import (
	"encoding/json"
	"os"
	"strings"
	"testing"
	"time"
)

func TestNetworkOwnershipPersistsVersionedStorageIdentity(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-network-provenance"
	saveCreatedContainer(t, st, id)
	if err := st.MarkRunning(id, 101, 202, time.Now()); err != nil {
		t.Fatal(err)
	}
	ownership := testNetworkOwnership(101, 202)
	if err := st.MarkNetworkOwnedIfIdentity(id, ownership); err != nil {
		t.Fatal(err)
	}

	data, err := os.ReadFile(networkOwnershipPath(st.ctrDir, id))
	if err != nil {
		t.Fatal(err)
	}
	var persisted persistedNetworkOwnership
	if err := json.Unmarshal(data, &persisted); err != nil {
		t.Fatal(err)
	}
	if persisted.SchemaVersion != networkOwnershipSchemaVersion {
		t.Fatalf("schema_version=%d want=%d", persisted.SchemaVersion, networkOwnershipSchemaVersion)
	}
	if persisted.ContainerID != id {
		t.Fatalf("container_id=%q want=%q", persisted.ContainerID, id)
	}
	if persisted.Owner != ownership.Owner || persisted.PID != ownership.PID || persisted.PIDStartTime != ownership.PIDStartTime || len(persisted.Mappings) != len(ownership.Mappings) {
		t.Fatalf("persisted ownership=%+v", persisted)
	}
}

func TestNetworkOwnershipRejectsMovedVersionedSidecarAtReadBoundary(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const (
		sourceID = "ctr-network-source"
		targetID = "ctr-network-target"
	)
	saveCreatedContainer(t, st, sourceID)
	saveCreatedContainer(t, st, targetID)
	if err := st.MarkRunning(sourceID, 301, 302, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkNetworkOwnedIfIdentity(sourceID, testNetworkOwnership(301, 302)); err != nil {
		t.Fatal(err)
	}

	data, err := os.ReadFile(networkOwnershipPath(st.ctrDir, sourceID))
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(networkOwnershipPath(st.ctrDir, targetID), data, 0o600); err != nil {
		t.Fatal(err)
	}

	_, ok, err := st.GetNetworkOwnership(targetID)
	if err == nil || !strings.Contains(err.Error(), "storage key") || !strings.Contains(err.Error(), sourceID) {
		t.Fatalf("moved sidecar error=%v", err)
	}
	if ok {
		t.Fatal("moved sidecar reported as valid ownership")
	}
}

func TestNetworkOwnershipRejectsFutureAndPartialProvenance(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-network-schema-guard"
	saveCreatedContainer(t, st, id)
	path := networkOwnershipPath(st.ctrDir, id)

	future := `{"schema_version":2,"container_id":"ctr-network-schema-guard","owner":"minicontainer:test-owner","pid":1,"pid_start_time":2,"mappings":[{"host_port":18080,"container_port":8080,"container_ip":"10.88.0.2","protocol":"tcp"}]}`
	future = strings.ReplaceAll(future, `\"`, `"`)
	if err := os.WriteFile(path, []byte(future), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, ok, err := st.GetNetworkOwnership(id); err == nil || ok || !strings.Contains(err.Error(), "unsupported persisted network ownership schema_version 2") {
		t.Fatalf("future schema ok=%v err=%v", ok, err)
	}

	partial := `{"schema_version":1,"owner":"minicontainer:test-owner","pid":1,"pid_start_time":2,"mappings":[{"host_port":18080,"container_port":8080,"container_ip":"10.88.0.2","protocol":"tcp"}]}`
	partial = strings.ReplaceAll(partial, `\"`, `"`)
	if err := os.WriteFile(path, []byte(partial), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, ok, err := st.GetNetworkOwnership(id); err == nil || ok || !strings.Contains(err.Error(), "must both be present") {
		t.Fatalf("partial provenance ok=%v err=%v", ok, err)
	}
}

func TestNetworkOwnershipKeepsPreSchemaCompatibility(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-network-legacy"
	saveCreatedContainer(t, st, id)
	legacy := `{"owner":"minicontainer:test-owner","pid":11,"pid_start_time":22,"mappings":[{"host_port":18080,"container_port":8080,"container_ip":"10.88.0.2","protocol":"tcp"}]}`
	legacy = strings.ReplaceAll(legacy, `\"`, `"`)
	if err := os.WriteFile(networkOwnershipPath(st.ctrDir, id), []byte(legacy), 0o600); err != nil {
		t.Fatal(err)
	}
	got, ok, err := st.GetNetworkOwnership(id)
	if err != nil || !ok {
		t.Fatalf("legacy ownership ok=%v err=%v", ok, err)
	}
	want := NetworkOwnership{
		Owner:        "minicontainer:test-owner",
		PID:          11,
		PIDStartTime: 22,
		Mappings: []PortForwardingOwnership{{
			HostPort:      18080,
			ContainerPort: 8080,
			ContainerIP:   "10.88.0.2",
			Protocol:      "tcp",
		}},
	}
	if !networkOwnershipEqual(got, want) {
		t.Fatalf("legacy ownership=%+v want=%+v", got, want)
	}
}
