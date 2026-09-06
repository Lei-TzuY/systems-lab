package state

import (
	"os"
	"strings"
	"testing"
	"time"
)

func testNetworkOwnership(pid int, start uint64) NetworkOwnership {
	return NetworkOwnership{
		Owner:        "minicontainer:test-network-owner",
		PID:          pid,
		PIDStartTime: start,
		Mappings: []PortForwardingOwnership{
			{HostPort: 8080, ContainerPort: 80, ContainerIP: "172.20.0.2", Protocol: "tcp"},
			{HostPort: 5353, ContainerPort: 53, ContainerIP: "172.20.0.2", Protocol: "udp"},
		},
	}
}

func TestNetworkOwnershipPersistsAcrossStopAndClearsExactly(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-network-owner"
	saveCreatedContainer(t, st, id)
	if err := st.MarkRunning(id, 101, 202, time.Now()); err != nil {
		t.Fatal(err)
	}
	ownership := testNetworkOwnership(101, 202)
	if err := st.MarkNetworkOwnedIfIdentity(id, ownership); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkNetworkOwnedIfIdentity(id, ownership); err != nil {
		t.Fatalf("idempotent ownership write: %v", err)
	}
	if _, err := st.MarkStoppedIfIdentity(id, ownership.PID, ownership.PIDStartTime, -1, time.Now()); err != nil {
		t.Fatal(err)
	}
	got, ok, err := st.GetNetworkOwnership(id)
	if err != nil || !ok || !networkOwnershipEqual(got, ownership) {
		t.Fatalf("stop lost ownership: got=%+v ok=%v err=%v", got, ok, err)
	}

	wrong := ownership
	wrong.Owner = "minicontainer:other-owner"
	if changed, err := st.ClearNetworkOwnershipIfMatch(id, wrong); err != nil || changed {
		t.Fatalf("stale clear changed=%v err=%v", changed, err)
	}
	if changed, err := st.ClearNetworkOwnershipIfMatch(id, ownership); err != nil || !changed {
		t.Fatalf("matching clear changed=%v err=%v", changed, err)
	}
	if _, ok, err := st.GetNetworkOwnership(id); err != nil || ok {
		t.Fatalf("ownership remains after clear: ok=%v err=%v", ok, err)
	}
}

func TestNetworkOwnershipCanClearExactRunningGenerationAfterTeardown(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-network-running-clear"
	saveCreatedContainer(t, st, id)
	if err := st.MarkRunning(id, 11, 22, time.Now()); err != nil {
		t.Fatal(err)
	}
	ownership := testNetworkOwnership(11, 22)
	if err := st.MarkNetworkOwnedIfIdentity(id, ownership); err != nil {
		t.Fatal(err)
	}
	if changed, err := st.ClearNetworkOwnershipIfMatch(id, ownership); err != nil || !changed {
		t.Fatalf("exact running clear changed=%v err=%v", changed, err)
	}
}

func TestPendingNetworkOwnershipBlocksRestartAndDelete(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-network-pending"
	saveCreatedContainer(t, st, id)
	if err := st.MarkRunning(id, 41, 42, time.Now()); err != nil {
		t.Fatal(err)
	}
	ownership := testNetworkOwnership(41, 42)
	if err := st.MarkNetworkOwnedIfIdentity(id, ownership); err != nil {
		t.Fatal(err)
	}
	if _, err := st.MarkStoppedIfIdentity(id, ownership.PID, ownership.PIDStartTime, -1, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err := st.MarkRunning(id, 51, 52, time.Now()); err == nil || !strings.Contains(err.Error(), "pending network cleanup") {
		t.Fatalf("restart with pending network cleanup error=%v", err)
	}
	if err := st.Delete(id); err == nil || !strings.Contains(err.Error(), "pending network cleanup") {
		t.Fatalf("legacy delete with pending network cleanup error=%v", err)
	}
	if err := st.DeleteIfNotRunning(id); err == nil || !strings.Contains(err.Error(), "pending network cleanup") {
		t.Fatalf("guarded delete with pending network cleanup error=%v", err)
	}
	if changed, err := st.ClearNetworkOwnershipIfMatch(id, ownership); err != nil || !changed {
		t.Fatalf("clear pending ownership changed=%v err=%v", changed, err)
	}
	if err := st.MarkRunning(id, 51, 52, time.Now()); err != nil {
		t.Fatalf("restart after network cleanup: %v", err)
	}
}

func TestNetworkOwnershipRequiresExactRunningIdentity(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-network-identity"
	saveCreatedContainer(t, st, id)
	if err := st.MarkRunning(id, 111, 222, time.Now()); err != nil {
		t.Fatal(err)
	}
	ownership := testNetworkOwnership(111, 333)
	if err := st.MarkNetworkOwnedIfIdentity(id, ownership); err == nil || !strings.Contains(err.Error(), "not bound") {
		t.Fatalf("wrong identity ownership error=%v", err)
	}
	if _, ok, err := st.GetNetworkOwnership(id); err != nil || ok {
		t.Fatalf("wrong identity persisted ownership: ok=%v err=%v", ok, err)
	}
}

func TestNetworkOwnershipRejectsCorruptAndSymlinkedSidecars(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-network-corrupt"
	saveCreatedContainer(t, st, id)
	path := networkOwnershipPath(st.ctrDir, id)
	if err := os.WriteFile(path, []byte(`{"owner":"bad","pid":1,"pid_start_time":2,"mappings":[{"host_port":1,"container_port":2,"container_ip":"172.20.0.2","protocol":"tcp"}]}`), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, ok, err := st.GetNetworkOwnership(id); err == nil || ok || !strings.Contains(err.Error(), "invalid persisted network ownership") {
		t.Fatalf("corrupt ownership ok=%v err=%v", ok, err)
	}
	if err := os.Remove(path); err != nil {
		t.Fatal(err)
	}
	outside := t.TempDir() + "/ownership"
	if err := os.WriteFile(outside, []byte(`{"owner":"minicontainer:safe","pid":1,"pid_start_time":2,"mappings":[{"host_port":1,"container_port":2,"container_ip":"172.20.0.2","protocol":"tcp"}]}`), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, path); err != nil {
		t.Fatal(err)
	}
	if _, _, err := st.GetNetworkOwnership(id); err == nil {
		t.Fatal("symlinked network ownership sidecar was followed")
	}
}

func TestNetworkOwnershipSidecarIsPrivate(t *testing.T) {
	st, err := Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	const id = "ctr-network-private"
	saveCreatedContainer(t, st, id)
	if err := st.MarkRunning(id, 7, 8, time.Now()); err != nil {
		t.Fatal(err)
	}
	ownership := testNetworkOwnership(7, 8)
	if err := st.MarkNetworkOwnedIfIdentity(id, ownership); err != nil {
		t.Fatal(err)
	}
	info, err := os.Lstat(networkOwnershipPath(st.ctrDir, id))
	if err != nil {
		t.Fatal(err)
	}
	if !info.Mode().IsRegular() || info.Mode().Perm() != 0o600 {
		t.Fatalf("network ownership sidecar mode=%v perm=%#o", info.Mode(), info.Mode().Perm())
	}
}
