//go:build linux

package container

import (
	"os"
	"strings"
	"testing"

	"minicontainer/internal/state"
)

func saveRunningTopRecord(t *testing.T, id string, pid int, startTime uint64) {
	t.Helper()
	store, err := state.Open(state.DefaultDir())
	if err != nil {
		t.Fatalf("open state store: %v", err)
	}
	if err := store.Save(&state.Container{
		ID:           id,
		Status:       state.StatusRunning,
		PID:          pid,
		PIDStartTime: startTime,
		RootFS:       t.TempDir(),
	}); err != nil {
		t.Fatalf("save state record: %v", err)
	}
}

func TestGetContainerProcessesRequiresPersistedGeneration(t *testing.T) {
	t.Setenv("HOME", t.TempDir())
	pid := os.Getpid()
	startTime, err := ProcessStartTime(pid)
	if err != nil {
		t.Fatalf("ProcessStartTime: %v", err)
	}
	saveRunningTopRecord(t, "top-live", pid, startTime)

	procs, err := GetContainerProcesses(pid)
	if err != nil {
		t.Fatalf("GetContainerProcesses: %v", err)
	}
	found := false
	for _, proc := range procs {
		if proc.PID == pid {
			found = true
			break
		}
	}
	if !found {
		t.Fatalf("captured task list does not contain container PID %d: %+v", pid, procs)
	}
}

func TestGetContainerProcessesRejectsStalePIDGeneration(t *testing.T) {
	t.Setenv("HOME", t.TempDir())
	pid := os.Getpid()
	startTime, err := ProcessStartTime(pid)
	if err != nil {
		t.Fatalf("ProcessStartTime: %v", err)
	}
	saveRunningTopRecord(t, "top-stale", pid, startTime+1)

	if _, err := GetContainerProcesses(pid); err == nil {
		t.Fatal("GetContainerProcesses accepted stale PID generation")
	} else if !strings.Contains(err.Error(), "process verification") {
		t.Fatalf("unexpected stale-generation error: %v", err)
	}
}

func TestGetContainerProcessesRejectsAmbiguousPersistedPID(t *testing.T) {
	t.Setenv("HOME", t.TempDir())
	pid := os.Getpid()
	startTime, err := ProcessStartTime(pid)
	if err != nil {
		t.Fatalf("ProcessStartTime: %v", err)
	}
	saveRunningTopRecord(t, "top-one", pid, startTime)
	saveRunningTopRecord(t, "top-two", pid, startTime)

	if _, err := GetContainerProcesses(pid); err == nil {
		t.Fatal("GetContainerProcesses accepted ambiguous persisted PID")
	} else if !strings.Contains(err.Error(), "ambiguous persisted top identity") {
		t.Fatalf("unexpected ambiguity error: %v", err)
	}
}

func TestGetContainerProcessesRejectsUntrackedPID(t *testing.T) {
	t.Setenv("HOME", t.TempDir())
	if _, err := GetContainerProcesses(os.Getpid()); err == nil {
		t.Fatal("GetContainerProcesses accepted an untracked host PID")
	}
}
