//go:build linux

// internal/container/top_linux.go
//
// Container Process Listing (`minictl top`)
// ──────────────────────────────────────────
// `docker top` lists processes running inside a container.
//
// Mechanism:
// We examine `/proc/<containerPID>/task` to find threads/tasks belonging to the
// container init process and read each task's status through a stable procfs
// directory file descriptor. The persisted PID start time is verified before
// that directory is accepted so PID reuse cannot redirect `top` to an unrelated
// host process.

package container

import (
	"bufio"
	"fmt"
	"os"
	"strings"

	"minicontainer/internal/state"
	"golang.org/x/sys/unix"
)

// ProcessInfo holds information about a single process/thread inside the container.
type ProcessInfo struct {
	PID   int
	PPID  int
	Name  string
	State string
}

// GetContainerProcesses lists tasks for the exact running process generation
// persisted for containerPID. It deliberately resolves the state record again
// instead of trusting the integer PID supplied by the CLI: a stale state record
// whose PID has been reused must never expose metadata for the replacement host
// process.
func GetContainerProcesses(containerPID int) ([]ProcessInfo, error) {
	if containerPID <= 0 {
		return nil, fmt.Errorf("invalid container PID %d", containerPID)
	}

	store, err := state.Open(state.DefaultDir())
	if err != nil {
		return nil, fmt.Errorf("open container state for top identity: %w", err)
	}
	records, err := store.List()
	if err != nil {
		return nil, fmt.Errorf("list container state for top identity: %w", err)
	}

	var match *state.Container
	for _, rec := range records {
		if rec.PID != containerPID || rec.Status != state.StatusRunning {
			continue
		}
		if match != nil {
			return nil, fmt.Errorf("ambiguous persisted top identity for PID %d", containerPID)
		}
		match = rec
	}
	if match == nil {
		return nil, fmt.Errorf("no running container state matches top PID %d", containerPID)
	}

	current, handle, err := openRunningProcess(store, match.ID)
	if err != nil {
		return nil, fmt.Errorf("verify top target: %w", err)
	}
	defer handle.Close()
	if current.PID != containerPID {
		return nil, fmt.Errorf("container %s changed PID from %d to %d while preparing top", shortProcessID(current.ID), containerPID, current.PID)
	}

	taskDir, err := openStableTaskDir(containerPID, current.PIDStartTime)
	if err != nil {
		return nil, err
	}
	defer taskDir.Close()

	return readTaskProcesses(taskDir)
}

func openStableTaskDir(containerPID int, expectedStartTime uint64) (*os.File, error) {
	before, err := ProcessStartTime(containerPID)
	if err != nil {
		return nil, fmt.Errorf("verify top target PID %d before task capture: %w", containerPID, err)
	}
	if before != expectedStartTime {
		return nil, fmt.Errorf("top target PID %d does not match persisted identity: expected start time %d, current %d", containerPID, expectedStartTime, before)
	}

	taskPath := fmt.Sprintf("/proc/%d/task", containerPID)
	taskDir, err := os.Open(taskPath)
	if err != nil {
		return nil, fmt.Errorf("open task dir for PID %d: %w", containerPID, err)
	}

	after, err := ProcessStartTime(containerPID)
	if err != nil {
		_ = taskDir.Close()
		return nil, fmt.Errorf("verify top target PID %d after task capture: %w", containerPID, err)
	}
	if after != expectedStartTime {
		_ = taskDir.Close()
		return nil, fmt.Errorf("top target PID %d changed identity during task capture", containerPID)
	}
	return taskDir, nil
}

func readTaskProcesses(taskDir *os.File) ([]ProcessInfo, error) {
	entries, err := taskDir.ReadDir(-1)
	if err != nil {
		return nil, fmt.Errorf("read captured task directory: %w", err)
	}

	var procs []ProcessInfo
	for _, entry := range entries {
		tid := entry.Name()
		fd, err := unix.Openat(int(taskDir.Fd()), tid+"/status", unix.O_RDONLY|unix.O_CLOEXEC|unix.O_NOFOLLOW, 0)
		if err != nil {
			// Tasks can legitimately disappear while top is collecting a snapshot.
			continue
		}
		f := os.NewFile(uintptr(fd), "status")
		if f == nil {
			_ = unix.Close(fd)
			continue
		}

		info := ProcessInfo{}
		scanner := bufio.NewScanner(f)
		for scanner.Scan() {
			line := scanner.Text()
			parts := strings.SplitN(line, ":", 2)
			if len(parts) != 2 {
				continue
			}
			key := strings.TrimSpace(parts[0])
			val := strings.TrimSpace(parts[1])

			switch key {
			case "Name":
				info.Name = val
			case "State":
				info.State = val
			case "Pid":
				info.PID, _ = strconvAtoi(val)
			case "PPid":
				info.PPID, _ = strconvAtoi(val)
			}
		}
		_ = f.Close()

		if info.PID > 0 {
			procs = append(procs, info)
		}
	}

	return procs, nil
}

func shortProcessID(id string) string {
	if len(id) > 8 {
		return id[:8]
	}
	return id
}

func strconvAtoi(s string) (int, error) {
	var n int
	_, err := fmt.Sscanf(s, "%d", &n)
	return n, err
}
