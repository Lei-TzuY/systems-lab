//go:build linux

package dns

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"minicontainer/internal/state"
)

func markDNSBridgeOwnership(t *testing.T, st *state.Store, containerID string, pid int, startTime uint64) {
	t.Helper()
	if err := st.MarkNetworkOwnedIfIdentity(containerID, state.NetworkOwnership{
		Owner:        "minicontainer:dns-adoption-test",
		PID:          pid,
		PIDStartTime: startTime,
		VethHost:     "vhabcdefghijklm",
	}); err != nil {
		t.Fatalf("persist bridge ownership: %v", err)
	}
}

func saveDeadRegistrarEntry(t *testing.T, entry HostEntry) {
	t.Helper()
	dir, err := ensureDNSDir()
	if err != nil {
		t.Fatal(err)
	}
	if err := saveEntriesAtomic(dir, filepath.Join(dir, "default.json"), "default", []HostEntry{entry}); err != nil {
		t.Fatal(err)
	}
}

func TestDNSDeadRegistrarEntryIsAdoptedByLiveContainerGeneration(t *testing.T) {
	useTempDNSHome(t)
	identity, err := currentRegistrarIdentity()
	if err != nil {
		t.Fatal(err)
	}

	const containerID = "orphan-live-container"
	st, err := state.Open(state.DefaultDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&state.Container{
		ID:        containerID,
		Status:    state.StatusCreated,
		RootFS:    "/tmp/rootfs",
		Command:   []string{"true"},
		Hostname:  "orphan-host",
		CreatedAt: time.Now(),
	}); err != nil {
		_ = st.Close()
		t.Fatal(err)
	}
	if err := st.MarkRunning(containerID, os.Getpid(), identity.StartTime, time.Now()); err != nil {
		_ = st.Close()
		t.Fatal(err)
	}
	markDNSBridgeOwnership(t, st, containerID, os.Getpid(), identity.StartTime)
	if err := st.Close(); err != nil {
		t.Fatal(err)
	}

	saveDeadRegistrarEntry(t, HostEntry{
		ContainerID:    containerID,
		Hostname:       "orphan-host",
		IP:             "10.0.0.7",
		OwnerPID:       99999999,
		OwnerStartTime: 1,
	})

	content, err := GenerateHostsContentChecked("default")
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(content, "10.0.0.7\torphan-host") {
		t.Fatalf("live orphan container registration was pruned:\n%s", content)
	}

	st, err = state.Open(state.DefaultDir())
	if err != nil {
		t.Fatal(err)
	}
	changed, err := st.MarkStoppedIfIdentity(containerID, os.Getpid(), identity.StartTime, 0, time.Now())
	if err != nil {
		_ = st.Close()
		t.Fatal(err)
	}
	if !changed {
		_ = st.Close()
		t.Fatal("live orphan generation did not transition to stopped")
	}
	if err := st.Close(); err != nil {
		t.Fatal(err)
	}

	content, err = GenerateHostsContentChecked("default")
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(content, "orphan-host") {
		t.Fatalf("stopped orphan registration remained:\n%s", content)
	}
}

func TestDNSDeadRegistrarRunningStateWithoutBridgeOwnershipDoesNotAdoptEntry(t *testing.T) {
	useTempDNSHome(t)
	identity, err := currentRegistrarIdentity()
	if err != nil {
		t.Fatal(err)
	}
	const containerID = "running-before-bridge-commit"
	st, err := state.Open(state.DefaultDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&state.Container{
		ID:        containerID,
		Status:    state.StatusCreated,
		RootFS:    "/tmp/rootfs",
		Command:   []string{"true"},
		Hostname:  "prebridge-host",
		CreatedAt: time.Now(),
	}); err != nil {
		_ = st.Close()
		t.Fatal(err)
	}
	if err := st.MarkRunning(containerID, os.Getpid(), identity.StartTime, time.Now()); err != nil {
		_ = st.Close()
		t.Fatal(err)
	}
	if err := st.Close(); err != nil {
		t.Fatal(err)
	}

	saveDeadRegistrarEntry(t, HostEntry{
		ContainerID:    containerID,
		Hostname:       "prebridge-host",
		IP:             "10.0.0.9",
		OwnerPID:       99999999,
		OwnerStartTime: 1,
	})
	content, err := GenerateHostsContentChecked("default")
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(content, "prebridge-host") {
		t.Fatalf("uncommitted bridge generation adopted stale registration:\n%s", content)
	}
}

func TestDNSDeadRegistrarDifferentHostnameDoesNotCrossAdoptReusedContainerID(t *testing.T) {
	useTempDNSHome(t)
	identity, err := currentRegistrarIdentity()
	if err != nil {
		t.Fatal(err)
	}
	const containerID = "reused-container-id"
	st, err := state.Open(state.DefaultDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&state.Container{
		ID:        containerID,
		Status:    state.StatusCreated,
		RootFS:    "/tmp/new-rootfs",
		Command:   []string{"true"},
		Hostname:  "new-host",
		CreatedAt: time.Now(),
	}); err != nil {
		_ = st.Close()
		t.Fatal(err)
	}
	if err := st.MarkRunning(containerID, os.Getpid(), identity.StartTime, time.Now()); err != nil {
		_ = st.Close()
		t.Fatal(err)
	}
	markDNSBridgeOwnership(t, st, containerID, os.Getpid(), identity.StartTime)
	if err := st.Close(); err != nil {
		t.Fatal(err)
	}

	saveDeadRegistrarEntry(t, HostEntry{
		ContainerID:    containerID,
		Hostname:       "old-host",
		IP:             "10.0.0.10",
		OwnerPID:       99999999,
		OwnerStartTime: 1,
	})
	content, err := GenerateHostsContentChecked("default")
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(content, "old-host") {
		t.Fatalf("reused container ID cross-adopted stale hostname:\n%s", content)
	}
}

func TestDNSDeadRegistrarCreatedStateDoesNotKeepEntryAlive(t *testing.T) {
	useTempDNSHome(t)
	const containerID = "abandoned-created-container"
	st, err := state.Open(state.DefaultDir())
	if err != nil {
		t.Fatal(err)
	}
	if err := st.Save(&state.Container{
		ID:        containerID,
		Status:    state.StatusCreated,
		RootFS:    "/tmp/rootfs",
		Command:   []string{"true"},
		Hostname:  "abandoned-host",
		CreatedAt: time.Now(),
	}); err != nil {
		_ = st.Close()
		t.Fatal(err)
	}
	if err := st.Close(); err != nil {
		t.Fatal(err)
	}

	saveDeadRegistrarEntry(t, HostEntry{
		ContainerID:    containerID,
		Hostname:       "abandoned-host",
		IP:             "10.0.0.8",
		OwnerPID:       99999999,
		OwnerStartTime: 1,
	})
	content, err := GenerateHostsContentChecked("default")
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(content, "abandoned-host") {
		t.Fatalf("abandoned created-state registration survived:\n%s", content)
	}
}
