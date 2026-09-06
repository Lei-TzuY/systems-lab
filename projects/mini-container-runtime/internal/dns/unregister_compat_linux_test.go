//go:build linux

package dns

import (
	"os/exec"
	"path/filepath"
	"testing"
)

func TestUnregisterHostPreservesLiveForeignRegistrarEntry(t *testing.T) {
	tmpHome := t.TempDir()
	t.Setenv("HOME", tmpHome)
	t.Setenv("USERPROFILE", tmpHome)

	child := exec.Command("sleep", "30")
	if err := child.Start(); err != nil {
		t.Fatal(err)
	}
	defer func() {
		_ = child.Process.Kill()
		_ = child.Wait()
	}()

	start, err := readProcessStartTime(child.Process.Pid)
	if err != nil {
		t.Fatalf("read child process start time: %v", err)
	}

	const networkName = "default"
	const containerID = "ctr-new-owner"
	dir, err := ensureDNSDir()
	if err != nil {
		t.Fatal(err)
	}
	netFile := filepath.Join(dir, networkName+".json")
	entry := HostEntry{
		ContainerID:    containerID,
		Hostname:       "new-owner",
		IP:             "172.20.0.9",
		OwnerPID:       child.Process.Pid,
		OwnerStartTime: start,
	}
	if err := saveEntriesAtomic(dir, netFile, networkName, []HostEntry{entry}); err != nil {
		t.Fatalf("persist foreign registration: %v", err)
	}

	// This process models a stale CLI defer. It does not own the live entry and
	// therefore must not be able to delete it by container ID alone.
	if err := UnregisterHost(networkName, containerID); err != nil {
		t.Fatalf("legacy unregister: %v", err)
	}

	entries, exists, err := loadEntriesChecked(netFile, networkName)
	if err != nil {
		t.Fatalf("reload registry: %v", err)
	}
	if !exists || len(entries) != 1 || entries[0] != entry {
		t.Fatalf("live foreign registration changed: exists=%v entries=%+v", exists, entries)
	}
}
