package dns

import (
	"os"
	"path/filepath"
	"testing"
)

func writeOwnedDNSRegistry(t *testing.T, network string, entries []HostEntry) {
	t.Helper()
	dir := DefaultDNSDir()
	if err := os.MkdirAll(dir, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := saveEntriesAtomic(dir, filepath.Join(dir, network+".json"), network, entries); err != nil {
		t.Fatal(err)
	}
}

func readOwnedDNSRegistry(t *testing.T, network string) []HostEntry {
	t.Helper()
	entries, exists, err := loadEntriesChecked(filepath.Join(DefaultDNSDir(), network+".json"), network)
	if err != nil {
		t.Fatal(err)
	}
	if !exists {
		t.Fatal("DNS registry unexpectedly missing")
	}
	return entries
}

func TestUnregisterHostIfOwnedBySkipsNewerRegistrarGeneration(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("USERPROFILE", home)

	const network = "default"
	const containerID = "ctr-reused-id"
	newOwner := registrarIdentity{PID: 2222, StartTime: 200}
	writeOwnedDNSRegistry(t, network, []HostEntry{{
		ContainerID:    containerID,
		Hostname:       "new-generation",
		IP:             "172.20.0.2",
		OwnerPID:       newOwner.PID,
		OwnerStartTime: newOwner.StartTime,
	}})

	staleOwner := registrarIdentity{PID: 1111, StartTime: 100}
	if err := unregisterHostIfOwnedBy(network, containerID, staleOwner); err != nil {
		t.Fatalf("stale unregister: %v", err)
	}
	entries := readOwnedDNSRegistry(t, network)
	if len(entries) != 1 || entries[0].OwnerPID != newOwner.PID || entries[0].OwnerStartTime != newOwner.StartTime {
		t.Fatalf("newer DNS registration changed by stale finalizer: %+v", entries)
	}
}

func TestUnregisterHostIfOwnedByConsumesMatchingRegistrarGeneration(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("USERPROFILE", home)

	const network = "default"
	const containerID = "ctr-owned-id"
	owner := registrarIdentity{PID: 3333, StartTime: 300}
	writeOwnedDNSRegistry(t, network, []HostEntry{{
		ContainerID:    containerID,
		Hostname:       "owned",
		IP:             "172.20.0.2",
		OwnerPID:       owner.PID,
		OwnerStartTime: owner.StartTime,
	}})

	if err := unregisterHostIfOwnedBy(network, containerID, owner); err != nil {
		t.Fatalf("matching unregister: %v", err)
	}
	entries := readOwnedDNSRegistry(t, network)
	if len(entries) != 0 {
		t.Fatalf("matching DNS registration remains: %+v", entries)
	}
}

func TestUnregisterHostIfOwnedByMissingDNSDirIsSideEffectFree(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("USERPROFILE", home)

	owner := registrarIdentity{PID: 4444, StartTime: 400}
	if err := unregisterHostIfOwnedBy("default", "ctr-no-dns", owner); err != nil {
		t.Fatalf("missing registry unregister: %v", err)
	}
	if _, err := os.Lstat(DefaultDNSDir()); !os.IsNotExist(err) {
		t.Fatalf("DNS directory created during no-op cleanup: %v", err)
	}
}
